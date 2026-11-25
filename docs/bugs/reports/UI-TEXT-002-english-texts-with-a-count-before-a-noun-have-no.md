---
id: UI-TEXT-002
title: English texts with a count before a noun have no singular form (1 beats, 1 steps, 1 pts)
area: UI-TEXT
kind: parity
severity: low
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
