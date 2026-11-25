---
id: UI-ARRANGE-006
title: Marker and tempo point drags with Shift show an unsnapped place, but Rust always snaps them to the beat
area: UI-ARRANGE
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-006: Marker and tempo point drags with Shift show an unsnapped place, but Rust always snaps them to the beat

## Where

ui/src/components/timeline/MarkerRow.svelte:67-68 (`toTick = e.shiftKey ? Math.round(at) : nearestBeatTick(map, at)`)
ui/src/components/timeline/tempoGeom.ts:252-258 (`dragPoint`: `shift ? Math.round(at) : nearestBeatTick(map, at)`)
crates/meloet-core/src/play/timeline.rs:427-434 (`move_marker`: `let tick = self.snapped(tick)`, always)
crates/meloet-core/src/play/timeline.rs:262-286 (`move_tempo_point`: `self.snapped(to_tick)`, always, when the point moves)

## What happens

While a marker tag or a tempo point is dragged with Shift held, the screen does not snap: the tag or point follows the pointer tick by tick and the reading in place shows that position (for example 「9. 2. 0480」). On release the intent carries that tick, but Rust snaps it to the nearest beat, so the marker or point jumps to the beat the person did not pick. Shift does nothing for these two drags, though the screen says it does.

## What should happen

The screen and Rust should agree. Spec 4 lists "While dragging, do not snap to the grid: Shift + drag — Works" for every drag; spec 3 (Y18, Y17) says marker and tempo point moves snap to the beat. Either Rust takes the tick as sent when the person held Shift, or the screen does not offer Shift for these two drags (always snaps), so what is shown is what is put. The other drags of the arrange agree on both sides (clip moves: Rust uses the sent amount; automation point moves: Rust uses the sent `dTick`).

## How to trigger

Read from the code, not run. Open 「テンポ ・ 拍子の変更」, hold Shift and drag a tempo point sideways a little off a beat; the reading shows the off-beat place; release: the point lands on the nearest beat. The same with a marker tag in the marker row.

## Cause

`move_marker` and `move_tempo_point` call `snapped()` whatever the screen sent, while the screen leaves out snapping with Shift.

## Suggested fix

Needs a decision by the lead. (a) Drop the Shift branch on the screen (always `nearestBeatTick`), matching what Rust does today; no protocol change. Or (b) let these intents carry the "no snap" wish (a protocol field such as `snap: bool` on `MoveMarker` and `MoveTempoPoint`) and skip `snapped()` when it is false (architecture 21-15 says "the core snaps again by the same rule"; that line would change).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
