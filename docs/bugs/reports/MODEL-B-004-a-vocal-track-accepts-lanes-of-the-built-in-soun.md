---
id: MODEL-B-004
title: A vocal track accepts lanes of the built-in sound knobs, which move nothing
area: MODEL-B
kind: bug
severity: low
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
