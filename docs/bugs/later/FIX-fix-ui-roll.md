# FIX-fix-ui-roll — seen while fixing the UI-ROLL reports (not fixed)

- ui/src/components/pianoroll/Roll.svelte:121-130 — with the 「1 小節」 grid in a song with time signature changes, the drum squares take one width per view (the bar at the view's left edge); a square per bar would need `DrumGeom.hitW` per note in paint.ts and geom.ts.
