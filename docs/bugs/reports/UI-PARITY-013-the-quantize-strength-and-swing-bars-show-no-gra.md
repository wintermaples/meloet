---
id: UI-PARITY-013
title: The quantize strength and swing bars show no gray.7 start mark while dragged, unlike the parameter and metronome bars
area: UI-PARITY
kind: parity
severity: low
status: confirmed
found: 2026-11-25
---

# UI-PARITY-013: The quantize strength and swing bars show no gray.7 start mark while dragged, unlike the parameter and metronome bars

## Where

ui/src/components/pianoroll/PercentSlider.svelte:47-53 (`ondown` does not keep the start value) and :122-124 (the bar draws only the track, the fill and the knob)
Siblings that have it: ui/src/components/params/ParamRow.svelte:62, :107, :228 (`from`, and `{#if dragging}<div class="knob ghost" …>` with a gray.7 border, :359-363) and ui/src/components/MetronomeWindow.svelte:44, :85, :267 (the same, :401-404)

## What happens

While the 「強さ」 or 「スウィング」 bar of quantize in 「選んだノート」 is dragged, only the moving knob is drawn; where the value was before the drag is not shown. The same kind of bar in the params panel and in the metronome window shows a gray.7-framed knob at the start value for the whole drag.

## What should happen

CLAUDE.md rules: "While dragging, only the screen moves first, and on release one intent is sent ... The original position is shown with a gray.7 border." PercentSlider's own header says it is the "smoothly moving value" of spec 2 "Parameter row", and 7-readings line 79 ("Hover and moving of the strength and swing bars") gives it the parameter row's moving look; ParamRow and MetronomeWindow, the other bars of that form, draw the start mark.

## How to trigger

Read from the code, not run.
1. Open the piano roll and 「選んだノート」.
2. Press the 「強さ」 bar of quantize and drag sideways: the knob follows; no gray.7 mark stays at the old value. Compare with dragging a parameter bar of a built-in effect.

## Cause

PercentSlider keeps no `from` value and has no ghost knob element.

## Suggested fix

Keep `from = value` in `ondown` and draw `{#if dragging}<div class="knob ghost" style:left="{from}%"></div>{/if}` with the same `.knob.ghost` style as ParamRow (border `line.hair` gray.7, no fill). Uses existing tokens only; no protocol or wording change.

## Verification

Confirmed by reading (not run). `PercentSlider.svelte` keeps no start value in `ondown` (47-53) and draws only the track, the fill and the knob (no `ghost`, grep); `ParamRow.svelte` and `MetronomeWindow.svelte` draw a gray.7-framed knob at the start value while dragging. CLAUDE.md rule: "While dragging, only the screen moves first … The original position is shown with a gray.7 border." 7-readings line 79 gives these bars the parameter row's hover and moving looks and does not say the start mark is left out (where it is left out on purpose, 7-readings says so, as for insert reordering, line 299).

Fix plan (as suggested): in `ui/src/components/pianoroll/PercentSlider.svelte` keep `from = value` in `ondown` and draw `{#if dragging}<div class="knob ghost" style:left="{from}%"></div>{/if}` with the `.knob.ghost` rule of `ParamRow.svelte` (existing tokens only).

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
