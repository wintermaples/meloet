//! The shelf of instruments: trying one out with the keys before it is in the song, adding a
//! track with it, and giving a track another one.

use std::path::Path;

use meloet_model::effects::EffectKind as ModelEffect;
use meloet_model::{INSERT_SLOTS, InstrumentRef, StripId, TrackId, edit};
use meloet_protocol::{
    EffectKind, InstrumentKind, LibraryView, SamplePlaying, ShelfMode, ShelfView, SoundRef,
    SoundSampleView, StatusMessage,
};

use super::mixer::{effect_kind, model_effect, strip_id, strip_ref};

use super::seats::{AUDITION_SEAT, MAX_TRACKS};
use super::sounds::{Sample, SampleWaiting};
use super::{EngineOp, Play};
use crate::added::{builtin_or_sine, effect_ref, missing, sound_name, sound_ref};
use crate::instruments;
use crate::state::Effect;

/// What Enter does in the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Mode {
    #[default]
    NewTrack,
    Replace(TrackId),
    /// The shelf lists the effects; Enter puts the chosen one into this empty slot.
    InsertEffect(StripId, usize),
}

#[derive(Debug, Default)]
pub(super) struct Shelf {
    pub(super) mode: Mode,
    /// The row that is auditioned. `NewTrack`: it is on the audition seat. `Replace`: it is
    /// on the track's seat, with the track's own instrument set aside.
    pub(super) trial: Option<InstrumentRef>,
    /// The row auditioned before this one, while the shelf is open: its samples stay in
    /// memory, so that going back to it sounds at once.
    pub(super) before: Option<InstrumentRef>,
    pub(super) sample: Option<Sample>,
    pub(super) sample_waiting: Option<SampleWaiting>,
    /// `InsertEffect`: the chosen row.
    pub(super) effect: Option<ModelEffect>,
}

fn kind_of(instrument: &InstrumentRef) -> InstrumentKind {
    instruments::kind_of(builtin_or_sine(instrument))
}

fn builtin_sound(instrument: InstrumentKind) -> SoundRef {
    SoundRef::Builtin { instrument }
}

/// The effects starred before the person stars any (spec 3, 86-4).
const DEFAULT_FAVORITE_EFFECTS: [EffectKind; 2] = [EffectKind::Reverb, EffectKind::Delay];

impl Play {
    /// While the shelf auditions, the keys sound with the row and nothing is recorded.
    pub fn auditioning(&self) -> bool {
        self.shelf.trial.is_some()
    }

    /// The seat the keys play while the shelf auditions.
    pub(super) fn audition_seat(&self) -> Option<usize> {
        self.shelf.trial.as_ref()?;
        match self.shelf.mode {
            Mode::NewTrack => Some(AUDITION_SEAT),
            Mode::Replace(track) => self.seats.of(track),
            Mode::InsertEffect(..) => None,
        }
    }

    /// `OpenShelf`. Returns whether the shelf is to be shown. While counting in or recording
    /// only for an effect: instruments are not tried then.
    pub fn open_shelf(&mut self, mode: ShelfMode, effects: &mut Vec<Effect>) -> bool {
        if self.busy() && !matches!(mode, ShelfMode::InsertEffect { .. }) {
            return false;
        }
        let mode = match mode {
            ShelfMode::NewTrack => Mode::NewTrack,
            ShelfMode::Replace { track } if self.doc().track(TrackId(track)).is_some() => {
                Mode::Replace(TrackId(track))
            }
            ShelfMode::Replace { .. } => return false,
            ShelfMode::InsertEffect { strip, slot } => {
                let (strip, slot) = (strip_id(strip), usize::from(slot));
                if !self.slot_is_empty(strip, slot) {
                    return false;
                }
                Mode::InsertEffect(strip, slot)
            }
        };
        self.end_audition(effects);
        self.shelf.mode = mode;
        self.shelf.effect = None;
        true
    }

    fn slot_is_empty(&self, strip: StripId, slot: usize) -> bool {
        slot < INSERT_SLOTS
            && (self.doc().channel(strip)).is_some_and(|c| c.inserts[slot].is_none())
    }

    /// `ShelfSelectEffect`: only while the shelf lists the effects.
    pub fn shelf_select_effect(&mut self, effect: Option<EffectKind>) {
        if matches!(self.shelf.mode, Mode::InsertEffect(..)) {
            self.shelf.effect = effect.map(model_effect);
        }
    }

    /// `ShelfSelect`: the keys sound with `instrument` from now on.
    pub fn shelf_select(&mut self, instrument: Option<InstrumentKind>, effects: &mut Vec<Effect>) {
        if self.busy() || matches!(self.shelf.mode, Mode::InsertEffect(..)) {
            return;
        }
        let Some(builtin) = instrument
            .map(instruments::builtin_of)
            .filter(|b| self.sounds.available().contains(b))
        else {
            self.end_audition(effects);
            return;
        };
        let instrument = InstrumentRef::Builtin(builtin);
        if self.shelf.trial.as_ref() == Some(&instrument) {
            return;
        }
        match self.shelf.mode {
            Mode::NewTrack => effects.push(Effect::Engine(EngineOp::Instrument {
                seat: AUDITION_SEAT,
                instrument: instrument.clone(),
            })),
            Mode::Replace(track) => {
                let Some(seat) = self.seats.of(track) else {
                    return;
                };
                effects.push(Effect::Engine(EngineOp::TryInstrument {
                    seat,
                    instrument: instrument.clone(),
                }));
            }
            Mode::InsertEffect(..) => return,
        }
        if let Some(before) = self.shelf.trial.replace(instrument) {
            self.shelf.before = Some(before);
        }
        self.sync_target(effects);
    }

    /// `ShelfConfirm`. Returns the notice to show.
    pub fn shelf_confirm(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if let Mode::InsertEffect(strip, slot) = self.shelf.mode {
            let kind = self.shelf.effect?;
            if self.add_effect(strip, slot, kind, effects) {
                self.shelf.mode = Mode::NewTrack;
                self.shelf.effect = None;
            }
            return None;
        }
        if self.busy() {
            return None;
        }
        let instrument = self.shelf.trial.clone()?;
        match self.shelf.mode {
            Mode::NewTrack => {
                if self.doc().tracks.len() >= MAX_TRACKS {
                    return None;
                }
                let below = self.song.session.play_target;
                let (e, id) = edit::add_track(self.doc(), below, instrument.clone()).ok()?;
                let seat = self.seats.take(id, instrument)?;
                self.shelf.trial = None;
                self.song.session.play_target = Some(id);
                self.apply(e, effects);
                // The instrument that was auditioned becomes the track's, as it sounds. After
                // the step, which gives the still empty seat its fader and sends at once: the
                // sound then does not change as the instrument moves.
                effects.push(Effect::Engine(EngineOp::MoveInstrument {
                    from: AUDITION_SEAT,
                    to: seat,
                }));
                self.sync_target(effects);
                Some(StatusMessage::TrackAdded {
                    track: self.track_ref(id)?,
                })
            }
            Mode::Replace(track) => {
                let Ok(e) = edit::set_instrument(self.doc(), track, instrument.clone()) else {
                    self.end_audition(effects);
                    return None;
                };
                let kind = kind_of(&instrument);
                self.shelf.trial = None;
                if let Some(seat) = self.seats.of(track) {
                    self.seats.set_instrument(seat, instrument);
                    effects.push(Effect::Engine(EngineOp::EndTrial { seat, keep: true }));
                }
                self.apply(e, effects);
                Some(StatusMessage::InstrumentReplaced {
                    track: self.track_ref(track)?,
                    instrument: kind,
                })
            }
            Mode::InsertEffect(..) => None,
        }
    }

    /// `ShelfCancel`: the keys sound on the selected track again, and a track that was tried
    /// with another instrument has its own back. Returns whether a row was auditioned.
    pub fn end_audition(&mut self, effects: &mut Vec<Effect>) -> bool {
        let seat = self.audition_seat();
        let Some(tried) = self.shelf.trial.take() else {
            return false;
        };
        self.shelf.before = Some(tried);
        self.sync_target(effects);
        match (self.shelf.mode, seat) {
            (Mode::NewTrack, _) => self.clear_seat(AUDITION_SEAT, effects),
            (Mode::Replace(_), Some(seat)) => {
                effects.push(Effect::Engine(EngineOp::EndTrial { seat, keep: false }));
            }
            (Mode::Replace(_), None) | (Mode::InsertEffect(..), _) => {}
        }
        true
    }

    /// The shelf is not open (any more).
    pub fn shelf_closed(&mut self, effects: &mut Vec<Effect>) {
        self.end_audition(effects);
        self.stop_sample(effects);
        self.shelf.before = None;
        self.shelf.mode = Mode::NewTrack;
        self.shelf.effect = None;
    }

    /// The `Doc` changed under the shelf: a trial on a track that is gone, or whose own
    /// instrument is another one now (undo), ends; "入れかえる" of a track that is gone
    /// becomes "新しいトラック".
    pub(super) fn end_stale_audition(&mut self, effects: &mut Vec<Effect>) {
        if let Mode::InsertEffect(strip, slot) = self.shelf.mode
            && !self.slot_is_empty(strip, slot)
        {
            self.shelf.mode = Mode::NewTrack;
            self.shelf.effect = None;
        }
        let Mode::Replace(track) = self.shelf.mode else {
            return;
        };
        let own = self
            .seats
            .of(track)
            .and_then(|seat| self.seats.instrument(seat));
        let there = self.doc().track(track);
        if there.map(|t| &t.instrument) != own {
            self.end_audition(effects);
        }
        if self.doc().track(track).is_none() {
            self.shelf.mode = Mode::NewTrack;
        }
    }

    /// `favorites`: the starred instruments, a setting of meloet. `library`: what the shelf
    /// has of SF2 files and plugins; `home`: the home folder, shown as "~".
    pub fn shelf_view(
        &self,
        favorites: Vec<InstrumentKind>,
        library: &LibraryView,
        home: Option<&Path>,
    ) -> ShelfView {
        let current = match self.shelf.mode {
            Mode::NewTrack | Mode::InsertEffect(..) => None,
            Mode::Replace(track) => self.doc().track(track).map(|t| &t.instrument),
        };
        let builtin = |i: &InstrumentRef| match i {
            InstrumentRef::Builtin(b) => Some(instruments::kind_of(*b)),
            InstrumentRef::Plugin(_) | InstrumentRef::Sf2(_) => None,
        };
        let trial = self.shelf.trial.as_ref();
        let loading = trial.filter(|i| !self.sounds.ready(builtin_or_sine(i)));
        let sample = self.sample_view();
        let chosen = self.shelf.effect.map(effect_kind);
        ShelfView {
            selected_sound: trial.map(sound_ref),
            current_sound: current.map(sound_ref),
            loading_sound: loading.map(sound_ref),
            sound_sample: sample.map(|s| SoundSampleView {
                sound: builtin_sound(s.instrument),
                playing: Some(SamplePlaying {
                    started_us: s.started_us,
                    length_us: s.length_us,
                }),
            }),
            chosen_effect: chosen.map(effect_ref),
            unreadable: None,
            favorite_sounds: favorites.iter().copied().map(builtin_sound).collect(),
            favorite_effects: DEFAULT_FAVORITE_EFFECTS.map(effect_ref).to_vec(),
            missing: missing(self.doc(), library, home),
            selected_effect: chosen,
            mode: match self.shelf.mode {
                Mode::NewTrack => ShelfMode::NewTrack,
                Mode::Replace(track) => ShelfMode::Replace { track: track.0 },
                Mode::InsertEffect(strip, slot) => ShelfMode::InsertEffect {
                    strip: strip_ref(strip),
                    slot: slot as u8,
                },
            },
            available: (self.sounds.available().into_iter())
                .map(instruments::kind_of)
                .collect(),
            favorites,
            selected: trial.and_then(builtin),
            current: current.and_then(builtin),
            loading: loading.and_then(builtin),
            sample,
        }
    }

    /// The status line while a row is auditioned.
    pub fn audition_status(&self) -> Option<StatusMessage> {
        let trial = self.shelf.trial.as_ref()?;
        let instrument = kind_of(trial);
        Some(match self.shelf.mode {
            Mode::NewTrack => StatusMessage::AuditionNewTrack { instrument },
            Mode::Replace(track) => match &self.doc().track(track)?.instrument {
                InstrumentRef::Builtin(current) => StatusMessage::AuditionReplace {
                    instrument,
                    track: self.track_ref(track)?,
                    current: instruments::kind_of(*current),
                },
                current => StatusMessage::AuditionReplaceSound {
                    sound: sound_name(trial),
                    track: self.track_ref(track)?,
                    current: sound_name(current),
                },
            },
            Mode::InsertEffect(..) => return None,
        })
    }
}

#[cfg(test)]
mod tests;
