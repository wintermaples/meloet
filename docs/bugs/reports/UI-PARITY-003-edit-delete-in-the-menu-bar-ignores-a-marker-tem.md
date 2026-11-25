---
id: UI-PARITY-003
title: Edit > Delete in the menu bar ignores a marker, tempo change or automation point chosen last, unlike the Delete key it shows
area: UI-PARITY
kind: parity
severity: medium
status: reported
found: 2026-11-25
---

# UI-PARITY-003: Edit > Delete in the menu bar ignores a marker, tempo change or automation point chosen last, unlike the Delete key it shows

## Where

ui/src/components/menu/menus.ts:145 (「消す」 row: `act("menu.delete", { kind: "deleteSelected" }, t("key.delete"), toArrange ? { usable: state.arrange.selectedClips.length > 0 && !busy } : {})`)
ui/src/keys/attach.ts:205-206 (`deleteSelected` → `{ type: "deleteSelected" }`, clips only)
ui/src/keys/attach.ts:476-501 (the Delete key first goes through `laneDeleteCommand` and `timelineKeyCommand`; the menu row's `runKeyAction` never does)
ui/src/components/automation/laneKeys.ts:15-18, ui/src/components/timeline/keys.ts:62-66 (what the key does when `lastPick` is "lane" / "timeline")
crates/meloet-core/src/play.rs:1132-1139 (`delete_selected` deletes only the chosen clips)

## What happens

When the keys go to the arrange and the thing pressed last is a marker, a tempo point, a time signature flag or automation points (`lastPick` "timeline" or "lane"):
- The Delete key deletes that marker / point / flag / those points (`deleteTimeline`, `deleteAutoPoints`).
- 「編集 ▸ 消す」, which shows the key 「Delete」 on its right, is disabled when no clip is chosen. When clips are also still chosen (pressing a marker or a lane does not clear the clip choice), the row is usable and deletes the chosen clips instead of the marker or points.

## What should happen

ui/src/components/menu/menus.ts:1 and attach.ts:2: "Rows with a key go the same way as the key". 7-readings A-21: "Delete works on the thing pressed and chosen last (clips, tempo points and flags, markers, automation points)". The menu row should be usable and act exactly when the Delete key would.

## How to trigger

Read from the code, not run. Choose a clip in the arrange, then press a marker in the marker row (it becomes chosen; the clip stays chosen). Open 「編集」 and press 「消す」: the clip is deleted, the marker stays. Pressing Delete instead deletes the marker. With no clip chosen, the 「消す」 row is disabled although Delete works.

## Cause

The key path asks `laneDeleteCommand` and `timelineKeyCommand` (which look at `lastPick`) before `keyAction`; the menu row calls `runKeyAction({ kind: "deleteSelected" })` directly, and its `usable` looks only at `state.arrange.selectedClips`.

## Suggested fix

Give the menu row the same choice as the key: when the keys go to the arrange, build the command from `laneDeleteCommand` / `timelineKeyCommand` (with a synthetic plain Delete input and the same context) before falling back to `deleteSelected`, and base `usable` on whichever of them applies (a chosen timeline item that is not the start of the song, chosen automation points, or chosen clips). Best placed in `keyCommand` / `keyActionUsable` in attach.ts so the menu and the key share it. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
