//! The whole state of a plugin: read as the plugin says it now, and put in as one undo step.
//!
//! The bytes are the plugin host's own container (docs/architecture.md 24-14): `MV3S` and the
//! component's and the controller's VST3 states for VST3, `ML2S` and lilv's Turtle for LV2.
//! A state of one format does not fit the other format of the same plugin.

mod readable;

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_core::ai::{AiAnswer, AiAsk, AiDone, AiSong, AiState};
use meloet_model::{PluginPlace, PluginRef, StripId};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::mixer::{BusArg, slot_of, strip_of};
use super::{Tools, closing, done, done_value, failed, path_ok, refusal_text};
use readable::readable;

/// The largest state read or put in, in bytes: samplers keep their sounds in their state.
pub(super) const MAX_STATE: usize = 64 << 20;
/// The longest base64 text an answer carries (`inline`).
const MAX_INLINE: usize = 256 << 10;
/// How long the plugin may take to say its state.
const STATE_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct GetStateArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// Also give the state in the answer as base64, when that is at most 256 KiB of text.
    /// Default false: the state is written to a file.
    #[serde(default)]
    pub inline: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SetStateArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// The state as base64, as get_plugin_state gives it. Give this or `file`.
    #[serde(default)]
    pub base64: Option<String>,
    /// A file holding the state: an absolute path, such as get_plugin_state writes. Give this
    /// or `base64`.
    #[serde(default)]
    #[schemars(length(min = 1, max = 4096))]
    pub file: Option<String>,
}

/// A plugin's state as the plugin said it.
#[derive(Debug, Clone)]
pub(super) struct PluginStateGot {
    /// As the song has it: key, name, vendor.
    pub plugin: Arc<PluginRef>,
    /// "vst3" or "lv2", from the key.
    pub format: String,
    pub bytes: Arc<[u8]>,
}

/// Where a plugin is: an insert slot of a track, a return or the master, or the instrument
/// of a track. The tools that name a plugin take these three arguments.
pub(super) fn plugin_place(
    read: &AiSong,
    track: Option<u32>,
    bus: Option<BusArg>,
    slot: Option<u32>,
) -> Result<PluginPlace, CallToolResult> {
    let strip = strip_of(read, track, bus)?;
    match (slot, strip) {
        (Some(slot), strip) => Ok(PluginPlace::Insert(strip, slot_of(slot)?)),
        (None, StripId::Track(id)) => Ok(PluginPlace::Instrument(id)),
        (None, _) => Err(failed("give the insert `slot` of the plugin")),
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

fn format_of(key: &str) -> String {
    key.split_once(':').map_or("", |(format, _)| format).into()
}

/// What a blocking piece of work gives, or "closing" when the pool lost it.
async fn off_thread<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CallToolResult> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| closing())
}

/// The bytes of a state file the AI app names.
fn read_state_file(text: &str) -> Result<Vec<u8>, CallToolResult> {
    let path = path_ok(text)?;
    let meta = std::fs::metadata(&path)
        .map_err(|e| failed(format!("{} cannot be read ({e})", path.display())))?;
    if !meta.is_file() {
        return Err(failed(format!("{} is not a file", path.display())));
    }
    if meta.len() > MAX_STATE as u64 {
        return Err(failed(too_large()));
    }
    std::fs::read(&path).map_err(|e| failed(format!("{} cannot be read ({e})", path.display())))
}

fn too_large() -> String {
    format!("a state is at most {} MiB", MAX_STATE >> 20)
}

/// Writes the state into a new file only this user can read.
fn write_state_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

impl Tools {
    /// The whole state of the plugin at `place`, asked of the plugin now (not what the song
    /// last kept).
    pub(super) async fn plugin_state(
        &self,
        place: PluginPlace,
    ) -> Result<PluginStateGot, CallToolResult> {
        let until = Instant::now() + STATE_WAIT;
        loop {
            match self.answer(AiAsk::PluginState { place }).await? {
                AiAnswer::State(AiState::Got(got)) => {
                    return Ok(PluginStateGot {
                        format: format_of(&got.plugin.key),
                        plugin: got.plugin,
                        bytes: got.bytes,
                    });
                }
                AiAnswer::State(AiState::Waiting) if Instant::now() < until => {}
                AiAnswer::State(AiState::Waiting) => {
                    return Err(failed(format!(
                        "the plugin did not say its state within {} s",
                        STATE_WAIT.as_secs()
                    )));
                }
                AiAnswer::Refused(r) => {
                    return Err(failed(format!(
                        "Could not read the plugin's state: {}.",
                        refusal_text(&r)
                    )));
                }
                _ => return Err(closing()),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Puts a whole state into the plugin at `place`, and waits until it is one step of the
    /// song, or why it is not (the plugin refused it, or its process stopped: the song and
    /// the plugin then keep the song's state).
    pub(super) async fn put_plugin_state(
        &self,
        place: PluginPlace,
        bytes: Arc<[u8]>,
    ) -> Result<AiDone, CallToolResult> {
        if bytes.is_empty() {
            return Err(failed("the state is empty"));
        }
        if bytes.len() > MAX_STATE {
            return Err(failed(too_large()));
        }
        let (done, _) = self
            .ticketed(AiAsk::SetPluginState {
                place,
                state: bytes,
            })
            .await?;
        Ok(done)
    }

    /// A new file in the private temporary folder with the state in it.
    async fn state_file(&self, got: &PluginStateGot) -> Result<PathBuf, CallToolResult> {
        self.scratch.clean(crate::scratch::KEEP);
        let dir = self.scratch.dir().map_err(|e| {
            failed(format!(
                "the private temporary folder is not usable ({e}); ask with `inline: true`"
            ))
        })?;
        let name = format!("state-{}-{}", got.format, got.plugin.name);
        let path = self.scratch.name(&dir, &name, "bin");
        let (to, bytes) = (path.clone(), got.bytes.clone());
        off_thread(move || write_state_file(&to, &bytes))
            .await?
            .map_err(|e| failed(format!("the state could not be written ({e})")))?;
        Ok(path)
    }
}

#[tool_router(router = plugstate_router, vis = "pub(super)")]
impl Tools {
    /// The whole state of a plugin (a track's instrument, or an insert with `slot`), asked
    /// of it now: name, format, key, size, sha256, and the bytes, written to a private
    /// temporary file (deleted after an hour) or, with `inline`, as base64. Only the same
    /// plugin in the same format can take it back.
    #[tool(annotations(
        title = "Get plugin state",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_plugin_state(
        &self,
        Parameters(a): Parameters<GetStateArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let place = check!(plugin_place(&read, a.track, a.bus, a.slot));
        let got = check!(self.plugin_state(place).await);
        let inline = a.inline && got.bytes.len().div_ceil(3) * 4 <= MAX_INLINE;
        let file = match inline {
            true => None,
            false => Some(check!(self.state_file(&got).await)),
        };
        let (plugin, bytes) = (got.plugin.clone(), got.bytes.clone());
        let (sha256, base64, readable) = check!(
            off_thread(move || {
                let text = inline.then(|| base64::encode(&bytes));
                (sha256_hex(&bytes), text, readable(&plugin, &bytes))
            })
            .await
        );
        let value = json!({
            "plugin": {
                "name": &*got.plugin.name,
                "vendor": &*got.plugin.vendor,
                "key": &*got.plugin.key,
                "format": got.format,
            },
            "bytes": got.bytes.len(),
            "sha256": sha256,
            "file": file.as_ref().map(|f| f.display().to_string()),
            "base64": base64,
            "readable": readable.map(|r| json!({ "kind": r.kind, "text": r.text })),
        });
        let lines = [
            format!(
                "The state of {} ({}) now: {} bytes, sha256 {}.",
                got.plugin.name,
                got.format,
                got.bytes.len(),
                value["sha256"].as_str().unwrap_or("")
            ),
            match &file {
                Some(f) => format!("Written to {} (deleted after an hour).", f.display()),
                None => "Given as base64 below.".into(),
            },
            value.to_string(),
        ];
        Ok(done(lines.join("\n"), value))
    }

    /// Puts a whole state into a plugin (as get_plugin_state gives it: base64 or a file), as
    /// one undo step once the plugin has taken it. A state the plugin refuses, or that stops
    /// it, changes nothing in the song: the plugin gets the song's state back.
    #[tool(annotations(
        title = "Set plugin state",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn set_plugin_state(
        &self,
        Parameters(a): Parameters<SetStateArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let bytes = match (a.base64, a.file) {
            (Some(text), None) => {
                if text.len() > MAX_STATE.div_ceil(3) * 4 + MAX_STATE / 32 {
                    return Ok(failed(too_large()));
                }
                let decoded = check!(off_thread(move || base64::decode(&text)).await);
                check!(decoded.ok_or_else(|| failed("`base64` is not base64 text")))
            }
            (None, Some(file)) => check!(check!(off_thread(move || read_state_file(&file)).await)),
            _ => return Ok(failed("give `base64` or `file`, one of them")),
        };
        if bytes.is_empty() {
            return Ok(failed("the state is empty"));
        }
        if bytes.len() > MAX_STATE {
            return Ok(failed(too_large()));
        }
        let read = check!(self.read().await);
        let place = check!(plugin_place(&read, a.track, a.bus, a.slot));
        let (given, sha256) = check!(
            off_thread(move || {
                let sha256 = sha256_hex(&bytes);
                (Arc::<[u8]>::from(bytes), sha256)
            })
            .await
        );
        let size = given.len();
        let d = check!(self.put_plugin_state(place, given).await);
        let mut value = done_value(&d);
        value["given"] = json!({ "bytes": size, "sha256": sha256 });
        value["now"] = match self.plugin_state(place).await {
            Ok(now) => {
                let bytes = now.bytes.clone();
                let sha256 = check!(off_thread(move || sha256_hex(&bytes)).await);
                json!({ "bytes": now.bytes.len(), "sha256": sha256 })
            }
            Err(_) => Value::Null,
        };
        Ok(done(
            format!("Done: set the plugin's state.\n{value}"),
            value,
        ))
    }
}

/// Base64 with the usual alphabet and padding, as the song file keeps plugin states.
mod base64 {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let n = (chunk.iter().enumerate())
                .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    fn six(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    }

    /// Line breaks and spaces are passed over. `None` for anything else that `encode` does
    /// not write.
    pub fn decode(text: &str) -> Option<Vec<u8>> {
        let text: Vec<u8> = (text.bytes())
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        if !text.len().is_multiple_of(4) {
            return None;
        }
        let mut out = Vec::with_capacity(text.len() / 4 * 3);
        let last = text.len().saturating_sub(4);
        for (at, chunk) in text.chunks(4).enumerate() {
            let pad = chunk.iter().rev().take_while(|c| **c == b'=').count();
            if pad > 2 || (pad > 0 && at * 4 != last) {
                return None;
            }
            let mut n = 0u32;
            for c in &chunk[..4 - pad] {
                n = n << 6 | six(*c)?;
            }
            n <<= 6 * pad as u32;
            out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
        }
        Some(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_goes_both_ways() {
            for bytes in [&b""[..], b"f", b"fo", b"foo", b"foob", b"fooba", b"foobar"] {
                assert_eq!(decode(&encode(bytes)).as_deref(), Some(bytes));
            }
            assert_eq!(encode(b"foobar"), "Zm9vYmFy");
            assert_eq!(encode(b"fo"), "Zm8=");
            let all: Vec<u8> = (0..=255).collect();
            assert_eq!(decode(&encode(&all)), Some(all));
            assert_eq!(decode("Zm9v\nYmFy\n").as_deref(), Some(&b"foobar"[..]));
            for bad in ["Zm9", "Zm=v", "Z===", "Zm9v!mFy", "Zg==Zg=="] {
                assert_eq!(decode(bad), None, "{bad}");
            }
        }
    }
}
