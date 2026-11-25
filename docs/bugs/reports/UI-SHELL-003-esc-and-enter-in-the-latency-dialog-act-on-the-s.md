---
id: UI-SHELL-003
title: Esc and Enter in the latency dialog act on the sound settings dialog under it
area: UI-SHELL
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# UI-SHELL-003: Esc and Enter in the latency dialog act on the sound settings dialog under it

## Where

ui/src/keys/attach.ts:538-540 (plain Enter with `workspace.settings` sends settingsApply)
ui/src/keys/attach.ts:555-564 (Esc sends `back`; nothing looks at `workspace.latencyDialog`)
ui/src/components/LatencyDialog.svelte:49-50 (`cancel` is written for Esc but only the button calls it)
crates/meloet-core/src/state.rs:1702-1710 (`Back` closes the settings dialog, `self.dialog`, and never the latency dialog)
ui/src/App.svelte:98-107 (the latency dialog is shown whenever `latencyDialog` is set, with or without the settings)

## What happens

The latency dialog is opened from 「遅れを測る…」 in the advanced sound settings, so the settings dialog is still open in core under it.
- Esc sends `back`; core closes the hidden sound settings, and the latency dialog stays on screen (measuring goes on if it was measuring). A second Esc folds a band or leaves the arrange; the latency dialog never closes with Esc.
- Enter sends `settingsApply`: the hidden settings dialog is applied and closed (a different draft output is chosen), and the latency dialog stays.

## What should happen

Spec 4: Esc closes a dialog / goes back one, and the dialog in front comes first (spec 3 Esc order). The latency dialog's own comment (LatencyDialog.svelte:49) says Esc is 「やめる」: while measuring it stops measuring (`stopLatencyMeasure`), otherwise it closes the dialog (`closeLatencyMeasure`). Enter should do nothing to the settings under it (the bright button of this dialog, or nothing).

## How to trigger

Read from the code, not run.
1. Mod , → 「くわしい設定」 → 「遅れを測る…」: the latency dialog opens over the settings.
2. Press Esc: the settings close in core; the latency dialog stays open.
   Or press Enter: `settingsApply` is sent; the latency dialog stays open.

## Cause

`handleKeydown` has no branch for `state.workspace.latencyDialog`. Esc falls through to `{ type: "back" }`, whose core handler closes `self.dialog` (the settings) first; plain Enter matches the `state.workspace.settings` branch because the settings are still open under the latency dialog.

## Suggested fix

In attach.ts, before the settings branches: when `state.workspace.latencyDialog` is set, Esc sends `stopLatencyMeasure` while measuring and `closeLatencyMeasure` otherwise (the same as the dialog's `cancel`), and plain Enter does not send `settingsApply` (it can press the dialog's bright button, or do nothing). No protocol, wording or looks change.

## Verification

Confirmed with a test that was run (scratch worktree, not kept): with `workspace.settings` and `workspace.latencyDialog` both set, `handleKeydown` sent exactly `{ type: "back" }` for Esc and `{ type: "settingsApply" }` for Enter. Read: core's `Back` (`crates/meloet-core/src/state.rs:1702-1710`) closes the scan-stopped dialog, the song dialog, then `self.dialog` (the sound settings); it never touches the latency dialog; `App.svelte:98-107` draws the latency dialog in place of the settings while both are set. `LatencyDialog.svelte:49-50` has `cancel` "(Esc)", called only by the button.

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report: in `ui/src/keys/attach.ts`, when `state.workspace.latencyDialog` is set: Esc sends `stopLatencyMeasure` while measuring, else `closeLatencyMeasure` (what `cancel` does; share one function with the dialog); plain Enter sends nothing to the settings (spec 3 gives this dialog no Enter rule, so nothing is the safe choice). No protocol or wording change.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
