---
id: UI-PARITY-002
title: A dragged marker, tempo point or time signature flag jumps back to its old place for a moment after release
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-002: A dragged marker, tempo point or time signature flag jumps back to its old place for a moment after release

## Where

ui/src/components/timeline/MarkerRow.svelte:73-79 (`up`: `drag = null` before `moveMarker` is answered)
ui/src/components/timeline/TempoBand.svelte:116-121 (`laneUp`: `pointDrag = null`, then `moveTempoPoint`)
ui/src/components/timeline/TempoBand.svelte:183-188 (`rowUp`: `flagDrag = null`, then `moveMeterChange`)
Siblings that keep the dragged look until the answer: ui/src/components/automation/AutoLane.svelte:68-79, 153-164 (`pending` until `current.rev >= rev`), ui/src/components/Arrange.svelte:1436-1439 (`clipPending`), ui/src/components/pianoroll/Roll.svelte:375-386 and CcLanes.svelte:160-172 (`pending`), ui/src/components/comp/CompBand.svelte:46-54, 116-118, ui/src/components/arrange/AudioClipHandles.svelte:50-61, 243.

## What happens

On release of a marker, a tempo point or a time signature flag, the drag state is cleared at once and the intent is sent. Until core's answer (the new timeline with its rev) comes back, the part is drawn from the old timeline, so the marker / point / flag is drawn at its original place for that time and then jumps to the new place. Every other drag part keeps the dragged look until the answer arrives.

## What should happen

The sibling parts do this on purpose; the comment at AutoLane.svelte:68 says: "The look kept after release and sending the intent, until the answer arrives (so the point does not jump back to the original place for a moment)". The tempo / marker parts are the same kind of drag (CLAUDE.md: only the screen moves first, one intent on release) and should not flash back either.

## How to trigger

Read from the code, not run. Drag a marker two beats to the right and release. Between the release and the next state from core (one or more frames, longer when core is busy, for example while a song with plugins plays), the tag is drawn at the old tick. The same for a tempo point and a time signature flag.

## Cause

`up`, `laneUp` and `rowUp` set the drag to null before sending and keep nothing; the shown positions (`tags` in MarkerRow, `T.draggedPoints(points, pointDrag)` and `flagsShown` in TempoBand) then fall back to `timeline` as it was before the move.

## Suggested fix

Keep the released drag as `pending` and use it for the shown positions until `current.rev` reaches the rev that `send(...)` resolves with (the same pattern as AutoLane.svelte:153-164 and its `$effect` at 74-77); clear it on a new press and when the marker / point / flag is gone. No protocol or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
