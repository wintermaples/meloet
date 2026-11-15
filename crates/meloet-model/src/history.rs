//! Undo and redo. Each step keeps the whole `Doc` of the other side; untouched tracks,
//! clips and takes are shared, so a step costs little more than what it changed.

use crate::TrackId;
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
    DeleteClips {
        count: usize,
    },
    Tempo {
        from: Tempo,
        to: Tempo,
    },
    Meter {
        from: Meter,
        to: Meter,
    },
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
        }
    }

    /// Puts `edit.doc` in `doc` as a new step. Steps that could be redone are dropped.
    pub fn apply(&mut self, doc: &mut Doc, edit: Edit) {
        let before = std::mem::replace(doc, edit.doc);
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
