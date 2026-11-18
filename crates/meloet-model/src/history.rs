//! Undo and redo. Each step keeps the whole `Doc` of the other side; untouched tracks,
//! clips and takes are shared, and so are the unchanged chunks of notes in a changed take, so a
//! step costs little more than what it changed.
//!
//! An id once given out is never given again: undo and redo bring back the `Doc` of the other
//! side, but never its lower `next_id`. The screen keeps takes by id, and an intent aimed at a
//! take that is gone must not find a different take with its id.

use crate::TrackId;
use crate::arrange::ClipEdge;
use crate::edit::Edit;
use crate::song::{ClipId, Doc};
use crate::time::{Meter, Tempo};

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
    Tempo {
        from: Tempo,
        to: Tempo,
    },
    Meter {
        from: Meter,
        to: Meter,
    },
    /// M; `muted`: the track is muted after the step.
    Mute {
        track: TrackId,
        muted: bool,
    },
    /// The write tool. `clip_made`: the clip was made for the note.
    WriteNote {
        clip: ClipId,
        clip_made: bool,
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
    /// From the song as last saved to the song brought back from the recovery file, at the
    /// start. The history band calls it "なおすためのファイルから戻す" (mock F-Recover2).
    Recovered,
    /// From a song brought back from the recovery file to the song as last saved. The history
    /// band calls it "戻した変更をすてる（最後に保存した形にする）" (spec 6).
    DiscardRecovered,
}

#[derive(Debug, Clone)]
struct Step {
    kind: StepKind,
    /// On the undo list the `Doc` before the step, on the redo list the one after.
    doc: Doc,
    version: u64,
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

    /// The number of the `Doc` as it is now. Undo and redo bring back the number it had.
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn mark_saved(&mut self) {
        self.saved = self.version;
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.saved != self.version
    }
}

#[cfg(test)]
mod tests;
