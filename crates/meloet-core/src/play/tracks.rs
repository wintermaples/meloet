//! Changes to the tracks themselves: delete, rename, move, color. Each is one step, and none
//! happens while counting in or recording.

use meloet_model::edit;
use meloet_model::{TrackColor, TrackId};
use meloet_protocol::StatusMessage;

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
