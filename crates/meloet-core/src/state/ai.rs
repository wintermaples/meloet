//! The switch 「AI とつなぐ」 (spec 3 「AI link」, A-27), what the screen shows of the socket,
//! and the AI link's asks (`crate::ai`).

use std::collections::VecDeque;
use std::sync::Arc;

use meloet_model::notes::{self, WriteAt};
use meloet_model::{
    Channel, Doc, Gain, InstrumentRef, Kept, Note, Sf2Ref, TICKS_PER_QUARTER, TickRange, Timeline,
    Track, TrackId,
};
use meloet_protocol::{AiLinkView, AiPort, SoundRef, StatusMessage};

use super::{Effect, Input, Model, PluginSaid, StateAsk};
use crate::ai::{
    AiAnswer, AiAsk, AiBusy, AiDone, AiEffect, AiInstrument, AiMeasure, AiOutcome,
    AiOutputsMeasure, AiRefusal, AiRender, AiRenderWhat, AiSong, batchable, capture_longest_s,
    screen_allowed, screen_no_step,
};
use crate::export::{ExportJob, ExportTarget, Measuring};
use crate::play::ai::{AiMark, AiPoints, AiPut, AiWrote};
use meloet_protocol::{ExportLoudness, ExportTail};

/// Outcomes of asks with a ticket that are kept for the AI link to read.
const OUTCOMES: usize = 16;

/// A batch with no ask for this long was left open by an asker that is gone: meloet closes
/// it and takes it back. The asks of a batch follow each other within milliseconds.
const BATCH_IDLE_US: u64 = 2_000_000;

/// How many inputs wait for an open batch at most: one more, and the batch is closed and
/// taken back rather than an input let go.
const HELD: usize = 256;

/// The open batch of the AI link (`AiAsk::Begin`).
#[derive(Debug)]
struct Batch {
    id: u64,
    mark: AiMark,
    /// `Play::ai_place` after the batch's last ask: any other place, and someone else has
    /// changed the song or its history.
    place: (u32, u64),
    /// When its last ask came, on the clock of [`Model::set_now`].
    asked_us: u64,
}

/// What an input does while a batch is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hold {
    /// It neither changes the song or its history nor depends on one that waits.
    Pass,
    /// It may change the song or its history: it waits for the batch to be closed.
    Wait,
    /// meloet is asked to end: the batch is taken back, then it is served after what waits.
    End,
}

fn hold_of(input: &Input) -> Hold {
    match input {
        Input::Command(_)
        | Input::KeyboardButton(_)
        | Input::KeyboardFader { .. }
        | Input::OpenGiven(_)
        | Input::Audio(_) => Hold::Wait,
        Input::PluginSaid { said, .. } if !answers_only(said) => Hold::Wait,
        Input::CloseRequested | Input::PcEnding => Hold::End,
        _ => Hold::Pass,
    }
}

/// What a plugin says that makes no step and stands in no order with what does: answers to
/// asks, and values to show. The rest (its window, a parameter moved by hand, its state)
/// can end in a step.
fn answers_only(said: &PluginSaid) -> bool {
    matches!(
        said,
        PluginSaid::Editor(_)
            | PluginSaid::Outputs(_)
            | PluginSaid::Param(_)
            | PluginSaid::ParamsChanged
            | PluginSaid::Params { .. }
            | PluginSaid::KeyNames(_)
            | PluginSaid::Texts { .. }
            | PluginSaid::State {
                ask: StateAsk::Ai,
                ..
            }
    )
}

/// A track a measurement renders alone: its number and name in the song, and whether the
/// song mutes it.
pub type AloneTrack = (u32, String, bool);

#[derive(Debug, Default)]
pub(super) struct AiSide {
    /// The switch, kept in the settings.
    pub(super) on: bool,
    /// What the socket's thread said last.
    port: AiPort,
    /// The answer to the last [`super::Input::Ai`], until the core thread takes it.
    answer: Option<AiAnswer>,
    last_ticket: u64,
    outcomes: VecDeque<(u64, AiOutcome)>,
    /// Files being brought in for the AI link: the audio side's ticket and the AI's.
    imports: Vec<(u64, u64)>,
    batch: Option<Batch>,
    /// The inputs that wait for the open batch, in the order they came.
    held: VecDeque<Input>,
    /// How many inputs ever waited, and how many of them were taken to be served.
    held_in: u64,
    held_out: u64,
    /// The last batch meloet closed by itself and took back.
    taken_back: Option<u64>,
}

impl AiSide {
    fn ticket(&mut self) -> u64 {
        self.last_ticket += 1;
        let ticket = self.last_ticket;
        if self.outcomes.len() >= OUTCOMES {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back((ticket, AiOutcome::Pending));
        ticket
    }

    fn ended(&mut self, ticket: u64, outcome: AiOutcome) {
        if let Some((_, o)) = self.outcomes.iter_mut().find(|(t, _)| *t == ticket) {
            *o = outcome;
        }
    }
}

impl AiSide {
    pub(super) fn new(on: bool) -> AiSide {
        AiSide {
            on,
            ..AiSide::default()
        }
    }

    /// Returns whether it changed (the settings are then saved and the socket told).
    pub(super) fn set(&mut self, on: bool) -> bool {
        let changed = self.on != on;
        self.on = on;
        if changed {
            self.port = AiPort::Closed;
        }
        changed
    }

    pub(super) fn port_said(&mut self, port: AiPort) {
        self.port = port;
    }

    pub(super) fn view(&self) -> AiLinkView {
        AiLinkView {
            on: self.on,
            port: if self.on { self.port } else { AiPort::Closed },
        }
    }
}

impl Model {
    pub fn ai_on(&self) -> bool {
        self.ai.on
    }

    fn ai_busy(&self) -> Option<AiBusy> {
        if self.play.counting_in() {
            Some(AiBusy::CountingIn)
        } else if self.play.busy() {
            Some(AiBusy::Recording)
        } else if self.export.running() {
            Some(AiBusy::Exporting)
        } else {
            None
        }
    }

    pub fn ai_song(&self) -> AiSong {
        AiSong {
            song: self.play.song().clone(),
            undo_count: self.play.undo_count(),
            busy: self.ai_busy(),
            library: self.library_view(),
            file: self.files.path.clone(),
            recent: self.recent_songs().to_vec(),
            songs_dir: self.songs_dir().map(|d| d.to_path_buf()),
            unsaved: self.play.has_unsaved_changes(),
            sample_rate: self.export_scene().sample_rate,
            outcomes: self.ai.outcomes.iter().cloned().collect(),
            instrument_sets: self.play.all_instrument_sets(),
        }
    }

    /// The answer to the last [`super::Input::Ai`].
    pub fn take_ai_answer(&mut self) -> Option<AiAnswer> {
        self.ai.answer.take()
    }

    pub(super) fn ai_ask(&mut self, ask: AiAsk, effects: &mut Vec<Effect>) {
        let answer = match ask {
            AiAsk::PluginParams { place } => match self.play.ai_params(place, effects) {
                Ok(params) => AiAnswer::Params(params),
                Err(refused) => AiAnswer::Refused(refused),
            },
            AiAsk::KeyNames { track } => match self.play.ai_key_names(track, &self.catalog) {
                Ok(map) => AiAnswer::KeyNames(Box::new(map)),
                Err(refused) => AiAnswer::Refused(refused),
            },
            AiAsk::PluginState { place } => match self.play.ai_plugin_state(place, effects) {
                Ok(state) => AiAnswer::State(state),
                Err(refused) => AiAnswer::Refused(refused),
            },
            AiAsk::StatePut { ticket } => match self.play.ai_state_put_end(ticket) {
                Ok(state) => AiAnswer::State(state),
                Err(refused) => AiAnswer::Refused(refused),
            },
            AiAsk::ParamsSet { ticket } => match self.play.ai_params_set(ticket) {
                Ok(params) => AiAnswer::Params(params),
                Err(refused) => AiAnswer::Refused(refused),
            },
            AiAsk::ParamTexts {
                place,
                param,
                values,
            } => match self.play.ai_param_texts(place, param, values, effects) {
                Ok(params) => AiAnswer::Params(params),
                Err(refused) => AiAnswer::Refused(refused),
            },
            ask => match self.ai_in_turn(ask, effects) {
                Ok(done) => AiAnswer::Done(Box::new(done)),
                Err(refused) => AiAnswer::Refused(refused),
            },
        };
        self.ai.answer = Some(answer);
    }

    /// A save, an open or a file brought in for the AI link ended.
    pub(super) fn ai_ended(&mut self, ticket: u64, outcome: AiOutcome) {
        self.ai.ended(ticket, outcome);
    }

    /// States the AI link put into plugins were taken, or not.
    pub(super) fn ai_puts_ended(&mut self) {
        for (ticket, outcome) in self.play.take_ai_puts_ended() {
            self.ai.ended(ticket, outcome);
        }
    }

    /// Files the audio side brought in under `bringing` went into the song, or did not.
    pub(super) fn ai_brought(&mut self, bringing: u64, put_in: bool) {
        let Some(at) = self.ai.imports.iter().position(|(b, _)| *b == bringing) else {
            return;
        };
        let (_, ticket) = self.ai.imports.remove(at);
        let outcome = match (put_in, &self.notice) {
            (true, _) => AiOutcome::Done(None),
            (false, Some(notice)) => AiOutcome::Failed(format!("{notice:?}")),
            (false, None) => AiOutcome::Failed("the file did not go into the song".into()),
        };
        self.ai.ended(ticket, outcome);
    }

    fn ai_instrument(&self, instrument: AiInstrument) -> Result<InstrumentRef, AiRefusal> {
        match instrument {
            AiInstrument::Builtin(b) => Ok(InstrumentRef::Builtin(b)),
            AiInstrument::Plugin(key) => {
                let sound = SoundRef::Plugin { key };
                let (made, _) = self
                    .catalog
                    .instrument(&sound)
                    .ok_or(AiRefusal::NoSuchSound)?;
                Ok(made)
            }
            AiInstrument::Sf2 {
                file,
                bank,
                program,
            } => {
                let (info, preset) = (self.catalog.sf2_preset(&file, bank, program))
                    .ok_or(AiRefusal::NoSuchSound)?;
                Ok(InstrumentRef::Sf2(Arc::new(Sf2Ref {
                    file: info.file.as_str().into(),
                    sha256: info.sha256.as_str().into(),
                    bank: preset.bank,
                    program: preset.program,
                    name: preset.name.as_str().into(),
                    kept: Kept::NONE,
                })))
            }
        }
    }

    /// An answer that names no step.
    fn ai_done(&self, step: Option<meloet_protocol::HistoryRow>) -> AiDone {
        AiDone {
            step,
            undo_count: self.play.undo_count(),
            clip: None,
            take: None,
            notes: None,
            track: None,
            ticket: None,
            pending: false,
        }
    }

    pub(super) fn ai_batch_open(&self) -> bool {
        self.ai.batch.is_some()
    }

    /// When the open batch counts as left open.
    pub(super) fn ai_batch_due_us(&self) -> Option<u64> {
        (self.ai.batch.as_ref()).map(|b| b.asked_us.saturating_add(BATCH_IDLE_US))
    }

    /// meloet closes the open batch by itself: what it did is taken back, where it can be.
    fn ai_end_batch(&mut self, effects: &mut Vec<Effect>) {
        if let Some(left) = self.ai.batch.take()
            && self.play.ai_rewind(left.mark, left.place, effects)
        {
            self.ai.taken_back = Some(left.id);
        }
    }

    /// Before every input: `input` when it is to be handled now. While a batch is open,
    /// what may change the song or its history waits instead, so that the batch can still
    /// become one step or be taken back; it is served in the order it came once the batch
    /// is closed ([`Model::take_held`]). A batch left open, one that more waits for than
    /// can, and one open when meloet is asked to end are closed and taken back here.
    pub(super) fn ai_hold(&mut self, input: Input, effects: &mut Vec<Effect>) -> Option<Input> {
        if self.ai_batch_due_us().is_some_and(|at| at <= self.now_us) {
            self.ai_end_batch(effects);
        }
        let hold = hold_of(&input);
        // What waits already goes first.
        if hold == Hold::Pass || (self.ai.batch.is_none() && self.ai.held.is_empty()) {
            return Some(input);
        }
        if hold == Hold::End || self.ai.held.len() >= HELD {
            self.ai_end_batch(effects);
        }
        self.ai.held.push_back(input);
        self.ai.held_in += 1;
        None
    }

    /// The next input that waited for a batch, once none is open, as an [`Input::Held`] to
    /// handle now.
    pub fn take_held(&mut self) -> Option<Input> {
        if self.ai.batch.is_some() {
            return None;
        }
        let held = self.ai.held.pop_front()?;
        self.ai.held_out += 1;
        Some(Input::Held(Box::new(held)))
    }

    /// How many inputs ever waited for a batch, and how many of them were taken to be
    /// served: the input that made the first number `n` is served once the second is `n`.
    pub fn held_counts(&self) -> (u64, u64) {
        (self.ai.held_in, self.ai.held_out)
    }

    /// A write, or an ask about the batch: while a batch is open, only its own asks are
    /// taken.
    fn ai_in_turn(&mut self, ask: AiAsk, effects: &mut Vec<Effect>) -> Result<AiDone, AiRefusal> {
        let open = self.ai.batch.as_ref().map(|b| b.id);
        match ask {
            AiAsk::Begin { .. } if open.is_some() => Err(AiRefusal::BatchOpen),
            AiAsk::Begin { batch } => {
                if let Some(busy) = self.ai_busy() {
                    return Err(AiRefusal::Busy(busy));
                }
                // A save, an open or a question to the person can end in another song.
                if !self.song_free() {
                    return Err(AiRefusal::FilesBusy);
                }
                self.ai.batch = Some(Batch {
                    id: batch,
                    mark: self.play.ai_mark(),
                    place: self.play.ai_place(),
                    asked_us: self.now_us,
                });
                Ok(self.ai_done(None))
            }
            AiAsk::InBatch { batch, ask } => {
                let place = self.play.ai_place();
                if (self.ai.batch.as_ref()).is_none_or(|b| b.id != batch || b.place != place) {
                    return Err(AiRefusal::BatchLost);
                }
                if !batchable(&ask) {
                    return Err(AiRefusal::NotInBatch);
                }
                let done = self.ai_write(*ask, effects);
                if let Some(b) = &mut self.ai.batch {
                    b.place = self.play.ai_place();
                    b.asked_us = self.now_us;
                }
                match done {
                    // It ends later, when the batch is closed.
                    Ok(d) if d.pending || d.ticket.is_some() => Err(AiRefusal::NotInBatch),
                    done => done,
                }
            }
            // meloet has taken it back already.
            AiAsk::Rollback { batch } if open.is_none() && self.ai.taken_back == Some(batch) => {
                Ok(self.ai_done(None))
            }
            AiAsk::Commit { batch } | AiAsk::Rollback { batch } if open != Some(batch) => {
                Err(AiRefusal::BatchLost)
            }
            AiAsk::Commit { .. } => {
                let b = self.ai.batch.take().ok_or(AiRefusal::BatchLost)?;
                let merged = self.play.ai_merge(&b.mark, b.place);
                let row = b.mark.undo_count() + 1;
                let step = (merged > 0)
                    .then(|| self.play.history_rows(row, row).pop())
                    .flatten();
                Ok(self.ai_done(step))
            }
            AiAsk::Rollback { .. } => {
                let b = self.ai.batch.take().ok_or(AiRefusal::BatchLost)?;
                if let Some(busy) = self.ai_busy() {
                    return Err(AiRefusal::Busy(busy));
                }
                if !self.play.ai_rewind(b.mark, b.place, effects) {
                    return Err(AiRefusal::BatchLost);
                }
                Ok(self.ai_done(None))
            }
            _ if open.is_some() => Err(AiRefusal::BatchOpen),
            ask => self.ai_write(ask, effects),
        }
    }

    fn ai_write(&mut self, ask: AiAsk, effects: &mut Vec<Effect>) -> Result<AiDone, AiRefusal> {
        if let Some(busy) = self.ai_busy() {
            return Err(AiRefusal::Busy(busy));
        }
        let before = self.play.undo_count();
        let mut ticket = None;
        let mut no_step = false;
        let mut notes_done = None;
        let wrote = match ask {
            AiAsk::WriteNotes { at, notes } => {
                let (wrote, told) = self.play.ai_write_notes(at, &notes, effects)?;
                notes_done = Some(Box::new(told));
                wrote
            }
            AiAsk::DeleteNotes { take, notes } => {
                self.play.ai_delete_notes(take, &notes, effects)?
            }
            AiAsk::CreateClip { track, tick } => self.play.ai_create_clip(track, tick, effects)?,
            AiAsk::CreateClipTo { track, tick, end } => {
                (self.play).ai_create_clip_to(track, tick, end, effects)?
            }
            AiAsk::PutNotes { at, notes } => {
                let (wrote, told) = self.play.ai_put_notes(at, &notes, None, effects)?;
                notes_done = Some(Box::new(told));
                wrote
            }
            AiAsk::CopyNotes {
                clip,
                take,
                pick,
                to,
                shift,
                transpose,
            } => {
                let (wrote, told) =
                    (self.play).ai_copy_notes(clip, take, &pick, to, shift, transpose, effects)?;
                notes_done = Some(Box::new(told));
                wrote
            }
            AiAsk::RenameClip { clip, name } => self.play.ai_rename_clip(clip, &name, effects)?,
            AiAsk::SetTempo { tick, bpm } => self.play.ai_set_tempo(tick, bpm, effects)?,
            AiAsk::AddMarker { tick, name } => self.play.ai_add_marker(tick, &name, effects)?,
            AiAsk::AddTrack { instrument } => {
                let instrument = self.ai_instrument(instrument)?;
                let added = self.play.ai_add_track(instrument, effects);
                if let Err(AiRefusal::TooManyTracks) = added {
                    self.notice = Some(StatusMessage::TooManyTracks);
                }
                added?
            }
            AiAsk::Undo => {
                let row = self.play.history_rows(before, before).pop();
                if row.is_none() {
                    return Err(AiRefusal::NothingToUndo);
                }
                self.notice = self.play.undo(effects);
                return Ok(AiDone {
                    step: row,
                    undo_count: self.play.undo_count(),
                    clip: None,
                    take: None,
                    notes: None,
                    track: None,
                    ticket: None,
                    pending: false,
                });
            }
            AiAsk::Begin { .. }
            | AiAsk::InBatch { .. }
            | AiAsk::Commit { .. }
            | AiAsk::Rollback { .. } => return Err(AiRefusal::NotInBatch),
            AiAsk::Screen(command) => {
                if !screen_allowed(&command) {
                    return Err(AiRefusal::Cannot("an AI may not send that intent".into()));
                }
                no_step = screen_no_step(&command);
                // What the line said before is not what it says of this intent.
                let said_before = self.notice.take();
                let was = (self.play.changes(), self.play.ai_session());
                self.command(command.clone(), effects);
                let said = self.notice.as_ref().map(|notice| format!("{notice:?}"));
                if said.is_none() {
                    self.notice = said_before;
                }
                if self.play.undo_count() == before {
                    if was == (self.play.changes(), self.play.ai_session()) {
                        return Err(self.play.ai_why_not(&command, said));
                    }
                    // The loop switched on by its range, or the name of the marker just
                    // made: a change that is no step of its own, as solo is.
                    no_step = true;
                }
                AiWrote::default()
            }
            AiAsk::EditNotes { take, notes, edit } => {
                self.play.ai_edit_notes(take, &notes, &edit, effects)?
            }
            AiAsk::DeleteClips { clips } => self.play.ai_delete_clips(&clips, effects)?,
            AiAsk::SwitchTake { clip, index } => self.play.ai_switch_take(clip, index, effects)?,
            AiAsk::SetInstrument { track, instrument } => {
                let instrument = self.ai_instrument(instrument)?;
                (self.play).ai_set_instrument(TrackId(track), instrument, effects)?
            }
            AiAsk::PutEffect {
                strip,
                slot,
                effect,
                replace,
            } => {
                let put = match effect {
                    AiEffect::Builtin(kind) => AiPut::Builtin(kind),
                    AiEffect::Plugin(key) => {
                        let found = (self.catalog.plugin(&key))
                            .filter(|f| !f.instrument)
                            .ok_or(AiRefusal::NoSuchEffect)?;
                        AiPut::Plugin(found.plugin)
                    }
                };
                self.play
                    .ai_put_effect(strip, slot, put, replace, effects)?
            }
            AiAsk::WriteAutomation {
                track,
                target,
                points,
            } => {
                let change = AiPoints::Put(points);
                self.play
                    .ai_points(TrackId(track), target, change, effects)?
            }
            AiAsk::MoveAutomation {
                track,
                target,
                points,
                ticks,
                value,
            } => {
                let change = AiPoints::Move {
                    picked: points,
                    ticks,
                    value,
                };
                self.play
                    .ai_points(TrackId(track), target, change, effects)?
            }
            AiAsk::DeleteAutomation {
                track,
                target,
                points,
            } => {
                let change = AiPoints::Delete(points);
                self.play
                    .ai_points(TrackId(track), target, change, effects)?
            }
            AiAsk::SetPluginParam {
                place,
                param,
                value,
            } => {
                let set = (self.play).ai_set_plugin_param(place, param, value, effects)?;
                return Ok(AiDone {
                    step: None,
                    undo_count: before,
                    clip: None,
                    take: None,
                    notes: None,
                    track: None,
                    ticket: Some(set),
                    pending: true,
                });
            }
            AiAsk::PluginParams { .. }
            | AiAsk::KeyNames { .. }
            | AiAsk::PluginState { .. }
            | AiAsk::StatePut { .. }
            | AiAsk::Preset { .. }
            | AiAsk::ParamsSet { .. }
            | AiAsk::ParamTexts { .. } => {
                return Err(AiRefusal::Cannot("that is a read, not a write".into()));
            }
            AiAsk::SetPluginState { place, state } => {
                let ticket = self.ai.last_ticket + 1;
                (self.play).ai_put_plugin_state(place, state, ticket, effects)?;
                let taken = self.ai.ticket();
                debug_assert_eq!(taken, ticket);
                return Ok(AiDone {
                    step: None,
                    undo_count: before,
                    clip: None,
                    take: None,
                    notes: None,
                    track: None,
                    ticket: Some(ticket),
                    pending: true,
                });
            }
            AiAsk::SetPluginParams { place, values } => {
                let set = (self.play).ai_set_plugin_params(place, &values, effects)?;
                return Ok(AiDone {
                    step: None,
                    undo_count: before,
                    clip: None,
                    take: None,
                    notes: None,
                    track: None,
                    ticket: Some(set),
                    pending: true,
                });
            }
            AiAsk::SetEffectParams {
                strip,
                slot,
                values,
            } => {
                no_step = !(self.play).ai_set_effect_params(strip, slot, &values, effects)?;
                AiWrote::default()
            }
            AiAsk::Save | AiAsk::SaveAs { .. } | AiAsk::Open { .. } | AiAsk::NewSong => {
                if !self.song_free() {
                    return Err(AiRefusal::FilesBusy);
                }
                let opens = matches!(ask, AiAsk::Open { .. } | AiAsk::NewSong);
                if opens && self.play.has_unsaved_changes() {
                    return Err(AiRefusal::Unsaved);
                }
                if matches!(ask, AiAsk::Save) && self.song_path().is_none() {
                    return Err(AiRefusal::NeverSaved);
                }
                no_step = true;
                match ask {
                    AiAsk::Save => {
                        let t = self.ai.ticket();
                        self.ai_save(t, None, effects);
                        ticket = Some(t);
                    }
                    AiAsk::SaveAs { folder, name } => {
                        let t = self.ai.ticket();
                        self.ai_save(t, Some((folder, name)), effects);
                        ticket = Some(t);
                    }
                    AiAsk::Open { path } => {
                        let t = self.ai.ticket();
                        self.ai_open(t, path, effects);
                        ticket = Some(t);
                    }
                    _ => self.ai_new_song(effects),
                }
                AiWrote::default()
            }
            AiAsk::SaveMuteSet { name, overwrite } => {
                no_step = true;
                self.play.ai_save_mute_set(&name, overwrite, effects)?
            }
            AiAsk::RecallMuteSet { name } => {
                no_step = true;
                self.play.ai_recall_mute_set(&name, effects)?
            }
            AiAsk::DeleteMuteSet { name } => self.play.ai_delete_mute_set(&name, effects)?,
            AiAsk::RenameMuteSet { name, to } => {
                no_step = true;
                self.play.ai_rename_mute_set(&name, &to, effects)?
            }
            AiAsk::SetPluginOutput {
                track,
                output,
                change,
            } => (self.play).ai_set_plugin_output(TrackId(track), &output, change, effects)?,
            AiAsk::ImportAudio { path, track, tick } => {
                if let Some(track) = track {
                    let t = (self.play.song().doc.track(TrackId(track)))
                        .ok_or(AiRefusal::NoSuchTrack(track))?;
                    if !t.kind.is_vocal() {
                        return Err(AiRefusal::NotKeys);
                    }
                }
                no_step = true;
                let bringing = self.ai_import(path, track, tick, effects);
                let t = self.ai.ticket();
                self.ai.imports.push((bringing, t));
                ticket = Some(t);
                AiWrote::default()
            }
        };
        let undo_count = self.play.undo_count();
        if undo_count == before && !no_step {
            return Err(AiRefusal::Unchanged);
        }
        let doc = &self.play.song().doc;
        let take = (wrote.clip)
            .and_then(|c| doc.clip(c))
            .filter(|(_, c)| c.is_keys())
            .map(|(_, c)| c.active_take_id().0);
        let track = (wrote.track)
            .and_then(|id| doc.tracks.iter().position(|t| t.id == id))
            .map(|i| i as u32 + 1);
        Ok(AiDone {
            step: (undo_count != before)
                .then(|| self.play.history_rows(undo_count, undo_count).pop())
                .flatten(),
            undo_count,
            clip: wrote.clip.map(|c| c.0),
            take,
            notes: notes_done,
            track,
            ticket,
            pending: false,
        })
    }

    /// Not while the song may not change, nor while the export's dialog or run is up (one
    /// second engine at a time), nor while a batch is open (its asks would find meloet
    /// exporting).
    fn ai_render_free(&self) -> Result<(), AiRefusal> {
        if self.ai.batch.is_some() {
            return Err(AiRefusal::BatchOpen);
        }
        if let Some(busy) = self.ai_busy() {
            return Err(AiRefusal::Busy(busy));
        }
        if self.export.dialog_open() || self.bouncing() {
            return Err(AiRefusal::Busy(AiBusy::Exporting));
        }
        Ok(())
    }

    /// The song with only the tracks of these numbers (from 1; `None`: all), and their
    /// numbers and names.
    fn ai_tracks(&self, numbers: Option<&[u32]>) -> Result<(Doc, Vec<(u32, String)>), AiRefusal> {
        let doc = &self.play.song().doc;
        let tracks: Vec<TrackId> = match numbers {
            None => doc.tracks.iter().map(|t| t.id).collect(),
            Some(numbers) => {
                let mut ids = Vec::new();
                for &n in numbers {
                    let index = n.checked_sub(1).ok_or(AiRefusal::NoSuchTrack(n))? as usize;
                    let track = doc.tracks.get(index).ok_or(AiRefusal::NoSuchTrack(n))?;
                    if !ids.contains(&track.id) {
                        ids.push(track.id);
                    }
                }
                ids
            }
        };
        let named = (doc.tracks.iter().enumerate())
            .filter(|(_, t)| tracks.contains(&t.id))
            .map(|(i, t)| (i as u32 + 1, t.name.clone()))
            .collect();
        let doc = Doc {
            tracks: (doc.tracks.iter())
                .filter(|t| tracks.contains(&t.id))
                .cloned()
                .collect(),
            ..doc.clone()
        };
        Ok((doc, named))
    }

    /// A job of the export that renders `ask` for its levels, with this id, and the tracks
    /// rendered alone after the mix.
    pub fn ai_measure_job(
        &self,
        ask: &AiMeasure,
        id: u64,
    ) -> Result<(ExportJob, Vec<AloneTrack>), AiRefusal> {
        self.ai_render_free()?;
        let (doc, mixed) = self.ai_tracks(ask.tracks.as_deref())?;
        let named = match &ask.alone {
            Some(alone) => self.ai_tracks(Some(alone))?.1,
            None => mixed,
        };
        let song = &self.play.song().doc;
        let mut alone = Vec::new();
        let mut muted = Vec::new();
        for (n, _) in &named {
            let track = &song.tracks[*n as usize - 1];
            if doc.track(track.id).is_none() {
                return Err(AiRefusal::NoSuchTrack(*n));
            }
            alone.push(track.id);
            muted.push(track.muted);
        }
        let analysis = ask.analysis.as_ref();
        let mut compare = Vec::new();
        for &n in analysis.map_or(&[][..], |a| &a.compare) {
            let at = (named.iter().position(|(m, _)| *m == n)).ok_or(AiRefusal::NoSuchTrack(n))?;
            if !compare.contains(&at) {
                compare.push(at);
            }
        }
        let mut cuts: Vec<u64> = (analysis.map_or(&[][..], |a| &a.cuts).iter())
            .copied()
            .filter(|&t| t > ask.span.start() && t < ask.span.end())
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let named = (named.into_iter().zip(muted))
            .map(|((n, name), muted)| (n, name, muted))
            .collect();
        let scene = self.export_scene();
        let job = ExportJob {
            id,
            doc: Arc::new(doc),
            span: ask.span,
            per_track: true,
            tail: ExportTail::Off,
            realtime: false,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
            measure: Some(Measuring {
                alone,
                ignore_mute: ask.ignore_mute,
                bands: analysis.map(|a| a.bands),
                compare,
                cuts,
                capture: None,
            }),
        };
        Ok((job, named))
    }

    /// A job of the export that renders the track of `ask` alone and measures each output
    /// set of its instrument plugin, with this id; and the track's number, name and mute.
    pub fn ai_outputs_job(
        &self,
        ask: &AiOutputsMeasure,
        id: u64,
    ) -> Result<(ExportJob, AloneTrack), AiRefusal> {
        self.ai_render_free()?;
        let (doc, mut named) = self.ai_tracks(Some(&[ask.track]))?;
        let (Some(track), Some((number, name))) = (doc.tracks.first(), named.pop()) else {
            return Err(AiRefusal::NoSuchTrack(ask.track));
        };
        let InstrumentRef::Plugin(plugin) = &track.instrument else {
            return Err(AiRefusal::NoSuchPlugin);
        };
        let place = meloet_model::PluginPlace::Instrument(track.id);
        match self.play.plugin_state(place) {
            meloet_protocol::PluginState::Missing => {
                return Err(AiRefusal::Cannot(format!(
                    "this PC does not have the plugin \"{}\"",
                    plugin.name
                )));
            }
            meloet_protocol::PluginState::Stopped => return Err(AiRefusal::PluginNotLoaded),
            meloet_protocol::PluginState::Loading | meloet_protocol::PluginState::Ready => {}
        }
        let scene = self.export_scene();
        // The plugin host checks this too; with the sets known it is said before rendering.
        if let Some(sets) = self.play.instrument_sets(track.id) {
            let map = meloet_model::TempoMap::new(&doc.timeline);
            let seconds =
                map.seconds_at(ask.span.end() as f64) - map.seconds_at(ask.span.start() as f64);
            let longest_s = capture_longest_s(sets.len(), scene.sample_rate);
            if seconds > longest_s {
                return Err(AiRefusal::CaptureTooLong {
                    sets: sets.len() as u32,
                    longest_s,
                });
            }
        }
        let alone = (number, name, track.muted);
        let job = ExportJob {
            id,
            measure: Some(Measuring {
                alone: vec![track.id],
                ignore_mute: true,
                bands: None,
                compare: Vec::new(),
                cuts: Vec::new(),
                capture: Some(track.id),
            }),
            doc: Arc::new(doc),
            span: ask.span,
            per_track: true,
            tail: ExportTail::Off,
            realtime: false,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
        };
        Ok((job, alone))
    }

    /// A job of the export that renders `ask` into files, with this id, and where they go.
    pub fn ai_render_job(
        &self,
        ask: &AiRender,
        id: u64,
    ) -> Result<(ExportJob, ExportTarget), AiRefusal> {
        self.ai_render_free()?;
        let song = &self.play.song().doc;
        let (doc, span, per_track, tail) = match ask.what {
            AiRenderWhat::Song {
                span,
                ref tracks,
                per_track,
                tail,
            } => {
                let (doc, _) = self.ai_tracks(tracks.as_deref())?;
                let tail = if tail {
                    ExportTail::UntilSilent
                } else {
                    ExportTail::Off
                };
                (doc, span, per_track, tail)
            }
            AiRenderWhat::Note {
                track,
                key,
                velocity,
                seconds,
                effects,
            } => {
                let (doc, span) = note_doc(song, track, key, velocity, seconds, effects)?;
                (doc, span, false, ExportTail::UntilSilent)
            }
            AiRenderWhat::Clip { clip, effects } => {
                let (doc, span) = clip_doc(song, clip, effects)?;
                let tail = if effects {
                    ExportTail::UntilSilent
                } else {
                    ExportTail::Off
                };
                (doc, span, false, tail)
            }
        };
        if doc.tracks.is_empty() {
            return Err(AiRefusal::NothingToRender);
        }
        let files: Vec<std::path::PathBuf> = if per_track {
            let mut taken = Vec::new();
            for t in &doc.tracks {
                let name = crate::export::job::free_name(&t.name, ask.format.extension(), &taken);
                taken.push(name);
            }
            taken.iter().map(|n| ask.to.join(n)).collect()
        } else {
            vec![ask.to.clone()]
        };
        if !ask.overwrite
            && let Some(there) = files.iter().find(|f| f.symlink_metadata().is_ok())
        {
            return Err(AiRefusal::Exists(there.clone()));
        }
        let scene = self.export_scene();
        let job = ExportJob {
            id,
            doc: Arc::new(doc),
            span,
            per_track,
            tail,
            realtime: false,
            skip: scene.stopped_plugins.iter().map(|p| p.slot).collect(),
            sample_rate: scene.sample_rate,
            measure: None,
        };
        let target = ExportTarget {
            path: ask.to.clone(),
            format: ask.format,
            sample_rate: ask.sample_rate.unwrap_or(scene.sample_rate),
            bits: ask.bits,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: meloet_protocol::EXPORT_MP3_DEFAULT_KBPS,
            // The AI link's files say nothing of themselves, as before 「曲の情報」.
            info: crate::export::tags::SongInfo::default(),
        };
        Ok((job, target))
    }
}

/// `track` (number from 1) as it is, but only for one note from the start: no clips, no
/// lanes, not muted; without `effects` with no inserts, a fader at 0 dB, no sends and a
/// master with nothing on it. The tempo and meter are the song's at its start.
fn note_doc(
    song: &Doc,
    track: u32,
    key: u8,
    velocity: u8,
    seconds: f64,
    effects: bool,
) -> Result<(Doc, TickRange), AiRefusal> {
    let t = (track.checked_sub(1))
        .and_then(|i| song.tracks.get(i as usize))
        .ok_or(AiRefusal::NoSuchTrack(track))?;
    if t.kind.is_vocal() {
        return Err(AiRefusal::NotKeys);
    }
    let tempo = song.timeline.tempo;
    let quarters = seconds * tempo.bpm() / 60.0;
    let length = (quarters * TICKS_PER_QUARTER as f64).round().max(1.0) as u64;
    let mut alone = Track {
        clips: Vec::new(),
        automation: Vec::new(),
        muted: false,
        ..Track::clone(t)
    };
    if !effects {
        alone.channel = Channel::default();
        alone.sends = [Gain::SILENT; 2];
    }
    let id = alone.id;
    let base = Doc {
        timeline: Arc::new(Timeline::new(tempo, song.timeline.meter)),
        markers: Arc::new(Vec::new()),
        loop_range: None,
        tracks: vec![Arc::new(alone)],
        master: if effects {
            song.master.clone()
        } else {
            Arc::new(Channel::default())
        },
        ..song.clone()
    };
    let note = Note {
        start: 0,
        length,
        key,
        velocity,
        from: None,
    };
    let written =
        notes::write_notes(&base, WriteAt::Track(id), &[note]).map_err(|_| AiRefusal::BadNote)?;
    let span = TickRange::new(0, length).ok_or(AiRefusal::BadNote)?;
    Ok((written.edit.doc, span))
}

/// The clip alone on its track, from its start to its end; without `effects` as
/// [`note_doc`] has the strip.
fn clip_doc(song: &Doc, clip: u32, effects: bool) -> Result<(Doc, TickRange), AiRefusal> {
    let (t, c) = (song.clip(meloet_model::ClipId(clip))).ok_or(AiRefusal::NoSuchClip(clip))?;
    let span = TickRange::new(c.start, c.end).ok_or(AiRefusal::NothingToRender)?;
    let mut alone = Track {
        clips: vec![c.clone()],
        muted: false,
        ..Track::clone(t)
    };
    if !effects {
        alone.channel = Channel::default();
        alone.sends = [Gain::SILENT; 2];
        alone.automation = Vec::new();
    }
    let doc = Doc {
        markers: Arc::new(Vec::new()),
        loop_range: None,
        tracks: vec![Arc::new(alone)],
        master: if effects {
            song.master.clone()
        } else {
            Arc::new(Channel::default())
        },
        ..song.clone()
    };
    Ok((doc, span))
}

#[cfg(test)]
mod batch_tests;

#[cfg(test)]
mod tests {
    use meloet_model::Note;
    use meloet_protocol::{AiLinkView, AiPort, Command, StripRef};

    use crate::ai::{AiAnswer, AiAsk, AiBusy, AiPlace, AiRefusal};
    use crate::settings::Settings;
    use crate::state::{Effect, Input, Model};

    fn saved(effects: &[Effect]) -> Vec<bool> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::SaveSettings(s) => Some(s.ai_link),
                _ => None,
            })
            .collect()
    }

    fn told(effects: &[Effect]) -> Vec<bool> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::AiLink(on) => Some(*on),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_switch_is_a_setting_and_no_step() {
        let m = &mut Model::new(Settings::default());
        let off = AiLinkView {
            on: false,
            port: AiPort::Closed,
        };
        assert_eq!(m.view().ai_link, off);
        let steps = m.view().song.undo_count;

        let effects = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert_eq!(saved(&effects), [true]);
        assert_eq!(told(&effects), [true]);
        // Closed until the socket's thread says it listens.
        assert_eq!(m.view().ai_link.port, AiPort::Closed);
        m.handle(Input::AiPort(AiPort::Waiting));
        assert_eq!(
            m.view().ai_link,
            AiLinkView {
                on: true,
                port: AiPort::Waiting
            }
        );
        m.handle(Input::AiPort(AiPort::Connected));
        assert_eq!(m.view().ai_link.port, AiPort::Connected);
        assert_eq!(m.view().song.undo_count, steps);
        let again = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert!(saved(&again).is_empty(), "the same again saves nothing");
        assert!(told(&again).is_empty());

        let effects = m.handle(Input::Command(Command::SetAiLink { on: false }));
        assert_eq!(saved(&effects), [false]);
        assert_eq!(told(&effects), [false]);
        assert_eq!(m.view().ai_link, off);
        // A late word from the socket does not show while the switch is off.
        m.handle(Input::AiPort(AiPort::Connected));
        assert_eq!(m.view().ai_link, off);
    }

    #[test]
    fn the_switch_is_read_at_the_start() {
        let m = Model::new(Settings {
            ai_link: true,
            ..Settings::default()
        });
        assert!(m.ai_on());
        assert!(m.view().ai_link.on);
    }

    fn ask(m: &mut Model, ask: AiAsk) -> AiAnswer {
        m.handle(Input::Ai(ask));
        m.take_ai_answer().expect("an answer")
    }

    fn note(start: u64, key: u8) -> Note {
        Note {
            start,
            length: 480,
            key,
            velocity: 90,
            from: None,
        }
    }

    #[test]
    fn a_write_is_one_step_and_undo_takes_it_back() {
        let m = &mut Model::new(Settings::default());
        let before = m.ai_song().song.doc;
        let notes = vec![note(0, 60), note(480, 64), note(960, 67)];
        let AiAnswer::Done(done) = ask(
            m,
            AiAsk::WriteNotes {
                at: AiPlace::Track(1),
                notes,
            },
        ) else {
            panic!("written")
        };
        assert_eq!(done.undo_count, 1);
        assert!(done.clip.is_some() && done.take.is_some());
        let song = m.ai_song().song;
        let clip = &song.doc.tracks[0].clips[0];
        assert_eq!(clip.active_take().take.notes.len(), 3);

        let AiAnswer::Done(undone) = ask(m, AiAsk::Undo) else {
            panic!("undone")
        };
        assert_eq!(undone.undo_count, 0);
        assert_eq!(undone.step, done.step);
        let after = m.ai_song().song.doc;
        assert_eq!(after.tracks, before.tracks);
        assert_eq!(
            ask(m, AiAsk::Undo),
            AiAnswer::Refused(AiRefusal::NothingToUndo)
        );
    }

    #[test]
    fn an_old_take_does_nothing() {
        let m = &mut Model::new(Settings::default());
        let AiAnswer::Done(done) = ask(
            m,
            AiAsk::WriteNotes {
                at: AiPlace::Track(1),
                notes: vec![note(0, 60), note(480, 62)],
            },
        ) else {
            panic!("written")
        };
        let take = done.take.unwrap();
        let AiAnswer::Done(deleted) = ask(
            m,
            AiAsk::DeleteNotes {
                take,
                notes: vec![0],
            },
        ) else {
            panic!("deleted")
        };
        assert_ne!(deleted.take, Some(take));
        assert_eq!(
            ask(
                m,
                AiAsk::DeleteNotes {
                    take,
                    notes: vec![0]
                }
            ),
            AiAnswer::Refused(AiRefusal::NoSuchTake(take))
        );
        assert_eq!(m.ai_song().undo_count, 2);
    }

    #[test]
    fn no_write_while_counting_in_or_recording() {
        let m = &mut crate::state::tests::playing();
        m.handle(Input::Command(Command::Record));
        let busy = m.ai_song().busy;
        assert!(
            matches!(busy, Some(AiBusy::CountingIn | AiBusy::Recording)),
            "{busy:?}"
        );
        let answer = ask(
            m,
            AiAsk::AddMarker {
                tick: 0,
                name: "A".into(),
            },
        );
        assert_eq!(answer, AiAnswer::Refused(AiRefusal::Busy(busy.unwrap())));
        assert_eq!(m.ai_song().undo_count, 0);
    }

    #[test]
    fn only_some_screen_intents_are_taken() {
        let m = &mut Model::new(Settings::default());
        let track = m.ai_song().song.doc.tracks[0].id.0;
        for command in [
            Command::SaveSong,
            Command::DeleteSelected,
            Command::SetGain {
                strip: StripRef::Track { id: track },
                db: Some(-3.0),
                commit: false,
            },
        ] {
            let answer = ask(m, AiAsk::Screen(command));
            assert!(matches!(answer, AiAnswer::Refused(AiRefusal::Cannot(_))));
        }
        let gain = Command::SetGain {
            strip: StripRef::Track { id: track },
            db: Some(-3.0),
            commit: true,
        };
        let AiAnswer::Done(done) = ask(m, AiAsk::Screen(gain.clone())) else {
            panic!("set")
        };
        assert_eq!(done.undo_count, 1);
        // The same value again is no step, and no failure.
        assert_eq!(
            ask(m, AiAsk::Screen(gain)),
            AiAnswer::Refused(AiRefusal::Unchanged)
        );
    }

    #[test]
    fn a_song_with_unsaved_changes_is_not_let_go() {
        let m = &mut Model::new(Settings::default());
        ask(
            m,
            AiAsk::WriteNotes {
                at: AiPlace::Track(1),
                notes: vec![note(0, 60)],
            },
        );
        assert!(m.ai_song().unsaved);
        assert_eq!(
            ask(m, AiAsk::NewSong),
            AiAnswer::Refused(AiRefusal::Unsaved)
        );
        let open = AiAsk::Open {
            path: "/nowhere/a.meloet".into(),
        };
        assert_eq!(ask(m, open), AiAnswer::Refused(AiRefusal::Unsaved));
        assert_eq!(
            ask(m, AiAsk::Save),
            AiAnswer::Refused(AiRefusal::NeverSaved)
        );
        assert_eq!(m.ai_song().undo_count, 1);
    }
}
