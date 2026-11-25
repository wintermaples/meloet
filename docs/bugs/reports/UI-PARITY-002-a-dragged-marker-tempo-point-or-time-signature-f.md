---
id: UI-PARITY-002
title: A dragged marker, tempo point or time signature flag jumps back to its old place for a moment after release
area: UI-PARITY
kind: parity
severity: low
status: fixed
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

Confirmed by reading (not run; the length of the jump was not measured). `MarkerRow.svelte:73-79` `up`, `TempoBand.svelte:116-121` `laneUp` and 183-188 `rowUp` set the drag to null and then send; the shown places come from `timeline` as it was until core's answer arrives, so the thing is drawn at its old place for those frames. The other drag parts keep the look until the answer (`AutoLane` `pending`, `Arrange` `clipPending`, `Roll` / `CcLanes` `pending`, `CompBand`).

Fix plan: the same kept look in `ui/src/components/timeline/MarkerRow.svelte` and `TempoBand.svelte`: keep the released drag until `current.rev` reaches the rev `send()` resolves with; drop it on a new press, on Esc, and when the marker / point / flag it names is gone. Write it so it cannot fail the way UI-ARRANGE-001 does: the kept look must be tied to the thing by its name (marker id; tempo point tick; flag bar), and used only while that thing is still in `timeline`, never by an index. Note Rust snaps the sent tick (architecture 21-15) and reselects the moved point; with UI-ARRANGE-006 fixed the kept place equals the answered one.

## Fix

Not committed yet (the lead commits).

- `ui/src/components/timeline/MarkerRow.svelte`, `ui/src/components/timeline/TempoBand.svelte`: the released drag is kept (`pending`, `pointPending`, `flagPending`) until `current.rev` reaches the rev `send()` resolves with, the same pattern as `AutoLane` / `CompBand`. It is tied to the thing by its name and used only while that thing is still as it was in `timeline`: the marker id at its old tick, the tempo point's tick with its old tempo, the flag's bar. A new press drops it; Esc does not (as in the siblings: the intent is already sent).

Tests: `ui/tests/round6.timeline.test.ts` — "after release a marker keeps its moved place until core's answer; the answer or a rev with no change ends it", "after release a tempo point and a time signature flag keep their moved place until core's answer".
