---
id: UI-SHELL-002
title: Keys stay with the shelf while the export dialog or the exported dialog is open
area: UI-SHELL
kind: bug
severity: medium
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
