---
id: MODEL-A-003
title: Overdub into an empty place drops the pedal, bend and modulation played before the first note's gap
area: MODEL-A
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# MODEL-A-003: Overdub into an empty place drops the pedal, bend and modulation played before the first note's gap

## Where

crates/meloet-model/src/record/overdub.rs:58-65 (no clip there: the take goes through `in_the_gap`)
crates/meloet-model/src/record/overdub.rs:252-287 (`in_the_gap`: `cut = low - take.origin`, then `.filter(|p| p.tick >= cut && …)` on the controls)

## What happens

With 「重ねる」 on, when no clip of the track holds a recorded note's start, `overdub_recording` cuts the take to the empty place around the notes: from `low`, the end of the last clip that ends at or before the first note (or the take's origin), to the next clip. Every control point before `cut` is thrown away. That includes the points the recorder writes at tick 0 of a take for a pedal, bend or modulation still held when recording (or a loop pass) started (`HeldControls`, `Lap::head`), and a pedal pressed after the start but before `low`. The new clip then has no pedal down (or no bend, no modulation) under its first notes, although the person held it and heard it while playing. Played back, the notes sound dry until the next pedal point. The points are gone from the song, not only trimmed off.

## What should happen

A recording keeps what was played. Where a clip does not hold the start of the controls, the model's own rule is to carry the last pedal, bend and modulation before the clip's start to the start: `track_events` does this for a trimmed head ("The last pedal, bend and modulation before the start, when not at rest, are put at the start", crates/meloet-model/src/playback.rs:30-34, architecture 13 "How clips play"), and the recorder writes a held pedal at the head of each pass (architecture 13 "Recording in a loop"). Without 「重ねる」, `add_recording` keeps every control point of the take.

## How to trigger

Read from the code, not run.
1. A keys track has clip A in bar 1 (any notes). Turn 「重ねる」 on.
2. Put the play line at bar 1, hold the sustain pedal down, press R.
3. Play notes only from bar 2 on (after A's end), keep the pedal down, stop.
4. The new clip starts at A's end; its take has no `CC 64 = 127` point, so its notes play without the pedal. The same happens in a loop recording over A and empty bars, where each pass starts with the held pedal at its head.

## Cause

`in_the_gap` moves the take's tick 0 to `low` by `cut` ticks and keeps only the control points in `[cut, cut + length)`. It does not look at the values that are in force at `cut`, so the head points (tick 0) and any pedal, bend or modulation change before `cut` vanish.

## Suggested fix

In `in_the_gap`, before filtering, find the last sustain (CC 64), modulation (CC 1) and pitch bend value at or before `cut` among the take's controls, and when one is not at rest, put it at tick 0 of the cut take (unless a point of the same controller is already at `cut`). This is the rule of `track_events` and of the recorder's pass heads. No protocol or file-shape change. A test next to `with_no_clip_there_a_new_clip_fills_only_the_empty_place` with `controls = [pedal(0, 127)]`.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-model --lib verify_model_a_003`): a clip in bars 1-2, then `overdub_recording` of a take from tick 0 with `CC 64 = 127` at tick 0 and one note in bar 3. The new clip (bars 3-4) has no control point, and the old clip gets none either: the pedal is gone from the song. Severity `medium` stands (pedal held from the start of a recording is the normal way to play; the way around is to write the point by hand).

Corrections: none.

Fix plan: in `crates/meloet-model/src/record/overdub.rs` `in_the_gap`, before the filter, take the last CC 64, CC 1 and pitch bend value at a tick `< cut` (the take's controls are sorted by tick); for each that is not at rest and has no point of the same controller at exactly `cut`, put it at tick 0 of the cut take. Reuse the rule `playback.rs` `track_events` already has for a trimmed head rather than writing a second one, if it can be shared. Test next to `with_no_clip_there_a_new_clip_fills_only_the_empty_place` in `record/overdub/tests.rs`.

## Fix

`crates/meloet-model/src/record/overdub.rs` `in_the_gap`: the points before the cut go through the recorder's own head rule (`record.rs` `set_head`), and the pedal, bend and modulation not at rest go to tick 0 of the cut take, except a controller that has a point right at the cut. `docs/architecture.md` 24-7 says so.

Test: `record::overdub::tests::with_no_clip_there_the_new_clip_starts_with_what_was_held` (meloet-model).
