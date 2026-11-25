---
id: MODEL-B-001
title: Removing a missing plugin instrument keeps its lanes of parameters 0-3, which then move the sine's knobs
area: MODEL-B
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# MODEL-B-001: Removing a missing plugin instrument keeps its lanes of parameters 0-3, which then move the sine's knobs

## Where

crates/meloet-model/src/mixer.rs:752-758 (`remove_missing`: `track.instrument = InstrumentRef::SINE`, then `drop_orphan_lanes`)
crates/meloet-model/src/automation.rs:163-167 (`Track::has_target` for `AutoTarget::Instrument` on a built-in sound)
crates/meloet-model/src/knobs.rs:84-91 (`knob_usable`: the sine uses all four knobs)

## What happens

「見つからない物を曲から外す」 on a plugin that is a track's instrument gives the track the sine wave. Lanes of that plugin's parameters whose plugin id is 0, 1, 2 or 3 stay in the song. They are now read as lanes of the sine's knobs (`SoundKnob::ALL[param]`: 「明るさ」, 「立ち上がり」, 「のび」, 「ゆれ」), so the plugin's old points move the sine's brightness, attack, release or vibrato while the song plays. Lanes of the same plugin with ids 4 and up are dropped, so the result also depends on the number the plugin happened to give.

## What should happen

All lanes of the old instrument's parameters go in the same step, and the knobs start again, as for every other way the instrument is replaced: docs/architecture.md 21-5 ("Removing or replacing an effect, or replacing the instrument, removes that parameter's lanes in the same undo step ... `RemoveMissing` in model do this") and 25-27 ("Replacing the sound puts the knobs back and drops their lanes in the same step"). The sibling `edit::with_instrument` (crates/meloet-model/src/edit.rs:783-798), used by `set_instrument` and `remove_instrument` (「外す」 of a stopped plugin), drops every `AutoTarget::Instrument` lane unless the new instrument is the same plugin, and resets `knobs`.

## How to trigger

Read from the code, not run.
1. A keys track whose instrument is a plugin, with a lane of one of its parameters whose id is 0..=3 (LV2 control ports use the port index as the id, `crates/meloet-plughost/src/formats/lv2.rs:691`, so small ids are common), with some points.
2. Open the song on a PC without that plugin; the track plays the sine and `send_automation` leaves the instrument lanes out (crates/meloet-core/src/play/autosync.rs:136-143).
3. Choose 「曲から外す」 for the plugin (`MissingRef::Plugin`, crates/meloet-core/src/state.rs:2419 → `Play::remove_missing` → `mixer::remove_missing`).
4. The track's instrument is now `InstrumentRef::SINE`, a built-in sound, so autosync sends the kept lane to the seat and the sine's knob follows the plugin's points. The lane also shows under a knob name.

## Cause

`remove_missing` sets the instrument field directly and relies on `drop_orphan_lanes`, which keeps every lane for which `has_target` is true. For a built-in instrument `has_target(Instrument { param })` is `SoundKnob::from_index(param).is_some_and(knob_usable)`, and `knob_usable(SINE, _)` is true for all four knobs, so plugin lanes with `param` 0..=3 count as having a target. `track.knobs` is also not reset here, unlike `with_instrument`.

## Suggested fix

In `remove_missing`, when `uses(&track.instrument)`, replace the instrument the way `edit::with_instrument` does (drop every `AutoTarget::Instrument` lane and reset `knobs`), for example by making `with_instrument` `pub(crate)` and calling it before `drop_orphan_lanes`. No protocol or song-file change. A test like `lanes_go_with_what_they_move` with an instrument plugin lane of param 2 would watch it.

## Verification

Confirmed by reading (no test run). `mixer::remove_missing` sets `track.instrument = InstrumentRef::SINE` by hand and then calls `drop_orphan_lanes`; `Track::has_target(Instrument { param })` on a built-in sound is `SoundKnob::from_index(param)` + `knob_usable`, and `knob_usable(SINE, _)` is true for all four knobs, so lanes of the plugin's parameters 0 to 3 stay and are read as knob lanes. `edit::with_instrument` (the path of `set_instrument` and `remove_instrument`) drops every `AutoTarget::Instrument` lane and resets `knobs`; this path does neither. Architecture 21-5 names `RemoveMissing` among the steps that remove the lanes. Severity `medium` stands (wrong sound after a normal action on a song opened without its plugin; LV2 parameter ids are port indexes, so 0 to 3 are common).

Corrections: none.

Fix plan: `crates/meloet-model/src/edit.rs`: make `with_instrument` `pub(crate)`; `crates/meloet-model/src/mixer.rs` `remove_missing`: when `uses(&track.instrument)`, build the track with `with_instrument(&track, InstrumentRef::SINE)` before emptying the inserts and `drop_orphan_lanes`. Test next to `lanes_go_with_what_they_move` in the mixer tests: a missing plugin instrument with lanes of param 2 and param 9; after `remove_missing` neither is there and `knobs` is the default.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
