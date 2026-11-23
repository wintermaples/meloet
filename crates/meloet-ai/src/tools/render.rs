//! Rendering into audio files through the export's own engine: into the private temporary
//! folder for the AI app to read, and (`export_song`, in `files`) where the person wants.

use std::path::PathBuf;

use meloet_core::ai::{AiRender, AiRenderWhat, AiRendered};
use meloet_model::{TempoMap, TickRange};
use meloet_protocol::{ExportFormat, RecordFormat};
use rmcp::model::{CallToolResult, ProgressNotificationParam, RequestMetaObject};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::analyze::numbers;
use super::{MAX_MEASURE_S, Tools, closing, done, failed, refusal_text};
use crate::MAX_BAR;
use crate::trackref::TrackRef;

/// A note `render_note` plays at most, in seconds.
const MAX_NOTE_S: f64 = 30.0;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RenderArgs {
    /// The first bar (from 1).
    #[schemars(range(min = 1, max = 9999))]
    pub from_bar: u32,
    /// The last bar, rendered to its end.
    #[schemars(range(min = 1, max = 9999))]
    pub to_bar: u32,
    /// Tracks (numbers from 1 or ids like "id:7") to render; the mix then holds only these.
    /// Leave out for every track.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub tracks: Option<Vec<TrackRef>>,
    /// A file for each track (played alone with its sends and the master) in place of one
    /// mix.
    #[serde(default)]
    pub per_track: bool,
    /// Let the sound ring out after the last bar (up to 10 s; default false).
    #[serde(default)]
    pub tail: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct NoteArgs {
    /// The keys track whose instrument plays it.
    pub track: TrackRef,
    /// MIDI key, 0 to 127 (60 is C4).
    #[schemars(range(min = 0, max = 127))]
    pub key: u32,
    /// 1 to 127 (default 100).
    #[serde(default)]
    #[schemars(range(min = 1, max = 127))]
    pub velocity: Option<u32>,
    /// How long the key is held, 0.05 to 30 s (default 1). The release rings after it.
    #[serde(default)]
    #[schemars(range(min = 0.05, max = 30.0))]
    pub seconds: Option<f64>,
    /// Through the track's effects, fader, sends and the master (default false: the
    /// instrument alone).
    #[serde(default)]
    pub effects: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ClipArgs {
    /// A clip's id from song_overview.
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// Through the track's effects, fader, sends and the master (default false: the clip
    /// alone, with its own gain, fades and stretch).
    #[serde(default)]
    pub effects: bool,
}

pub(super) fn rendered_value(r: &AiRendered) -> Value {
    json!({
        "files": r.files.iter().map(|f| f.display().to_string()).collect::<Vec<_>>(),
        "sample_rate": r.sample_rate,
        "seconds": r.seconds,
        "peak_dbfs": r.peak_db.map(|db| (f64::from(db) * 100.0).round() / 100.0),
    })
}

impl Tools {
    /// Renders `ask` on a thread of the blocking pool, telling the progress when a
    /// `progressToken` came.
    pub(super) async fn run_render(
        &self,
        ask: AiRender,
        meta: &RequestMetaObject,
        peer: &Peer<RoleServer>,
        ct: &CancellationToken,
    ) -> Result<AiRendered, CallToolResult> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let meloet = self.meloet.clone();
        let stop = ct.clone();
        let mut job = tokio::task::spawn_blocking(move || {
            let mut progress = |percent| {
                let _ = tx.send(percent);
            };
            meloet.render(ask, &mut progress, &|| stop.is_cancelled())
        });
        let token = meta.get_progress_token();
        let mut last = None;
        let answer = loop {
            tokio::select! {
                answer = &mut job => break answer,
                Some(percent) = rx.recv() => {
                    if let Some(token) = &token
                        && last.is_none_or(|l| percent > l)
                    {
                        last = Some(percent);
                        let note = ProgressNotificationParam::new(token.clone(), f64::from(percent))
                            .with_total(100.0);
                        let _ = peer.notify_progress(note).await;
                    }
                }
            }
        };
        match answer {
            Ok(Ok(rendered)) => Ok(rendered),
            Ok(Err(r)) => Err(failed(format!("Could not render: {}.", refusal_text(&r)))),
            Err(_) => Err(closing()),
        }
    }

    /// A new place in the temporary folder, or why there is none.
    fn scratch_place(&self, what: &str, ending: &str) -> Result<PathBuf, CallToolResult> {
        self.scratch.clean(crate::scratch::KEEP);
        let dir = self.scratch.dir().map_err(|e| {
            failed(format!(
                "this tool cannot be used: the private temporary folder is not usable ({e})"
            ))
        })?;
        let place = self.scratch.name(&dir, what, ending);
        if ending.is_empty() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&place)
                .map_err(|e| failed(format!("this tool cannot be used now ({e})")))?;
        }
        Ok(place)
    }

    async fn render_scratch(
        &self,
        what: AiRenderWhat,
        name: &str,
        folder: bool,
        meta: &RequestMetaObject,
        peer: &Peer<RoleServer>,
        ct: &CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        let read = check!(self.read().await);
        let to = check!(self.scratch_place(name, if folder { "" } else { "wav" }));
        let ask = AiRender {
            what,
            to: to.clone(),
            format: ExportFormat::Wav,
            bits: RecordFormat::Bits16,
            sample_rate: Some(read.sample_rate),
            overwrite: false,
        };
        let rendered = match self.run_render(ask, meta, peer, ct).await {
            Ok(r) => r,
            Err(result) => {
                let _ = match folder {
                    true => std::fs::remove_dir_all(&to),
                    false => std::fs::remove_file(&to),
                };
                return Ok(result);
            }
        };
        let value = rendered_value(&rendered);
        let mut lines = vec![format!(
            "Wrote {} WAV file(s), 16-bit, {} Hz, {:.2} s; peak {}. They are deleted after an \
             hour:",
            rendered.files.len(),
            rendered.sample_rate,
            rendered.seconds,
            rendered
                .peak_db
                .map_or_else(|| "silent".to_owned(), |db| format!("{db:.1} dBFS")),
        )];
        lines.extend(rendered.files.iter().map(|f| f.display().to_string()));
        Ok(done(lines.join("\n"), value))
    }
}

#[tool_router(router = render_router, vis = "pub(super)")]
impl Tools {
    /// Renders bars of the song (all tracks or some, as one mix or a file each) into WAV
    /// files in a private temporary folder, and gives their paths. Changes nothing.
    #[tool(annotations(title = "Render audio", read_only_hint = true, open_world_hint = false))]
    async fn render_audio(
        &self,
        Parameters(a): Parameters<RenderArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.from_bar == 0 || a.to_bar < a.from_bar || u64::from(a.to_bar) > MAX_BAR {
            return Ok(failed(format!("give 1 <= from_bar <= to_bar <= {MAX_BAR}")));
        }
        if a.tracks
            .as_ref()
            .is_some_and(|t| t.is_empty() || t.len() > 64)
        {
            return Ok(failed("give 1 to 64 track numbers, or leave `tracks` out"));
        }
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let start = map.tick_of_bar(u64::from(a.from_bar) - 1);
        let end = map.tick_of_bar(u64::from(a.to_bar));
        let seconds = map.seconds_at(end as f64) - map.seconds_at(start as f64);
        if seconds > MAX_MEASURE_S {
            return Ok(failed(format!(
                "that is {seconds:.0} s of the song; render at most {MAX_MEASURE_S:.0} s at a time"
            )));
        }
        let Some(span) = TickRange::new(start, end) else {
            return Ok(failed("the range is empty"));
        };
        let what = AiRenderWhat::Song {
            span,
            tracks: match &a.tracks {
                Some(tracks) => Some(check!(numbers(&read, tracks))),
                None => None,
            },
            per_track: a.per_track,
            tail: a.tail,
        };
        let name = format!("bars-{}-{}", a.from_bar, a.to_bar);
        self.render_scratch(what, &name, a.per_track, &meta, &peer, &ct)
            .await
    }

    /// Plays one note through a track's instrument (plugin, SF2 or built-in) and writes it
    /// into a WAV file in a private temporary folder, to learn how the sound is. Gives the
    /// path. Changes nothing.
    #[tool(annotations(title = "Render note", read_only_hint = true, open_world_hint = false))]
    async fn render_note(
        &self,
        Parameters(a): Parameters<NoteArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let velocity = a.velocity.unwrap_or(100);
        let seconds = a.seconds.unwrap_or(1.0);
        let (Ok(key), Ok(velocity)) = (u8::try_from(a.key), u8::try_from(velocity)) else {
            return Ok(failed("the key is 0 to 127 and the velocity 1 to 127"));
        };
        if key > 127 || !(1..=127).contains(&velocity) {
            return Ok(failed("the key is 0 to 127 and the velocity 1 to 127"));
        }
        if !(0.05..=MAX_NOTE_S).contains(&seconds) {
            return Ok(failed(format!("hold it 0.05 to {MAX_NOTE_S} s")));
        }
        let read = check!(self.read().await);
        let track = check!(a.track.number(&read).map_err(failed));
        let what = AiRenderWhat::Note {
            track,
            key,
            velocity,
            seconds,
            effects: a.effects,
        };
        let name = format!(
            "note-track{}-{}-v{velocity}",
            track,
            crate::song::key_name(key)
        );
        self.render_scratch(what, &name, false, &meta, &peer, &ct)
            .await
    }

    /// Writes a clip as it sounds (gain, fades, stretch and pitch) into a WAV file in a
    /// private temporary folder, and gives the path. Changes nothing.
    #[tool(annotations(
        title = "Get clip audio",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_clip_audio(
        &self,
        Parameters(a): Parameters<ClipArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let Some((_, clip)) = read.song.doc.clip(meloet_model::ClipId(a.clip)) else {
            return Ok(failed(format!(
                "there is no clip {}; call song_overview to see the clips",
                a.clip
            )));
        };
        let map = TempoMap::new(&read.song.doc.timeline);
        let seconds = map.seconds_at(clip.end as f64) - map.seconds_at(clip.start as f64);
        if seconds > MAX_MEASURE_S {
            return Ok(failed(format!(
                "the clip is {seconds:.0} s long; at most {MAX_MEASURE_S:.0} s are written"
            )));
        }
        let what = AiRenderWhat::Clip {
            clip: a.clip,
            effects: a.effects,
        };
        let name = format!("clip-{}", a.clip);
        self.render_scratch(what, &name, false, &meta, &peer, &ct)
            .await
    }
}
