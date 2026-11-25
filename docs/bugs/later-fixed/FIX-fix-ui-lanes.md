# FIX-fix-ui-lanes — fixed in the second pass (2026-11-25)

- ui/src/components/automation/autoGeom.ts:214-223 — `draggedPoints` clamps each moved point's value to 0..1 on its own, while Rust's `auto::move_points` (crates/meloet-model/src/automation.rs:439-445) clamps `dValue` for the group (the points keep their distances). Pushing several points past the top or bottom shows them squashed while dragging, then they come back apart in the answer.
