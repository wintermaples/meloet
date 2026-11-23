//! 「トラックを複製」 (round 12, A3) of a track whose plugins sound: the copy starts from the
//! states they have now, which the song may not have yet (a visit to a window becomes a step
//! only as it ends). They are asked first, and the step is made with what they said once every
//! one has answered: the copy's plugins then have the state the song has for them.

use std::sync::Arc;

use meloet_model::{PluginPlace, StripId, TrackId};

use super::{Phase, Play, Spot, StripSeat};
use crate::state::{Effect, PluginAsk, StateAsk};

#[derive(Debug, Default)]
pub(in crate::play) struct Copying {
    serial: u64,
    waiting: Option<Waiting>,
}

#[derive(Debug)]
struct Waiting {
    serial: u64,
    track: TrackId,
    asks: Vec<CopyAsk>,
}

#[derive(Debug)]
struct CopyAsk {
    ticket: u64,
    /// Where it is on the track.
    place: PluginPlace,
    /// `Plug::puts` when it was asked: a state put back into it since makes the answer old.
    puts: u32,
    /// `None` until it answers; then the state to give the copy, `None` for the song's.
    said: Option<Option<Arc<[u8]>>>,
}

/// What the track's plugins said, by their places on the track: the state to give the plugin
/// in the same place of the copy, `None` for the one the song has.
pub(in crate::play) type CopyStates = Vec<(PluginPlace, Option<Arc<[u8]>>)>;

impl Play {
    /// Asks the plugins of `track` that sound for their states. `false`: there is none to ask.
    /// One still loading has no state the host knows yet, and one an AI state goes into may
    /// yet go back to the song's: the copy has the song's state for those.
    pub(in crate::play) fn ask_copy_states(
        &mut self,
        track: TrackId,
        effects: &mut Vec<Effect>,
    ) -> bool {
        let Some(seat) = self.seats.of(track) else {
            return false;
        };
        let (plugs, doc) = (&self.plugs, &self.song.doc);
        let asks: Vec<CopyAsk> = (plugs.list.iter())
            .filter(|p| p.phase == Phase::Ready && !plugs.ai.putting_into(p.ticket))
            .filter_map(|p| {
                let place = match p.spot {
                    Spot::Seat(s) if s == seat => PluginPlace::Instrument(track),
                    Spot::Insert(StripSeat::Track(s), slot) if s == seat => {
                        PluginPlace::Insert(StripId::Track(track), slot)
                    }
                    _ => return None,
                };
                doc.plugin(place).filter(|q| q.key == p.plugin.key)?;
                Some(CopyAsk {
                    ticket: p.ticket,
                    place,
                    puts: p.puts,
                    said: None,
                })
            })
            .collect();
        if asks.is_empty() {
            return false;
        }
        let copying = &mut self.plugs.copying;
        copying.serial += 1;
        let serial = copying.serial;
        for ask in &asks {
            effects.push(Effect::AskPlugin {
                ticket: ask.ticket,
                ask: PluginAsk::State(StateAsk::Copy(serial)),
            });
        }
        copying.waiting = Some(Waiting {
            serial,
            track,
            asks,
        });
        true
    }

    /// A duplicate waits for the states of its plugins.
    pub(in crate::play) fn copy_waits(&self) -> bool {
        self.plugs.copying.waiting.is_some()
    }

    /// The duplicate that waits is not made: the song is another one, or a step was taken back
    /// or done again.
    pub(in crate::play) fn forget_copy(&mut self) {
        self.plugs.copying.waiting = None;
    }

    /// The answer to an ask of [`Play::ask_copy_states`]. Once every plugin answered: the track
    /// to copy, and what its plugins said.
    pub(in crate::play) fn copy_state_said(
        &mut self,
        ticket: u64,
        serial: u64,
        state: Option<Arc<[u8]>>,
    ) -> Option<(TrackId, CopyStates)> {
        let waiting = (self.plugs.copying.waiting.as_ref()).filter(|w| w.serial == serial)?;
        let at = (waiting.asks.iter()).position(|a| a.ticket == ticket && a.said.is_none())?;
        let puts = waiting.asks[at].puts;
        let state = state.filter(|_| self.plugs.of(ticket).is_some_and(|p| p.puts == puts));
        // Of a plugin the song has "as it starts" and that is still as it started, the copy
        // is "as it starts" too. With no `base` known that cannot be told (what the window
        // or a lane changed is not known to the song): the copy gets the state it said.
        let as_it_starts = |state: &Arc<[u8]>| {
            (self.plugs.of(ticket))
                .is_some_and(|p| p.plugin.state.is_empty() && p.base.as_ref() == Some(state))
        };
        let state = state.filter(|state| !as_it_starts(state));
        let waiting = self.plugs.copying.waiting.as_mut()?;
        waiting.asks[at].said = Some(state);
        if waiting.asks.iter().any(|a| a.said.is_none()) {
            return None;
        }
        let waiting = self.plugs.copying.waiting.take()?;
        let said = (waiting.asks.into_iter()).map(|a| (a.place, a.said.flatten()));
        Some((waiting.track, said.collect()))
    }
}
