//! Tracks and clips: deleting, naming, moving, cutting, joining, takes, and audio clips
//! (bringing a file in, gain, fades, pitch and stretch).

use meloet_core::ai::AiAsk;
use meloet_model::TempoMap;
use meloet_protocol::{ClipField, Command, StretchChange};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::json;

use super::{Tools, at_or_tick, done, done_value, failed, name_ok, path_ok, track_of};
use crate::trackref::TrackRef;

/// Clips one call takes.
const MAX_CLIPS: usize = 256;

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct TrackArgs {
    pub track: TrackRef,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct RenameTrackArgs {
    pub track: TrackRef,
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MoveTrackArgs {
    pub track: TrackRef,
    /// Its number afterwards (from 1).
    #[schemars(range(min = 1, max = 64))]
    pub to: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ClipsArgs {
    /// Clip ids from song_overview.
    #[schemars(length(min = 1, max = 256))]
    pub clips: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MoveClipsArgs {
    #[schemars(length(min = 1, max = 256))]
    pub clips: Vec<u32>,
    /// Ticks later (earlier below 0).
    #[serde(default)]
    pub ticks: i64,
    /// Tracks down (up below 0).
    #[serde(default)]
    #[schemars(range(min = -63, max = 63))]
    pub tracks: i32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SplitArgs {
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// Where to cut, as "bar.beat.tick". Give this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct TakeArgs {
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// The take's number, from 1 (song_overview gives how many there are).
    #[schemars(range(min = 1))]
    pub take: u32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ImportArgs {
    /// The audio file (WAV, FLAC, MP3, OGG …), an absolute path.
    #[schemars(length(min = 2, max = 4096))]
    pub path: String,
    /// A vocal track (number or id); leave out for a new vocal track.
    #[serde(default)]
    pub track: Option<TrackRef>,
    /// Where the clip starts, as "bar.beat.tick". Give this or `tick`.
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub tick: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FieldArg {
    /// dB.
    GainDb,
    /// Ticks.
    FadeIn,
    /// Ticks.
    FadeOut,
    /// Ticks: where it starts (it moves as it is).
    Start,
    /// Ticks: its end moves.
    Length,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ClipFieldArgs {
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// What to set: "gain_db", "fade_in", "fade_out" (audio clips), "start", "length".
    pub field: FieldArg,
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct StretchArgs {
    /// An audio clip's id.
    #[schemars(range(min = 1))]
    pub clip: u32,
    /// Follow the song's tempo (stretching with it).
    #[serde(default)]
    pub follow_tempo: Option<bool>,
    /// The audio's own tempo, quarter notes a minute.
    #[serde(default)]
    pub source_bpm: Option<f64>,
    /// The pitch in semitones.
    #[serde(default)]
    pub semitones: Option<f64>,
    /// The pitch's cents.
    #[serde(default)]
    pub cents: Option<f64>,
    /// Keep the character of a voice when the pitch moves.
    #[serde(default)]
    pub keep_formant: Option<bool>,
}

fn clips_ok(clips: &[u32]) -> Result<(), CallToolResult> {
    if clips.is_empty() || clips.len() > MAX_CLIPS {
        return Err(failed(format!("give 1 to {MAX_CLIPS} clips")));
    }
    Ok(())
}

#[tool_router(router = arrange_router, vis = "pub(super)")]
impl Tools {
    /// Deletes a track with its clips and lanes, as one undo step.
    #[tool(annotations(
        title = "Delete track",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_track(
        &self,
        Parameters(a): Parameters<TrackArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let track = check!(track_of(&read, a.track)).id.0;
        Ok(self.screen(Command::DeleteTrack { track }).await)
    }

    /// Renames a track, as one undo step.
    #[tool(annotations(
        title = "Rename track",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn rename_track(
        &self,
        Parameters(a): Parameters<RenameTrackArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok(&a.name));
        let read = check!(self.read().await);
        let track = check!(track_of(&read, a.track)).id.0;
        Ok(self.screen(Command::RenameTrack { track, name }).await)
    }

    /// Moves a track to another place among the tracks, as one undo step.
    #[tool(annotations(
        title = "Move track",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn move_track(
        &self,
        Parameters(a): Parameters<MoveTrackArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let track = check!(track_of(&read, a.track)).id.0;
        check!(track_of(&read, TrackRef::Number(a.to)));
        Ok(self
            .screen(Command::MoveTrack {
                track,
                to: a.to - 1,
            })
            .await)
    }

    /// Adds a vocal track (for audio), as one undo step.
    #[tool(annotations(
        title = "Add vocal track",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn add_vocal_track(&self) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        Ok(self.screen(Command::AddVocalTrack).await)
    }

    /// Deletes clips, as one undo step.
    #[tool(annotations(
        title = "Delete clips",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_clips(
        &self,
        Parameters(a): Parameters<ClipsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        check!(clips_ok(&a.clips));
        Ok(self.write(AiAsk::DeleteClips { clips: a.clips }).await)
    }

    /// Moves clips along and to other tracks, together, as one undo step. Nothing moves when
    /// one would overlap another clip.
    #[tool(annotations(
        title = "Move clips",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn move_clips(
        &self,
        Parameters(a): Parameters<MoveClipsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        check!(clips_ok(&a.clips));
        Ok(self
            .screen(Command::MoveClips {
                clips: a.clips,
                ticks: a
                    .ticks
                    .clamp(-(crate::MAX_TICK as i64), crate::MAX_TICK as i64),
                tracks: a.tracks.clamp(-63, 63),
                copy: false,
            })
            .await)
    }

    /// Copies clips right after themselves, as one undo step.
    #[tool(annotations(
        title = "Duplicate clips",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn duplicate_clips(
        &self,
        Parameters(a): Parameters<ClipsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        check!(clips_ok(&a.clips));
        Ok(self
            .screen(Command::DuplicateClips { clips: a.clips })
            .await)
    }

    /// Cuts a clip in two at a place, as one undo step.
    #[tool(annotations(
        title = "Split clip",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn split_clip(
        &self,
        Parameters(a): Parameters<SplitArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = check!(at_or_tick(&map, a.at.as_deref(), a.tick));
        Ok(self
            .screen(Command::SplitClipAt { clip: a.clip, tick })
            .await)
    }

    /// Joins keys clips of one track into one, as one undo step.
    #[tool(annotations(
        title = "Join clips",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn join_clips(
        &self,
        Parameters(a): Parameters<ClipsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.clips.len() < 2 || a.clips.len() > MAX_CLIPS {
            return Ok(failed(format!("give 2 to {MAX_CLIPS} clips")));
        }
        Ok(self.screen(Command::JoinClips { clips: a.clips }).await)
    }

    /// Shows another take of a clip, as one undo step.
    #[tool(annotations(
        title = "Switch take",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn switch_take(
        &self,
        Parameters(a): Parameters<TakeArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if a.take == 0 {
            return Ok(failed("takes are numbered from 1"));
        }
        Ok(self
            .write(AiAsk::SwitchTake {
                clip: a.clip,
                index: a.take - 1,
            })
            .await)
    }

    /// Sets a clip's gain, fades, start or length, as one undo step.
    #[tool(annotations(
        title = "Set clip field",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_clip(
        &self,
        Parameters(a): Parameters<ClipFieldArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        if !a.value.is_finite() {
            return Ok(failed("the value must be a number"));
        }
        let field = match a.field {
            FieldArg::GainDb => ClipField::GainDb,
            FieldArg::FadeIn => ClipField::FadeIn,
            FieldArg::FadeOut => ClipField::FadeOut,
            FieldArg::Start => ClipField::Start,
            FieldArg::Length => ClipField::Length,
        };
        Ok(self
            .screen(Command::SetClipField {
                clip: a.clip,
                field,
                value: a.value,
            })
            .await)
    }

    /// Sets one of an audio clip's stretch and pitch values (give exactly one), as one undo
    /// step.
    #[tool(annotations(
        title = "Set clip stretch",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn set_clip_stretch(
        &self,
        Parameters(a): Parameters<StretchArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let mut changes = Vec::new();
        if let Some(on) = a.follow_tempo {
            changes.push(StretchChange::Follow { on });
        }
        if let Some(bpm) = a.source_bpm {
            changes.push(StretchChange::SourceBpm { bpm });
        }
        if let Some(semitones) = a.semitones {
            changes.push(StretchChange::Semitones { semitones });
        }
        if let Some(cents) = a.cents {
            changes.push(StretchChange::Cents { cents });
        }
        if let Some(on) = a.keep_formant {
            changes.push(StretchChange::KeepFormant { on });
        }
        let [change] = changes[..] else {
            return Ok(failed("give exactly one value to change"));
        };
        let finite = match change {
            StretchChange::SourceBpm { bpm: v }
            | StretchChange::Semitones { semitones: v }
            | StretchChange::Cents { cents: v } => v.is_finite(),
            _ => true,
        };
        if !finite {
            return Ok(failed("the value must be a number"));
        }
        Ok(self
            .screen(Command::SetClipStretch {
                clip: a.clip,
                change,
            })
            .await)
    }

    /// Brings an audio file into the song as a clip on a vocal track (a new one when no
    /// track is given), as one undo step. The file is copied into the song's folder.
    #[tool(annotations(
        title = "Import audio",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn import_audio(
        &self,
        Parameters(a): Parameters<ImportArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let path = check!(path_ok(&a.path));
        if !path.is_file() {
            return Ok(failed(format!("{} is not a file", path.display())));
        }
        let read = check!(self.read().await);
        let map = TempoMap::new(&read.song.doc.timeline);
        let tick = match (&a.at, a.tick) {
            (None, None) => 0,
            (at, tick) => check!(at_or_tick(&map, at.as_deref(), tick)),
        };
        let track = match a.track {
            Some(n) => Some(check!(track_of(&read, n)).id.0),
            None => None,
        };
        let (d, _) = check!(
            self.ticketed(AiAsk::ImportAudio { path, track, tick })
                .await
        );
        let mut value = done_value(&d);
        value["file"] = json!(a.path);
        Ok(done(format!("Done: import audio.\n{value}"), value))
    }
}
