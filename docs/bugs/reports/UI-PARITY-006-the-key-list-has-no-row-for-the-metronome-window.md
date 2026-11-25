---
id: UI-PARITY-006
title: The key list has no row for the metronome window keys that spec 4 lists
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-006: The key list has no row for the metronome window keys that spec 4 lists

## Where

ui/src/keys/keyList.ts:97-118 (`OTHER_ROWS`: no row for the metronome window)
ui/src/components/MetronomeWindow.svelte:136-146 (← → move the volume), 181-195 and 323 (`tapKey`: Space / Enter tap the tempo)
docs/spec/4-keys.md:82 (the row 「In the metronome window: move the volume by 1 dB / tap the tempo on 「タップ」」)

## What happens

The key list dialog (「ヘルプ ▸ キーの一覧」) has rows for every other key of spec 4 that lives outside keys.ts (the mixer's fader / pan / send keys, the parameter keys, the tempo box, the note list …), but none for the keys of the metronome window: ← → move the volume by 1 dB, Space / Enter on 「タップ」 tap the tempo. A person who searches the list for 「メトロノーム」 or 「タップ」 finds only 「メトロノーム」 (C).

## What should happen

docs/spec/4-keys.md:10 and spec 3 G1: "A key added to the app must also be added to the key list dialog (`ui/src/keys/keyList.ts`)". Its siblings (the fader keys `keyList.fader`, the parameter keys `keyList.paramMove`, the tempo box `keyList.tempoBox`) are listed.

## How to trigger

Read from the code, not run. Open 「ヘルプ ▸ キーの一覧」 and type 「タップ」 or 「音量」 in the search box: no row.

## Cause

The 33 rows of `OTHER_ROWS` (7-readings G1, approved user:2026-11-24) were made without the metronome window's row of spec 4 (row 82).

## Suggested fix

Add one row in the 「再生と録音」 group (or 「表示」) to `OTHER_ROWS`, keys `← →` / `Space Enter`, with a label. This needs new wording (ja and en) decided by the lead or the owner (for example in the form of the other rows, 「メトロノームの窓：音量を動かす ／ タップ」); none exists in ui/src/text yet.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
