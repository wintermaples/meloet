//! The shelf of instruments: trying one out with the keys before it is in the song, adding a
//! track with it, and giving a track another one.

use std::path::Path;
use std::sync::Arc;

use meloet_model::effects::EffectKind as ModelEffect;
use meloet_model::{INSERT_SLOTS, InstrumentRef, PluginRef, StripId, TrackId, edit};
use meloet_protocol::{
    EffectKind, EffectRef, InstrumentKind, LibraryView, SamplePlaying, ShelfMode, ShelfView,
    SoundRef, SoundSampleView, StatusMessage,
};

use super::mixer::{effect_kind, model_effect, strip_id, strip_ref};

use super::plugs::{Spot, Want};
use super::seats::{AUDITION_SEAT, MAX_TRACKS};
use super::sounds::{SAMPLE_US, Sample, SampleWaiting};
use super::{EngineOp, Play};
use crate::added::{effect_ref, missing, sound_name, sound_ref};
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

/// The chosen row of the shelf's effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Chosen {
    Builtin(ModelEffect),
    Plugin(Arc<PluginRef>),
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
    pub(super) effect: Option<Chosen>,
    /// The plugin selected last did not load.
    pub(super) unreadable: Option<SoundRef>,
}

fn builtin(instrument: &InstrumentRef) -> Option<InstrumentKind> {
    match instrument {
        InstrumentRef::Builtin(b) => Some(instruments::kind_of(*b)),
        InstrumentRef::Plugin(_) | InstrumentRef::Sf2(_) => None,
    }
}

fn builtin_sound(instrument: InstrumentKind) -> SoundRef {
    SoundRef::Builtin { instrument }
}

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

    /// Where in the engine the row that is auditioned is.
    fn trial_spot(&self) -> Option<Spot> {
        self.shelf.trial.as_ref()?;
        match self.shelf.mode {
            Mode::NewTrack => Some(Spot::Seat(AUDITION_SEAT)),
            Mode::Replace(track) => self.seats.of(track).map(Spot::Trial),
            Mode::InsertEffect(..) => None,
        }
    }

    /// The row that is auditioned is still being read: the keys do not sound with it yet.
    fn trial_loading(&self) -> bool {
        match (&self.shelf.trial, self.trial_spot()) {
            (Some(trial), Some(spot)) => !self.sounds_now(trial, spot),
            _ => false,
        }
    }

    /// `OpenShelf`. Returns whether the shelf is to be shown. While counting in or recording
    /// only for an effect: instruments are not tried then.
    pub fn open_shelf(&mut self, mode: ShelfMode, effects: &mut Vec<Effect>) -> bool {
        if self.busy() && !matches!(mode, ShelfMode::InsertEffect { .. }) {
            return false;
        }
        // Replacing an effect is not made yet: the shelf stays as it is.
        if matches!(mode, ShelfMode::ReplaceEffect { .. }) {
            return false;
        }
        let mode = match mode {
            ShelfMode::NewTrack => Mode::NewTrack,
            ShelfMode::Replace { track } if self.doc().track(TrackId(track)).is_some() => {
                Mode::Replace(TrackId(track))
            }
            ShelfMode::Replace { .. } | ShelfMode::ReplaceEffect { .. } => return false,
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
        self.shelf.unreadable = None;
        true
    }

    fn slot_is_empty(&self, strip: StripId, slot: usize) -> bool {
        slot < INSERT_SLOTS
            && (self.doc().channel(strip)).is_some_and(|c| c.inserts[slot].is_none())
    }

    /// `ShelfSelectEffect`: only while the shelf lists the effects.
    pub fn shelf_select_effect(&mut self, effect: Option<EffectKind>) {
        if matches!(self.shelf.mode, Mode::InsertEffect(..)) {
            self.shelf.effect = effect.map(|kind| Chosen::Builtin(model_effect(kind)));
        }
    }

    /// `ShelfChooseEffect` with a plugin, as the shelf has it.
    pub fn shelf_choose_plugin(&mut self, plugin: Option<Arc<PluginRef>>) {
        if matches!(self.shelf.mode, Mode::InsertEffect(..)) {
            self.shelf.effect = plugin.map(Chosen::Plugin);
        }
    }

    /// `ShelfSelect`: the keys sound with `instrument` from now on.
    pub fn shelf_select(&mut self, instrument: Option<InstrumentKind>, effects: &mut Vec<Effect>) {
        let instrument = instrument
            .map(instruments::builtin_of)
            .filter(|b| self.sounds.available().contains(b))
            .map(InstrumentRef::Builtin);
        self.shelf_select_sound(instrument, effects);
    }

    /// `ShelfSelectSound`: the keys sound with `instrument`, a sound the shelf has, from now
    /// on; a plugin once it is loaded.
    pub fn shelf_select_sound(
        &mut self,
        instrument: Option<InstrumentRef>,
        effects: &mut Vec<Effect>,
    ) {
        if self.busy() || matches!(self.shelf.mode, Mode::InsertEffect(..)) {
            return;
        }
        let Some(instrument) = instrument else {
            self.end_audition(effects);
            return;
        };
        self.shelf.unreadable = None;
        if self.shelf.trial.as_ref() == Some(&instrument) {
            return;
        }
        match self.shelf.mode {
            Mode::NewTrack => self.seat_instrument(AUDITION_SEAT, &instrument, effects),
            Mode::Replace(track) => {
                let Some(seat) = self.seats.of(track) else {
                    return;
                };
                match &instrument {
                    InstrumentRef::Plugin(p) => {
                        match self.want_plug(Spot::Trial(seat), p, None, effects) {
                            Want::Kept => {}
                            Want::Loading => {
                                effects.push(Effect::Engine(EngineOp::TryInstrument {
                                    seat,
                                    instrument: instrument.clone(),
                                }))
                            }
                            Want::Missing => return,
                        }
                    }
                    other => {
                        self.drop_plug(Spot::Trial(seat), effects);
                        effects.push(Effect::Engine(EngineOp::TryInstrument {
                            seat,
                            instrument: other.clone(),
                        }));
                    }
                }
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
            let went_in = match self.shelf.effect.clone()? {
                Chosen::Builtin(kind) => self.add_effect(strip, slot, kind, effects),
                Chosen::Plugin(plugin) => self.add_plugin_effect(strip, slot, plugin, effects),
            };
            if went_in {
                self.shelf.mode = Mode::NewTrack;
                self.shelf.effect = None;
            }
            return None;
        }
        // A plugin that is still being read may yet not load: no track is made for it.
        if self.busy() || self.trial_loading() && self.trial_is_plugin() {
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
                self.move_plug(Spot::Seat(AUDITION_SEAT), Spot::Seat(seat), effects);
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
                self.shelf.trial = None;
                if let Some(seat) = self.seats.of(track) {
                    self.seats.set_instrument(seat, instrument.clone());
                    effects.push(Effect::Engine(EngineOp::EndTrial { seat, keep: true }));
                    // What was tried is the seat's own now; the engine hands back what was.
                    self.drop_plug(Spot::Seat(seat), effects);
                    self.move_plug(Spot::Trial(seat), Spot::Seat(seat), effects);
                }
                self.apply(e, effects);
                let track = self.track_ref(track)?;
                Some(match builtin(&instrument) {
                    Some(instrument) => StatusMessage::InstrumentReplaced { track, instrument },
                    None => StatusMessage::InstrumentReplacedSound {
                        track,
                        sound: sound_name(&instrument),
                    },
                })
            }
            Mode::InsertEffect(..) => None,
        }
    }

    fn trial_is_plugin(&self) -> bool {
        matches!(self.shelf.trial, Some(InstrumentRef::Plugin(_)))
    }

    /// `ShelfCancel`: the keys sound on the selected track again, and a track that was tried
    /// with another instrument has its own back. Returns whether a row was auditioned.
    pub fn end_audition(&mut self, effects: &mut Vec<Effect>) -> bool {
        let seat = self.audition_seat();
        let Some(tried) = self.shelf.trial.take() else {
            return false;
        };
        let plugin_tried = matches!(tried, InstrumentRef::Plugin(_));
        self.shelf.before = Some(tried);
        self.sync_target(effects);
        match (self.shelf.mode, seat) {
            (Mode::NewTrack, _) => self.clear_seat(AUDITION_SEAT, effects),
            (Mode::Replace(_), Some(seat)) => {
                self.drop_plug(Spot::Trial(seat), effects);
                effects.push(Effect::Engine(EngineOp::EndTrial { seat, keep: false }));
                match self.seats.instrument(seat) {
                    // Its samples may have come while the plugin was tried.
                    Some(own) if plugin_tried && !matches!(own, InstrumentRef::Plugin(_)) => {
                        effects.push(Effect::Engine(EngineOp::Instrument {
                            seat,
                            instrument: own.clone(),
                        }));
                    }
                    _ => self.place_waiting(seat, effects),
                }
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
        self.shelf.unreadable = None;
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

    /// `favorites`: the starred instruments of meloet, a setting. `stars`: the other starred
    /// sounds, and the starred effects. `library`: what the shelf has of SF2 files and
    /// plugins; `home`: the home folder, shown as "~".
    pub fn shelf_view(
        &self,
        favorites: Vec<InstrumentKind>,
        stars: (Vec<SoundRef>, Vec<EffectRef>),
        library: &LibraryView,
        home: Option<&Path>,
    ) -> ShelfView {
        let current = match self.shelf.mode {
            Mode::NewTrack | Mode::InsertEffect(..) => None,
            Mode::Replace(track) => self.doc().track(track).map(|t| &t.instrument),
        };
        let trial = self.shelf.trial.as_ref();
        let loading = trial.filter(|_| self.trial_loading());
        let playing = self.shelf.sample.as_ref().map(|s| SoundSampleView {
            sound: sound_ref(&s.instrument),
            playing: Some(SamplePlaying {
                started_us: s.started_us,
                length_us: SAMPLE_US,
            }),
        });
        let waiting = self.shelf.sample_waiting.as_ref().map(|w| SoundSampleView {
            sound: sound_ref(&w.instrument),
            playing: None,
        });
        let chosen_effect = self.shelf.effect.as_ref().map(|chosen| match chosen {
            Chosen::Builtin(kind) => effect_ref(effect_kind(*kind)),
            Chosen::Plugin(p) => EffectRef::Plugin {
                key: p.key.to_string(),
            },
        });
        let (starred_sounds, favorite_effects) = stars;
        ShelfView {
            selected_sound: trial.map(sound_ref),
            current_sound: current.map(sound_ref),
            loading_sound: loading.map(sound_ref),
            sound_sample: playing.or(waiting),
            chosen_effect,
            current_effect: None,
            unreadable: self.shelf.unreadable.clone(),
            favorite_sounds: (favorites.iter().copied().map(builtin_sound))
                .chain(starred_sounds)
                .collect(),
            favorite_effects,
            missing: missing(self.doc(), library, home),
            mode: match self.shelf.mode {
                Mode::NewTrack => ShelfMode::NewTrack,
                Mode::Replace(track) => ShelfMode::Replace { track: track.0 },
                Mode::InsertEffect(strip, slot) => ShelfMode::InsertEffect {
                    strip: strip_ref(strip),
                    slot: slot as u8,
                },
            },
            available: (self.sounds.shelf_rows().into_iter())
                .map(instruments::kind_of)
                .collect(),
        }
    }

    /// The status line while a row is auditioned.
    pub fn audition_status(&self) -> Option<StatusMessage> {
        let trial = self.shelf.trial.as_ref()?;
        if let (InstrumentRef::Plugin(p), true) = (trial, self.trial_loading()) {
            return Some(StatusMessage::PluginLoading {
                name: p.name.to_string(),
            });
        }
        Some(match self.shelf.mode {
            Mode::NewTrack => match builtin(trial) {
                Some(instrument) => StatusMessage::AuditionNewTrack { instrument },
                None => StatusMessage::AuditionNewTrackSound {
                    sound: sound_name(trial),
                },
            },
            Mode::Replace(track) => {
                let current = &self.doc().track(track)?.instrument;
                let track = self.track_ref(track)?;
                match (builtin(trial), builtin(current)) {
                    (Some(instrument), Some(current)) => StatusMessage::AuditionReplace {
                        instrument,
                        track,
                        current,
                    },
                    _ => StatusMessage::AuditionReplaceSound {
                        sound: sound_name(trial),
                        track,
                        current: sound_name(current),
                    },
                }
            }
            Mode::InsertEffect(..) => return None,
        })
    }
}

#[cfg(test)]
mod tests;
