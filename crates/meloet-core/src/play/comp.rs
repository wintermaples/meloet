//! 「いいとこ取り」 of a vocal clip (round 12, A8): the two intents of the band. What a comp
//! is and what happens to it when the clip changes is `meloet_model::audio::comp`; how it
//! sounds is `pcm::feed_pieces`.

use meloet_model::{ClipId, CompPiece, TakeId, clear_comp, set_comp};
use meloet_protocol::TickSpan;

use super::Play;
use crate::state::Effect;

impl Play {
    /// The end of a drag over a take's row: the pieces name their takes by id. Nothing for
    /// a clip that is gone, a take the clip does not have, or pieces laid out for other
    /// edges than the clip has now.
    pub fn set_comp(
        &mut self,
        clip: u32,
        span: TickSpan,
        pieces: &[meloet_protocol::CompPiece],
        effects: &mut Vec<Effect>,
    ) {
        if self.busy() {
            return;
        }
        let Some((_, c)) = self.doc().clip(ClipId(clip)) else {
            return;
        };
        let takes = c.takes.vocal();
        let named: Option<Vec<CompPiece>> = (pieces.iter())
            .map(|p| {
                let take = takes.iter().position(|t| t.take.id == TakeId(p.take))?;
                Some(CompPiece { from: p.from, take })
            })
            .collect();
        let Some(named) = named else {
            return;
        };
        if let Ok(e) = set_comp(self.doc(), ClipId(clip), (span.start, span.end), &named) {
            self.apply(e, effects);
        }
    }

    /// 「もとに戻す」 of the band.
    pub fn clear_comp(&mut self, clip: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = clear_comp(self.doc(), ClipId(clip)) {
            self.apply(e, effects);
        }
    }
}

#[cfg(test)]
mod tests;
