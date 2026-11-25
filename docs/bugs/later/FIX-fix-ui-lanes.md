# fix-ui-lanes — seen while fixing, not fixed

- ui/src/components/automation/autoGeom.ts:214-223 — `draggedPoints` clamps each moved point's value to 0..1 on its own, while Rust's `auto::move_points` (crates/meloet-model/src/automation.rs:439-445) clamps `dValue` for the group (the points keep their distances). Pushing several points past the top or bottom shows them squashed while dragging, then they come back apart in the answer.
- ui/src/components/NumberField.svelte — if a box is still open when the count-in or the recording starts, it stays open and still takes Enter (core refuses the value, and an out-of-range one still sends `clampValue`); the new `disabled` only stops opening it. Not tried whether anything can start the count-in while a box has the focus.
