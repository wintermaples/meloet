# Later: FIX-fix-model-a (met by the fixer of MODEL-A and CORE-FILES in the sweep of 2026-11-25)

- crates/meloet-model/src/record.rs:208 — a key that comes late for an ended pass keeps its tick, so it lies near the end of the pass going on; when the person then stops earlier in that pass, `drop_after` drops it as a note said ahead of the stop. After MODEL-A-004 this also happens when nothing of the next pass came before the close (before, such a key became a take of its own).
