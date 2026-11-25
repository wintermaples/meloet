---
id: UI-MIX-004
title: Esc does not close the preset list of a built-in effect unless focus is inside it
area: UI-MIX
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# UI-MIX-004: Esc does not close the preset list of a built-in effect unless focus is inside it

## Where

ui/src/components/params/PresetMenu.svelte:58-68, 72 (Esc is read only by `onkeydown` on the `.menu` div)
ui/src/components/params/PresetMenu.svelte:62 (the first Esc in the name box removes the box, and focus leaves the menu)
ui/src/components/params/BuiltinList.svelte:132 (the list is opened here; the 「プリセット ▾」 button that opens it sits outside the menu)

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

Confirmed by reading (not run). Lines corrected: Esc is read only by `onkeydown` on the `.menu` div (`PresetMenu.svelte:58-68`, 72); the list is opened from `BuiltinList.svelte:132` by a button outside it; nothing focuses the list on open, and `naming = false` (62) removes the focused name box. The list is not registered with `menuOpened` (`lib/menu.ts`), so `closeMenus()` in `handleKeydown` does not know it and Esc goes on to `back` (which folds a band or leaves the place the person was in). Spec 3 line 409: "Esc puts the row back, Esc again closes the list". Not checked in WebKitGTK whether a press on 「プリセット ▾」 leaves the focus on the button or on the body; in both cases it is outside the list.

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report (as suggested): in `ui/src/components/params/PresetMenu.svelte` register the open list with `menuOpened(onclose)` while it is mounted, as `InputMeter.svelte:96` and `MuteSetBox.svelte:65` do, so the third turn of Esc closes it wherever the focus is; keep "Esc in the name box puts the row back" in the list's own handler, and after `naming = false` focus the list (tabindex −1) so the second Esc is the list's. 

## Fix

`ui/src/components/params/PresetMenu.svelte`: the open list registers with `menuOpened` (`lib/menu.ts`) while mounted, so the menus' turn of Esc (`closeMenus` in `keys/attach.ts`) closes it wherever the focus is. Esc in the name box still puts the row back first (the list's own handler stops it); the next Esc closes the list through `closeMenus`, so the list itself is not focused. Test: `ui/tests/round12.presets.test.ts` "UI-MIX-004: Esc closes the list wherever the focus is; in the name box the first Esc puts the row back".
