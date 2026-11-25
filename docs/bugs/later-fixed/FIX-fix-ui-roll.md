# FIX-fix-ui-roll — fixed in the second pass (2026-11-25)

- ui/src/components/pianoroll/drag.ts:132-155 (`hover`) and ui/src/components/pianoroll/CcLanes.svelte:139-145 (`cursorAt`) — while recording or counting in, the cursor still shows the pencil, the move or the resize shape (and the draw tool still draws its chord shadow) over places where a press now only selects or does nothing.
