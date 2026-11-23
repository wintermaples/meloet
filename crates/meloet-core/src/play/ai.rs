//! The AI link's writes (`crate::ai`). Each is one step through `apply`, as the screen's
//! intents are, with the step names those already have. Nothing is selected or opened on
//! the screen: the AI is not the one looking at it.

use std::sync::Arc;

use meloet_model::automation as auto_edit;
use meloet_model::edit::{self, EditError};
use meloet_model::effects::EffectKind;
use meloet_model::notes::{self, WriteAt};
use meloet_model::{
    AutoPoint, AutoTarget, ClipId, INSERT_SLOTS, InstrumentRef, Note, PluginRef, StripId, TakeId,
    Tempo, TrackId, arrange, mixer, quantize, timeline as timeline_edit,
};

use super::{MAX_TRACKS, Play};
use crate::ai::{AiNoteEdit, AiPlace, AiRefusal};
use crate::state::Effect;

/// What an AI puts into an insert slot, as the song takes it.
pub(crate) enum AiPut {
    Builtin(EffectKind),
    Plugin(Arc<PluginRef>),
}

/// Which change of the points of a lane.
pub(crate) enum AiPoints {
    Put(Vec<AutoPoint>),
    Move {
        picked: Vec<u32>,
        ticks: i64,
        value: f32,
    },
    Delete(Vec<u32>),
}

/// What a write left: the clip it was about and the track added.
#[derive(Debug, Default)]
pub(crate) struct AiWrote {
    pub clip: Option<ClipId>,
    pub track: Option<TrackId>,
}

fn refusal(e: EditError) -> AiRefusal {
    match e {
        EditError::NoSuchTrack(t) => AiRefusal::NoSuchTrack(t.0),
        EditError::NoSuchClip(c) => AiRefusal::NoSuchClip(c.0),
        EditError::NoSuchNote(i) => AiRefusal::NoSuchNote(i as u32),
        EditError::NotKeys | EditError::WrongKind => AiRefusal::NotKeys,
        EditError::Overlap => AiRefusal::Overlap,
        EditError::OffTheSong | EditError::OutOfClip => AiRefusal::OffTheSong,
        EditError::BadNote => AiRefusal::BadNote,
        EditError::NoSuchInsert => AiRefusal::SlotEmpty,
        EditError::SlotTaken => AiRefusal::SlotTaken,
        EditError::NoSuchPlugin => AiRefusal::NoSuchPlugin,
        EditError::NoSuchPoint => AiRefusal::NoSuchPoint,
        _ => AiRefusal::Unchanged,
    }
}

impl Play {
    pub fn counting_in(&self) -> bool {
        self.state == meloet_protocol::TransportState::CountIn
    }

    /// The id of the track of number `number` (from 1).
    fn track_numbered(&self, number: u32) -> Result<TrackId, AiRefusal> {
        let index = number
            .checked_sub(1)
            .ok_or(AiRefusal::NoSuchTrack(number))?;
        (self.doc().tracks.get(index as usize))
            .map(|t| t.id)
            .ok_or(AiRefusal::NoSuchTrack(number))
    }

    pub(crate) fn ai_write_notes(
        &mut self,
        at: AiPlace,
        notes: &[Note],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let at = match at {
            AiPlace::Track(number) => WriteAt::Track(self.track_numbered(number)?),
            AiPlace::Clip(clip) => WriteAt::Clip(ClipId(clip)),
        };
        let written = notes::write_notes(self.doc(), at, notes).map_err(refusal)?;
        self.apply(written.edit, effects);
        Ok(AiWrote {
            clip: Some(written.clip),
            track: None,
        })
    }

    /// The keys clip that shows `take` now.
    fn clip_showing(&self, take: u32) -> Result<ClipId, AiRefusal> {
        (self.doc().tracks.iter())
            .flat_map(|t| t.clips.iter())
            .find(|c| c.is_keys() && c.active_take().take.id == TakeId(take))
            .map(|c| c.id)
            .ok_or(AiRefusal::NoSuchTake(take))
    }

    pub(crate) fn ai_delete_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let clip = self.clip_showing(take)?;
        let picked: Vec<usize> = picked.iter().map(|&i| i as usize).collect();
        let deleted = notes::delete_notes(self.doc(), clip, &picked, false).map_err(refusal)?;
        self.apply(deleted.edit, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_create_clip(
        &mut self,
        track: u32,
        tick: u64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let track = self.track_numbered(track)?;
        let (e, clip) = notes::create_clip(self.doc(), track, tick).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_rename_clip(
        &mut self,
        clip: u32,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let e = arrange::rename_clip(self.doc(), ClipId(clip), name).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(ClipId(clip)),
            track: None,
        })
    }

    pub(crate) fn ai_set_tempo(
        &mut self,
        tick: u64,
        bpm: f64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let tempo = Tempo::from_bpm(bpm).ok_or(AiRefusal::BadTempo)?;
        let tick = self.snapped(tick);
        let e = timeline_edit::add_tempo_point(self.doc(), tick, tempo).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_add_marker(
        &mut self,
        tick: u64,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let tick = self.snapped(tick);
        let (e, _) = timeline_edit::add_marker(self.doc(), tick, name).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_edit_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        how: &AiNoteEdit,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let clip = self.clip_showing(take)?;
        let picked: Vec<usize> = picked.iter().map(|&i| i as usize).collect();
        let doc = self.doc();
        let edited = match *how {
            AiNoteEdit::Move { ticks, keys, copy } => {
                notes::move_notes(doc, clip, &picked, ticks, keys, copy)
            }
            AiNoteEdit::Resize { edge, ticks } => {
                notes::resize_notes(doc, clip, &picked, edge, ticks)
            }
            AiNoteEdit::Velocity { delta } => notes::change_velocity(doc, clip, &picked, delta),
            AiNoteEdit::Set { field, value } => {
                notes::set_notes_field(doc, clip, &picked, field, value)
            }
            AiNoteEdit::Quantize(settings) => {
                quantize::quantize(doc, clip, &picked, settings).map(|(e, _)| e)
            }
            AiNoteEdit::Unquantize => quantize::unquantize(doc, clip, &picked),
        };
        self.apply(edited.map_err(refusal)?.edit, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_delete_clips(
        &mut self,
        clips: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let ids: Vec<ClipId> = clips.iter().map(|&c| ClipId(c)).collect();
        let e = edit::delete_clips(self.doc(), &ids, false).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_switch_take(
        &mut self,
        clip: u32,
        index: u32,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let e = match edit::switch_take(self.doc(), ClipId(clip), index as usize) {
            Err(EditError::NoSuchTake(_)) => return Err(AiRefusal::NoSuchTake(index)),
            e => e.map_err(refusal)?,
        };
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(ClipId(clip)),
            track: None,
        })
    }

    pub(crate) fn ai_set_instrument(
        &mut self,
        track: TrackId,
        instrument: InstrumentRef,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let t = (self.doc().track(track)).ok_or(AiRefusal::NoSuchTrack(track.0))?;
        if t.kind.is_vocal() {
            return Err(AiRefusal::NotKeys);
        }
        let e = edit::set_instrument(self.doc(), track, instrument).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_put_effect(
        &mut self,
        strip: StripId,
        slot: usize,
        put: AiPut,
        replace: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let doc = self.doc();
        if doc.channel(strip).is_none() {
            return Err(match strip {
                StripId::Track(t) => AiRefusal::NoSuchTrack(t.0),
                _ => AiRefusal::Unchanged,
            });
        }
        if slot >= INSERT_SLOTS {
            return Err(AiRefusal::SlotEmpty);
        }
        match (doc.insert(strip, slot).is_some(), replace) {
            (true, false) => return Err(AiRefusal::SlotTaken),
            (false, true) => return Err(AiRefusal::SlotEmpty),
            _ => {}
        }
        let e = match (put, replace) {
            (AiPut::Builtin(kind), false) => mixer::add_effect(doc, strip, slot, kind),
            (AiPut::Plugin(plugin), false) => mixer::add_plugin_effect(doc, strip, slot, plugin),
            (AiPut::Builtin(kind), true) => mixer::replace_effect(doc, strip, slot, kind),
            (AiPut::Plugin(plugin), true) => mixer::replace_plugin_effect(doc, strip, slot, plugin),
        };
        self.apply(e.map_err(refusal)?, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_points(
        &mut self,
        track: TrackId,
        target: AutoTarget,
        change: AiPoints,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let t = (self.doc().track(track)).ok_or(AiRefusal::NoSuchTrack(track.0))?;
        let lane = t.lane(target).ok_or(AiRefusal::NoSuchLane)?.id;
        let picked = |p: &[u32]| p.iter().map(|&i| i as usize).collect::<Vec<_>>();
        let doc = self.doc();
        let point = |e: EditError| match e {
            EditError::NoSuchPoint => AiRefusal::NoSuchPoint,
            e => refusal(e),
        };
        let e = match change {
            AiPoints::Put(points) => auto_edit::put_points(doc, track, target, &points)
                .map(|(e, _)| e)
                .map_err(refusal)?,
            AiPoints::Move {
                picked: p,
                ticks,
                value,
            } => auto_edit::move_points(doc, lane, &picked(&p), ticks, value)
                .map(|(e, _, _)| e)
                .map_err(point)?,
            AiPoints::Delete(p) => auto_edit::delete_points(doc, lane, &picked(&p))
                .map(|(e, _)| e)
                .map_err(point)?,
        };
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_add_track(
        &mut self,
        instrument: meloet_model::InstrumentRef,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        if self.doc().tracks.len() >= MAX_TRACKS {
            return Err(AiRefusal::TooManyTracks);
        }
        let below = self.doc().tracks.last().map(|t| t.id);
        let (e, id) = edit::add_track(self.doc(), below, instrument).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: None,
            track: Some(id),
        })
    }
}
