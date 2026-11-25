---
id: UI-TEXT-002
title: English texts with a count before a noun have no singular form (1 beats, 1 steps, 1 pts)
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-002: English texts with a count before a noun have no singular form (1 beats, 1 steps, 1 pts)

## Where

ui/src/text/en.ts:34 (`msg.countingInRest`; also :33 `msg.countingIn`, not used now)
ui/src/text/en.ts:1140 (`history.head`: "{total} steps" and "The {redo} above are undone")
ui/src/text/en.ts:1141 (`history.headLatest`)
ui/src/text/en.ts:1142 (`history.headAllUndone`)
ui/src/text/en.ts:1101 (`lane.now`: "{count} pts")
ui/src/text/en.ts:47 (`msg.scanDone`: "{stopped} stopped … and were left out")
Callers: ui/src/components/StatusLine.svelte:451, ui/src/components/history/HistoryHead.svelte:14-16, ui/src/components/automation/LaneHead.svelte:119, ui/src/components/StatusLine.svelte:384

## What happens

In English these texts are plain strings, so a count of 1 gives broken English: "Recording starts in 1 beats. Space to stop" (the last beat of every count-in), "1 steps · now at step 1 (the newest)" (the history band heading after the first change), "The 1 above are undone and can be redone", "Now 50 % · 1 pts" (a lane with one point), "1 stopped while being checked and were left out" (a scan where one plugin stopped).

## What should happen

spec 7-readings, D4 "Numbers": "A text whose noun follows a number has a form for 1 and one for any other number (`n(by, one, other)` in en.ts)". 60 other texts of en.ts already do this (for example `history.notes`, `msg.undoPending`, `transport.countInBars`).

## How to trigger

Read from the code, not run. English page: start a recording with a count-in (`countInBeatsLeft` goes down to 1; `crates/meloet-engine/tests/tempo_map.rs:847`); or make one change and open 「取り消しの履歴」; or add one automation point to a lane; or a scan in which one plugin stops.

## Cause

These keys were written as plain strings in en.ts, not as `n(...)` pairs. The test in ui/tests/text.test.ts checks holes, not the singular.

## Suggested fix

Make each an `n(by, one, other)` pair in en.ts: `msg.countingIn`/`msg.countingInRest` by `beats` ("1 beat"), `history.head`/`headLatest`/`headAllUndone` by `total` ("1 step"; `history.head` also needs the `redo` part to read for 1, for example "The {redo} above is undone and can be redone", which needs the pair by `redo` or a rewording that works for both), `lane.now` by `count` ("1 pt"), `msg.scanDone` by `stopped` ("was left out"). English wording only; no Japanese change, no protocol change. `history.head` has two counts in one text, so the lead may want a rewording that needs only one pair.

## Verification

Confirmed by reading (not run). The keys named are plain strings in `ui/src/text/en.ts` (33-34, 47, 1101, 1140-1142), and each can get the count 1 (the last beat of a count-in, the first step, one point, one stopped plugin). 7-readings D4: "Numbers | A text whose noun follows a number has a form for 1 and one for any other number (`n(by, one, other)` in en.ts); Japanese is unchanged." 61 other texts use `n(...)`.

Fix plan (English only, the forms the rule asks for; no Japanese change): in `ui/src/text/en.ts`
- `msg.countingIn`, `msg.countingInRest`: `n("beats", "… in {beats} beat. …", "… in {beats} beats. …")`;
- `history.headLatest`, `history.headAllUndone`: `n("total", "{total} step · …", "{total} steps · …")`;
- `history.head` has two counts and `n` takes one: pair by `total` and word the second half so it reads for any number, for example "… now at step {current}. Undone above: {redo}, can be redone" (the lead or owner may choose other words; the English table is the developers' translation approved by the owner, so tell the owner in the fix's list of wording changes);
- `lane.now`: `n("count", "Now {value} · {count} pt", "Now {value} · {count} pts")`;
- `msg.scanDone`: `n("stopped", "… {stopped} stopped while being checked and was left out. …", "… were left out. …")`.
Add the singular cases to `ui/tests/language.test.ts`. Do UI-TEXT-003 first or together (the same `english()`).

## Fix

`ui/src/text/en.ts`: `msg.countingIn`, `msg.countingInRest` (by `beats`), `history.headLatest`, `history.headAllUndone` (by `total`), `history.head` (by `redo`: `total` is at least 2 there, so only "The 1 above is undone" needed the form for 1; no rewording), `lane.now` (by `count`, "1 pt"), `msg.scanDone` (by `stopped`, "was left out") are now `n(by, one, other)` pairs. Japanese unchanged. English wording changes for the owner's list: "1 beat", "1 step", "The 1 above is undone and can be redone", "1 pt", "1 stopped … and was left out".
Test: `ui/tests/language.test.ts` "English: a count of 1 takes the form for 1, also when signed with 「−」".
