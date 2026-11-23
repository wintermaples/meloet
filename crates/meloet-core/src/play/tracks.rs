//! Changes to the tracks themselves: delete, duplicate, rename, move, color. Each is one
//! step, and none happens while counting in or recording.

use meloet_model::edit;
use meloet_model::mixer::{self, Missing};
use meloet_model::{PluginChange, PluginPlace, StripId, TrackColor, TrackId};
use meloet_protocol::{PluginSlotRef, StatusMessage};

use super::plugs::CopyStates;
use super::{MAX_TRACKS, Play};
use crate::state::Effect;

impl Play {
    /// When the selected track goes, the one below it is selected (the one above when it was
    /// the last). Returns the notice to show.
    pub fn delete_track(&mut self, track: u32, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let id = TrackId(track);
        let index = self.track_index(id)?;
        let said = self.track_ref(id)?;
        let e = edit::delete_track(self.doc(), id).ok()?;
        if self.song.session.play_target == Some(id) {
            let left = &e.doc.tracks;
            self.song.session.play_target = left.get(index).or_else(|| left.last()).map(|t| t.id);
        }
        self.apply(e, effects);
        Some(StatusMessage::TrackDeleted { track: said })
    }

    /// "外す" on a plugin that stopped: an instrument leaves its track to the sine, an effect
    /// leaves its slot empty. The notes stay. One step. Returns what the status line says of
    /// an instrument taken out (spec 3, 89-5).
    pub fn remove_plugin(
        &mut self,
        slot: PluginSlotRef,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        match slot {
            PluginSlotRef::Instrument { track } => {
                if self.busy() {
                    return None;
                }
                let e = edit::remove_instrument(self.doc(), TrackId(track)).ok()?;
                self.apply(e, effects);
                let track = self.track_ref(TrackId(track))?;
                Some(StatusMessage::InstrumentPluginRemoved { track })
            }
            PluginSlotRef::Insert { insert } => {
                self.remove_effect(insert, effects);
                None
            }
        }
    }

    /// 「曲から外す」 on something this PC does not have: its tracks get the sine wave and
    /// its insert slots are emptied, as one step. Returns whether the song changed.
    pub fn remove_missing(&mut self, missing: Missing, effects: &mut Vec<Effect>) -> bool {
        if self.busy() {
            return false;
        }
        match mixer::remove_missing(self.doc(), missing) {
            Ok(e) => {
                self.apply(e, effects);
                true
            }
            Err(_) => false,
        }
    }

    /// 「トラックを複製」 (round 12, A3): a copy of the track right below it, selected. Its
    /// plugins start from the states the track's have now: when it has plugins that sound,
    /// the copy is made once they said them (`plugs/copies.rs`). Returns the notice to show.
    pub fn duplicate_track(
        &mut self,
        track: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || self.copy_waits() {
            return None;
        }
        let id = TrackId(track);
        self.doc().track(id)?;
        if self.doc().tracks.len() >= MAX_TRACKS {
            return Some(StatusMessage::TooManyTracks);
        }
        if self.ask_copy_states(id, effects) {
            return None;
        }
        self.make_copy(id, Vec::new(), effects)
    }

    /// The step of 「トラックを複製」, as the song is now. `states`: what the track's plugins
    /// said. The copy's folded lanes are those of the track.
    pub(super) fn make_copy(
        &mut self,
        track: TrackId,
        states: CopyStates,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        self.doc().track(track)?;
        if self.doc().tracks.len() >= MAX_TRACKS {
            return Some(StatusMessage::TooManyTracks);
        }
        let mut made = edit::duplicate_track(self.doc(), track).ok()?;
        for (place, state) in states {
            let at = match place {
                PluginPlace::Instrument(_) => PluginPlace::Instrument(made.track),
                PluginPlace::Insert(_, slot) => {
                    PluginPlace::Insert(StripId::Track(made.track), slot)
                }
            };
            let doc = &made.edit.doc;
            let Some((key, state)) = doc.plugin(at).map(|p| p.key.clone()).zip(state) else {
                continue;
            };
            if let Ok(e) = mixer::set_plugin_state(doc, at, &key, state, PluginChange::Window) {
                made.edit.doc = e.doc;
            }
        }
        let folded = made.folded(&self.song.session.folded_lanes);
        let before = self.song.session.play_target.replace(made.track);
        self.apply(made.edit, effects);
        self.song.session.folded_lanes.extend(folded);
        self.selected_track_changed(before);
        None
    }

    pub fn rename_track(&mut self, track: u32, name: &str, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::rename_track(self.doc(), TrackId(track), name) {
            self.apply(e, effects);
        }
    }

    /// `to`: the place among the tracks as they are afterwards, from 0.
    pub fn move_track(&mut self, track: u32, to: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::move_track(self.doc(), TrackId(track), to as usize) {
            self.apply(e, effects);
        }
    }

    pub fn set_track_color(&mut self, track: u32, color: u8, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let Some(color) = TrackColor::new(color) else {
            return;
        };
        if let Ok(e) = edit::set_track_color(self.doc(), TrackId(track), color) {
            self.apply(e, effects);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod duplicate_tests;
#[cfg(test)]
mod full_tests;
