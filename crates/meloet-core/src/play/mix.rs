//! Mute and solo: which tracks the song plays. Mute is part of the song (a step); solo is only
//! for listening, kept in the session.

use meloet_model::TrackId;
use meloet_model::edit;

use super::{EngineOp, Play};
use crate::state::Effect;

impl Play {
    /// Not while counting in or recording: it changes the song.
    pub fn toggle_mute(&mut self, track: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::toggle_mute(self.doc(), TrackId(track)) {
            self.apply(e, effects);
        }
    }

    pub fn toggle_solo(&mut self, track: u32) {
        let id = TrackId(track);
        if self.doc().track(id).is_none() {
            return;
        }
        let soloed = &mut self.song.session.soloed;
        if !soloed.remove(&id) {
            soloed.insert(id);
        }
    }

    pub fn is_soloed(&self, track: TrackId) -> bool {
        self.song.session.soloed.contains(&track)
    }

    /// The seats of the tracks that do not play, in order of seat.
    fn silent_tracks(&self) -> Vec<usize> {
        let doc = self.doc();
        let soloed = &self.song.session.soloed;
        let mut seats: Vec<usize> = doc
            .tracks
            .iter()
            .filter(|t| !doc.plays(t, soloed))
            .filter_map(|t| self.seats.of(t.id))
            .collect();
        seats.sort_unstable();
        seats
    }

    pub(super) fn send_silent(&mut self, effects: &mut Vec<Effect>) {
        self.silent_sent = self.silent_tracks();
        effects.push(Effect::Engine(EngineOp::SilentTracks {
            tracks: self.silent_sent.clone(),
        }));
    }

    /// Tells the engine when the tracks that play changed (mute, solo, undo, another song).
    pub fn sync_silent(&mut self, effects: &mut Vec<Effect>) {
        if self.silent_tracks() != self.silent_sent {
            self.send_silent(effects);
        }
    }
}

#[cfg(test)]
pub(super) mod tests;
