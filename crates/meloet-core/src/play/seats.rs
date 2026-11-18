//! Which of the engine's tracks ("seats") holds which track of the song. A track keeps its
//! seat for as long as it is in the song, wherever it moves in the order: the instrument on
//! the seat, and what it is sounding, is then never made anew for a move or for a change to
//! another track.

use std::sync::Arc;

use meloet_model::playback::track_events;
use meloet_model::{Doc, InstrumentRef, TrackId};

use super::{EngineOp, Play};
use crate::state::Effect;

/// A song has at most this many tracks: the seats of the engine.
pub const MAX_TRACKS: usize = 64;
/// The engine's track after the seats: where an instrument is tried out before it is on a
/// track of the song (the shelf's "新しいトラックの音源をえらぶ").
pub const AUDITION_SEAT: usize = MAX_TRACKS;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    track: TrackId,
    /// The track's own instrument as the engine has it.
    instrument: InstrumentRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seats {
    held: Vec<Option<Held>>,
}

impl Seats {
    pub fn empty() -> Seats {
        Seats {
            held: vec![None; MAX_TRACKS],
        }
    }

    /// The tracks of `doc` on the first seats, in their order.
    pub fn for_song(doc: &Doc) -> Seats {
        let mut seats = Seats::empty();
        for (slot, t) in seats.held.iter_mut().zip(&doc.tracks) {
            *slot = Some(Held {
                track: t.id,
                instrument: t.instrument.clone(),
            });
        }
        seats
    }

    pub fn of(&self, track: TrackId) -> Option<usize> {
        self.held
            .iter()
            .position(|h| h.as_ref().is_some_and(|h| h.track == track))
    }

    pub fn instrument(&self, seat: usize) -> Option<&InstrumentRef> {
        self.held.get(seat)?.as_ref().map(|h| &h.instrument)
    }

    /// Gives `track` the first free seat. `None` when every seat is taken.
    pub fn take(&mut self, track: TrackId, instrument: InstrumentRef) -> Option<usize> {
        let seat = self.held.iter().position(Option::is_none)?;
        self.held[seat] = Some(Held { track, instrument });
        Some(seat)
    }

    pub fn set_instrument(&mut self, seat: usize, instrument: InstrumentRef) {
        if let Some(Some(held)) = self.held.get_mut(seat) {
            held.instrument = instrument;
        }
    }

    fn free(&mut self, seat: usize) {
        self.held[seat] = None;
    }

    /// (seat, track, instrument) of every seat taken.
    fn taken(&self) -> impl Iterator<Item = (usize, TrackId, &InstrumentRef)> {
        self.held
            .iter()
            .enumerate()
            .filter_map(|(seat, h)| h.as_ref().map(|h| (seat, h.track, &h.instrument)))
    }
}

impl Play {
    /// The seat of each track, in the order of the song. `None`: a track past the last seat,
    /// which makes no sound.
    pub fn track_seats(&self) -> Vec<Option<usize>> {
        let seats = &self.seats;
        self.doc().tracks.iter().map(|t| seats.of(t.id)).collect()
    }

    /// The instruments and the play target, for an engine that has none yet.
    pub fn engine_seats(&self) -> Vec<EngineOp> {
        let mut ops: Vec<EngineOp> = self
            .seats
            .taken()
            .map(|(seat, _, instrument)| EngineOp::Instrument {
                seat,
                instrument: instrument.clone(),
            })
            .collect();
        ops.push(EngineOp::PlayTarget {
            seat: self.target_seat(),
        });
        ops
    }

    /// Where the keys sound: the selected track, or what the shelf auditions.
    pub(super) fn target_seat(&self) -> Option<usize> {
        self.audition_seat()
            .or_else(|| self.seats.of(self.song.session.play_target?))
    }

    /// Tells the engine when the keys sound somewhere else now.
    pub fn sync_target(&mut self, effects: &mut Vec<Effect>) {
        let seat = self.target_seat();
        if seat != self.target_sent {
            self.send_target(effects);
        }
    }

    pub(super) fn send_target(&mut self, effects: &mut Vec<Effect>) {
        self.target_sent = self.target_seat();
        effects.push(Effect::Engine(EngineOp::PlayTarget {
            seat: self.target_sent,
        }));
    }

    /// After the `Doc` changed: seats of tracks that are gone are emptied, new tracks get a
    /// seat, and a seat is sent again only what changed for it. `before`: the `Doc` the
    /// tracks are compared with, to send the notes of the changed ones only.
    pub(super) fn sync_seats(&mut self, before: &Doc, effects: &mut Vec<Effect>) {
        self.end_stale_audition(effects);
        let doc = self.song.doc.clone();
        let gone: Vec<usize> = self
            .seats
            .taken()
            .filter(|(_, track, _)| doc.track(*track).is_none())
            .map(|(seat, _, _)| seat)
            .collect();
        for seat in gone {
            self.seats.free(seat);
            effects.push(Effect::Engine(EngineOp::ClearSeat { seat }));
        }
        for t in &doc.tracks {
            let seat = match self.seats.of(t.id) {
                Some(seat) => {
                    if self.seats.instrument(seat) != Some(&t.instrument) {
                        self.seats.set_instrument(seat, t.instrument.clone());
                        effects.push(Effect::Engine(EngineOp::Instrument {
                            seat,
                            instrument: t.instrument.clone(),
                        }));
                    }
                    if before.track(t.id).is_some_and(|b| Arc::ptr_eq(b, t)) {
                        continue;
                    }
                    seat
                }
                None => {
                    let Some(seat) = self.seats.take(t.id, t.instrument.clone()) else {
                        continue;
                    };
                    effects.push(Effect::Engine(EngineOp::Instrument {
                        seat,
                        instrument: t.instrument.clone(),
                    }));
                    seat
                }
            };
            effects.push(Effect::Engine(EngineOp::Sequence {
                track: seat,
                events: track_events(&doc, t.id),
            }));
        }
    }

    /// Another song: its tracks take the first seats, each with an instrument made anew, and
    /// the seats left over are emptied.
    pub(super) fn reseat(&mut self, effects: &mut Vec<Effect>) {
        let old: Vec<usize> = self.seats.taken().map(|(seat, _, _)| seat).collect();
        self.seats = Seats::for_song(&self.song.doc);
        for (seat, _, instrument) in self.seats.taken() {
            effects.push(Effect::Engine(EngineOp::Instrument {
                seat,
                instrument: instrument.clone(),
            }));
        }
        for seat in old {
            if self.seats.instrument(seat).is_none() {
                effects.push(Effect::Engine(EngineOp::ClearSeat { seat }));
            }
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
