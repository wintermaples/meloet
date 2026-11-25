---
id: UI-SHELL-008
title: Key list lacks the metronome window keys and the Enter of 「書き出しました」 from spec 4
area: UI-SHELL
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-SHELL-008: Key list lacks the metronome window keys and the Enter of 「書き出しました」 from spec 4

## Where

ui/src/keys/keyList.ts:96-118 (`OTHER_ROWS`: keys taken outside keys.ts)
ui/src/components/MetronomeWindow.svelte:136-145, 184-189 (← → move the volume by 1 dB; Space / Enter tap on 「タップ」)
ui/src/keys/attach.ts:534-537 (Enter closes 「書き出しました」)
docs/spec/4-keys.md:81-82 (the two rows of spec 4; line 82 was also reported as UI-PARITY-006)

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

Real (read, not run): `OTHER_ROWS` (`ui/src/keys/keyList.ts:96-118`) has no row for spec 4 line 82 ("In the metronome window: move the volume by 1 dB / tap the tempo on 「タップ」 — ← → / Space Enter") nor for the Enter of line 81 ("Close 「書き出しました」 — Esc / Enter"); both are built (`MetronomeWindow.svelte:136-145, 184-189`; `attach.ts:534-537`). Spec 4 line 10: "A key added to the app must also be added to the key list dialog". 7-readings G1 "Where the other keys went" leaves out only the output chooser's keys on purpose. `docs/progress.md` knows only that nothing checks the list against spec 4.

Also reported as UI-PARITY-006 (the metronome half; merged into this report and deleted).

`needs-lead` because the two rows need wording that does not exist. I looked for a fitting key in `ui/src/text/ja.ts`: every `keyList.*` label is one made-up phrase (`builder:2026-11-23`, listed in 7-readings G1); `row()` can join existing keys with 「 ／ 」, but 「音量 ／ タップ」 (`metronome.volume`, `metronome.tap`) does not say where, and nothing says 「書き出しました」 is closed.

Recommended wording (in the form of the rows there are):
- metronome row, group 「再生と録音」 (`play`, where C 「メトロノーム」 is), keys `← → ／ Space Enter`:
  ja 「メトロノーム：音量を動かす ／ テンポを手拍子で」, en "Metronome: move the volume / tap the tempo". Words from: `menu.metronome` 「メトロノーム」 / "Metronome"; `metronome.volume` 「音量」 / "Volume"; the verb of `keyList.fader` 「…を動かす」 / "Move …"; `metronome.tapRow` 「テンポを手拍子で」 / "Tap the tempo" (the owner's wording of that row); the 「名前：…」 form of `keyList.shelfSample`.
- exported row, group 「曲とファイル」 (`file`), keys `Esc ／ Enter`:
  ja 「「書き出しました」を閉じる」, en "Close “Exported”". Words from: `exported.title` 「書き出しました」 / "Exported"; 「閉じる」 of `button.close` (spec 6) and `keyList.back`; spec 4 line 81 itself ("Close 「書き出しました」").

Choices: (a) add the two rows with this wording (source `builder:<date>` until the owner agrees, a line in 7-readings G1); (b) only the metronome row, and write in 7-readings G1 that dialog Enter keys are not listed (no other dialog's Enter is in the list; Esc is covered by 「ひとつ戻る ／ 閉じる」); (c) leave both out and say so in 7-readings G1. I recommend (a): spec 4 lists both rows and the rule above its table is plain. Files for the fixer: `ui/src/keys/keyList.ts`, `ui/src/text/ja.ts`, `ui/src/text/en.ts`, `docs/spec/7-readings.md` (G1), the key list test.

## Fix

Choice (a), the lead's decision. `ui/src/keys/keyList.ts` `OTHER_ROWS`: 「曲とファイル」 「「書き出しました」を閉じる」 (Esc ／ Enter) and 「再生と録音」 「メトロノーム：音量を動かす ／ テンポを手拍子で」 (← → ／ Space Enter). `ui/src/text/ja.ts`, `en.ts`: `keyList.metronome`, `keyList.closeExported` (`builder:2026-11-25`, with a comment pointing at spec 7 G1). `docs/spec/7-readings.md` G1: the row "Rows added in the bug sweep" (decided by the lead, 2026-11-25, bug sweep UI-SHELL-008; where each word comes from). Test: `ui/tests/keyList.test.ts` "UI-SHELL-008: the metronome window's keys and Enter of 「書き出しました」 are listed (spec 4)".
