//! Everything between meloet and a plugin host process that is not sound: messages on a Unix
//! socket. The host's standard output and standard error are never used for this (plugins
//! write there as they please).
//!
//! A frame is `[u32 text length][u32 blob length][text][blob]`, lengths little endian. The
//! text is one JSON value ([`ToHost`] or [`FromHost`]); the blob carries bytes that would be
//! wasteful as JSON (a plugin's state) and is empty otherwise.
//!
//! meloet sends requests, each with an `id`; the host answers each one with a reply of the
//! same `id`, in any order, and sends notices in between whenever something happens in a
//! plugin.

use std::io::{Read, Write};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::key::{PluginFormat, PluginKey};
use crate::scan::PluginInfo;

pub const WIRE_VERSION: u32 = 1;

/// No message's text is longer than this.
pub const MAX_TEXT_LEN: usize = 16 << 20;
/// No blob is longer than this. A plugin's state is tens of kilobytes; samplers that keep
/// their sounds in the state may come to megabytes.
pub const MAX_BLOB_LEN: usize = 256 << 20;

/// meloet to the host process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToHost {
    pub id: u64,
    pub request: Request,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Request {
    /// First on every connection. The host answers with its own version; when they differ
    /// meloet lets the process go.
    Hello {
        version: u32,
    },
    /// Loads a plugin into a slot of the shared memory and makes it ready to run. The engine's
    /// end of the slot exists already. Answered by `Loaded` once the audio thread runs it.
    Load {
        slot: u32,
        key: PluginKey,
        bundle: PathBuf,
        sample_rate: u32,
        max_frames: u32,
    },
    /// Takes the plugin out of the slot (closing its window). After the `Ok` the host does not
    /// touch the slot any more.
    Unload {
        slot: u32,
    },
    /// The plugin's whole state. The reply `State` carries it in the frame's blob.
    GetState {
        slot: u32,
    },
    /// Puts back a state taken with `GetState` from the same plugin; it is this frame's blob.
    SetState {
        slot: u32,
    },
    /// The list of parameters, in the plugin's order.
    Params {
        slot: u32,
    },
    /// One parameter's value now, with the text the plugin shows for it.
    GetParam {
        slot: u32,
        id: u32,
    },
    /// Sets a parameter outside a block (the audio path uses the slot's ring). `value` 0..=1.
    SetParam {
        slot: u32,
        id: u32,
        value: f64,
    },
    /// Opens the plugin's window, or brings it to the front when it is open.
    OpenEditor {
        slot: u32,
    },
    CloseEditor {
        slot: u32,
    },
    /// Scans one bundle in this process and answers `Scanned`. A scan that must not take the
    /// process's other plugins down with it uses the host's `scan` mode instead.
    Scan {
        format: PluginFormat,
        bundle: PathBuf,
    },
    /// Unloads everything and ends the process.
    Quit,
}

/// The host process to meloet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FromHost {
    Reply { id: u64, reply: Reply },
    Notice { notice: Notice },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Reply {
    Hello {
        version: u32,
    },
    Ok,
    /// The request could not be done. `message` is for the log, not for the screen.
    Error {
        message: String,
    },
    Loaded {
        plugin: LoadedPlugin,
    },
    /// The state is the frame's blob.
    State,
    Params {
        params: Vec<ParamInfo>,
    },
    Param {
        value: f64,
        text: String,
    },
    Scanned {
        output: ScanOutput,
    },
}

/// What meloet needs to know about a plugin it just loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedPlugin {
    /// Its delay in frames at the sample rate it was loaded for (20-8).
    pub latency: u32,
    pub audio_inputs: u32,
    pub audio_outputs: u32,
    /// It has a window meloet can open.
    pub editor: bool,
}

/// One parameter (spec 3: 「パラメータの行」). Values are 0..=1 at this boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamInfo {
    pub id: u32,
    /// As the plugin says it.
    pub name: String,
    /// The group the plugin puts it in, if it names one.
    pub group: Option<String>,
    pub default: f64,
    pub shape: ParamShape,
    /// It may be written by automation.
    pub automatable: bool,
}

/// Which of the three kinds of row the parameter is shown as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ParamShape {
    Continuous,
    /// `steps` values, 3 or more, evenly spread over 0..=1.
    Stepped {
        steps: u32,
    },
    Toggle,
}

/// Things that happen in a plugin without being asked for. The four about the window are what
/// meloet folds a visit to the plugin's window into one step with (spec 3, 15-C).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Notice {
    EditorOpened {
        slot: u32,
    },
    /// Closed by `CloseEditor`, by the person, or by the plugin.
    EditorClosed {
        slot: u32,
    },
    /// The window got the keyboard: the person is in the plugin now.
    EditorEntered {
        slot: u32,
    },
    /// The window lost the keyboard: the person went somewhere else.
    EditorLeft {
        slot: u32,
    },
    /// The person took hold of a parameter in the plugin's window …
    ParamBegin {
        slot: u32,
        id: u32,
    },
    /// … moved it …
    ParamChanged {
        slot: u32,
        id: u32,
        value: f64,
    },
    /// … and let it go.
    ParamEnd {
        slot: u32,
        id: u32,
    },
    /// The plugin says its delay changed, outside a block (VST3 `kLatencyChanged`).
    LatencyChanged {
        slot: u32,
        frames: u32,
    },
}

/// What the host's `scan` mode writes, once, before it ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "result",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScanOutput {
    Found {
        plugins: Vec<PluginInfo>,
    },
    /// 「作りがこわれています」. `message` is for the log.
    Broken {
        message: String,
    },
}

#[derive(Debug)]
pub enum WireError {
    /// The other side closed the socket between two frames.
    Closed,
    Io(std::io::Error),
    /// A length over the limit, or text that is not the message expected.
    Malformed(String),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::Closed => write!(f, "the socket was closed"),
            WireError::Io(e) => write!(f, "socket: {e}"),
            WireError::Malformed(what) => write!(f, "malformed frame: {what}"),
        }
    }
}

impl std::error::Error for WireError {}

impl From<std::io::Error> for WireError {
    fn from(e: std::io::Error) -> WireError {
        WireError::Io(e)
    }
}

/// Writes one frame. A partly written frame cannot be told from a good one by the reader, so
/// the socket is of no more use after an error.
pub fn write_frame<T: Serialize>(
    to: &mut impl Write,
    message: &T,
    blob: &[u8],
) -> Result<(), WireError> {
    let text = serde_json::to_vec(message).map_err(|e| WireError::Malformed(e.to_string()))?;
    if text.len() > MAX_TEXT_LEN || blob.len() > MAX_BLOB_LEN {
        return Err(WireError::Malformed("too long".into()));
    }
    let mut head = [0u8; 8];
    head[..4].copy_from_slice(&(text.len() as u32).to_le_bytes());
    head[4..].copy_from_slice(&(blob.len() as u32).to_le_bytes());
    to.write_all(&head)?;
    to.write_all(&text)?;
    to.write_all(blob)?;
    to.flush()?;
    Ok(())
}

/// Reads one frame: the message and its blob.
pub fn read_frame<T: DeserializeOwned>(from: &mut impl Read) -> Result<(T, Vec<u8>), WireError> {
    let mut head = [0u8; 8];
    let mut got = 0;
    while got < head.len() {
        match from.read(&mut head[got..]) {
            Ok(0) if got == 0 => return Err(WireError::Closed),
            Ok(0) => return Err(WireError::Io(std::io::ErrorKind::UnexpectedEof.into())),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(WireError::Io(e)),
        }
    }
    let text_len = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
    let blob_len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    if text_len > MAX_TEXT_LEN || blob_len > MAX_BLOB_LEN {
        return Err(WireError::Malformed(format!(
            "lengths {text_len} and {blob_len}"
        )));
    }
    let mut text = vec![0u8; text_len];
    from.read_exact(&mut text)?;
    let mut blob = vec![0u8; blob_len];
    from.read_exact(&mut blob)?;
    let message = serde_json::from_slice(&text).map_err(|e| WireError::Malformed(e.to_string()))?;
    Ok((message, blob))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn frames_round_trip_with_their_blobs() {
        let mut bytes = Vec::new();
        let load = ToHost {
            id: 7,
            request: Request::Load {
                slot: 3,
                key: "lv2:urn:zamaudio:ZamComp".parse().unwrap(),
                bundle: "/usr/lib/lv2/ZamComp.lv2".into(),
                sample_rate: 48_000,
                max_frames: 1024,
            },
        };
        let set = ToHost {
            id: 8,
            request: Request::SetState { slot: 3 },
        };
        let state: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
        write_frame(&mut bytes, &load, &[]).unwrap();
        write_frame(&mut bytes, &set, &state).unwrap();
        let mut from = Cursor::new(bytes);
        assert_eq!(read_frame::<ToHost>(&mut from).unwrap(), (load, vec![]));
        assert_eq!(read_frame::<ToHost>(&mut from).unwrap(), (set, state));
        assert!(matches!(
            read_frame::<ToHost>(&mut from),
            Err(WireError::Closed)
        ));
    }

    #[test]
    fn bad_frames_are_errors_not_panics() {
        // Cut off inside the head, and inside the text.
        assert!(matches!(
            read_frame::<ToHost>(&mut Cursor::new(vec![1, 0, 0])),
            Err(WireError::Io(_))
        ));
        let mut bytes = Vec::new();
        write_frame(
            &mut bytes,
            &ToHost {
                id: 1,
                request: Request::Quit,
            },
            &[],
        )
        .unwrap();
        bytes.truncate(bytes.len() - 2);
        assert!(matches!(
            read_frame::<ToHost>(&mut Cursor::new(bytes)),
            Err(WireError::Io(_))
        ));
        // Lengths over the limit are refused before anything is allocated for them.
        let mut head = Vec::new();
        head.extend_from_slice(&u32::MAX.to_le_bytes());
        head.extend_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            read_frame::<ToHost>(&mut Cursor::new(head)),
            Err(WireError::Malformed(_))
        ));
        let mut head = Vec::new();
        head.extend_from_slice(&2u32.to_le_bytes());
        head.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            read_frame::<ToHost>(&mut Cursor::new(head)),
            Err(WireError::Malformed(_))
        ));
        // Text that is not the message.
        let mut bytes = Vec::new();
        write_frame(
            &mut bytes,
            &json!({ "id": 1, "request": { "type": "dance" } }),
            &[],
        )
        .unwrap();
        assert!(matches!(
            read_frame::<ToHost>(&mut Cursor::new(bytes)),
            Err(WireError::Malformed(_))
        ));
    }

    #[test]
    fn message_json_shape() {
        let to = |request: Request| serde_json::to_value(ToHost { id: 1, request }).unwrap();
        assert_eq!(
            to(Request::Hello { version: 1 }),
            json!({ "id": 1, "request": { "type": "hello", "version": 1 } })
        );
        assert_eq!(
            to(Request::Load {
                slot: 0,
                key: PluginKey::vst3("647370206B31732020202020756E7363").unwrap(),
                bundle: "/usr/lib/vst3/lsp-plugins.vst3".into(),
                sample_rate: 48_000,
                max_frames: 256
            }),
            json!({ "id": 1, "request": { "type": "load", "slot": 0, "key": "vst3:647370206B31732020202020756E7363",
                "bundle": "/usr/lib/vst3/lsp-plugins.vst3", "sampleRate": 48000, "maxFrames": 256 } })
        );
        assert_eq!(
            to(Request::SetParam {
                slot: 2,
                id: 9,
                value: 0.5
            }),
            json!({ "id": 1, "request": { "type": "setParam", "slot": 2, "id": 9, "value": 0.5 } })
        );
        assert_eq!(
            to(Request::Scan {
                format: PluginFormat::Lv2,
                bundle: "/usr/lib/lv2/ZamComp.lv2".into()
            }),
            json!({ "id": 1, "request": { "type": "scan", "format": "lv2", "bundle": "/usr/lib/lv2/ZamComp.lv2" } })
        );
        assert_eq!(
            to(Request::Quit),
            json!({ "id": 1, "request": { "type": "quit" } })
        );

        let from = |message: FromHost| serde_json::to_value(message).unwrap();
        assert_eq!(
            from(FromHost::Reply {
                id: 4,
                reply: Reply::Loaded {
                    plugin: LoadedPlugin {
                        latency: 64,
                        audio_inputs: 0,
                        audio_outputs: 2,
                        editor: true
                    }
                }
            }),
            json!({ "kind": "reply", "id": 4, "reply": { "type": "loaded", "plugin": { "latency": 64, "audioInputs": 0, "audioOutputs": 2, "editor": true } } })
        );
        assert_eq!(
            from(FromHost::Reply {
                id: 5,
                reply: Reply::Ok
            }),
            json!({ "kind": "reply", "id": 5, "reply": { "type": "ok" } })
        );
        assert_eq!(
            from(FromHost::Notice {
                notice: Notice::ParamChanged {
                    slot: 1,
                    id: 2,
                    value: 0.25
                }
            }),
            json!({ "kind": "notice", "notice": { "type": "paramChanged", "slot": 1, "id": 2, "value": 0.25 } })
        );
        assert_eq!(
            from(FromHost::Notice {
                notice: Notice::EditorLeft { slot: 1 }
            }),
            json!({ "kind": "notice", "notice": { "type": "editorLeft", "slot": 1 } })
        );
        let param = ParamInfo {
            id: 3,
            name: "Attack".into(),
            group: Some("Envelope".into()),
            default: 0.1,
            shape: ParamShape::Stepped { steps: 5 },
            automatable: true,
        };
        assert_eq!(
            from(FromHost::Reply {
                id: 6,
                reply: Reply::Params {
                    params: vec![param]
                }
            }),
            json!({ "kind": "reply", "id": 6, "reply": { "type": "params", "params": [
                { "id": 3, "name": "Attack", "group": "Envelope", "default": 0.1, "shape": { "kind": "stepped", "steps": 5 }, "automatable": true } ] } })
        );
        assert_eq!(
            serde_json::to_value(ScanOutput::Broken {
                message: "no factory".into()
            })
            .unwrap(),
            json!({ "result": "broken", "message": "no factory" })
        );
    }

    #[test]
    fn every_message_reads_back() {
        let requests = [
            Request::Hello {
                version: WIRE_VERSION,
            },
            Request::Unload { slot: 1 },
            Request::GetState { slot: 1 },
            Request::SetState { slot: 1 },
            Request::Params { slot: 1 },
            Request::GetParam { slot: 1, id: 2 },
            Request::OpenEditor { slot: 1 },
            Request::CloseEditor { slot: 1 },
            Request::Quit,
        ];
        for request in requests {
            let text = serde_json::to_string(&ToHost {
                id: 9,
                request: request.clone(),
            })
            .unwrap();
            assert_eq!(
                serde_json::from_str::<ToHost>(&text).unwrap().request,
                request
            );
        }
        let notices = [
            Notice::EditorOpened { slot: 1 },
            Notice::EditorClosed { slot: 1 },
            Notice::EditorEntered { slot: 1 },
            Notice::EditorLeft { slot: 1 },
            Notice::ParamBegin { slot: 1, id: 2 },
            Notice::ParamChanged {
                slot: 1,
                id: 2,
                value: 1.0,
            },
            Notice::ParamEnd { slot: 1, id: 2 },
            Notice::LatencyChanged {
                slot: 1,
                frames: 128,
            },
        ];
        for notice in notices {
            let message = FromHost::Notice { notice };
            let text = serde_json::to_string(&message).unwrap();
            assert_eq!(serde_json::from_str::<FromHost>(&text).unwrap(), message);
        }
        let replies = [
            Reply::Hello {
                version: WIRE_VERSION,
            },
            Reply::Error {
                message: "x".into(),
            },
            Reply::State,
            Reply::Param {
                value: 0.5,
                text: "-6.0 dB".into(),
            },
            Reply::Scanned {
                output: ScanOutput::Found { plugins: vec![] },
            },
        ];
        for reply in replies {
            let message = FromHost::Reply { id: 2, reply };
            let text = serde_json::to_string(&message).unwrap();
            assert_eq!(serde_json::from_str::<FromHost>(&text).unwrap(), message);
        }
    }
}
