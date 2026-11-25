---
id: UI-ARRANGE-006
title: Marker and tempo point drags with Shift show an unsnapped place, but Rust always snaps them to the beat
area: UI-ARRANGE
kind: parity
severity: low
status: confirmed
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

Confirmed by reading (not run). The screen leaves out snapping with Shift (`MarkerRow.svelte:68`, `tempoGeom.ts:255`), Rust always snaps (`play/timeline.rs` `move_marker`: `let tick = self.snapped(tick)`; `move_tempo_point` the same; `snapped` = nearest beat).

Which side to follow is decided by the documents (so not `needs-lead`): Rust is right, the screen changes.
- spec 3 (Y17): "A tempo can change at the start of a beat."
- spec 3 (Y18): "Move: drag sideways (snaps to the beat; the reading is the position)."
- architecture 21-15: "**Snapping to beats is done by both**: the screen snaps with the map (`nextBeat`) for the look while dragging, and the core snaps again by the same rule (nearest beat; at exactly half, the later)."
- spec 4's row "While dragging, do not snap to the grid — Shift + drag" speaks of the grid; these two drags do not use the grid but the beat rule above. 7-readings has no line that gives Shift to marker or tempo point drags (A-20), while A-21 gives it to automation points in words ("The start of a beat. With Shift, ticks.").

Fix plan (choice (a)): drop the Shift branch in `ui/src/components/timeline/MarkerRow.svelte:68` and in `dragPoint` of `ui/src/components/timeline/tempoGeom.ts:255` (always `nearestBeatTick`), and fix the comment above `dragPoint` ("ticks with Shift"). Check `ui/tests/` for a test that expects the Shift form (`tempoGeom` tests) and change it. No protocol change.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
