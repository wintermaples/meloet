//! Mute and solo: which tracks the song plays. Mute is part of the song (a step); solo is only
//! for listening, kept in the session.

use meloet_model::edit;
use meloet_model::{AutoTarget, TrackId};

use super::{EngineOp, Play};
use crate::state::Effect;

impl Play {
    /// Also while counting in or recording: it does not move where the take is recorded.
    /// A lane of the mute takes the press as the mixer's controls do (docs/architecture.md
    /// 21-11): with 「● 書く」 on while the song plays, a point where it was pressed; else,
    /// when it moves the mute, the mute and its rest as one step.
    pub fn toggle_mute(&mut self, track: u32, effects: &mut Vec<Effect>) {
        let track = TrackId(track);
        if self.mute_pressed(track, effects) {
            return;
        }
        let edit = edit::toggle_mute(self.doc(), track);
        let key = (track, AutoTarget::Mute);
        if self.lane_at(key).is_some_and(|l| l.moves()) {
            self.rest_with(key, edit, effects);
        } else if let Ok(e) = edit {
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

    /// The seats of the tracks that do not play, in order of seat. A lane of the mute that
    /// moves it takes the place of the mute put by hand; solo stays.
    fn silent_tracks(&self) -> Vec<usize> {
        let doc = self.doc();
        let soloed = &self.song.session.soloed;
        let any_solo = doc.tracks.iter().any(|t| soloed.contains(&t.id));
        let mut seats: Vec<usize> = doc
            .tracks
            .iter()
            .filter(|t| {
                let laned = t.lane(AutoTarget::Mute).is_some_and(|l| l.moves());
                (t.muted && !laned) || (any_solo && !soloed.contains(&t.id))
            })
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
