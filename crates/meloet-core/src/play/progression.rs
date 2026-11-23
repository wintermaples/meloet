//! 「並べる」 of 「進行から並べる ▾」 (round 12, P6): notes of different starts written as one
//! step (`Command::AddNotesAt`).

use std::collections::BTreeSet;

use meloet_model::notes;
use meloet_model::{EditError, Note};
use meloet_protocol::{NoteAt, StatusMessage};

use super::Play;
use crate::state::Effect;

impl Play {
    /// As `add_notes`, with a start for each note. Out of the clip, the clip grows by bars as
    /// pasting does; the notes from the first start that would make it overlap the next clip
    /// on are left out. Keys past 127 are left out, and a note given twice is written once.
    pub fn add_notes_at(
        &mut self,
        take: Option<u32>,
        given: &[NoteAt],
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let at = self.write_at(take)?;
        let kept: BTreeSet<(u64, u8, u64, u8)> = (given.iter())
            .filter(|n| n.key <= 127)
            .map(|n| (n.start, n.key, n.length.max(1), n.velocity.clamp(1, 127)))
            .collect();
        let mut notes: Vec<Note> = Vec::with_capacity(kept.len());
        for (start, key, length, velocity) in kept {
            if notes
                .last()
                .is_some_and(|n| (n.start, n.key) == (start, key))
            {
                continue;
            }
            notes.push(Note {
                from: None,
                start,
                length,
                key,
                velocity,
            });
        }
        while let Some(last) = notes.last().map(|n| n.start) {
            if !matches!(
                notes::write_notes(self.doc(), at, &notes),
                Err(EditError::Overlap)
            ) {
                return self.write_notes(at, &notes, effects);
            }
            notes.retain(|n| n.start < last);
        }
        None
    }
}

#[cfg(test)]
mod tests;
