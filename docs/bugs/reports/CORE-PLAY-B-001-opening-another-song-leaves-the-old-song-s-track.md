---
id: CORE-PLAY-B-001
title: Opening another song leaves the old song's track lanes playing in the engine
area: CORE-PLAY-B
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# CORE-PLAY-B-001: Opening another song leaves the old song's track lanes playing in the engine

## Where

crates/meloet-core/src/play/autosync.rs:147-152 (`all` clears `sent`)
crates/meloet-core/src/play/autosync.rs:183-186 (a seat with no lanes and no entry in `sent` is skipped)
crates/meloet-core/src/play/seats.rs:195-210 (`reseat` keeps the seats the new song takes again, with no `ClearSeat`)
crates/meloet-core/src/play.rs:2076-2106 (`replace_song` → `reseat` → `stream_opened` → `send_automation(true)`)

## What happens

When a song with automation lanes on its tracks is replaced by another song (「新しい曲」, open a song, open a recent song) while the output is open, a track of the new song that sits on a seat the old song also used, and that has no lane of its own, is still moved by the old song's lanes. For example, the old track 1 had a volume lane that goes down to −∞; the new song's track 1 has no lanes. After the new song opens, its track 1 follows the old volume line (also when stopped, at the play line), and its fader in the song does nothing. The same for pan, sends, mute (the mute lane makes it silent), the knobs of a built-in sound, and the parameters of whatever effect or plugin the new song has in the same slot or as its instrument (the old lane's parameter number is sent to the new plugin). It stays so until that track's lanes change (a lane is added to it).

## What should happen

The engine plays the lanes of the song that is open, and nothing else (docs/architecture.md 21-7: "the core packs each track's non-resting lane points into one box"; 21-5: a lane with no points, or none, leaves the hand-set value). The sibling `send_audio(false)` in `play/audiosync.rs:119-130`, called by the same `reseat`, does it right: with `fresh == false` it sends `EngineOp::Clips` also with an empty list to every taken seat, so the old song's audio clips go. The master's lane is also always told, "none" included (`autosync.rs:153-161`, architecture 25-32).

This is not the known hole of docs/progress.md line 377 (section "Round 12") ("a track's lanes deleted (or rested) while no stream runs stay in the engine"): here the stream runs the whole time, and nothing is deleted or rested by the person.

## How to trigger

Read from the code, not run.
1. With the output open, make a song whose track 1 has a volume lane with points (or a mute lane with a point at 1.0).
2. 「新しい曲」 (answer 「保存しない」), or open another song whose track 1 has no lanes.
3. Play, or just move the play line: track 1 of the new song follows the old lane; with the old mute lane it is silent.

## Cause

`replace_song` calls `reseat`, which gives the new song's tracks seats 0..n. A seat that both songs use is not emptied (`clear_seat` only runs for old seats the new song does not take, `seats.rs:204-208`), so the engine's track keeps its `AutoSet` (`EngineCommand::ClearTrack` is the only thing besides `SetAutomation` that drops it). Then `stream_opened` calls `send_automation(true)`, which first clears `self.autosync.sent` and then, for each seat, skips it when `before.map_or(heads.is_empty(), …)` is true: with `sent` empty, `before` is `None`, so every seat whose new track has no playing lane is skipped and no `EngineOp::Automation { seat, lanes: [] }` is sent. The `gone` loop cannot help either, because it walks the `sent` map that was just cleared.

## Suggested fix

In `send_automation`, when `all` is true, send `EngineOp::Automation { seat, lanes: Vec::new() }` for every seat in `now` whose heads are empty (or: do not skip a seat with empty heads when `all` is true), as the master's lane is sent "also none". This also closes the known hole of progress.md line 377 (a new stream). No protocol or file change.
## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-core --features dev-hooks --lib verify_core_play_b_001`): with the stream open (`ready()`), a volume lane with two points on track 1, then `Play::replace_song(Song::new_default(), ..)`. Track 1 of the old song and of the new song both sit on seat 0, and among the effects of the replace there is no `EngineOp::Automation` and no `EngineOp::ClearSeat` at all. On the engine side (read): a track's `AutoSet` is replaced only by `EngineCommand::SetAutomation` (`engine.rs:1951-1957`) and dropped only by `ClearTrack` (`engine.rs:2093-2112`); seating another instrument does not touch it. So the old song's lanes keep moving the new song's track on that seat. I did not run the engine to hear it. Severity `high` stands (wrong sound, down to silence, in a song that has no lanes; the fader does nothing).

Corrections: none. This is wider than the known line in `docs/progress.md` ("lanes deleted (or rested) while no stream runs stay in the engine"): here a stream runs the whole time.

Fix plan: `crates/meloet-core/src/play/autosync.rs` `send_automation`: with `all`, do not pass over a seat whose heads are empty: send `EngineOp::Automation { seat, lanes: Vec::new() }` for it (as the master's lane is told "also none"). Seats the new song does not take are emptied by `reseat` already. Test in `crates/meloet-core/src/play/automation/tests.rs`: after a replace, every taken seat got an `Automation` op, empty for a track without lanes. The fix also closes the known line of `docs/progress.md` for a new stream; take that line out with the fix if a test shows it.

## Fix

`crates/meloet-core/src/play/autosync.rs` `send_automation`: with `all` (a new stream, another song) a seat whose track has no playing lane is no longer passed over: it is sent `EngineOp::Automation { seat, lanes: [] }`, as the master's lane is told "also none". This also closes the known hole of `docs/progress.md` (a track's lanes deleted or rested while no stream runs stayed in the engine): a new stream is now told every seat's lanes. `docs/architecture.md` 25-32 says so.

Tests (in `play/autosync.rs`): `another_song_leaves_no_lane_of_the_one_before_on_its_seats` (「新しい曲」 → 「保存しない」 after a volume lane on track 1: every seat of the new song is told, with no lane) and `a_new_stream_hears_of_a_lane_deleted_while_none_ran`. Changed to the right behavior: `a_song_without_lanes_tells_the_engine_nothing` is now `a_song_without_lanes_tells_a_new_stream_so_and_then_nothing` (a new stream is told "no lane" once per seat, and nothing after), and `play/tests.rs` `the_stream_opening_gets_everything` now also expects `Automation { seat: 0, lanes: [] }`.
