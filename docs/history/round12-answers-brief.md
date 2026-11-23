# Round 12, after the features: the owner's answers (2026-11-24)

The owner answered all the open questions of `docs/spec/open-questions.md` (as of commit 378e5ed) with a form. Source tag for everything decided here: `user:2026-11-24`.

**Every question not listed below was answered "as it is now"** (for the listening questions B-98-10, 19, 21.1, 23.1, 24: "heard, good as it is"; B-98-11: the sounds are good and a chosen sound is not played at once; B-98-28 and B-98-32: all the developers' wording and the English translation are approved; B-98-34: all three measured edges are accepted; B-95-他2: the folder stays `sounds`).

## Answers that change something

| Question | Answer | Who builds it |
|---|---|---|
| B-98-1 mp3 | Ship it as it sounds now (the pink-noise condition is dropped) | helper "mp3" |
| B-98-2 AppImage | Decide later (stays open) | — |
| B-98-6.2 arpeggio in small windows | "Add a way", but which way is not given yet: stays open, the lead asks | — |
| B-98-6.3 arpeggio on the mixer strip | A row on every strip (empty while off); everything below moves down | helper "small" |
| B-98-16.1 M5 where the fade ends | The fade ends at the last clip's end (the same place as the export's 「曲ぜんぶ」) | helper "fade" |
| B-98-16.2 M5 fade again | A new fade starts from the fader's value (the value before the old fade), so it can be shortened | helper "fade" |
| B-98-22 P3 out-of-range values | The status line says it, like the other boxes (`ValueField::Percent` / `Amount` exist) | helper "small" |
| B-98-27 A3 the 64-track notice | Every way of adding a track says it | helper "small" |
| B-98-29 A9 file cannot be read / written | A message; the wording is left to the developers (`clipWorkFailed {count}` exists) | helper "small" |
| B-98-33 the damage tool | `tools/measure/webkit-damage.sh` stops only at paints | helper "small" |
| B-98-35.2 English mixer instrument button | Shorten the English word so the sound's name shows | helper "small" |
| B-97-1 batch row | A new history row 「まとめて変える（N 個）」 | helper "core" |
| B-97-2 mute set step names | 「ミュートのセットを保存する」, 「ミュートのセットを呼び出す」, 「ミュートのセットを消す」 (and 「ミュートのセットの名前を変える」) | helper "core" |
| B-97-3 mute sets on the screen | In the menu at the head of the mixer band | helper "core" (shapes), then a screen helper |
| B-97-5 rename a mute set | An AI tool and a way on the screen | same |
| B-96-1 「このままだと音が割れます」 | The spec is changed to what the screen does | helper "docs" |
| B-96-2 empty arrange hint, third sentence | "Fix the sentence", but the new sentence is not given yet: stays open, the lead asks | — |
| B-96-3 key 47 of the built-in kits | 「ミッドタム 2」, as in the GM table | helper "small" |
| B-95-他4 pan and send value text | 15 times a second stays; the spec is changed | helper "docs" |

## Rules for these helpers

`docs/history/round12-helper-brief.md` applies (start steps, hard rules, checks, report), with these differences:

- There is no `docs/round12/` notes folder. Edit the real documents directly, small and local: the rule in `docs/spec/` (3-bands "Rules added in round 12" or the place the rule lives), `docs/architecture.md` (the section of the feature), `docs/spec/7-readings.md` for decisions and made-up wording, and **remove your own answered items from `docs/spec/open-questions.md`** (the file's head says: put the answer into the spec, remove the item). Do not touch other helpers' items, `docs/progress.md`, `README*`, `CLAUDE.md` unless your request says so.
- Wording: Japanese in `ui/src/text/ja.ts` (the source), and the same key in `ui/src/text/en.ts` (English, words of `docs/spec/glossary.md`, sentence case). `ui/tests/text.test.ts` checks the two tables.
- Only the helper "core" may change the protocol (to version 34). The others treat a missing shape as the brief says (report it).
- Several helpers work at the same time on the same base. Shared files (`ja.ts`, `en.ts`, `fake.ts`, `state.rs`, `open-questions.md`, `7-readings.md`, `3-bands.md`, `architecture.md`): add only your own lines, do not reorder or reformat.
- No background wait loops; no commit; leave no process running.
