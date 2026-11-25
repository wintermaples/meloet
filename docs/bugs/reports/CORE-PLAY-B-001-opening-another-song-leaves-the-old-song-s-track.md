---
id: CORE-PLAY-B-001
title: Opening another song leaves the old song's track lanes playing in the engine
area: CORE-PLAY-B
kind: bug
severity: high
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
