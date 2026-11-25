---
id: UI-PARITY-001
title: Automation point, marker and tempo or time signature drags show their reading in place but not on the status line
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-001: Automation point, marker and tempo or time signature drags show their reading in place but not on the status line

## Where

ui/src/components/automation/AutoLane.svelte:72, 100-115 (`reading` is a local state only; `dragReading` is never set)
ui/src/components/timeline/MarkerRow.svelte:43, 61-72 (marker drag: local `reading` only)
ui/src/components/timeline/TempoBand.svelte:75-79 (`readAt`: tempo point and time signature flag drags, local `reading` only)
Siblings that do it right: ui/src/components/pianoroll/Roll.svelte:339-343, ui/src/components/pianoroll/CcLanes.svelte:121-124, ui/src/components/Arrange.svelte:1409 (clip drag), ui/src/components/arrange/AudioClipHandles.svelte:218 all set `dragReading.text`, which StatusLine.svelte:958 shows.

## What happens

While an automation point is moved (or the pointer is held to add one), while a marker is moved, and while a tempo point or a time signature flag is moved, the reading in place appears near the cursor, but the status line keeps its usual text. In the piano roll (notes, velocity, CC points) and in the arrange (clips, audio clip handles, the repeat handle) the same reading is also shown on the status line.

## What should happen

Spec 2, part "Reading in place": "The same value is also shown in the status line". Spec 3 (piano roll rules): "The value being moved … is shown both in the 「その場の読み値」 near the cursor and on the status line." The three timeline / automation parts should do what Roll.svelte, CcLanes.svelte and Arrange.svelte do. 7-readings A-20 ("The reading while a point or flag is dragged: only the position") only says what the reading holds, not that it stays off the status line.

## How to trigger

Read from the code, not run. Open the 「テンポ ・ 拍子の変更」 row of the arrange and drag a tempo point sideways: 「9. 2. 0000」 shows by the cursor, the status line does not change. The same with a marker, a time signature flag, or a point of an automation lane. Compare with dragging a clip, where the status line shows the reading.

## Cause

The three parts keep their own `reading` state for the in-place box and never write `dragReading.text` (ui/src/components/pianoroll/view.svelte.ts:152), which is the only way a drag reading reaches StatusLine.svelte.

## Suggested fix

In each part, set `dragReading.text` to the same text as the in-place reading whenever it changes, and set it back to null on release, on cancel (the `onDragCancel` callback) and on destroy, as Arrange.svelte:1409/1432/1451 does. For MarkerRow the text is `positionText(drag.toTick, map)`. No protocol, wording or look change.

## Verification

Confirmed by reading (not run). `dragReading.text` (`pianoroll/view.svelte.ts:152`) is the only way a drag reading reaches the status line; `AutoLane.svelte`, `TempoBand.svelte` and the marker drag of `MarkerRow.svelte` keep a local `reading` only (grep: `dragReading` appears in `MarkerRow.svelte` only for the 「この前に空きを入れる…」 box, not in the other two). Spec 2 "Reading in place": "Shown only while dragging, 6 away to the upper left of the cursor. At the edge of the screen, moved inward. The same value is also shown in the status line". 7-readings A-20 only says what the reading holds.

Fix plan (as suggested): in `ui/src/components/automation/AutoLane.svelte`, `ui/src/components/timeline/TempoBand.svelte` and `MarkerRow.svelte`, write the same text to `dragReading.text` whenever the local reading changes, and null on release, on cancel and on destroy (as `Arrange.svelte` does for clip drags). The status line then changes while these drags move: run `tools/measure/webkit-damage.sh` once with a point drag (the status line and the arrange are far apart on the page).

## Fix

Not committed yet (the lead commits).

- `ui/src/components/automation/AutoLane.svelte`, `ui/src/components/timeline/TempoBand.svelte`, `ui/src/components/timeline/MarkerRow.svelte`: the text of the reading in place is also written to `dragReading.text` (point moves and the place a press would add a point; tempo points and flags; markers), and set back to null on release, on Esc and when the part goes away. `tools/measure/webkit-damage.sh` was not run (it needs the real app).

Tests: `ui/tests/automation.lane-drag.test.ts` — "moving a point, and holding an empty place, put the in-place reading's text on the status line; release and Esc take it away"; `ui/tests/round6.timeline.test.ts` — "the reading in place is on the status line too; it goes on release and on Esc (spec 2 「その場の読み値」)".
