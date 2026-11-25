# fix-ui-lanes — 直す途中で見つけたが、直していない点

- ui/src/components/automation/autoGeom.ts:214-223 — `draggedPoints` は、動かした点ごとに値を 0..1 へ個別に収めます。一方、Rust の `auto::move_points`(crates/meloet-model/src/automation.rs:439-445)は、グループ全体の `dValue` を収めます(点同士の間隔は保たれます)。いくつかの点を上や下へ超えて動かすと、ドラッグ中は点が潰れて見え、その後、答えが届くと離れて戻ります。
- ui/src/components/NumberField.svelte — 入力の枠が開いたまま、カウントインや録音が始まると、枠は開いたままで Enter も受け取ります(コアは値を拒みますが、範囲外の値は `clampValue` を送ります)。今回追加した `disabled` は、開くことだけを止めます。枠にフォーカスがあるときに、カウントインを始められるものがあるかは試していません。
