---
id: CORE-PLAY-A-002
title: L with no loop range while playing makes the 4-bar range at the bar where play started, not at the play line
area: CORE-PLAY-A
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# CORE-PLAY-A-002: L with no loop range while playing makes the 4-bar range at the bar where play started, not at the play line

## Where

- crates/meloet-core/src/play.rs:991-996 (`Play::toggle_loop`: `map.meter_at(self.position)`)
- crates/meloet-core/src/play.rs:490-491 (`position`: "while moving, where it started")

## What happens

With no loop range in the song, L (or the keyboard's Cycle / Middle button, `state/buttons.rs` 84) while the song plays makes the range 4 bars from the bar where play was started (or last located), not from the bar the play line is in. Played from bar 1 and pressed at bar 20, the loop becomes bars 1–5. The engine is past that loop's end, so it does not go round (`engine.rs` `SetLoop`: `before_loop_end` is false); the ruler shows a loop far behind the play line, and the next Space jumps back to bar 1 (`play_stop` starts at the loop's head when the position is outside it).

## What should happen

Spec 3 (3-bands.md line 52): "Turning the loop on when there is no range gives 4 bars from the bar of the current position." While playing, the current position is the play line (what the listener hears). Every other command in this file that works "where the play line is" while moving uses `Play::play_line()` (play.rs 961-970): `paste_clips` (1242), `set_tempo` and `set_meter` (timeline.rs 197, 226), `add_marker` (timeline.rs 378), `step_marker`, `open_piano_roll`, `place_pattern`, `bar_step`.

## How to trigger

Read from the code, not run.
1. New song with no loop range; Space at bar 1.
2. When the play line is at bar 20, press L.
3. The loop is bars 1–5 (`EngineOp::Loop { start: 0, end: 4 * bar }`), the song keeps playing past it.
The test `the_loop_turned_on_with_no_range_takes_four_bars_from_the_current_one` (play/tests.rs 323) only checks the stopped case, where `position` is the play line.

## Cause

`toggle_loop` reads `self.position`, which while playing is the tick play started from (or the last `Locate`), not the heard position.

## Suggested fix

Use `self.play_line()` in place of `self.position` at play.rs 994 (`let head = map.meter_at(self.play_line());`). Stopped, `play_line()` is `position`, so the stopped case does not change. No protocol, file or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
