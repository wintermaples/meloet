//! The shelf of instruments: trying one out with the keys before it is in the song, adding a
//! track with it, and giving a track another one.

use meloet_model::{InstrumentRef, TrackId, edit};
use meloet_protocol::{InstrumentKind, ShelfMode, ShelfView, StatusMessage};

use super::seats::{AUDITION_SEAT, MAX_TRACKS};
use super::{EngineOp, Play};
use crate::instruments;
use crate::state::Effect;

/// What Enter does in the shelf.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    NewTrack,
    Replace(TrackId),
}

#[derive(Debug, Default)]
pub(super) struct Shelf {
    mode: Mode,
    /// The row that is auditioned. `NewTrack`: it is on the audition seat. `Replace`: it is
    /// on the track's seat, with the track's own instrument set aside.
    trial: Option<InstrumentRef>,
}

fn kind_of(instrument: &InstrumentRef) -> InstrumentKind {
    match instrument {
        InstrumentRef::Builtin(b) => instruments::kind_of(*b),
    }
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
        }
    }

    /// `OpenShelf`. Returns whether the shelf is to be shown.
    pub fn open_shelf(&mut self, mode: ShelfMode, effects: &mut Vec<Effect>) -> bool {
        if self.busy() {
            return false;
        }
        let mode = match mode {
            ShelfMode::NewTrack => Mode::NewTrack,
            ShelfMode::Replace { track } if self.doc().track(TrackId(track)).is_some() => {
                Mode::Replace(TrackId(track))
            }
            ShelfMode::Replace { .. } => return false,
        };
        self.end_audition(effects);
        self.shelf.mode = mode;
        true
    }

    /// `ShelfSelect`: the keys sound with `instrument` from now on.
    pub fn shelf_select(&mut self, instrument: Option<InstrumentKind>, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let Some(builtin) = instrument
            .map(instruments::builtin_of)
            .filter(|b| instruments::available().contains(b))
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
        }
        self.shelf.trial = Some(instrument);
        self.sync_target(effects);
    }

    /// `ShelfConfirm`. Returns the notice to show.
    pub fn shelf_confirm(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
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
                // The instrument that was auditioned becomes the track's, as it sounds.
                let seat = self.seats.take(id, instrument)?;
                effects.push(Effect::Engine(EngineOp::MoveInstrument {
                    from: AUDITION_SEAT,
                    to: seat,
                }));
                self.shelf.trial = None;
                self.song.session.play_target = Some(id);
                self.apply(e, effects);
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
        }
    }

    /// `ShelfCancel`: the keys sound on the selected track again, and a track that was tried
    /// with another instrument has its own back. Returns whether a row was auditioned.
    pub fn end_audition(&mut self, effects: &mut Vec<Effect>) -> bool {
        let seat = self.audition_seat();
        if self.shelf.trial.take().is_none() {
            return false;
        }
        self.sync_target(effects);
        match (self.shelf.mode, seat) {
            (Mode::NewTrack, _) => effects.push(Effect::Engine(EngineOp::ClearSeat {
                seat: AUDITION_SEAT,
            })),
            (Mode::Replace(_), Some(seat)) => {
                effects.push(Effect::Engine(EngineOp::EndTrial { seat, keep: false }));
            }
            (Mode::Replace(_), None) => {}
        }
        true
    }

    /// The shelf is not open (any more).
    pub fn shelf_closed(&mut self, effects: &mut Vec<Effect>) {
        self.end_audition(effects);
        self.shelf.mode = Mode::NewTrack;
    }

    /// The `Doc` changed under the shelf: a trial on a track that is gone, or whose own
    /// instrument is another one now (undo), ends; "入れかえる" of a track that is gone
    /// becomes "新しいトラック".
    pub(super) fn end_stale_audition(&mut self, effects: &mut Vec<Effect>) {
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

    /// `favorites`: the starred instruments, a setting of meloet.
    pub fn shelf_view(&self, favorites: Vec<InstrumentKind>) -> ShelfView {
        let current = match self.shelf.mode {
            Mode::NewTrack => None,
            Mode::Replace(track) => self.doc().track(track).map(|t| kind_of(&t.instrument)),
        };
        ShelfView {
            mode: match self.shelf.mode {
                Mode::NewTrack => ShelfMode::NewTrack,
                Mode::Replace(track) => ShelfMode::Replace { track: track.0 },
            },
            available: instruments::available()
                .into_iter()
                .map(instruments::kind_of)
                .collect(),
            favorites,
            selected: self.shelf.trial.as_ref().map(kind_of),
            current,
            loading: None,
            sample: None,
        }
    }

    /// The status line while a row is auditioned.
    pub fn audition_status(&self) -> Option<StatusMessage> {
        let instrument = kind_of(self.shelf.trial.as_ref()?);
        Some(match self.shelf.mode {
            Mode::NewTrack => StatusMessage::AuditionNewTrack { instrument },
            Mode::Replace(track) => StatusMessage::AuditionReplace {
                instrument,
                track: self.track_ref(track)?,
                current: kind_of(&self.doc().track(track)?.instrument),
            },
        })
    }
}

#[cfg(test)]
mod tests;
