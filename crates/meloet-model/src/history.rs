//! Undo and redo. Each step keeps the whole `Doc` of the other side; untouched tracks,
//! clips and takes are shared, and so are the unchanged chunks of notes in a changed take, so a
//! step costs little more than what it changed.
//!
//! An id once given out is never given again: undo and redo bring back the `Doc` of the other
//! side, but never its lower `next_id`. The screen keeps takes by id, and an intent aimed at a
//! take that is gone must not find a different take with its id.

use crate::TrackId;
use crate::arrange::ClipEdge;
use crate::audio::AudioId;
use crate::automation::AutoTarget;
use crate::edit::Edit;
use crate::effects::EffectKind;
use crate::mixer::{Bus, PluginPlace, StripId};
use crate::notes::NoteField;
use crate::song::{ClipId, Doc, MarkerId};
use crate::time::{Meter, Tempo, TickRange};

/// What a step did, for the history band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    Record {
        track: TrackId,
        clip: ClipId,
        /// Takes added.
        takes: usize,
    },
    SwitchTake {
        clip: ClipId,
        from: usize,
        to: usize,
    },
    /// Band: "テイクを消す". `index`: the place the take had in the clip.
    DeleteTake {
        clip: ClipId,
        index: usize,
    },
    /// Delete in the arrange view; `cut`: Mod X. Band: "クリップを消す（N 個）",
    /// "クリップを切り取る（N 個）".
    DeleteClips {
        count: usize,
        cut: bool,
    },
    /// The tempo at the song's head.
    Tempo {
        from: Tempo,
        to: Tempo,
    },
    /// The meter at the song's head.
    Meter {
        from: Meter,
        to: Meter,
    },
    /// Band: "テンポ ・ 拍子の変更を足す".
    AddTempoPoint {
        tick: u64,
        tempo: Tempo,
    },
    /// A drag of a tempo point: it is at `to` afterwards and says `tempo`. A point that was
    /// at `to` is gone; the `Doc` before the step has it.
    MoveTempoPoint {
        from: u64,
        to: u64,
        tempo: Tempo,
    },
    /// A value typed on a tempo point, or in the transport while the point is in force.
    SetTempoPoint {
        tick: u64,
        from: Tempo,
        to: Tempo,
    },
    /// "次の点まで、だんだん"; `tick` 0: from the head. `on`: there is a ramp after the step.
    SetTempoRamp {
        tick: u64,
        on: bool,
    },
    DeleteTempoPoint {
        tick: u64,
    },
    /// `bar` counts from 0, as in `MeterChange`.
    AddMeterChange {
        bar: u32,
        meter: Meter,
    },
    /// A flag that was at `to` is gone; the `Doc` before the step has it.
    MoveMeterChange {
        from: u32,
        to: u32,
    },
    SetMeterChange {
        bar: u32,
        from: Meter,
        to: Meter,
    },
    DeleteMeterChange {
        bar: u32,
    },
    /// K, and the name typed into the field it opens. The `Doc` after the step has the
    /// marker, for its name.
    AddMarker {
        marker: MarkerId,
    },
    /// Either `Doc` has the marker: the one before the step the old name.
    RenameMarker {
        marker: MarkerId,
    },
    MoveMarker {
        marker: MarkerId,
    },
    /// The `Doc` before the step has the marker.
    DeleteMarker {
        marker: MarkerId,
    },
    /// A drag on the ruler, L with no range yet, "ここから次のマーカーまでをループに". Band:
    /// "ループの範囲を変える".
    SetLoop {
        range: TickRange,
    },
    /// M; `muted`: the track is muted after the step.
    Mute {
        track: TrackId,
        muted: bool,
    },
    RenameTrack {
        track: TrackId,
    },
    SetTrackColor {
        track: TrackId,
    },
    /// The `Doc` before the step still has the track, for its name.
    DeleteTrack {
        track: TrackId,
    },
    /// A drag of the header, "上へ動かす", "下へ動かす".
    MoveTrack {
        track: TrackId,
    },
    /// The shelf's "トラック N を作る".
    AddTrack {
        track: TrackId,
    },
    /// The shelf's "入れかえる".
    SetInstrument {
        track: TrackId,
    },
    /// 「曲から外す」 on a notice of something this PC does not have: `tracks` tracks sound as
    /// the sine wave from now on, and `inserts` insert slots are empty.
    RemoveMissing {
        tracks: u32,
        inserts: u32,
    },
    /// The write tool and the chord stamp: `count` notes. `clip_made`: the clip was made for
    /// them.
    WriteNote {
        clip: ClipId,
        clip_made: bool,
        count: usize,
    },
    /// A drag, the arrow keys; `copy`: Alt + drag and Mod D.
    MoveNotes {
        clip: ClipId,
        count: usize,
        copy: bool,
    },
    ResizeNotes {
        clip: ClipId,
        count: usize,
    },
    /// The erase tool and Delete; `cut`: Mod X.
    DeleteNotes {
        clip: ClipId,
        count: usize,
        cut: bool,
    },
    ChangeVelocity {
        clip: ClipId,
        count: usize,
    },
    PasteNotes {
        clip: ClipId,
        count: usize,
    },
    /// Q and "かけなおす".
    Quantize {
        clip: ClipId,
        count: usize,
    },
    /// "もとの位置へ戻す".
    Unquantize {
        clip: ClipId,
        count: usize,
    },
    /// The note list and the "選んだノート" panel.
    SetNotesField {
        clip: ClipId,
        count: usize,
        field: NoteField,
    },
    AddControl {
        clip: ClipId,
    },
    MoveControls {
        clip: ClipId,
        count: usize,
    },
    DeleteControls {
        clip: ClipId,
        count: usize,
    },
    /// A drag in the arrange view; `copy`: Alt + drag. Band: "クリップを動かす（N 個）",
    /// "クリップを写す（N 個）".
    MoveClips {
        count: usize,
        copy: bool,
    },
    /// Mod D in the arrange view. Band: "クリップを複製（N 個）".
    DuplicateClips {
        count: usize,
    },
    /// Mod V in the arrange view. Band: "クリップを貼りつける（N 個）".
    PasteClips {
        count: usize,
    },
    /// Band: "クリップのはしを動かす".
    TrimClip {
        clip: ClipId,
        edge: ClipEdge,
    },
    /// A drag of a fader, or a value typed.
    SetGain {
        strip: StripId,
    },
    SetPan {
        strip: StripId,
    },
    SetSend {
        track: TrackId,
        bus: Bus,
    },
    /// A parameter of an effect; `param`: its place in the effect's table.
    SetParam {
        strip: StripId,
        slot: usize,
        kind: EffectKind,
        param: u32,
    },
    /// The shelf's effects, from "空き ＋".
    AddEffect {
        strip: StripId,
        slot: usize,
        kind: EffectKind,
    },
    RemoveEffect {
        strip: StripId,
        slot: usize,
        kind: EffectKind,
    },
    /// A plugin put into an insert slot from the shelf. The `Doc` after the step has it, for
    /// its name.
    AddPlugin {
        place: PluginPlace,
    },
    /// A plugin taken out of an insert slot. The `Doc` before the step has it.
    RemovePlugin {
        place: PluginPlace,
    },
    /// 「とばす」 and 「もどす」; `bypassed`: the slot is passed by after the step. Either `Doc`
    /// has the effect, for its name.
    Bypass {
        strip: StripId,
        slot: usize,
        bypassed: bool,
    },
    /// A drag of a slot inside its strip, 「上へ動かす」, 「下へ動かす」: the slot `from` is at
    /// `to` after the step, and the slots between moved by one.
    MoveInsert {
        strip: StripId,
        from: usize,
        to: usize,
    },
    /// The shelf's 「入れかえる」 for a slot. The `Doc` before the step has the old effect or
    /// plugin, the one after it the new.
    ReplaceEffect {
        strip: StripId,
        slot: usize,
    },
    /// Band: "（プラグイン）の窓で変えた所", at "（トラック）・ プラグインの窓" (spec 3, 15-C).
    PluginWindow {
        place: PluginPlace,
    },
    /// A parameter of a plugin, moved in the parameters panel; `param`: the plugin's own
    /// number for it.
    SetPluginParam {
        place: PluginPlace,
        param: u32,
    },
    /// The M of a return; `muted`: the return is muted after the step.
    MuteReturn {
        bus: Bus,
        muted: bool,
    },
    /// A mute set kept, new or written over: `index` in the `Doc` after the step.
    SaveMuteSet {
        index: usize,
    },
    /// The mutes of several tracks and the returns, as the set at `index` says (either
    /// `Doc`). The solo that came with it is no part of the step.
    RecallMuteSet {
        index: usize,
    },
    /// `index` in the `Doc` before the step.
    DeleteMuteSet {
        index: usize,
    },
    /// 「外す」 on the stopped plugin that was the track's instrument (spec 3, 89-5): the track
    /// sounds as the sine wave. The `Doc` before the step has the plugin.
    RemoveInstrument {
        track: TrackId,
    },
    /// A lane made from the track's header or from a row of the parameters panel.
    AddLane {
        track: TrackId,
        target: AutoTarget,
    },
    /// 「帯を消す」: the lane goes with its points.
    DeleteLane {
        track: TrackId,
        target: AutoTarget,
    },
    /// A point put on the lane by hand.
    AddAutoPoint {
        track: TrackId,
        target: AutoTarget,
    },
    /// One drag of points.
    MoveAutoPoints {
        track: TrackId,
        target: AutoTarget,
        count: usize,
    },
    DeleteAutoPoints {
        track: TrackId,
        target: AutoTarget,
        count: usize,
    },
    /// 「● 書く」: what was written from one press to its release (spec 3, 16-B).
    WriteAutomation {
        track: TrackId,
        target: AutoTarget,
    },
    /// The lane rests without a value being set with it (spec 3, 17-A): the parameter was
    /// moved in the plugin's own window.
    RestLane {
        track: TrackId,
        target: AutoTarget,
    },
    /// 「もどす」.
    RestoreLane {
        track: TrackId,
        target: AutoTarget,
    },
    /// From the song as last saved to the song brought back from the recovery file, at the
    /// start. The history band calls it "なおすためのファイルから戻す" (mock F-Recover2).
    Recovered,
    /// From a song brought back from the recovery file to the song as last saved. The history
    /// band calls it "戻した変更をすてる（最後に保存した形にする）" (spec 6).
    DiscardRecovered,
    /// Another row of 「入口 1 ・ モノ ▾」 of a vocal track. Either `Doc` has the track: the
    /// one before the step the old input.
    SetVocalInput {
        track: TrackId,
    },
    /// S: `count` clips were cut in two at the play line (docs/architecture.md 22-10). The
    /// `Doc` after the step has both halves of each.
    SplitClips {
        count: usize,
    },
    /// Either `Doc` has the clip: the one before the step the old name.
    RenameClip {
        clip: ClipId,
    },
    /// `count` keys clips of one track became `clip`, which kept the id of the earliest.
    JoinClips {
        clip: ClipId,
        count: usize,
    },
    /// An empty keys clip made on an empty place. The `Doc` after the step has it.
    CreateClip {
        clip: ClipId,
    },
    /// Audio files brought in, however many at once: `count` clips were made.
    ImportAudio {
        count: usize,
    },
    /// A drag of the volume line of a vocal clip, a double click on it, or a value typed.
    /// Either `Doc` has the clip: the one before the step the old volume.
    SetClipGain {
        clip: ClipId,
    },
    /// A drag of a fade handle of a vocal clip, or a value typed.
    SetClipFade {
        clip: ClipId,
    },
    /// 「曲のテンポについていく」 of a vocal clip, on or off.
    SetClipFollow {
        clip: ClipId,
    },
    /// 「この音のテンポ」 of a vocal clip, typed or found: the clip shows a new take with the
    /// same audio. The `Doc` before the step has the old take.
    SetSourceTempo {
        clip: ClipId,
    },
    /// The semitones or the cents of a vocal clip, or 「声の感じを保つ」.
    SetClipPitch {
        clip: ClipId,
    },
    /// The volume of `count` selected vocal clips at once: a drag of the volume line of one
    /// of them, or 「音量」 of 「まとめて変える」 (spec 3, 39-B and 40-A).
    SetClipsGain {
        count: usize,
    },
    /// 「曲のテンポについていく」 of 「まとめて変える」, for `count` clips.
    SetClipsFollow {
        count: usize,
    },
    /// The semitones, the cents or 「声の感じを保つ」 of 「まとめて変える」, for `count` clips.
    SetClipsPitch {
        count: usize,
    },
    /// 「曲から外す」 on the notice of an audio file that is not found: `clips` clips that
    /// played it are gone. The `Doc` before the step has the file, for its name.
    RemoveAudio {
        audio: AudioId,
        clips: u32,
    },
}

#[derive(Debug, Clone)]
struct Step {
    kind: StepKind,
    /// On the undo list the `Doc` before the step, on the redo list the one after.
    doc: Doc,
    version: u64,
}

/// A step as the history band lists it.
#[derive(Debug, Clone, Copy)]
pub struct StepRow<'a> {
    pub kind: StepKind,
    /// Names the step for as long as the history holds it as it is: no other step of this
    /// history has had the number, and a step that was changed afterwards
    /// ([`History::amend`]) has a new one.
    pub number: u64,
    /// The `Doc` before the step and the one after it, for the names of what it touched.
    pub before: &'a Doc,
    pub after: &'a Doc,
}

/// A place in the history, kept to make one step of the steps that follow it
/// ([`History::merge`]) or to take them back ([`History::rewind`]).
#[derive(Debug, Clone)]
pub struct Mark {
    /// How many steps were done.
    undo: usize,
    /// The number the `Doc` had.
    version: u64,
    /// The steps that could be redone, which the first step after the mark drops.
    redo: Vec<Step>,
}

impl Mark {
    /// How many steps were done at the mark.
    pub fn undo_count(&self) -> usize {
        self.undo
    }

    /// How many steps could be redone at the mark.
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }
}

#[derive(Debug, Clone)]
pub struct History {
    undo: Vec<Step>,
    /// The next step to redo is last.
    redo: Vec<Step>,
    version: u64,
    next_version: u64,
    saved: u64,
    /// The highest `next_id` of every `Doc` seen.
    next_id: u32,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    /// The `Doc` as it is now counts as saved.
    pub fn new() -> History {
        History {
            undo: Vec::new(),
            redo: Vec::new(),
            version: 0,
            next_version: 1,
            saved: 0,
            next_id: 0,
        }
    }

    /// Puts `edit.doc` in `doc` as a new step. Steps that could be redone are dropped. The new
    /// `Doc` gives out no id given out before, even by a `Doc` it does not come from.
    pub fn apply(&mut self, doc: &mut Doc, edit: Edit) {
        let before = std::mem::replace(doc, edit.doc);
        self.next_id = self.next_id.max(before.next_id).max(doc.next_id);
        doc.next_id = self.next_id;
        self.undo.push(Step {
            kind: edit.step,
            doc: before,
            version: self.version,
        });
        self.redo.clear();
        self.version = self.next_version;
        self.next_version += 1;
    }

    /// Puts `new` in `doc` as what the last step made, in place of what it made: the step
    /// stays one. `false`, and nothing changes, when there is no last step or a step could be
    /// redone.
    pub fn amend(&mut self, doc: &mut Doc, new: Doc) -> bool {
        if self.undo.is_empty() || !self.redo.is_empty() {
            return false;
        }
        self.next_id = self.next_id.max(doc.next_id).max(new.next_id);
        *doc = new;
        doc.next_id = self.next_id;
        self.version = self.next_version;
        self.next_version += 1;
        true
    }

    pub fn mark(&self) -> Mark {
        Mark {
            undo: self.undo.len(),
            version: self.version,
            redo: self.redo.clone(),
        }
    }

    /// The steps made since `mark` are the newest ones done, nothing was undone since the
    /// last of them, and the `Doc` has the number `end`.
    fn since(&self, mark: &Mark, end: u64) -> bool {
        self.version == end
            && self.redo.is_empty()
            && (self.undo.get(mark.undo)).is_some_and(|first| first.version == mark.version)
    }

    /// Makes one step of the steps made since `mark`, named as the last of them: one row,
    /// one undo, one redo. The `Doc` stays as it is and gets a new number, as the step is
    /// another one now. Only when the `Doc` has the number `end` and those steps are the
    /// newest ones done. Returns how many steps they were; 0 or 1, and nothing changes,
    /// when there is nothing to make one of.
    pub fn merge(&mut self, mark: &Mark, end: u64) -> usize {
        if !self.since(mark, end) {
            return 0;
        }
        let steps = self.undo.len() - mark.undo;
        if steps < 2 {
            return steps;
        }
        let kind = self.undo[self.undo.len() - 1].kind;
        self.undo.truncate(mark.undo + 1);
        self.undo[mark.undo].kind = kind;
        if self.saved == self.version {
            self.saved = self.next_version;
        }
        self.version = self.next_version;
        self.next_version += 1;
        steps
    }

    /// Takes back every step made since `mark` and forgets them: the `Doc`, its number and
    /// the steps that can be redone are those of the mark again. Ids given out since are
    /// not given again. Only when the `Doc` has the number `end` and those steps are the
    /// newest ones done (or none was made); else `false`, and nothing changes.
    pub fn rewind(&mut self, doc: &mut Doc, mark: Mark, end: u64) -> bool {
        if self.version != end {
            return false;
        }
        if self.undo.len() == mark.undo && self.version == mark.version {
            return true;
        }
        if !self.since(&mark, end) {
            return false;
        }
        let first = self.undo.drain(mark.undo..).next().expect("a step");
        let other = std::mem::replace(doc, first.doc);
        self.next_id = self.next_id.max(other.next_id).max(doc.next_id);
        doc.next_id = self.next_id;
        self.version = mark.version;
        self.redo = mark.redo;
        true
    }

    /// Takes back the last step and tells which it was.
    pub fn undo(&mut self, doc: &mut Doc) -> Option<StepKind> {
        let step = self.undo.pop()?;
        let kind = step.kind;
        let back = self.swap(doc, step);
        self.redo.push(back);
        Some(kind)
    }

    pub fn redo(&mut self, doc: &mut Doc) -> Option<StepKind> {
        let step = self.redo.pop()?;
        let kind = step.kind;
        let back = self.swap(doc, step);
        self.undo.push(back);
        Some(kind)
    }

    /// Undoes or redoes until `to` steps are done (the row `to` of the history band is the
    /// last one done; more than there are is all of them). Returns how many steps that
    /// were. Each is one swap of what the steps share already, as in [`History::undo`].
    pub fn jump(&mut self, doc: &mut Doc, to: usize) -> usize {
        let to = to.min(self.undo.len() + self.redo.len());
        let steps = self.undo.len().abs_diff(to);
        while self.undo.len() > to {
            self.undo(doc);
        }
        while self.undo.len() < to {
            self.redo(doc);
        }
        steps
    }

    fn swap(&mut self, doc: &mut Doc, step: Step) -> Step {
        let other = std::mem::replace(doc, step.doc);
        self.next_id = self.next_id.max(other.next_id).max(doc.next_id);
        doc.next_id = self.next_id;
        let version = std::mem::replace(&mut self.version, step.version);
        Step {
            kind: step.kind,
            doc: other,
            version,
        }
    }

    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }

    /// Every step, oldest first: the ones that can be undone, then the ones that can be redone.
    pub fn steps(&self) -> impl Iterator<Item = &StepKind> {
        self.undo
            .iter()
            .chain(self.redo.iter().rev())
            .map(|s| &s.kind)
    }

    /// The step at `index` of [`History::steps`]. `doc`: the `Doc` as it is now.
    pub fn row<'a>(&'a self, doc: &'a Doc, index: usize) -> Option<StepRow<'a>> {
        let undone = self.undo.len();
        // The redo list holds, last first, the `Doc` and the version after each step.
        let redo = |i: usize| self.redo.len().checked_sub(1 + i).map(|at| &self.redo[at]);
        if index < undone {
            let after = self.undo.get(index + 1);
            Some(StepRow {
                kind: self.undo[index].kind,
                number: after.map_or(self.version, |s| s.version),
                before: &self.undo[index].doc,
                after: after.map_or(doc, |s| &s.doc),
            })
        } else {
            let i = index - undone;
            let step = redo(i)?;
            Some(StepRow {
                kind: step.kind,
                number: step.version,
                before: match i {
                    0 => doc,
                    _ => &redo(i - 1)?.doc,
                },
                after: &step.doc,
            })
        }
    }

    /// The number of the `Doc` as it is now. Undo and redo bring back the number it had.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.version;
    }

    /// The `Doc` that had `version` is the one that was saved: a save that takes a while
    /// does not count the steps made meanwhile as saved.
    pub fn mark_saved_at(&mut self, version: u64) {
        self.saved = version;
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.saved != self.version
    }
}

#[cfg(test)]
mod tests;
