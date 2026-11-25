---
id: UI-SHELL-008
title: Key list lacks the metronome window keys and the Enter of 「書き出しました」 from spec 4
area: UI-SHELL
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-SHELL-008: Key list lacks the metronome window keys and the Enter of 「書き出しました」 from spec 4

## Where

ui/src/keys/keyList.ts:96-118 (`OTHER_ROWS`: keys taken outside keys.ts)
ui/src/components/MetronomeWindow.svelte:136-145, 184-189 (← → move the volume by 1 dB; Space / Enter tap on 「タップ」)
ui/src/keys/attach.ts:534-537 (Enter closes 「書き出しました」)

## What happens

The key list dialog (Help ▸ 「キーの一覧」) has no row for two key rows of spec 4: "In the metronome window: move the volume by 1 dB / tap the tempo on 「タップ」 — ← → / Space Enter" and "Close 「書き出しました」 — Esc / Enter" (Esc is covered by 「ひとつ戻る ／ 閉じる」, Enter is not). Searching the key list for 「タップ」 or 「メトロノーム」 finds only the C key.

## What should happen

docs/spec/4-keys.md, rules above the table: "A key added to the app must also be added to the key list dialog (`ui/src/keys/keyList.ts`; round 12, G1)." Both rows are in the spec 4 table and both are built.

## How to trigger

Read from the code, not run. Help ▸ 「キーの一覧」, type 「タップ」: no row.

## Cause

`OTHER_ROWS` lists the keys of the timeline, mixer, parameters, drags and boxes, but the metronome window (round 12 R3) and E9's Enter were added after it and not put in. docs/progress.md notes that nothing checks the list row by row against spec 4.

## Suggested fix

Add rows to `OTHER_ROWS`: group "play" for the metronome window (← → ／ Space Enter) and group "file" or "play" for closing 「書き出しました」 with Enter. Needs new wording for the labels (`keyList.*`), so a decision of the lead / owner (the existing `keyList.*` labels are `builder:` wording approved by the owner).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
