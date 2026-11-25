---
id: UI-PARITY-011
title: Left and right arrows do not move the parameter row chosen with up and down; they go on to move the selected notes
area: UI-PARITY
kind: ui-ux
severity: medium
status: reported
found: 2026-11-25
---

# UI-PARITY-011: Left and right arrows do not move the parameter row chosen with up and down; they go on to move the selected notes

## Where

ui/src/components/params/BuiltinList.svelte:75-78 (`pick` focuses the list box `.list`, not the row's control) and :108-123 (`onkeydown` handles only ↑ ↓ Enter; ← → are let through)
ui/src/components/params/SoundKnobList.svelte:43-46 and :48-66 (the same)
ui/src/components/params/PluginList.svelte:116-119 and :156-172 (the same)
ui/src/components/params/ParamRow.svelte:157-165 and params/PluginParamRow.svelte:105-116 (← → are handled only on the row's own slider / steps control, which has the focus only after Tab)
ui/src/components/params/ParamRow.svelte:103-105 (a press on the bar calls `preventDefault`, so pressing the bar does not focus it either)
ui/src/keys/keys.ts:208-209 (with no one else taking them, ← → become the `nudge` action: move the selected notes one grid step)

## What happens

Choose a parameter row by pressing its name, or move the choice with ↑ ↓: the row is chosen (`SelectParam`) and the focus is on the panel's list box. Now ← → do not move the chosen row's value. The list's key handler lets them pass, they reach the document's key handler, and they become "move notes by one grid step": with the piano roll open and notes selected, the notes move (one undoable step each press). Only after pressing Tab until the slider itself has the focus do ← → move the value.

## What should happen

Spec 3 (E3, the knobs of a built-in sound): "← → move the chosen row, ↑ ↓ choose, Enter types". Spec 4: "In the parameters: choose the row above / below ↑ ↓", "move the value / move it finely ← → / Shift ← →", "type a value Enter" are one group; ↑ ↓ and Enter already act on the chosen row from the list box (`enter()` in BuiltinList and PluginList, `paramUi.typing = knobs.row` in SoundKnobList), ← → do not.

## How to trigger

Read from the code, not run.
1. Open the piano roll and select some notes. Put a compressor on the track and choose it so the params panel shows it.
2. Press the name 「しきい値」 in the panel (or press a row, then ↓): `pick` focuses `.list`.
3. Press →: `BuiltinList.onkeydown` returns at line 117 without stopping it; the document handler (`keys/attach.ts` `handleKeydown`, context "main") gets `nudge` and the selected notes move right by one grid step. The threshold does not change.
The same with a plugin's rows and with 「明るさ」 … of a built-in sound.

## Cause

← → are handled by the control element of each row (ParamRow / PluginParamRow), but choosing a row puts the focus on the list around the rows; the list handlers implement only ↑ ↓ and Enter of the group.

## Suggested fix

In the three list `onkeydown`s, take ← → (and Shift ← →) when a row is chosen and send them to that row's step: for example move the focus to the chosen row's control in `pick` (`[data-param="…"] [role=slider]`), or let the list call the same `keyStep` / plugin step the row uses. Stop the key either way so it does not reach the document. No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
