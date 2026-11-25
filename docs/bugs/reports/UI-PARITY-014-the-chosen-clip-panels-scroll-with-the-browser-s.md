---
id: UI-PARITY-014
title: The chosen-clip panels scroll with the browser's own scroll bar, while the other params panel lists use meloet's
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-014: The chosen-clip panels scroll with the browser's own scroll bar, while the other params panel lists use meloet's

## Where

ui/src/components/params/ClipPanel.svelte:276 (`<div class="list">` around the groups) and :466-469 (`.list { min-height: 0; overflow-y: auto; }`, no scroll bar hiding)
ui/src/components/params/ClipsPanel.svelte:117 and :186-189 (the same)
Siblings that do it right: ui/src/components/params/BuiltinList.svelte:129 and params/PluginList.svelte:210 (`<ScrollArea>`; lib/ScrollArea.svelte:120-128 hides the browser's bar and draws spec 2's own)

## What happens

When the chosen audio clip panel (groups 「クリップ」, 「のび縮み ・ 音の高さ」 with 「音程を寄せる」 and its scale row) or the several-clips panel is taller than the right rail (a low window, or a lower band open), it scrolls with WebKitGTK's own scroll bar, in the GTK look and colors and not in the theme. The built-in effect and plugin parameter lists in the same panel scroll with meloet's 8-wide bar (gray.3 ground, gray.6 handle).

## What should happen

Spec 2 "Scroll bar": "The bars of the browser and the OS are not used. The same one is used in lists, dialog content, the output chooser, parameters and the shelf." The parameter lists of the same panel already use `ScrollArea`.

## How to trigger

Read from the code, not run.
1. Make the window 1024 × 640 and open the mixer as a lower band so the right rail is short.
2. Choose one audio clip (with stretch on, so all groups show) with the params panel in front.
3. The groups do not fit; `.list` gets the native vertical scroll bar.

## Cause

ClipPanel and ClipsPanel scroll a plain `div` with `overflow-y: auto` instead of wrapping their groups in `ScrollArea`.

## Suggested fix

Wrap the content of `.list` in `<ScrollArea>` (as BuiltinList does) and drop `overflow-y: auto` from `.list`. No protocol or wording change; the look becomes spec 2's bar.

## Verification

Confirmed by reading (not run; no picture taken). `ClipPanel.svelte:466-469` and `ClipsPanel.svelte:182-185` scroll a plain `div` (`.list { min-height: 0; overflow-y: auto; }`) with nothing hiding the browser's bar (`scrollbar-width: none` / `::-webkit-scrollbar` appear only in `lib/ScrollArea.svelte` and `mixer/MixerStrips.svelte`). Spec 2 line 94, word for word: "Scroll bar | Width 8. Background gray.3, a 1px line.thin line on the left, handle gray.6. Corner 0. The bars of the browser and the OS are not used. The same one is used in lists, dialog content, the output chooser, parameters and the shelf".

Fix plan (as suggested): wrap the groups of `ui/src/components/params/ClipPanel.svelte` and `ClipsPanel.svelte` in `<ScrollArea>` as `BuiltinList.svelte:129` does and drop `overflow-y: auto` from `.list`; check with `tools/design/shoot_ui.py` at a low window that the bar is the 8-wide one and that the rows keep their width (the bar takes 8 px when shown).

## Fix

`ui/src/components/params/ClipPanel.svelte` and `ClipsPanel.svelte`: the groups are inside `<ScrollArea>` (as BuiltinList); the `.list` rule with `overflow-y: auto` is gone. No test (looks only); a shot of `multiClip` (shoot_ui, 1280x800, 100/125) shows the panel as before when it fits; a panel taller than the rail was not shot.
