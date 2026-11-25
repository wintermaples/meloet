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
        let before = self.song.session.play_target.replace(TrackId(track));
        if before != Some(TrackId(track)) {
            self.selected_track_changed(before);
        }
        self.fix_selection();
        true
    }

    /// 「ピアノロールでひらく」 of a keys track's header: the track is selected, and the piano
    /// roll shows its clip under the play line, or no clip. Returns whether it was a keys
    /// track (the piano roll band is then to open).
    pub fn open_piano_roll(&mut self, track: u32) -> bool {
        let id = TrackId(track);
        let Some(t) = self.doc().track(id).filter(|t| !t.kind.is_vocal()) else {
            return false;
        };
        let at = self.play_line();
        let clip = (t.clips.iter())
            .find(|c| c.start <= at && at < c.end)
            .map(|c| c.id);
        if !self.busy() && self.song.session.play_target != Some(id) {
            let before = self.song.session.play_target.replace(id);
            self.selected_track_changed(before);
        }
        match clip {
            Some(c) => {
                self.clips_sel = vec![c];
                self.click(c);
            }
            None if self.busy() => {}
            None => {
                self.clips_sel.clear();
                self.selected = None;
                self.piano_roll = None;
                self.fix_selection();
            }
        }
        true
    }
}
