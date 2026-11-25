---
id: UI-SHELL-005
title: Key list and About open over the export dialog, and Enter in them starts the export
area: UI-SHELL
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# UI-SHELL-005: Key list and About open over the export dialog, and Enter in them starts the export

## Where

ui/src/components/menu/menus.ts:452-458 (`noDialog` checks songDialog, settings and the scan-stopped dialog, not `export.dialog` nor 「書き出しました」)
ui/src/keys/attach.ts:511-541 (plain Enter with `state.export.dialog` sends `startExport`, also while the key list or About is in front)

## What happens

With the export dialog (or 「書き出しました」) open, 「ヘルプ ▸ キーの一覧」 and 「meloet について」 stay usable, and the dialog opens on top of the export dialog. Then plain Enter (for example in the key list's search box, or on the 「閉じる」 button) does not reach the key list: `handleKeydown` sees `state.export.dialog` and sends `startExport`, so the export starts behind the key list. In 「書き出しました」 Enter sends `closeExport`.

## What should happen

docs/spec/7-readings.md, "Who holds 「meloet について」": while another dialog is open, the way in is disabled. The comment at menus.ts:452 says the same ("Not opened while another dialog … is open (so the Esc order stays single)"). The export dialog and 「書き出しました」 are dialogs too (App.svelte:108-118).

## How to trigger

Read from the code, not run.
1. Press Mod Shift E: the export dialog opens.
2. Click 「ヘルプ」 ▸ 「キーの一覧」: the key list opens over it.
3. Type in its search box and press Enter: `startExport` is sent and the export starts.

## Cause

`noDialog` in `topMenus` was written before the export dialog and 「書き出しました」 existed and does not list them; the plain-Enter handling in attach.ts does not know the screen-only dialogs (About, key list) are in front.

## Suggested fix

Add `state.export.dialog === null && exportedOpen(state) === null` to `noDialog` in menus.ts (exportedOpen is in ui/src/components/export/exported.ts). No protocol, wording or looks change.

## Verification

Confirmed by reading (not run). `menus.ts:306`: `noDialog = !songDialog && !settings && !stoppedOpen`; the export dialog and 「書き出しました」 are not in it, so 「キーの一覧」 and 「meloet について」 stay usable over them. `attach.ts` then answers a plain Enter with `startExport` / `closeExport` whatever is in front (the Enter branches do not look at `about.open` / `keyList.open`). 7-readings: "Who holds 「meloet について」 | … While another dialog is open, the menu bar is disabled".

One root with UI-SHELL-001, -002, -003, -004, -005, UI-PARITY-011, UI-PARITY-015 and UI-MIX-004: a key that belongs to the part in front (a dialog, an open list, a focused control) falls through to the document key handler (`handleKeydown` in `ui/src/keys/attach.ts`), which does not know that part is in front. Fix them as one piece of work with one rule in `handleKeydown`: (1) one "which dialog is in front" check used by every context (`outputChooser`, `shelf`, `history`) and by the Enter / Esc branches, in the order the dialogs are drawn in `App.svelte`; (2) return early when `ev.defaultPrevented` (a part that took the key says so), which covers lists moved to `body`; then each part only has to take its own keys.

Fix plan for this report: in `ui/src/components/menu/menus.ts` add `state.export.dialog === null && exportedOpen(state) === null` (and the latency dialog) to `noDialog`; and in `attach.ts` the plain-Enter branches do nothing while About or the key list is open (the shared "dialog in front" check gives this for free).

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
