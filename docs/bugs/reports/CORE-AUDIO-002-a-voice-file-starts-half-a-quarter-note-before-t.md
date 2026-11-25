---
id: CORE-AUDIO-002
title: A voice file starts half a quarter note before the roll, not half a beat as the keys do: in 6/8 and 2/2 the voice and key windows differ
area: CORE-AUDIO
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-AUDIO-002: A voice file starts half a quarter note before the roll, not half a beat as the keys do: in 6/8 and 2/2 the voice and key windows differ

## Where

crates/meloet-core/src/service/input.rs:407-411 (`vocal_mark`: `half_beat_us = 30e6 / map.tempo_at(tick)`)
crates/meloet-engine/src/engine.rs:2712 (the key rule it should match: `t.record_from - t.record_beat / 2`, `record_beat` = `Meter::ticks_per_beat()` at `record_from`, engine.rs:1130-1132)

## What happens

The disk writer is told to start the voice files `half_beat_us` before the moment the song rolls. `tempo_at` is quarter notes per minute (docs/architecture.md 22-2), so `30e6 / tempo` µs is always half a quarter note (960 ticks), whatever the time signature.

The keys keep what was played in the last half beat of the count-in, where a beat is the note of the lower number (`Meter::ticks_per_beat` = 7680 / den): 960 ticks in x/4, but 480 ticks in x/8 and 1920 ticks in x/2.

- In 2/2 (and other x/2) the voice file starts only an eighth note before the roll, while the keys keep a quarter note: a sung pickup that starts between a quarter and an eighth note early is not in the file, so pulling the clip's left edge out shows silence there.
- In 6/8, 3/8, x/16 the voice file keeps twice (four times) as much as the keys.

The comment above the line says "Half a beat before the song rolls, as the keys keep."

## What should happen

docs/architecture.md 22-6: "the file is written from the head of the last half beat of the count-in (same length as the key rule of 12)", and 22-7 / 22-16 say the same ("from the count-in's last half beat"). The key rule is half of `Meter::ticks_per_beat()` (engine.rs:2712).

## How to trigger

Read from the code, not run.

1. Set the time signature to 2/2 at tempo 120 (a quarter note is 0.5 s).
2. Arm a vocal track, record with the count-in, and start singing 0.4 s before the first bar (between an eighth and a quarter note early).
3. Stop, pull the clip's left edge out to before the start: the first part of the pickup is not there (the file begins 0.25 s before the roll). A key played at the same moment is kept.

## Cause

The half beat is computed from the tempo alone. When tempo became quarter notes per minute in every time signature (22-2), this line kept meaning "half a quarter note" and was not tied to the meter at the record start, as the engine's `record_beat` is.

## Suggested fix

Take the half beat from the meter at the roll tick, as the engine does: `let beat = map.meter_at(tick.max(0.0) as u64).meter.ticks_per_beat() as f64;` then `half_beat_us = beat / 2.0 / TICKS_PER_QUARTER as f64 * 60e6 / map.tempo_at(tick).max(1.0)`. No protocol, file or wording change.

## Verification

Confirmed by reading (no test run). `vocal_mark` (`service/input.rs:407-411`): `half_beat_us = 30e6 / map.tempo_at(tick)`; the tempo is quarter notes a minute whatever the meter, so this is always half a quarter note. The keys' rule in the engine is `record_from - record_beat / 2` with `record_beat = Meter::ticks_per_beat()` = `TICKS_PER_QUARTER * 4 / den` (`engine.rs:1130-1132`, `2712`). Architecture 22-6: "the same length as the key rule". In x/2 the voice file starts half as early as the keys are kept, in x/8 twice as early. Severity `low` stands (only meters whose lower number is not 4, and only the part before the clip's start).

Corrections: none.

Fix plan: `crates/meloet-core/src/service/input.rs` `vocal_mark`: take the beat from the meter at the roll tick: `let beat = map.meter_at(tick.max(0.0) as u64).meter.ticks_per_beat() as f64; let half_beat_us = beat / 2.0 / TICKS_PER_QUARTER as f64 * 60e6 / map.tempo_at(tick).max(1.0);`. A unit test of the small calculation (pull it into a function): 4/4, 2/2 and 6/8 at one tempo.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
