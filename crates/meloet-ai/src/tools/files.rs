//! The song's files: saving, opening, a new song, and writing the song out as an audio file
//! as 「書き出し」 does.

use meloet_core::ai::{AiAsk, AiRender, AiRenderWhat};
use meloet_core::settings::SAMPLE_RATE_CHOICES;
use meloet_model::TickRange;
use meloet_protocol::{ExportFormat, RecordFormat};
use rmcp::model::{CallToolResult, RequestMetaObject};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::render::rendered_value;
use super::{Tools, done, done_value, failed, path_ok};

/// The ending of a song file.
const SONG_ENDING: &str = "meloet";

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SaveAsArgs {
    /// The folder the song's folder is made in: an absolute path that is there.
    #[schemars(length(min = 1, max = 4096))]
    pub folder: String,
    /// The song's name: its folder and its file `<name>.meloet` are named so.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct OpenArgs {
    /// A song file (`.meloet`), an absolute path.
    #[schemars(length(min = 1, max = 4096))]
    pub path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RangeArg {
    /// From the start to where the last clip ends.
    Song,
    /// The loop's bars.
    Loop,
    /// A file for each track, named as the track, into `folder`.
    Tracks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FormatArg {
    Wav,
    Flac,
    /// Not available yet, as in meloet's own export.
    Mp3,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ExportArgs {
    /// "song" (default), "loop" or "tracks".
    #[serde(default)]
    pub range: Option<RangeArg>,
    /// "wav" (default) or "flac".
    #[serde(default)]
    pub format: Option<FormatArg>,
    /// The folder the file goes into: an absolute path that is there.
    #[schemars(length(min = 1, max = 4096))]
    pub folder: String,
    /// The file's name without its ending (default: the song's name). Not for "tracks".
    #[serde(default)]
    #[schemars(length(min = 1, max = 128))]
    pub name: Option<String>,
    /// 44100, 48000, 88200 or 96000 (default: the rate meloet runs at).
    #[serde(default)]
    pub sample_rate: Option<u32>,
    /// 16, 24 (default) or 32 (float).
    #[serde(default)]
    pub bits: Option<u32>,
    /// Let the sound ring out after the end, up to 10 s (default true).
    #[serde(default)]
    pub tail: Option<bool>,
    /// Write over a file of the same name (default false: that is an error).
    #[serde(default)]
    pub overwrite: bool,
}

/// A name a file can have: no slash, no control characters, not "." or "..".
fn file_name_ok(name: &str, most: usize) -> Result<String, CallToolResult> {
    let name = name.trim();
    let bad = name.is_empty()
        || name.chars().count() > most
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control());
    if bad {
        return Err(failed(format!(
            "a name is 1 to {most} characters, with no slash or control characters"
        )));
    }
    Ok(name.to_owned())
}

fn folder_ok(text: &str) -> Result<std::path::PathBuf, CallToolResult> {
    let folder = path_ok(text)?;
    if !folder.is_dir() {
        return Err(failed(format!("{} is not a folder", folder.display())));
    }
    Ok(folder)
}

#[tool_router(router = files_router, vis = "pub(super)")]
impl Tools {
    /// Saves the song over its file. A song never saved needs save_song_as.
    #[tool(annotations(
        title = "Save song",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn save_song(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let (_, path) = check!(self.ticketed(AiAsk::Save).await);
        let value = json!({ "file": path.map(|p| p.display().to_string()) });
        Ok(done(format!("Saved.\n{value}"), value))
    }

    /// Saves the song as a new song: a folder `name` in `folder`, holding `name.meloet` (as
    /// 「名前をつけて保存」). The song is that file from then on.
    #[tool(annotations(
        title = "Save song as",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn save_song_as(
        &self,
        Parameters(a): Parameters<SaveAsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let folder = check!(folder_ok(&a.folder));
        let name = check!(file_name_ok(&a.name, 64));
        let name = name
            .strip_suffix(&format!(".{SONG_ENDING}"))
            .unwrap_or(&name)
            .to_owned();
        let (_, path) = check!(self.ticketed(AiAsk::SaveAs { folder, name }).await);
        let value = json!({ "file": path.map(|p| p.display().to_string()) });
        Ok(done(format!("Saved.\n{value}"), value))
    }

    /// Opens a song file in place of this song. Refused while the song has unsaved changes:
    /// save first, or ask the person.
    #[tool(annotations(
        title = "Open song",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn open_song(
        &self,
        Parameters(a): Parameters<OpenArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let path = check!(path_ok(&a.path));
        if path.extension().is_none_or(|e| e != SONG_ENDING) || !path.is_file() {
            return Ok(failed(format!(
                "{} is not a song file (.{SONG_ENDING})",
                path.display()
            )));
        }
        let (_, path) = check!(self.ticketed(AiAsk::Open { path }).await);
        let value = json!({ "file": path.map(|p| p.display().to_string()) });
        Ok(done(format!("Opened.\n{value}"), value))
    }

    /// Starts a new, empty song in place of this one. Refused while the song has unsaved
    /// changes: save first, or ask the person.
    #[tool(annotations(
        title = "New song",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn new_song(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let d = check!(self.ask(AiAsk::NewSong).await);
        let value = done_value(&d);
        Ok(done(format!("Done: a new song.\n{value}"), value))
    }

    /// Writes the song out as an audio file (or a file for each track), as meloet's export
    /// does. A file of the same name is written over only with `overwrite`. Changes nothing
    /// in the song.
    #[tool(annotations(
        title = "Export song",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn export_song(
        &self,
        Parameters(a): Parameters<ExportArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let format = match a.format.unwrap_or(FormatArg::Wav) {
            FormatArg::Wav => ExportFormat::Wav,
            FormatArg::Flac => ExportFormat::Flac,
            FormatArg::Mp3 => {
                return Ok(failed(
                    "MP3 cannot be written yet (meloet's export does not offer it either); use \
                     wav or flac",
                ));
            }
        };
        let bits = match a.bits.unwrap_or(24) {
            16 => RecordFormat::Bits16,
            24 => RecordFormat::Bits24,
            32 if format == ExportFormat::Wav => RecordFormat::Bits32,
            _ => return Ok(failed("bits are 16 or 24 (or 32 for wav)")),
        };
        if a.sample_rate
            .is_some_and(|r| !SAMPLE_RATE_CHOICES.contains(&r))
        {
            return Ok(failed("the sample rate is 44100, 48000, 88200 or 96000"));
        }
        let folder = check!(folder_ok(&a.folder));
        let range = a.range.unwrap_or(RangeArg::Song);
        let read = check!(self.read().await);
        let doc = &read.song.doc;
        let song_end = (doc.tracks.iter().flat_map(|t| t.clips.iter()))
            .map(|c| c.end)
            .max()
            .unwrap_or(0);
        let span = match range {
            RangeArg::Loop => doc.loop_range.or_else(|| TickRange::new(0, song_end)),
            RangeArg::Song | RangeArg::Tracks => TickRange::new(0, song_end),
        };
        let Some(span) = span else {
            return Ok(failed("the song is empty: there is nothing to write out"));
        };
        let per_track = range == RangeArg::Tracks;
        let to = match per_track {
            true => folder,
            false => {
                let name = match &a.name {
                    Some(name) => check!(file_name_ok(name, 128)),
                    None => (doc.name.chars())
                        .map(|c| if c == '/' || c.is_control() { '_' } else { c })
                        .collect(),
                };
                folder.join(format!("{name}.{}", format.extension()))
            }
        };
        let ask = AiRender {
            what: AiRenderWhat::Song {
                span,
                tracks: None,
                per_track,
                tail: a.tail.unwrap_or(true),
            },
            to,
            format,
            bits,
            sample_rate: a.sample_rate,
            overwrite: a.overwrite,
        };
        let rendered = check!(self.run_render(ask, &meta, &peer, &ct).await);
        let value = rendered_value(&rendered);
        let mut lines = vec![format!(
            "Wrote {} file(s), {:.0} s:",
            rendered.files.len(),
            rendered.seconds
        )];
        lines.extend(rendered.files.iter().map(|f| f.display().to_string()));
        Ok(done(lines.join("\n"), value))
    }
}
