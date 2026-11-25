---
id: MODEL-B-004
title: A vocal track accepts lanes of the built-in sound knobs, which move nothing
area: MODEL-B
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-B-004: A vocal track accepts lanes of the built-in sound knobs, which move nothing

## Where

crates/meloet-model/src/automation.rs:163-167 (`Track::has_target`, `AutoTarget::Instrument` on a track whose instrument is not a plugin)
crates/meloet-model/src/knobs.rs:84-91 (`knob_usable` does not look at the track's kind)

## What happens

`add_lane(doc, vocal_track, AutoTarget::Instrument { param: 0..=3 }, …)` succeeds: a vocal track gets a lane of 「明るさ」, 「立ち上がり」, 「のび」 or 「ゆれ」, with an undo step 「帯を足す」 and a lane row in the arrange. Its points are sent to the vocal track's seat as knob values, but a vocal track has no sound that uses knobs, so the lane changes nothing heard. `Doc::check_lanes` also accepts such a lane (from a song file too). The AI link reaches this: `add_lane` with target `instrument` and `param` 0..=3 on a vocal track is not refused (crates/meloet-ai/src/tools/automation.rs:137-143 → `Command::AddLane` → `Play::add_lane`, crates/meloet-core/src/play/automation.rs:508-521; the AI's dry run in crates/meloet-core/src/play/ai.rs:420-430 also says yes), so the AI is told it worked.

## What should happen

`has_target`'s own comment: the track "has what `target` moves: a plugin as its instrument, a knob its sound that comes with meloet uses". A vocal track has no instrument: crates/meloet-model/src/audio.rs:14-15 ("`Track::instrument` of one is the sine wave, which nothing reads"). Knobs belong to built-in sounds of keys tracks (docs/architecture.md 25-27; `Play::set_sound_knob` only acts on a track whose sound is the one named, and the screen shows knobs only for keys tracks, crates/meloet-core/src/play/mixer.rs:598). So `has_target(Instrument { .. })` should be false on a vocal track, and the AI should get "no such lane" (`AiRefusal::NoSuchLane` for `NoSuchInsert`, crates/meloet-core/src/play/ai.rs:451).

## How to trigger

Read from the code, not run. With the AI link on: add a vocal track, then call the tool `add_lane` with `{"track": <the vocal track>, "target": "instrument", "param": 0}`. The song gets a 「明るさ」 lane on the vocal track; `put_points` on it also works; nothing in the sound changes.

## Cause

For `AutoTarget::Instrument`, `has_target` asks only `knob_usable(&self.instrument, knob)`. A vocal track's `instrument` is `InstrumentRef::SINE`, a built-in sound, and `knob_usable(SINE, _)` is true for every knob, so the track kind is never looked at.

## Suggested fix

In `Track::has_target`, return false for `AutoTarget::Instrument { .. }` when `self.kind.is_vocal()` (and, for the same reason, let `knobs::set_sound_knob` refuse a vocal track). The song file reader already asks `has_target` for every lane and leaves out one without a target (crates/meloet-core/src/songfile.rs:684), so a file that holds such a lane still opens, without it. No protocol or song-file shape change.

## Verification

Confirmed by reading (no test run). `Track::has_target(Instrument { param })` looks only at `self.instrument`; a vocal track's instrument is `InstrumentRef::SINE` ("which nothing reads"), so params 0 to 3 pass. `auto::add_lane` (`automation.rs:323`) and the AI's dry run (`play/ai.rs:422-430`) both ask only `has_lane_target`, so the AI link's `add_lane` with `instrument` on a vocal track makes a lane and is told it worked. The screen does not offer knob lanes on a vocal track, so a person reaches it only through the AI link or a song file. Severity `low` stands.

Corrections: none.

Fix plan: `crates/meloet-model/src/automation.rs` `Track::has_target`: `AutoTarget::Instrument { .. }` is false when `self.kind.is_vocal()`. Then `add_lane` gives `NoSuchInsert` (the AI gets `NoSuchLane`), and the song file reader leaves such a lane out. Test in the automation tests (add_lane on a vocal track is refused). Do not touch `knobs::set_sound_knob` unless it is reachable for a vocal track; check while fixing.

## Fix

`crates/meloet-model/src/automation.rs` `Track::has_target`: `AutoTarget::Instrument` is false on a vocal track. `add_lane` then gives `NoSuchInsert` (the AI's `add_lane` is answered `NoSuchLane`), `check_lanes` refuses such a lane, and the song file reader leaves it out, so a file that holds one still opens without it. `knobs::set_sound_knob` was not touched: the screen offers no knobs for a vocal track (`picked_at` in `play/mixer.rs`) and no AI tool sets knobs. Tests: `automation::tests::a_vocal_track_has_no_lanes_of_an_instrument` (model) and `songfile::vocal_tests::a_lane_of_an_instrument_on_a_vocal_track_is_left_out` (core: the file opens, the lane is left out, the volume lane stays).
