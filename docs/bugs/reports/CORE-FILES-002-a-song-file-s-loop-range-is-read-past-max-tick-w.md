---
id: CORE-FILES-002
title: A song file's loop range is read past MAX_TICK, which set_loop refuses
area: CORE-FILES
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# CORE-FILES-002: A song file's loop range is read past MAX_TICK, which set_loop refuses

## Where

crates/meloet-core/src/songfile.rs:1823-1826 (`loop_range: Some(TickRange::new(r.start, r.end)?)`, no `MAX_TICK` check)
crates/meloet-model/src/timeline.rs:306-308 (`set_loop` refuses `range.end() > MAX_TICK`)
crates/meloet-core/src/play.rs:437 (`ceil`: `r.end() as i64`)

## What happens

A song file with `"loop": {"start": 0, "end": 18446744073709551615}` opens. The loop's end is then larger than any tick the model allows. Where the core takes it as i64 it wraps: `play.rs:437` gives a ceiling of -1 for an end of 2^63 or more; then, while recording with the loop on, `press` (`play.rs:470`) puts every live note at tick 0 (`.min(ceil - 1).max(0)`) and `release` (`play.rs:462`) gives it length 1, and `play.rs:2162` sends the raw u64 to the screen, where a JS number cannot hold it exactly. The engine is told the same end (`play.rs:1031-1037`, `EngineCommand::SetLoop`) and turns it into frames of the tempo map.

## What should happen

The loop range keeps the rule `set_loop` keeps: its end is at most `MAX_TICK` (meloet-model/src/timeline.rs:306). A song that breaks a rule of the model does not read (docs/architecture.md 14). The decoder already holds clip ends, markers, tempo points and automation points to `MAX_TICK`; the loop range is the one tick field it lets through.

## How to trigger

Read from the code, not run. Edit a saved song's `"loop"` to `{"start": 0, "end": 18446744073709551615}`, open it, turn the loop on, record and play a few keys: the live notes shown while recording sit at tick 0 with length 1.

## Cause

`to_song` builds the loop with `TickRange::new(r.start, r.end)`, which only checks `end > start`. The `MAX_TICK` test that `timeline::set_loop` does is missing here.

## Suggested fix

In `to_song`, also require `r.end <= MAX_TICK` (`TickRange::new(r.start, r.end).filter(|_| r.end <= MAX_TICK)?`, as for a clip at songfile.rs:1665). No protocol or file-shape change; meloet never writes such a loop.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
