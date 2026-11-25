---
id: UI-PARITY-005
title: Notes and CC points can be dragged during count-in and recording and then jump back, while every arrange drag only chooses
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-005: Notes and CC points can be dragged during count-in and recording and then jump back, while every arrange drag only chooses

## Where

ui/src/components/pianoroll/Roll.svelte:346-360 (`down`: no check of count-in or recording before `drag.down` / `drag.downVelocity`)
ui/src/components/pianoroll/drag.ts:191-250 (`RollDrag.down` starts move, resize, velocity, erase and draw drags with no busy check)
ui/src/components/pianoroll/CcLanes.svelte:137-145 and ui/src/components/pianoroll/cc.ts:255-270 (CC points: the same)
ui/src/components/pianoroll/Roll.svelte:133-135 (`liveMode` covers only the clip being recorded, and never the count-in, when `rec` is null)
crates/meloet-core/src/play.rs:2349-2354 (`aimed` returns None while `busy()`, so every note and CC edit is dropped)
Siblings that only choose while busy: ui/src/components/Arrange.svelte:1376-1380 (clips), ui/src/components/automation/AutoLane.svelte (`if (!busy)` before adding; moves refused), ui/src/components/timeline/TempoBand.svelte:99 and 166 (`if (busy) return;` after choosing), ui/src/components/timeline/MarkerRow.svelte:65 (`moved = !busy && …`), ui/src/components/comp/CompBand.svelte:90, ui/src/components/arrange/AudioClipHandles.svelte:174, 268.

## What happens

During the count-in (always) and while recording (when the piano roll shows a clip other than the one being recorded), notes, note ends, velocity bars and CC points can be dragged: they move on the screen, the reading shows, the moved note sounds, the draw tool draws a note frame. On release the intent goes to core, which drops it with no status line (`aimed` is None while busy), and the notes go back to where they were. Every drag part of the arrange (clips, automation points, markers, tempo points, time signature flags, comping, audio clip handles) instead only chooses while counting in or recording and does not start a move.

## What should happen

7-readings A-7 "Right-click during recording and count-in": rows that change the song are disabled (architecture 13); the same idea is kept by every arrange drag ("While recording or during count-in: only chooses (the core accepts only that)", TempoBand.svelte:28 and MarkerRow.svelte:24). CLAUDE.md: the screen is a copy of Rust's state and sends intents only; a move the core never takes should not be shown as made. The piano roll should choose (select) on press and not start a move, resize, velocity, erase or draw drag while counting in or recording.

## How to trigger

Read from the code, not run. Set the count-in to 2 bars, open the piano roll on a clip with notes, press R, and during the count-in drag a note up: it moves and sounds, then on release goes back. Or while recording, choose a clip on another track in the arrange and drag its notes in the piano roll.

## Cause

Neither `RollDrag` nor `LaneMouse` (cc.ts) gets a "busy" input, and Roll.svelte / CcLanes.svelte do not check `transport.state` before starting the drag. Only `liveMode` stops editing, and it is false during the count-in and for other clips.

## Suggested fix

Pass a `busy()` input (count-in or recording) to `RollDrag` and `LaneMouse`; when busy, `down` only sends the selection it would send now and leaves `drag` null (no draw, erase, resize, velocity or move drag), as Arrange.svelte:1376-1380 does. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
