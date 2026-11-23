//! Clips named, cut at a place, joined, and made empty on an empty place (spec 3, round 10).

use meloet_model::arrange;
use meloet_model::notes;
use meloet_model::{ClipId, TrackId};
use meloet_protocol::StatusMessage;

use super::{ClipPick, Play};
use crate::state::Effect;

impl Play {
    /// Enter in the name field of a clip.
    pub fn rename_clip(&mut self, clip: u32, name: &str, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = arrange::rename_clip(self.doc(), ClipId(clip), name) {
            self.apply(e, effects);
        }
    }

    /// 「ここで切る」: as S for that one clip at `tick`. Returns the notice to show.
    pub fn split_clip_at(
        &mut self,
        clip: u32,
        tick: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        self.split_at(&[ClipId(clip)], tick, effects)
    }

    /// Mod J and 「つなげる」: the joined clip is selected and clicked.
    pub fn join_clips(&mut self, clips: &[u32], effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok((e, joined)) = arrange::join_clips(self.doc(), &Self::clip_ids(clips)) {
            self.apply_selecting(e, None, Some(ClipPick::one(joined)), effects);
        }
    }

    /// A double click on an empty place of a keys track, 「ここにクリップを作る」: the new clip
    /// is selected, its track too, and the piano roll shows it. Returns whether it was made
    /// (the piano roll band is then to open).
    pub fn create_clip(&mut self, track: u32, tick: u64, effects: &mut Vec<Effect>) -> bool {
        if self.busy() {
            return false;
        }
        let Ok((e, clip)) = notes::create_clip(self.doc(), TrackId(track), tick) else {
            return false;
        };
        self.apply_selecting(e, None, Some(ClipPick::one(clip)), effects);
        self.song.session.play_target = Some(TrackId(track));
        self.fix_selection();
        true
    }
}
