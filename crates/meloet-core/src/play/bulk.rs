//! 「割合」 「だんだん」 「ばらつき」 and 「次の音までのばす」 of the 「選んだノート」 panel
//! (round 12, P3). Each is one step on the picked notes of the shown take, as
//! `match_velocity` is.

use std::collections::BTreeSet;

use meloet_model::notes::{self, NotesEdited};
use meloet_model::{ClipId, Doc, EditError};
use meloet_protocol::StatusMessage;

use super::{Play, indexes};
use crate::state::Effect;

impl Play {
    pub fn scale_velocity(
        &mut self,
        take: u32,
        picked: &[u32],
        percent: u16,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let percent = percent.clamp(*notes::SCALE_PERCENT.start(), *notes::SCALE_PERCENT.end());
        self.bulk_notes(take, picked, effects, |doc, clip, p| {
            notes::scale_velocity(doc, clip, p, percent)
        })
        .map(|count| StatusMessage::VelocityScaled { count, percent })
    }

    pub fn ramp_velocity(
        &mut self,
        take: u32,
        picked: &[u32],
        from: u8,
        to: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let (from, to) = (from.clamp(1, 127), to.clamp(1, 127));
        self.bulk_notes(take, picked, effects, |doc, clip, p| {
            notes::ramp_velocity(doc, clip, p, from, to)
        })
        .map(|count| StatusMessage::VelocityRamped { count, from, to })
    }

    pub fn random_velocity(
        &mut self,
        take: u32,
        picked: &[u32],
        amount: u8,
        seed: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let amount = amount.clamp(*notes::RANDOM_AMOUNT.start(), *notes::RANDOM_AMOUNT.end());
        self.bulk_notes(take, picked, effects, |doc, clip, p| {
            notes::random_velocity(doc, clip, p, amount, seed)
        })
        .map(|count| StatusMessage::VelocityRandomized { count, amount })
    }

    pub fn legato(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        self.bulk_notes(take, picked, effects, notes::legato)
            .map(|count| StatusMessage::NotesLegato { count })
    }

    /// Applies `edit` to the picked notes. The count of notes picked, also when nothing
    /// changed (said as `match_velocity` says it); `None` when the command does nothing.
    fn bulk_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
        edit: impl FnOnce(&Doc, ClipId, &[usize]) -> Result<NotesEdited, EditError>,
    ) -> Option<u32> {
        let clip = self.aimed(take)?;
        let picked = indexes(picked);
        let r = edit(self.doc(), clip, &picked);
        let count = picked.iter().collect::<BTreeSet<_>>().len();
        if count == 0 || !matches!(r, Ok(_) | Err(EditError::Unchanged)) {
            return None;
        }
        self.apply_notes(r, effects);
        Some(count as u32)
    }
}

#[cfg(test)]
mod tests;
