# FIX-fix-ui-lanes — 2 回目で直した点 (2026-11-25)

- ui/src/components/automation/autoGeom.ts:214-223 — `draggedPoints` は、動かした点ごとに値を 0..1 へ個別に収めます。一方、Rust の `auto::move_points`(crates/meloet-model/src/automation.rs:439-445)は、グループ全体の `dValue` を収めます(点同士の間隔は保たれます)。いくつかの点を上や下へ超えて動かすと、ドラッグ中は点が潰れて見え、その後、答えが届くと離れて戻ります。
