---
id: UI-ARRANGE-007
title: Adding an automation point: the screen snaps to the beat, Rust snaps again to the piano roll's grid and ignores Shift
area: UI-ARRANGE
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-007: Adding an automation point: the screen snaps to the beat, Rust snaps again to the piano roll's grid and ignores Shift

## Where

ui/src/components/automation/autoGeom.ts:348-352 (`addTick`: the start of the beat; the tick as it is with Shift)
ui/src/components/automation/AutoLane.svelte:109-115 (`addReading`: the reading shows that place and the line's value there)
ui/src/components/automation/AutoLane.svelte:172-173 (`addAutoPoint` with that tick)
crates/meloet-core/src/play/automation.rs:587-615 (`add_auto_point` → `on_grid`: snaps again to `session.roll.grid`, the piano roll's grid, always)

## What happens

Pressing an empty place of an automation lane shows the reading 「位置 ・ 値」 for the start of the beat there (or, with Shift, the exact tick) and sends `addAutoPoint` with that tick. Rust then puts the point on the piano roll's grid instead. Whenever that grid is coarser than a beat or does not hold the beat (grid 1 bar, 1/2, 1/4 triplet; or x/8 with a 1/4 grid), the point lands somewhere else than the reading said, with a value taken there; and Shift never adds a point between grid steps. Moving points in the same lane follows beats (`dragPoints`, and Rust uses the sent `dTick` as it is), so adding and moving in one lane use two different grids.

## What should happen

One rule on both sides, so the reading is where the point goes. Spec 3 (Y16): "pressing an empty place in the band adds a point (with the value of the line at the pressed position) … sideways for the position (snaps to the grid)"; spec 4: Shift + drag does not snap. The screen's rule (beats, ticks with Shift) is the one its tests check (`ui/tests/automation.test.ts:112`, `:140`).

## How to trigger

Read from the code, not run. Set the piano roll's grid to 1 bar (「ノート ▸ グリッド ▸」). In the arrange, add a volume lane and press an empty place of it on beat 3 of bar 5: the reading says 「5. 3. 0000 …」, the point appears on a bar line (bar 5 or 6). Hold Shift and press between beats: the point still lands on the grid.

## Cause

`add_auto_point` always runs `on_grid` with the piano roll's grid, while the screen already snapped by its own (beat) rule and offers Shift for "no snap".

## Suggested fix

Needs a decision by the lead on which grid automation points use. The smallest change that keeps the screen's tested rule: make `add_auto_point` take the tick as sent (the screen has snapped it), like `move_auto_points` takes `dTick`; the doc comment on `add_auto_point` changes with it. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
