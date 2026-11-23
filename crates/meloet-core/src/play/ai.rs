//! The AI link's writes (`crate::ai`). Each is one step through `apply`, as the screen's
//! intents are, with the step names those already have. Nothing is selected or opened on
//! the screen: the AI is not the one looking at it.

use std::collections::BTreeSet;
use std::sync::Arc;

use meloet_model::automation as auto_edit;
use meloet_model::edit::{self, EditError};
use meloet_model::effects::EffectKind;
use meloet_model::history::Mark;
use meloet_model::notes::{self, WriteAt};
use meloet_model::{
    AutoPoint, AutoTarget, Bus, ClipGain, ClipId, FadeEnd, Gain, INSERT_SLOTS, InstrumentRef,
    MarkerId, Meter, Note, Pan, PluginPlace, PluginRef, StripId, TakeId, Tempo, TrackId, arrange,
    mixer, quantize, timeline as timeline_edit,
};
use meloet_protocol::{ClipField, Command, PluginState, SendBus, TimelineRef};

use super::automation::model_target;
use super::history::StepNotes;
use super::mixer::{model_effect, strip_id};
use super::{MAX_TRACKS, Play, Sel};
use crate::ai::{AiNoteEdit, AiPlace, AiRefusal};
use crate::state::Effect;

/// A failed batch gives back the steps that could be redone when it began, when they are
/// at most this many: the screen keeps the rows of the history band up to the last step it
/// saw done, and lets go of them only from the newest row on (`follow` in
/// `ui/src/bridge/historyRows.ts`), so more rows than one put back under those would show
/// the batch's steps still.
const REDO_BACK: usize = 1;

/// Where the song and its history were when a batch of the AI link began.
#[derive(Debug)]
pub(crate) struct AiMark {
    history: Mark,
    sel_redo: Vec<(Sel, Sel)>,
    step_notes: StepNotes,
    loop_on: bool,
    soloed: BTreeSet<TrackId>,
}

impl AiMark {
    /// Steps done when the batch began.
    pub(crate) fn undo_count(&self) -> u32 {
        self.history.undo_count() as u32
    }
}

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
        EditError::Unchanged => AiRefusal::Unchanged,
        e => AiRefusal::Cannot(format!("meloet could not do it ({e:?})")),
    }
}

fn cannot(why: &str) -> AiRefusal {
    AiRefusal::Cannot(why.to_owned())
}

fn bus_of(bus: SendBus) -> Bus {
    match bus {
        SendBus::A => Bus::A,
        SendBus::B => Bus::B,
    }
}

fn gain_of(db: Option<f32>) -> Gain {
    db.map_or(Gain::SILENT, |db| Gain::from_db(f64::from(db)))
}

impl Play {
    pub fn counting_in(&self) -> bool {
        self.state == meloet_protocol::TransportState::CountIn
    }

    /// The song, and the place in its history: another value once anyone has changed the
    /// song, undone or redone a step, or put another song in its place.
    pub(crate) fn ai_place(&self) -> (u32, u64) {
        (self.serial, self.history.version())
    }

    /// What of the session an intent of the screen can change with no step.
    pub(crate) fn ai_session(&self) -> (bool, BTreeSet<TrackId>) {
        let session = &self.song.session;
        (session.loop_on, session.soloed.clone())
    }

    /// A batch begins here. The name field of a marker just made is no part of that
    /// marker's step any more: a rename in the batch is a step of the batch.
    pub(crate) fn ai_mark(&mut self) -> AiMark {
        self.tl.forget_fresh();
        let (loop_on, soloed) = self.ai_session();
        AiMark {
            history: self.history.mark(),
            sel_redo: self.sel_redo.clone(),
            step_notes: self.step_notes.clone(),
            loop_on,
            soloed,
        }
    }

    /// Makes one step of the steps made since `mark`, when the song is at `end` still:
    /// its row is named as the last of them, and undoing it brings back what was selected
    /// before the first. Returns how many steps they were (0: none, or not at `end`).
    pub(crate) fn ai_merge(&mut self, mark: &AiMark, end: (u32, u64)) -> usize {
        if end != self.ai_place() {
            return 0;
        }
        let steps = self.history.merge(&mark.history, end.1);
        if steps < 2 {
            return steps;
        }
        let first = mark.history.undo_count();
        if let Some((_, after)) = self.sel_undo.last().cloned()
            && self.sel_undo.len() == first + steps
        {
            self.sel_undo.truncate(first + 1);
            self.sel_undo[first].1 = after;
        }
        // The same song under another number.
        let now = self.history.version();
        for version in [&mut self.saving, &mut self.saved_too] {
            if *version == Some(end.1) {
                *version = Some(now);
            }
        }
        steps
    }

    /// Takes back every step made since `mark`, and the solo marks and the loop's switch,
    /// when the song is at `end` still and meloet does not record: the engine, the plugins
    /// and the selection hear of it once, as of a jump in the history. The steps are not
    /// kept to redo. Returns whether it was done.
    pub(crate) fn ai_rewind(
        &mut self,
        mark: AiMark,
        end: (u32, u64),
        effects: &mut Vec<Effect>,
    ) -> bool {
        if end != self.ai_place() || self.busy() {
            return false;
        }
        let AiMark {
            mut history,
            sel_redo,
            step_notes,
            loop_on,
            soloed,
        } = mark;
        let redo_back = history.redo_count() <= REDO_BACK;
        if !redo_back {
            history.forget_redo();
        }
        let first = history.undo_count();
        let before = self.doc().clone();
        if !self.history.rewind(&mut self.song.doc, history, end.1) {
            return false;
        }
        self.tl.forget_fresh();
        self.song.doc.name.clone_from(&before.name);
        let session = &mut self.song.session;
        session.loop_on = loop_on;
        session.soloed = soloed;
        if self.sel_undo.len() > first {
            self.after_change(&before, effects);
            let sel = self.sel_undo.drain(first..).next();
            if redo_back {
                self.sel_redo = sel_redo;
                self.step_notes = step_notes;
            }
            if let Some((sel, _)) = sel {
                self.put_sel(sel, effects);
            }
        }
        self.send_loop(effects);
        self.sync_silent(effects);
        true
    }

    /// Why an intent of the screen changed nothing, found by trying its edit on the song as
    /// it is: the song is so already (`Unchanged`), or what is wrong. `said`: what the
    /// status line said of the intent.
    pub(crate) fn ai_why_not(&self, command: &Command, said: Option<String>) -> AiRefusal {
        use Command as C;
        let doc = self.doc();
        let slot_of = |insert: &meloet_protocol::InsertRef| {
            (strip_id(insert.strip), usize::from(insert.slot))
        };
        let meter = |num: u8, den: u8| Meter::new(num, den).ok_or(EditError::BadNote);
        // `Ok`: the edit can be made, and yet the intent did not make it.
        let tried: Result<(), EditError> = match command {
            C::SetGain { strip, db, .. } => {
                mixer::set_gain(doc, strip_id(*strip), gain_of(*db)).map(drop)
            }
            C::SetPan { strip, pan, .. } => {
                mixer::set_pan(doc, strip_id(*strip), Pan::new(*pan)).map(drop)
            }
            C::SetSend { track, bus, db, .. } => {
                mixer::set_send(doc, TrackId(*track), bus_of(*bus), gain_of(*db)).map(drop)
            }
            C::SetParam {
                insert,
                effect,
                param,
                value,
                ..
            } => {
                let (strip, slot) = slot_of(insert);
                let kind = model_effect(*effect);
                let Some(value) = kind.clamp(*param, *value) else {
                    return cannot("the effect has no such parameter");
                };
                mixer::set_param(doc, strip, slot, kind, *param, value).map(drop)
            }
            C::ToggleMute { track } => edit::toggle_mute(doc, TrackId(*track)).map(drop),
            C::ToggleSolo { track } => match doc.track(TrackId(*track)) {
                Some(_) => Ok(()),
                None => Err(EditError::NoSuchTrack(TrackId(*track))),
            },
            C::RemoveEffect { insert } => {
                let (strip, slot) = slot_of(insert);
                mixer::remove_effect(doc, strip, slot).map(drop)
            }
            C::ToggleBypass { insert } => {
                let (strip, slot) = slot_of(insert);
                let place = PluginPlace::Insert(strip, slot);
                let plugin = (doc.insert(strip, slot)).is_some_and(|i| i.plugin_ref().is_some());
                if plugin && self.plugin_state(place) == PluginState::Stopped {
                    return AiRefusal::PluginNotLoaded;
                }
                mixer::toggle_bypass(doc, strip, slot).map(drop)
            }
            C::MoveInsert { insert, to } => {
                let (strip, slot) = slot_of(insert);
                mixer::move_insert(doc, strip, slot, usize::from(*to)).map(drop)
            }
            // The meter in force at the play line, which is there.
            C::SetMeter { num, den } => meter(*num, *den).and(Err(EditError::Unchanged)),
            C::AddMeterChange { bar, num, den } => meter(*num, *den)
                .and_then(|m| timeline_edit::add_meter_change(doc, *bar, m).map(drop)),
            C::SetMeterChange { bar, num, den } => {
                meter(*num, *den).and_then(|m| timeline_edit::set_meter_at(doc, *bar, m).map(drop))
            }
            C::MoveMeterChange { bar: 0, .. } => {
                return cannot("the meter of bar 1 does not move; set it with set_meter");
            }
            C::MoveMeterChange { bar, to_bar } => {
                timeline_edit::move_meter_change(doc, *bar, (*to_bar).max(1)).map(drop)
            }
            C::MoveTempoPoint { tick, bpm, .. } => {
                let points = &doc.timeline.tempo_points;
                if Tempo::from_bpm(*bpm).is_none() {
                    return AiRefusal::BadTempo;
                }
                match *tick == 0 || points.iter().any(|p| p.tick == *tick) {
                    true => Err(EditError::Unchanged),
                    false => Err(EditError::NoSuchPoint),
                }
            }
            C::SetTempoRamp { tick, on } => {
                timeline_edit::set_tempo_ramp(doc, *tick, *on).map(drop)
            }
            C::DeleteTimeline { target } => match *target {
                TimelineRef::TempoPoint { tick } => timeline_edit::delete_tempo_point(doc, tick),
                TimelineRef::MeterChange { bar } => timeline_edit::delete_meter_change(doc, bar),
                TimelineRef::Marker { id } => timeline_edit::delete_marker(doc, MarkerId(id)),
            }
            .map(drop),
            C::RenameMarker { id, name } => {
                timeline_edit::rename_marker(doc, MarkerId(*id), name).map(drop)
            }
            C::MoveMarker { id, tick } => {
                timeline_edit::move_marker(doc, MarkerId(*id), self.snapped(*tick)).map(drop)
            }
            // The range is the loop's range already, and the loop is on.
            C::SetLoop { .. } => Err(EditError::Unchanged),
            C::ToggleLoop => Ok(()),
            C::DeleteTrack { track } => edit::delete_track(doc, TrackId(*track)).map(drop),
            C::RenameTrack { track, name } => {
                edit::rename_track(doc, TrackId(*track), name).map(drop)
            }
            C::MoveTrack { track, to } => {
                edit::move_track(doc, TrackId(*track), *to as usize).map(drop)
            }
            C::AddVocalTrack if doc.tracks.len() >= MAX_TRACKS => {
                return AiRefusal::TooManyTracks;
            }
            C::AddVocalTrack => edit::add_vocal_track(doc, None).map(drop),
            C::MoveClips {
                clips,
                ticks,
                tracks,
                copy,
            } => arrange::move_clips(doc, &Self::clip_ids(clips), *ticks, *tracks, *copy).map(drop),
            C::DuplicateClips { clips } => {
                arrange::duplicate_clips(doc, &Self::clip_ids(clips)).map(drop)
            }
            C::SplitClipAt { clip, tick } => {
                if doc.clip(ClipId(*clip)).is_none() {
                    return AiRefusal::NoSuchClip(*clip);
                }
                match arrange::split_clips(doc, &[ClipId(*clip)], *tick) {
                    Err(EditError::Unchanged) => {
                        return cannot("the place is not inside the clip (its edges are not)");
                    }
                    tried => tried.map(drop),
                }
            }
            C::JoinClips { clips } => {
                if let Some(&gone) = clips.iter().find(|&&c| doc.clip(ClipId(c)).is_none()) {
                    return AiRefusal::NoSuchClip(gone);
                }
                match arrange::join_clips(doc, &Self::clip_ids(clips)) {
                    Err(EditError::Unchanged) => {
                        return cannot(
                            "give two or more keys clips of one track with no other clip of \
                             the track between them",
                        );
                    }
                    tried => tried.map(drop),
                }
            }
            C::SetClipField { clip, field, value } => {
                let id = ClipId(*clip);
                let Some((_, c)) = doc.clip(id) else {
                    return AiRefusal::NoSuchClip(*clip);
                };
                if value.is_nan() {
                    return cannot("the value is not a number");
                }
                let ticks = value.round().clamp(0.0, edit::MAX_TICK as f64) as u64;
                match field {
                    ClipField::Start => {
                        let last = edit::MAX_TICK - (c.end - c.start);
                        arrange::set_clip_start(doc, id, ticks.min(last)).map(drop)
                    }
                    ClipField::Length => arrange::set_clip_length(doc, id, ticks).map(drop),
                    ClipField::GainDb => match ClipGain::from_db(*value) {
                        Some(gain) => meloet_model::set_clip_gain(doc, id, gain).map(drop),
                        None => return cannot("the gain is out of range"),
                    },
                    ClipField::FadeIn => {
                        meloet_model::set_clip_fade(doc, id, FadeEnd::In, ticks).map(drop)
                    }
                    ClipField::FadeOut => {
                        meloet_model::set_clip_fade(doc, id, FadeEnd::Out, ticks).map(drop)
                    }
                }
            }
            C::SetClipStretch { clip, .. } => match doc.clip(ClipId(*clip)) {
                None => return AiRefusal::NoSuchClip(*clip),
                Some((_, c)) if c.is_keys() => Err(EditError::WrongKind),
                // A value that is no number, or the value the clip has.
                Some(_) => Err(EditError::Unchanged),
            },
            C::AddLane { track, target } => {
                let (track, target) = (TrackId(*track), model_target(*target));
                match doc.track(track) {
                    None => Err(EditError::NoSuchTrack(track)),
                    // Unfolded, which is no step.
                    Some(t) if t.lane(target).is_some() => Err(EditError::Unchanged),
                    Some(_) => auto_edit::add_lane(doc, track, target, false).map(drop),
                }
            }
            C::DeleteLane { track, target } => {
                auto_edit::delete_lane(doc, TrackId(*track), model_target(*target)).map(drop)
            }
            _ => return cannot("an AI may not send that intent"),
        };
        match tried {
            Err(EditError::NoSuchPoint) => cannot(
                "there is no such thing in the song (no tempo change at that place, no meter \
                 change at that bar, or no marker of that id); call song_overview",
            ),
            Err(EditError::BadNote) => cannot("there is no such meter"),
            Err(EditError::WrongKind | EditError::NotKeys) => {
                cannot("that does not fit the kind of the clip or the track (keys or vocal)")
            }
            Err(EditError::NoSuchInsert) => match command {
                C::SetParam { .. } => {
                    cannot("that slot does not hold that effect, or it has no such parameter")
                }
                C::AddLane { .. } | C::DeleteLane { .. } => AiRefusal::NoSuchLane,
                _ => AiRefusal::SlotEmpty,
            },
            Err(e) => refusal(e),
            // A lane of the control took the value, or is being written.
            Ok(()) if self.ai_lane_of(command) => AiRefusal::HasLane,
            Ok(()) => match said {
                Some(said) => AiRefusal::Said(said),
                None => cannot("meloet did not do it"),
            },
        }
    }

    /// The control the intent sets has an automation lane.
    fn ai_lane_of(&self, command: &Command) -> bool {
        use Command as C;
        let doc = self.doc();
        let track_of = |strip: StripId| match strip {
            StripId::Track(track) => Some(track),
            _ => None,
        };
        let lane = match command {
            C::SetGain { strip, .. } => track_of(strip_id(*strip)).map(|t| (t, AutoTarget::Volume)),
            C::SetPan { strip, .. } => track_of(strip_id(*strip)).map(|t| (t, AutoTarget::Pan)),
            C::SetSend { track, bus, .. } => {
                Some((TrackId(*track), AutoTarget::Send(bus_of(*bus))))
            }
            C::ToggleMute { track } => Some((TrackId(*track), AutoTarget::Mute)),
            C::SetParam { insert, param, .. } => {
                let strip = strip_id(insert.strip);
                let id = doc.insert(strip, usize::from(insert.slot)).map(|i| i.id);
                track_of(strip).zip(id).map(|(t, insert)| {
                    let param = *param;
                    (t, AutoTarget::Insert { insert, param })
                })
            }
            _ => None,
        };
        lane.is_some_and(|key| self.lane_at(key).is_some())
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
                _ => cannot("there is no such strip"),
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
