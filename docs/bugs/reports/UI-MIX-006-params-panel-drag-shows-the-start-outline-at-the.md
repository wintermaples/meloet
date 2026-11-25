---
id: UI-MIX-006
title: Params panel drag shows the start outline at the song value, not where an automation lane had the handle
area: UI-MIX
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-006: Params panel drag shows the start outline at the song value, not where an automation lane had the handle

## Where

ui/src/components/params/ParamRow.svelte:103-113 (`from = value` in ondown)
ui/src/components/params/ParamRow.svelte:228 (the gray.7 start outline is drawn at `fractionOf(spec, from)`)

## What happens

When an automation lane moves a built-in effect parameter or a sound knob (`auto` is set, so the handle is at the line's value), pressing the bar takes `from` from `value`, the parameter's own value in the song, not from the value shown. The gray.7 outline of the start position is then drawn where the handle never was (the song value under the lane), not where the handle was when the drag began. The release test `v === from && !previewed` also compares with that other value.

## What should happen

CLAUDE.md: "While dragging, only the screen moves first ... The original position is shown with a gray.7 border." The original position is where the handle was. The mixer does it right: mixControl takes `from = opts.value()`, which MixerStrips feeds with the lane's shown value (`shownGain`, `shownPan`, `shownSends`). PluginParamRow takes it from the shown row value.

## How to trigger

Read from the code, not run. Add a lane for the reverb's 「ミックス」 with a line from 0 % to 100 %, play so the handle moves along it (the song value is, say, 30 %), and press the bar while the handle is at 80 %: the start outline appears at 30 %.

## Cause

ondown reads the `value` prop instead of `shown` (`held ?? auto ?? value`).

## Suggested fix

Take `from = shown` in ondown (before `held` is set). No protocol change, no new wording or looks.

## Verification

Confirmed by reading (not run). `ParamRow.svelte` `ondown`: `from = value` (the song value) while the handle is drawn at `shown = held ?? auto ?? value`; the gray.7 outline is drawn at `fractionOf(spec, from)`. With a lane moving the parameter, `auto !== value`, so the outline is not where the handle was. `mixControl` takes `from = opts.value()`, which the strips feed with the shown (lane) value.

Fix plan (as suggested): `from = shown` in `ondown`, read before `held` is set, in `ui/src/components/params/ParamRow.svelte`; the release test `v === from && !previewed` then compares with the shown value too, which is the right "nothing moved" test.

## Fix

`ui/src/components/params/ParamRow.svelte` `ondown`: `from = shown` (the handle's place, the lane's value when a lane moves it). Test: `ui/tests/fix-ui-values.test.ts` "the start frame is where the handle was (the lane's value), not the song value".
