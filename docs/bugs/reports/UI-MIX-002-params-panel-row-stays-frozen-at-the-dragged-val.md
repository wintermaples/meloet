---
id: UI-MIX-002
title: Params panel row stays frozen at the dragged value after a drag that writes automation
area: UI-MIX
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-MIX-002: Params panel row stays frozen at the dragged value after a drag that writes automation

## Where

ui/src/components/params/ParamRow.svelte:60-72 (`held`, `shown = held ?? auto ?? value`, and the effect that clears `held` only when `value` changes)
ui/src/components/params/ParamRow.svelte:121-132 (onup keeps `held = v` after `commit: true`)

## What happens

On a parameter row of a built-in effect (BuiltinList) or a sound knob (SoundKnobList) whose automation lane is armed with 「● 書く」 while playing, a drag writes the lane: core answers the release with `LetGo::Written` (crates/meloet-core/src/play/automation.rs:756-762) and changes only the lane points, not the parameter's own value in the song. So the `value` prop of the row does not change, the effect that clears `held` never runs, and `held` keeps the last dragged value. Because `held` comes before `auto` in `shown`, the bar, the handle and the value text stay frozen at that value while the lane goes on moving the sound. It stays so until the song value of that parameter changes for some other reason.

## What should happen

Architecture 21-8: what a band moves, the screen shows at the value on the line (`auto`, as the header comment of ParamRow.svelte says: "While a band moves it, the value is the one on the line"). The mixer does this right: mixControl drops its drag value (`live`) at release, and PluginParamRow clears `held` whenever new values come (`tick`).

## How to trigger

Read from the code, not run. 1. Put a built-in reverb on a track insert and select it. 2. Press the row's automation mark to add a lane for 「ミックス」, arm it for writing (「● 書く」) and start playback. 3. Drag the 「ミックス」 bar from 30 % to 80 % and let go. 4. The lane now holds the written movement and plays it back, but the row keeps showing 80 % and the handle does not move.

## Cause

`held` is cleared only by `$effect(() => { void value; ... held = null })`. A release that writes automation is not a change of `value` (Svelte does not re-run the effect for the same number), so nothing ever clears it.

## Suggested fix

Clear `held` at the end of a drag when the answer cannot be the song value, or simply also when `auto` changes: e.g. make the effect read `auto` as well (`void value; void auto;`), or set `held = null` in onup when `auto !== null`. No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
