# FIX-fix-ui-roll — UI-ROLL の報告を直している間に見つけた点(直していない)

- ui/src/components/pianoroll/Roll.svelte:121-130 — 拍子が変わる曲で「1 小節」のグリッドを使うと、ドラムの四角は表示ごとに一つの幅（表示の左端の小節の幅）になります。小節ごとに四角を置くには、paint.ts と geom.ts のノートごとの `DrumGeom.hitW` が必要です。
