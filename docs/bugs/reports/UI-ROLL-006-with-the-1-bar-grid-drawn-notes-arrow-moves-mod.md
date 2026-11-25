---
id: UI-ROLL-006
title: With the 1-bar grid, drawn notes, arrow moves, Mod D and drum squares use the first bar's length in bars of another time signature
area: UI-ROLL
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# UI-ROLL-006: With the 1-bar grid, drawn notes, arrow moves, Mod D and drum squares use the first bar's length in bars of another time signature

## Where

- `ui/src/components/pianoroll/Roll.svelte:117` — `step = G.gridTicks(pr.grid, meter)` with `meter` = the time signature at the song's start (`Roll.svelte:75`).
- `ui/src/components/pianoroll/geom.ts:207-210` — for division 0 (「1 小節」) the step is `ticksPerBar(meter)`.
- `ui/src/components/pianoroll/drag.ts:242`, `:251` — a drawn note starts one `step` long; `drag.ts:340` with `geom.ts:392-394` — it never gets shorter than `step` while drawing.
- `ui/src/components/pianoroll/geom.ts:358` — a resized note cannot get shorter than one `step` (or its own length).
- `ui/src/components/pianoroll/commands.ts:33-34`, `:67`, `:74` — ← → move by `step`; Mod D copies by whole `step`s (the same head-meter step).
- `ui/src/components/pianoroll/Roll.svelte:124` — drum squares are `step * ppt` wide.

## What happens

With the grid 「1 小節」 every length and move uses the length of a bar of the first time signature, also in a part of the song with another time signature. Example: 4/4 at the start, 3/4 from bar 5. In bar 6: a note drawn with 「書く」 is 7680 ticks long (a 4/4 bar), so it runs 1920 ticks into bar 7; ← → move notes by 7680 ticks, off the bar lines; Mod D puts the copy 7680 ticks later; drum squares are as wide as a 4/4 bar; a note cannot be resized shorter than 7680 ticks. The start snapping is right there (`gridTick(…, bar)` goes to the bar's head), so only the lengths and distances are wrong. Core's quantize uses the bar at the note (`crates/meloet-model/src/quantize.rs:22-23`, `grid.ticks(meter)` of `meter_stretch(tick)`).

## What should happen

Spec 3: "The draw tool: pressing an empty place makes a note one grid step long"; spec 4: "Move notes by one grid step"; drums: "Its width is one grid cell minus 2". Architecture 16: "division 0 is 1 bar (from the time signature)". With 「1 小節」 one grid step is the bar at that place, as the snapping (`time/bars.ts` `gridTick` with `bar`) and core's quantize already count it.

## How to trigger

Read from the code, not run: a 4/4 song with 3/4 from bar 5; grid 「1 小節」; draw a note at the head of bar 6 and look at its length in the note list (it reads one 4/4 bar, 「4.0000」 in 3/4 beats); select it and press →.

## Cause

`gridTicks` is given only the head's time signature (`Roll.svelte:75`, `commands.ts:33`), so for division 0 the step is always the first bar's length.

## Suggested fix

For division 0 take the step from the bar at the place: the drawn start for draw and resize, the earliest moved note for ← → and Mod D, the view's left edge (or each square's bar) for drum squares, e.g. `ticksPerBar(meterAtOf(map, tick))`. No protocol change.

## Verification

Confirmed by reading (not run). `Roll.svelte:117` `step = G.gridTicks(pr.grid, meter)` with the first time signature (75); `gridTicks` (`geom.ts:207-210`) gives `ticksPerBar(meter)` for division 0, so with 「1 小節」 every length and distance (drawn note, shortest resize, ← →, Mod D in `commands.ts:33`, drum square width) is the first bar's length everywhere. Other grids do not depend on the meter, so only 「1 小節」 in a song with a time signature change is hit (low). Rust's own grid uses the bar at the place (`crates/meloet-core/src/play/automation.rs` `on_grid`, `meloet-model/src/quantize.rs`).

One root with UI-ROLL-004 and UI-ROLL-005; fix together (the map instead of the first meter).

Fix plan for this report: make the step a function of the place for division 0, `ticksPerBar(meterAtOf(map, tick))`: in `ui/src/components/pianoroll/drag.ts` (the drawn start for draw and resize), `commands.ts` (the earliest moved note for ← → and Mod D), and `Roll.svelte:124` (drum squares: the bar of each square, or of the view's left edge if one width per view is needed by the drawing). Check what Rust does with the sent `ticks` (it takes them as sent), so the screen alone decides. Tests with a 4/4 → 3/4 song.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
