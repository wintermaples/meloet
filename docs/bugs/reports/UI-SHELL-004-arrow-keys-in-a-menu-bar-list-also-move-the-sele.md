---
id: UI-SHELL-004
title: Arrow keys in a menu bar list also move the selected notes in the piano roll
area: UI-SHELL
kind: bug
severity: medium
status: confirmed
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

Confirmed with a test that was run (scratch worktree, not kept): `MenuBar` mounted in a `#app` div as in the real app (its list moves itself to `body`), `handleKeydown` on `document`, the piano roll open with one note selected and pressed last. F10, then ↓: the focus moved to the next row of the list **and** a `moveNotes` intent was sent. The existing test (`ui/tests/menu.test.ts:581-630`) mounts into `body` and has the piano roll closed, so it sees neither.

Why (read in `ui/node_modules/svelte` 5.57.3, `src/internal/client/render.js`): delegated events are heard on the mount target and on `document`; a node outside the mount target is served by the `document` listener, and `stopPropagation()` there cannot stop `handleKeydown`, which is on the same node.

This is more than the known note in `docs/progress.md` ("Lists moved to body … stop key events with stopPropagation, which does not stop the document key handler on the same node (R3 found it; not checked whether it does harm)"): here is the harm, an undo step in the song per arrow key, in the menu bar and context menu lists. The fix should close that note too (`ChoiceMenu`, the zoom lists).

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report: `handleKeydown` returns early when `ev.defaultPrevented` (`MenuList.svelte:117` already calls `preventDefault` for the keys it takes), placed after the `typedKeys` / hint lines if those must still see the key. Check the other `preventDefault` callers in `ui/src` so no key that the document handler must still see is prevented earlier (grep `preventDefault` in keydown handlers). Add the test above to `ui/tests/menu.test.ts` (mount into a child of `body`).

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
