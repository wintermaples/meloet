---
id: UI-SHELL-004
title: Arrow keys in a menu bar list also move the selected notes in the piano roll
area: UI-SHELL
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-SHELL-004: Arrow keys in a menu bar list also move the selected notes in the piano roll

## Where

ui/src/components/menu/MenuList.svelte:90-118 (`onkeydown` ends with `ev.stopPropagation()`, not `stopImmediatePropagation()`)
ui/src/components/menu/MenuBar.svelte:31-34, 103 (the list is moved to `body`) and ui/src/components/menu/ContextMenu.svelte (same)
ui/src/keys/attach.ts:452-583 (`handleKeydown` on `document` does not look at `ev.defaultPrevented` or at whether the focus is in a menu)
ui/src/keys/keys.ts:402-403 (↑ ↓ ← → become `nudge`)

## What happens

The menu bar list and the context menu are moved to `body`. Svelte 5.57.3 hears delegated `keydown` for such nodes with its listener on `document` (node_modules/svelte/src/internal/client/render.js:230-250), which was added at mount, before `attachKeys` added its own listener on `document`. `stopPropagation()` in MenuList does not stop another listener on the same node, so every ↑ ↓ ← → and Enter used to move in the list also runs `handleKeydown`:
- With the piano roll open, pressed last, and notes selected, each ↑ ↓ in the list moves the selected notes a semitone and each ← → moves them a grid step (a `moveNotes` step each, plus the audition of the moved key).
- With the shelf having the keys, ↑ ↓ change the audition; with the output chooser shown, they move its highlight.
- Enter on a row while the sound settings are open also sends `settingsApply`.

## What should happen

Keys used inside an open list belong to the list (spec 2 "List": ↑ ↓ move between rows, Enter acts, ← → open or close child lists). They must not edit the song. MetronomeWindow.svelte:130-134 shows the way that works for a node in `body`: `stopImmediatePropagation()`.

## How to trigger

Read from the code, not run.
1. Open the piano roll (E), click in it, select some notes.
2. Press F10 (or click 「ファイル」) and press ↓ three times: the focus walks down the rows, and the selected notes go down three semitones (three undo steps).
3. Press → to step to the next menu: the notes also move one grid step right.
The test in ui/tests/menu.test.ts:581-630 does not see it because the piano roll is closed there (a nudge then has no command).

## Cause

Order of listeners on `document`: Svelte's delegated handler (calls MenuList's `onkeydown`, which only calls `stopPropagation`) runs first, then `handleKeydown` from keys/attach.ts runs anyway, because both listeners are on the same node. `handleKeydown` does not check `ev.defaultPrevented`. docs/progress.md lists the same mechanism for `ChoiceMenu` and the zoom lists as "not checked whether it does harm"; this is the harm, in the menu bar and context menu lists.

## Suggested fix

In MenuList's `onkeydown`, call `ev.stopImmediatePropagation()` (as MetronomeWindow does) for the keys it takes. Or make `handleKeydown` return early when `ev.defaultPrevented` is true (this would also cover ChoiceMenu and the zoom lists). No protocol, wording or looks change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
