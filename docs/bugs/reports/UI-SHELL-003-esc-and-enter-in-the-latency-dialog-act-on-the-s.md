---
id: UI-SHELL-003
title: Esc and Enter in the latency dialog act on the sound settings dialog under it
area: UI-SHELL
kind: bug
severity: medium
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
