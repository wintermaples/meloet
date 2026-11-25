//! The song's files: saving, opening, a new song, and writing the song out as an audio file
//! as 「書き出し」 does.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use meloet_core::ai::{AiAsk, AiRender, AiRenderWhat, AiSong};
use meloet_core::settings::SAMPLE_RATE_CHOICES;
use meloet_model::TickRange;
use meloet_protocol::{ExportFormat, RecordFormat};
use rmcp::model::{CallToolResult, RequestMetaObject};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::render::rendered_value;
use super::{Tools, closing, done, done_value, failed, path_ok};

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
    /// Not for the AI link: only meloet's own export dialog writes mp3.
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

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ListSongsArgs {
    /// A folder to look in, an absolute path: the song files in it and in the folders right
    /// under it. Leave out for the songs opened or saved lately, the song open now and
    /// meloet's songs folder.
    #[serde(default)]
    #[schemars(length(min = 1, max = 4096))]
    pub folder: Option<String>,
    /// How many songs of the folder to give at most (default 100, most 500).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    /// How many songs of the folder to skip first, newest first (default 0).
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Songs a folder is looked through for, at most.
const MOST_SONGS: usize = 500;
/// Entries of a folder and the folders right under it that are looked at, at most.
const MOST_ENTRIES: usize = 20_000;
const DEFAULT_LIMIT: usize = 100;

/// "2026-11-23T05:01:02Z".
fn utc_text(time: SystemTime) -> Option<String> {
    let secs = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    ))
}

/// A song file as list_songs gives it. `meta`: `None` for a file that is gone.
fn song_value(path: &Path, meta: Option<&fs::Metadata>, open: Option<&Path>) -> Value {
    json!({
        "path": path.display().to_string(),
        "name": path.file_stem().map(|n| n.to_string_lossy().into_owned()),
        "modified": meta.and_then(|m| m.modified().ok()).and_then(utc_text),
        "size": meta.map(fs::Metadata::len),
        "open_now": open == Some(path),
    })
}

/// A song file's name: `<name>.meloet`, not hidden. Not `.prev`, not a temporary file.
fn is_song(name: &OsStr) -> bool {
    let name = Path::new(name);
    !name.to_string_lossy().starts_with('.')
        && name.extension().is_some_and(|e| e == SONG_ENDING)
        && name.file_stem().is_some_and(|s| !s.is_empty())
}

/// What a look through folders found so far.
struct Look {
    songs: Vec<(PathBuf, fs::Metadata)>,
    looked: usize,
}

impl Look {
    /// The song files of `dir`; its folders go into `under` when it is given. `false` once a
    /// limit is reached.
    fn dir(&mut self, dir: &Path, mut under: Option<&mut Vec<PathBuf>>) -> std::io::Result<bool> {
        for entry in fs::read_dir(dir)? {
            self.looked += 1;
            if self.looked > MOST_ENTRIES || self.songs.len() >= MOST_SONGS {
                return Ok(false);
            }
            let Ok(entry) = entry else { continue };
            // The kind of the entry itself: a symbolic link is neither.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            if kind.is_file() && is_song(&name) {
                if let Ok(meta) = entry.metadata() {
                    self.songs.push((entry.path(), meta));
                }
            } else if kind.is_dir()
                && !name.to_string_lossy().starts_with('.')
                && let Some(under) = under.as_deref_mut()
            {
                under.push(entry.path());
            }
        }
        Ok(true)
    }
}

/// The song files of `folder` and of the folders right under it, newest first, and whether
/// every place was looked at. Symbolic links are not followed.
fn songs_in(folder: &Path) -> std::io::Result<(Vec<(PathBuf, fs::Metadata)>, bool)> {
    let mut look = Look {
        songs: Vec::new(),
        looked: 0,
    };
    let mut under = Vec::new();
    let mut complete = look.dir(folder, Some(&mut under))?;
    under.sort();
    for dir in &under {
        if !complete {
            break;
        }
        // A folder that cannot be read is passed by.
        complete = look.dir(dir, None).unwrap_or(true);
    }
    let modified = |m: &fs::Metadata| m.modified().ok();
    (look.songs).sort_by(|a, b| modified(&b.1).cmp(&modified(&a.1)).then(a.0.cmp(&b.0)));
    Ok((look.songs, complete))
}

/// A page of the songs of `folder`.
fn folder_value(
    folder: &Path,
    open: Option<&Path>,
    limit: usize,
    offset: usize,
) -> Result<Value, String> {
    let (songs, complete) =
        songs_in(folder).map_err(|e| format!("{} cannot be read ({e})", folder.display()))?;
    let page: Vec<Value> = (songs.iter().skip(offset).take(limit))
        .map(|(path, meta)| song_value(path, Some(meta), open))
        .collect();
    Ok(json!({
        "folder": folder.display().to_string(),
        "total": songs.len(),
        "offset": offset,
        "songs": page,
        "complete": complete,
    }))
}

/// What list_songs gives: of `folder`, or with none of the songs meloet knows.
fn songs_listed(
    folder: Option<&Path>,
    read: &AiSong,
    limit: usize,
    offset: usize,
) -> Result<Value, String> {
    let open = read.file.as_deref();
    if let Some(folder) = folder {
        return folder_value(folder, open, limit, offset);
    }
    let recent: Vec<Value> = (read.recent.iter())
        .filter_map(|path| {
            let meta = fs::metadata(path).ok().filter(fs::Metadata::is_file)?;
            Some(song_value(path, Some(&meta), open))
        })
        .collect();
    let open_now = open.map(|path| song_value(path, fs::metadata(path).ok().as_ref(), open));
    let songs_folder = (read.songs_dir.as_deref()).map(|dir| {
        folder_value(dir, open, limit, offset).unwrap_or_else(|_| {
            json!({ "folder": dir.display().to_string(), "total": 0, "offset": offset,
                    "songs": [], "complete": true, "exists": false })
        })
    });
    Ok(json!({
        "song_open_now": open_now,
        "recent": recent,
        "songs_folder": songs_folder,
    }))
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

    /// Song files to give to open_song, newest first. No `folder`: the songs opened or saved
    /// lately, the song open now and meloet's songs folder. With `folder`: the songs in it and
    /// one folder down (meloet saves `<name>/<name>.meloet`). Each with its name, when it was
    /// written (UTC), its size and `open_now`. Only song files; symbolic links are not
    /// followed.
    #[tool(annotations(title = "List songs", read_only_hint = true, open_world_hint = false))]
    async fn list_songs(
        &self,
        Parameters(a): Parameters<ListSongsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let limit = a
            .limit
            .map_or(DEFAULT_LIMIT, |l| (l as usize).clamp(1, MOST_SONGS));
        let offset = a.offset.unwrap_or(0) as usize;
        let folder = match &a.folder {
            Some(text) => Some(check!(folder_ok(text))),
            None => None,
        };
        let read = check!(self.read().await);
        let look = move || songs_listed(folder.as_deref(), &read, limit, offset);
        let value = match tokio::task::spawn_blocking(look).await {
            Ok(Ok(value)) => value,
            Ok(Err(why)) => return Ok(failed(why)),
            Err(_) => return Ok(closing()),
        };
        let mut lines = Vec::new();
        if a.folder.is_none() {
            lines.push(match value["song_open_now"]["path"].as_str() {
                Some(path) => format!("Open now: {path}"),
                None => "Open now: a song never saved.".to_owned(),
            });
            let recent = value["recent"].as_array().map_or(0, Vec::len);
            lines.push(format!("{recent} song(s) opened or saved lately."));
        }
        let shown = match &a.folder {
            Some(_) => &value,
            None => &value["songs_folder"],
        };
        if let Some(folder) = shown["folder"].as_str() {
            lines.push(format!(
                "{} song(s) in {folder}{}.",
                shown["total"],
                if shown["complete"] == false {
                    " (not all looked at)"
                } else {
                    ""
                }
            ));
        }
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
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
                    "the AI link writes wav or flac; MP3 is only written from meloet's own \
                     export dialog",
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
            RangeArg::Loop => match doc.loop_range {
                Some(range) => Some(range),
                None => {
                    return Ok(failed(
                        "the song has no loop range; set one with set_loop, or give range \"song\"",
                    ));
                }
            },
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn times_are_utc_in_iso_8601() {
        let at = |secs: u64| utc_text(UNIX_EPOCH + Duration::from_secs(secs)).unwrap();
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_868_799), "2000-02-29T23:59:59Z");
        assert_eq!(at(1_795_410_062), "2026-11-23T05:01:02Z");
    }

    #[test]
    fn only_song_files_are_songs() {
        for (name, song) in [
            ("曲.meloet", true),
            ("曲.meloet.prev", false),
            (".曲.meloet.new", false),
            (".hidden.meloet", false),
            ("recovery.json", false),
            ("曲.MELOET", false),
            (".meloet", false),
        ] {
            assert_eq!(is_song(OsStr::new(name)), song, "{name}");
        }
    }
}
