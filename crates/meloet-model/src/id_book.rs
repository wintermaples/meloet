//! Test help: every id given out so far, to check that none is given twice.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::song::{ClipId, Doc, Take, TakeId};

/// The doc with `next_id` left out, for comparing what the songs hold: undo and redo bring the
/// rest back, but never a lower `next_id`.
pub(crate) fn held(doc: &Doc) -> Doc {
    Doc {
        next_id: 0,
        ..doc.clone()
    }
}

fn takes(doc: &Doc) -> impl Iterator<Item = &Arc<Take>> {
    doc.tracks
        .iter()
        .flat_map(|t| &t.clips)
        .flat_map(|c| &c.takes)
        .flat_map(|ct| std::iter::once(&ct.take).chain(ct.take.origin.as_ref().map(|o| &o.take)))
}

#[derive(Default)]
pub(crate) struct IdBook {
    takes: HashMap<TakeId, Arc<Take>>,
    clips: HashSet<ClipId>,
    next_id: u32,
}

impl IdBook {
    pub(crate) fn new(doc: &Doc) -> IdBook {
        let mut book = IdBook::default();
        book.saw(doc, None).expect("a fresh book takes any doc");
        book
    }

    /// Checks `doc`, then keeps its ids. `before`: the doc a new step was made on; a clip not
    /// there must have an id never given out. Without it (undo, redo) clips may come back.
    pub(crate) fn saw(&mut self, doc: &Doc, before: Option<&Doc>) -> Result<(), String> {
        if doc.next_id < self.next_id {
            return Err(format!(
                "next_id went back: {} < {}",
                doc.next_id, self.next_id
            ));
        }
        self.next_id = doc.next_id;
        for t in takes(doc) {
            if t.id.0 >= doc.next_id || self.clips.contains(&ClipId(t.id.0)) {
                return Err(format!("take id {} not given out as a take", t.id.0));
            }
            match self.takes.get(&t.id) {
                Some(had) if had != t => return Err(format!("take id {} given twice", t.id.0)),
                Some(_) => {}
                None => {
                    self.takes.insert(t.id, t.clone());
                }
            }
        }
        for c in doc.tracks.iter().flat_map(|t| &t.clips) {
            let new = before.is_some_and(|b| b.clip(c.id).is_none());
            if c.id.0 >= doc.next_id
                || self.takes.contains_key(&TakeId(c.id.0))
                || (new && self.clips.contains(&c.id))
            {
                return Err(format!("clip id {} given twice", c.id.0));
            }
            self.clips.insert(c.id);
        }
        Ok(())
    }
}
