---
id: UI-SHELL-002
title: Keys stay with the shelf while the export dialog or the exported dialog is open
area: UI-SHELL
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# UI-SHELL-002: Keys stay with the shelf while the export dialog or the exported dialog is open

## Where

ui/src/components/tracks.svelte.ts:58-67 (`shelfHasKeys` checks songDialog, settings and the scan-stopped dialog only)
ui/src/keys/attach.ts:469-470 (the "shelf" context comes from `shelfHasKeys`)
ui/src/keys/attach.ts:530-537 (the Enter branches of the export dialog and 「書き出しました」 run only when keyAction returned null)
ui/src/keys/attach.ts:618-619 (a press inside a dialog does not clear `shelfUi.keys`: dialogs are outside `.app`)

## What happens

When the shelf had the keys (a shelf row was clicked) and the export dialog is opened with Mod Shift E, the keys still go to the shelf:
- Enter sends `shelfConfirm` (makes a new track from the sound being auditioned, or puts in the chosen effect) instead of 「書き出す」. With nothing chosen in the shelf, Enter does nothing at all.
- ↑ ↓ change the audition in the shelf behind the dialog; Space plays the shelf sample; F sets a favorite.
The same happens in the 「書き出しました」 dialog: Enter does not close it.
Clicking inside the dialog does not help, because `handlePointerdown` clears `shelfUi.keys` only for presses inside `.app`, and dialogs are outside it.

## What should happen

A dialog in front takes the keys. Spec 4: 「書き出しました」 closes with Esc / Enter; attach.ts:530-533 says Enter in the export dialog is 「書き出す」. The sibling check `historyKeys` (attach.ts:424-435) already leaves out `export.dialog` and `exportedOpen(state)`; `shelfHasKeys` should do the same.

## How to trigger

Read from the code, not run.
1. Open the shelf (P) and click an instrument row: the audition starts and the shelf has the keys.
2. Press Mod Shift E: the export dialog opens (core does not end the audition on OpenExport, crates/meloet-core/src/export.rs:413).
3. Press Enter: `shelfConfirm` is sent and a new track is made behind the dialog; the export does not start.

## Cause

`shelfHasKeys` returns true while `export.dialog` or the exported dialog is open, so `handleKeydown` uses the "shelf" context; `keyAction` maps Enter to `shelfConfirm` (an action, even when its command is null), so the plain-Enter branch for the export dialog is skipped.

## Suggested fix

In `shelfHasKeys` also return false when `state.export.dialog !== null` or `exportedOpen(state) !== null` (as `historyKeys` does). No protocol, wording or looks change. (About and the key list are already listed as known in docs/progress.md, "one-letter keys still work"; they could get the same check.)

## Verification

Confirmed with a test that was run (scratch worktree, not kept): with `shelfUi.keys = true`, the shelf in front and `export.dialog` set, Enter did not send `startExport`; with `shelfUi.keys = false` it did. Read: `shelfHasKeys` (`tracks.svelte.ts:58-67`) leaves out `export.dialog` and the exported dialog, which `historyKeys` (`attach.ts:424-435`) has; `handlePointerdown` clears `shelfUi.keys` only for presses inside `.app`, and dialogs are outside it (`App.svelte`).

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report: in `ui/src/components/tracks.svelte.ts` `shelfHasKeys` also returns false for `state.export.dialog !== null`, `exportedOpen(state) !== null`, the latency dialog, About and the key list (the shared "dialog in front" check). `docs/progress.md` lists "one-letter keys still work" under About and the key list as known; that note is about letters, not about the shelf's Enter and arrows.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
