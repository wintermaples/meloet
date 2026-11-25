---
id: CORE-FILES-001
title: A song file's notes and control points are read with any tick; Note::end then overflows
area: CORE-FILES
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-FILES-001: A song file's notes and control points are read with any tick; Note::end then overflows

## Where

crates/meloet-core/src/songfile.rs:1909 (`to_take`: a note is checked for length >= 1, key and velocity only; `start` and `length` are any u64)
crates/meloet-core/src/songfile.rs:1922-1931 (a control point's `tick` is any u64)
crates/meloet-core/src/songfile.rs:1683 (only the take's `length` is held to `MAX_TICK`, not what is inside it)
crates/meloet-model/src/song.rs:58-60 (`Note::end` is `self.start + self.length`)
crates/meloet-model/src/playback.rs:46,60 (`at(tick) = zero + tick as i64`, `at(n.end())`)

## What happens

A song file whose take holds a note such as `[1, 18446744073709551615, 60, 100]`, or `[0, 9223372036854775808, 60, 100]`, opens without a word. When the song's events are made for the engine (`playback::track_events`, after every open), `n.end()` overflows u64 in the first case and `zero + tick as i64` overflows i64 in the second. A debug build (the one the owner runs from VS Code) panics on the core thread. A release build wraps: the note-off lands at a negative tick, and `.min(end) as u64` turns it into a huge tick, so the note never ends (or ends before it starts). Control points with ticks above `i64::MAX` wrap the same way in `at(c.tick)` and land before the clip.

## What should happen

A song that breaks a rule the model keeps does not read (docs/architecture.md 14, "Does not read: ... a break of a model rule"; the brief: bad or hostile JSON must not panic). The model's own edits never make such notes: `meloet-model/src/notes.rs:190` refuses a note starting at or past `MAX_TICK`, and `notes.rs:211` caps a length to `MAX_TICK - start`. `songfile.rs` already refuses a clip end, a clip offset, a marker, a tempo point and an automation point past `MAX_TICK`; notes and control points are the gap.

## How to trigger

Read from the code, not run. Save any song with one keys clip, edit the `.meloet` file by hand so that one note of the take the clip shows is `[1, 18446744073709551615, 60, 100]`, and open it in a debug build: the core thread panics with "attempt to add with overflow" in `Note::end`. With `[0, 9223372036854775808, 60, 100]` the panic is in `playback.rs:46` (`zero + tick as i64`). In a release build the second note sounds until play stops.

## Cause

`to_take` (songfile.rs:1896-1944) checks the order of the notes and control points, the key, the velocity and a length of at least 1, but not that `start`, `start + length` and a control point's `tick` stay within `MAX_TICK` (`i64::MAX / 2`). Every later use assumes they do: `Note::end` adds without a check, and `playback.rs` casts ticks to i64.

## Suggested fix

In `to_take`, refuse the take (return `None`, so the song does not read as with every other broken rule) when a note has `start >= MAX_TICK` or `start.checked_add(length)` is `None` or above `MAX_TICK`, and when a control point's `tick > MAX_TICK`. No protocol or file-shape change; a song meloet wrote never has such values, so no real song stops reading.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-core --features dev-hooks --lib verify_core_files_001`): a song with one clip, encoded, the note `[37,200,60,90]` changed in the JSON to `[37,18446744073709551615,60,90]`; `songfile::decode` gives `Ok`, and `meloet_model::playback::track_events` then panics at `crates/meloet-model/src/song.rs:59` ("attempt to add with overflow", debug build). Release builds wrap, as the report says (read, not run). Severity `low` stands: only a file changed by hand or damaged has such a note; meloet never writes one.

Corrections: none.

Fix plan: `crates/meloet-core/src/songfile.rs` `to_take`: a note is taken only when `start < MAX_TICK` and `start.checked_add(length)` is `Some(end)` with `end <= MAX_TICK`; a control point only when `tick <= MAX_TICK`; else the take is `None` and the song does not read, as for the other broken rules. Tests in `songfile/tests.rs` (both notes of the report and a control point past `MAX_TICK` give `Err(Broken)`).

## Fix

`crates/meloet-core/src/songfile.rs` `to_take`: a take with a note whose `start + length` overflows or is above `MAX_TICK`, or with a control point above `MAX_TICK`, does not read (`ReadError::Broken`), as the other broken rules. `docs/architecture.md` 14 says so.

Test: `songfile::tests::ticks_past_max_tick_do_not_read` (meloet-core; both notes of the report, a note ending past `MAX_TICK` and a control point past it are broken; the same values at `MAX_TICK` read and `track_events` does not panic).
