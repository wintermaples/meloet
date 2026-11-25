---
id: UI-ROLL-003
title: A note or CC drag sends the take id shown at release with the indexes picked at the press
area: UI-ROLL
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-ROLL-003: A note or CC drag sends the take id shown at release with the indexes picked at the press

## Where

- `ui/src/components/pianoroll/drag.ts:191-255` — `down` picks `notes`, `grab` and `hits` as indexes of the take shown at the press; the drag does not keep that take's id.
- `ui/src/components/pianoroll/drag.ts:375-381` — `up` reads `this.takeId()` (the take shown at the release) and passes it to `finish`.
- `ui/src/components/pianoroll/drag.ts:452-473` — `finish` puts that id together with the press-time indexes into `moveNotes`, `resizeNotes`, `deleteNotes`, `changeVelocity`, `selectNotes`.
- `ui/src/components/pianoroll/cc.ts:352-357` — `LaneMouse.up` does the same for `moveControls`, `deleteControls`, `selectControls`.
- `ui/src/components/pianoroll/Roll.svelte:280` and `CcLanes.svelte:85` — `shown()` returns the take shown now; nothing ends or marks a drag when it changes.

## What happens

If the shown take changes while the mouse button is held, the release sends the new take's id with indexes taken from the old take. Core accepts it, because the id is the take shown now, and edits whatever notes have those indexes in the new take: other notes are moved, resized, given another velocity, or erased, and the wrong notes are selected. While the button is held the take can change by keys (the key handler does not wait for a drag: Mod Z, Mod Shift Z, Delete, ↑ ↓ ← →, Q, T), by an edit from the AI link, or when the new take of the edit just before (a drag let go a moment ago, a ↑ ↓) arrives after the press.

## What should happen

Architecture 16, "Note order and how notes are pointed to": "An intent points by take id and index in that take. If the id is not the shown take, Rust does nothing (so an old index does not edit another note, e.g. when a key is pressed right after letting go of a drag)." This guard works only when the id sent is the id of the take the indexes were read from.

## How to trigger

Read from the code, not run.
1. In a clip with notes A, B, C (indexes 0, 1, 2), draw a new note X before A (one undo step; the take is now X, A, B, C, and B is index 2).
2. Press B and start dragging it to the right; keep the button down.
3. Press Mod Z: the take goes back to A, B, C (the earlier id), where index 2 is C.
4. Let go: `moveNotes { take: <earlier id>, notes: [2] }` is sent and C moves instead of B. With the erase tool the same steps delete C.

## Cause

`RollDrag.up()` and `LaneMouse.up()` take the id from `input.shown()` at the release, not from the press. The indexes in the drag belong to the take of the press.

## Suggested fix

Keep the take id in the drag at the press (a `take` field in `Drag` and `LaneDrag`) and use that id in `finish` / `finishLane`; core then does nothing for a drag whose take is gone. Optionally cancel the drag (as Esc does) when `n.take.id` changes during it. No protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
