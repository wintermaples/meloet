# FIX-fix-ui-roll — seen while fixing the UI-ROLL reports (not fixed)

- ui/src/components/pianoroll/drag.ts:132-155 (`hover`) and ui/src/components/pianoroll/CcLanes.svelte:139-145 (`cursorAt`) — while recording or counting in, the cursor still shows the pencil, the move or the resize shape (and the draw tool still draws its chord shadow) over places where a press now only selects or does nothing.
- ui/src/components/pianoroll/Roll.svelte:121-130 — with the 「1 小節」 grid in a song with time signature changes, the drum squares take one width per view (the bar at the view's left edge); a square per bar would need `DrumGeom.hitW` per note in paint.ts and geom.ts.
