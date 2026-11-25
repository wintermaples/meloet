---
id: UI-ROLL-005
title: CC points and pasted notes snap to an even grid from the song start, off the grid lines after a time signature change
area: UI-ROLL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-005: CC points and pasted notes snap to an even grid from the song start, off the grid lines after a time signature change

## Where

- `ui/src/components/pianoroll/cc.ts:294-297` — adding a CC point: `snapTick(tick, step)`.
- `ui/src/components/pianoroll/cc.ts:408` — moving CC points: `snapTick(target, step)`.
- `ui/src/components/pianoroll/commands.ts:40` — Mod V (and 「貼りつけ」 on a note) at the play line: `snapTick(c.playTick, step)`.
- `ui/src/components/pianoroll/Roll.svelte:1106` — 「貼りつけ」 from the right-click on an empty place: `G.snapTick(G.xToTick(g, p.x), step)`.
- Compare `ui/src/components/pianoroll/Roll.svelte:283` (note drags and the draw tool) and `:1049` (pressing the ruler), which use `gridTick(map, …)`, and `ui/src/components/pianoroll/paint.ts:84-87`, which draws the lines of the note area and the CC lanes from the map (`eachGridTick`, `eachLine`).

## What happens

After a time signature change the grid lines are counted again from the start of each time signature part (and bar by bar for 「1 小節」), and notes snap to those lines. But a CC point that is added or moved, and notes pasted at the play line or at the right-clicked place, are put on multiples of the grid step counted from tick 0. Example: a 3/4 song with 4/4 from bar 2 (tick 5760), grid 1/2 (3840 ticks): the lines are at 3840, 5760, 9600, 13440 …; a CC point drawn at tick 9000 is placed at 7680, between two lines (the notes' snapping gives 9600). With 「1 小節」 the points and pastes go to multiples of the first bar's length, not to bar heads.

## What should happen

Readings A-7: "Paste position from the right-click on an empty place: The nearest grid line (the same snapping as pressing the ruler)"; the ruler uses `gridTick(map, …)`. Architecture 24-2: Mod V puts notes "at the play line (snapped to the grid)". CC points should land on the lines the CC lane draws, as notes land on the lines of the note area.

## How to trigger

Read from the code, not run: a song in 3/4 with a 4/4 change at bar 2; grid 1/2; open 「サステイン」 and draw a point near the second line after bar 2. Or put the play line there and press Mod V with notes copied.

## Cause

These four places call the plain `snapTick(tick, step)` (`geom.ts:212`), which rounds to multiples of `step` from tick 0, instead of `gridTick` (`time/bars.ts`), which counts from the head of the time signature part and knows bar steps. `LaneInput` has no snapper at all (`RollInput.grid` has one).

## Suggested fix

Use `gridTick(mapOf(timeline), tick, step, false, grid.division === 0)` in these places: give `LaneInput` a snapper like `RollInput.grid`, and pass the map to `pianoRollCommand` (it has `state.timeline`) and to the right-click paste. No protocol change.

## Verification

Confirmed by reading (not run). `cc.ts:294-297` and 408, `commands.ts:40` and `Roll.svelte:1106` use the plain `snapTick(tick, step)` (`geom.ts:212`, multiples of `step` from tick 0), while notes and the ruler use `gridTick(map, …)` (`Roll.svelte:283`, 1049) and the lines are drawn from the map. After a time signature change whose head is not a multiple of the grid step (or with the 「1 小節」 grid), the two differ. 7-readings A-7: "Paste position from the right-click on an empty place | The nearest grid line (the same snapping as pressing the ruler)".

One root with UI-ROLL-004 and UI-ROLL-006 (the piano roll uses only the first time signature); fix together.

Fix plan for this report (as suggested): give `LaneInput` (`cc.ts`) a snapper like `RollInput.grid`, pass the map (or the same snapper) to `pianoRollCommand` (`commands.ts`, it has `state.timeline` → `mapOf`) and use `gridTick` at `Roll.svelte:1106`. Test: 3/4 with 4/4 from bar 2, grid 1/2, a CC point at tick 9000 lands on 9600.

## Fix

Fixed together with UI-ROLL-004 and UI-ROLL-006. No protocol change.

- `ui/src/components/pianoroll/cc.ts`: `LaneInput.grid()` (a snapper like `RollInput.grid`); adding and moving points snap with it (`moveAmount` takes it).
- `ui/src/components/pianoroll/CcLanes.svelte`: gives the lane `gridTick(map, …)`.
- `ui/src/components/pianoroll/commands.ts`: Mod V at the play line snaps with `gridTick(mapOf(timeline), …)`.
- `ui/src/components/pianoroll/Roll.svelte`: 「貼りつけ」 from the right-click on an empty place snaps with `gridTick(map, …)`.
- Tests: `ui/tests/pianoroll.meters.test.ts` "CC points and pastes land on the grid lines (UI-ROLL-005)" (3 tests).
