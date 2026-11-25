---
id: UI-SHELL-001
title: Arrow keys and Enter go to the output chooser even while a dialog is open
area: UI-SHELL
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# UI-SHELL-001: Arrow keys and Enter go to the output chooser even while a dialog is open

## Where

ui/src/keys/attach.ts:467-473 (the context is "outputChooser" whenever `state.workspace.chooser` is set, before any dialog is looked at)
ui/src/keys/keys.ts:376-380 (in that context ↑ ↓ Enter become chooserUp / chooserDown / chooserConfirm)
ui/src/keys/attach.ts:511-541 (the plain-Enter branches for the dialogs run only when keyAction returned null)
crates/meloet-core/src/state.rs:3018-3020 (`chooser_shown` does not look at dialogs, so the chooser stays in the state while a dialog is open)

## What happens

While the output chooser is shown (no output chosen yet and a keyboard is there; flow 2), the person can still open the sound settings (Mod ,), a song dialog (Mod N / Mod O with changes), the export dialog, or the scan-stopped dialog. In any of them:
- Enter sends `chooserConfirm` instead of the dialog's bright button. In the sound settings this plays from the output highlighted in the chooser behind, not the output picked in the dialog, and the dialog stays open. In a song dialog («保存しますか» …) Enter does not answer the dialog at all.
- ↑ ↓ move the highlight of the chooser behind the dialog.

## What should happen

Spec 3 (dialogs about saving): "Enter is the button at the right end". The sound settings: Enter = 「OK」 (settingsApply, attach.ts:538-540). A dialog is in front, so its keys come first (the Esc order of spec 3 puts the dialog before the parts behind it). The sibling contexts already do this: `shelfHasKeys` (ui/src/components/tracks.svelte.ts:58-67) and `historyKeys` (attach.ts:424-435) both return false while songDialog, settings or the scan-stopped dialog is open.

## How to trigger

Read from the code, not run.
1. Start with no output decided and one keyboard (or `meloet-vkeys appear`): the output chooser shows.
2. Press Mod , : the sound settings open. Click an output row other than the one highlighted in the chooser.
3. Press Enter: `chooserConfirm` is sent; the chooser's highlighted output is chosen, the dialog stays, and the draft output is not used.
Also: with the chooser shown, make a change, press Mod N, press Enter: the 「保存しますか」 dialog is not answered.

## Cause

`handleKeydown` picks `context = state.workspace.chooser ? "outputChooser" : …` without checking `songDialog`, `settings`, `export.dialog`, `exportedOpen(state)` or `plugins.stoppedOpen`. `keyAction` then returns a chooser action for ↑ ↓ Enter, so `action` is not null and the plain-Enter dialog branches (attach.ts:511-541) are never reached.

## Suggested fix

Use the "outputChooser" context only when no dialog is open (the same checks as `historyKeys`: songDialog, settings, export.dialog, exportedOpen, plugins.stoppedOpen, and about.open / keyList.open). No protocol, wording or looks change.

## Verification

Confirmed with a test that was run (scratch worktree, not kept): `handleKeydown` with a state that has `workspace.chooser` set and `workspace.settings` set sent `chooserConfirm` for Enter (and the chooser's step for ↓); with `songDialog` set instead, Enter did not send `answerSongDialog`. Read: `attach.ts:467-473` picks the "outputChooser" context from `state.workspace.chooser` alone, and `chooser_shown` in core (`crates/meloet-core/src/state.rs:3018-3020`) does not look at dialogs. Not checked in the real app that every dialog named can be opened while the chooser is shown (the menu rows and Mod , are not disabled by the chooser as far as I read).

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report: in `ui/src/keys/attach.ts` use the "outputChooser" context only when no dialog is in front (songDialog, settings, latencyDialog, export.dialog, `exportedOpen`, `plugins.stoppedOpen`, `about.open`, `keyList.open`); add the case to `ui/tests/keys*.test.ts`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
