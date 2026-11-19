//! Changes to the tracks themselves: delete, rename, move, color. Each is one step, and none
//! happens while counting in or recording.

use meloet_model::edit;
use meloet_model::mixer::{self, Missing};
use meloet_model::{BuiltinInstrument, InstrumentRef, TrackColor, TrackId};
use meloet_protocol::{PluginSlotRef, StatusMessage};

use super::Play;
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
    /// leaves its slot empty. The notes stay. One step. Returns whether the song changed.
    pub fn remove_plugin(&mut self, slot: PluginSlotRef, effects: &mut Vec<Effect>) -> bool {
        let before = self.changes;
        match slot {
            PluginSlotRef::Instrument { track } => {
                let sine = InstrumentRef::Builtin(BuiltinInstrument::Sine);
                if !self.busy()
                    && let Ok(e) = edit::set_instrument(self.doc(), TrackId(track), sine)
                {
                    self.apply(e, effects);
                }
            }
            PluginSlotRef::Insert { insert } => self.remove_effect(insert, effects),
        }
        self.changes != before
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
