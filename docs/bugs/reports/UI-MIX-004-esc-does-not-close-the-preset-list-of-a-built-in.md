---
id: UI-MIX-004
title: Esc does not close the preset list of a built-in effect unless focus is inside it
area: UI-MIX
kind: ui-ux
severity: medium
status: reported
found: 2026-11-25
---

# UI-MIX-004: Esc does not close the preset list of a built-in effect unless focus is inside it

## Where

ui/src/components/params/PresetMenu.svelte:363-378 (Esc is read only by `onkeydown` on the `.menu` div)
ui/src/components/params/PresetMenu.svelte:366-369 (the first Esc in the name box removes the box, and focus leaves the menu)
ui/src/components/params/BuiltinList.svelte:419 (the 「プリセット ▾」 button that opens it sits outside the menu)

## What happens

The preset list of a built-in effect closes with Esc only while the keyboard focus is inside the list. Pressing 「プリセット ▾」 leaves the focus on that button (or where it was), which is outside the list, so Esc goes to the app's key handler instead: no drag, no menu, so it sends the general `back` intent and the list stays open. In the name box of 「いまの設定を残す…」, the first Esc puts the row back (`naming = false`), which removes the focused input, so the focus falls to the page and the second Esc also misses the list.

## What should happen

Spec 3 (round 12 M1, 3-bands.md:409): 「Esc puts the row back, Esc again closes the list」; the header of PresetMenu.svelte: "Esc closes the list". Sibling small windows register with `menuOpened` (lib/menu.ts), so the global Esc order closes them wherever the focus is: the arpeggio window (InputMeter.svelte:96), the mute set name box (MuteSetBox.svelte:65).

## How to trigger

Read from the code, not run. 1. Select a built-in reverb in the mixer. 2. Press 「プリセット ▾」 in the params panel. 3. Press Esc: the list stays open (and `back` is sent). Or: 2. open the list, press 「いまの設定を残す…」, press Esc (the row comes back), press Esc again: the list stays open.

## Cause

Esc handling is a keydown handler on the list's own element; the list is not registered in the shared Esc order and does not take the focus when it opens or after the name box goes away.

## Suggested fix

Register the open list with `menuOpened(onclose)` (lib/menu.ts) while it is open, as the arpeggio window does, so Esc closes it through closeMenus first; or focus the list (tabindex=-1) on open and again after the name box closes. No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
