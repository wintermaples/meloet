---
id: PLUG-003
title: VST3: SetParam can answer with the value set before, because param() replays older audio-thread values over the new one
area: PLUG
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# PLUG-003: VST3: SetParam can answer with the value set before, because param() replays older audio-thread values over the new one

## Where

crates/meloet-plughost/src/control.rs:809-824 (`Request::SetParam`: `controller.set_param`, then `audio.set_param`, then `c.param(id)` for the answer)
crates/meloet-plughost/src/formats/vst3.rs:1410-1414 (`Vst3Controller::param` calls `hear_audio()` after the value was set)
crates/meloet-plughost/src/formats/vst3.rs:1167-1176 (`hear_audio` applies every queued value, oldest first, with `setParamNormalized`)
crates/meloet-plughost/src/formats/vst3.rs:987-997 (every block pushes the last value of each `Param` event, also the ones meloet set, to `to_controller`)

## What happens

For a VST3 plugin, `SetParam {value: v2}` sets v2 on the controller, then reads the answer with `param()`. `param()` first runs `hear_audio()`, which empties the ring from the audio thread and calls `setParamNormalized` with each value in it. If a block has processed the previous `SetParam {value: v1}` for the same parameter since the main thread last emptied the ring, the ring still holds `(id, v1)`, and it is applied over v2. The answer then says v1 (value and text), and the plugin's controller, and its window if open, shows v1 until the next block's echo of v2 is heard (a block plus up to 10 ms later).

The answer goes to the params panel row (`param_now` in crates/meloet-core/src/play/params.rs, the dragged row's value and text) and to an AI set. During a drag in the panel the row's text jumps back one step now and then; when the mouse is let go right after the last move, the `commit: true` answer can be the value before, and nothing reads the row again afterwards, so the panel keeps showing a value one step off from what the plugin has.

## What should happen

docs/architecture.md 20-12: "Even `false` returns value and text in `paramValues` (the plugin writes the dragged row's text)"; crates/meloet-plugin/src/wire.rs:89-90: `SetParam` is "Answered by `Param`: the value as the plugin took it, with its text." The value just set is the newest; values the audio thread reports from before it must not be put over it. LV2 does not have this: `Lv2Controller::param` reads the value kept in `Shared` and replays nothing (lv2.rs:2233-2236).

## How to trigger

Read from the code, not run. It needs an engine block longer than the host main thread's 10 ms tick (`TICK`, control.rs:33), so it shows with a quantum of 512 frames and more (about 30 % of the steps of a drag at 1024 frames / 48 kHz), rarely at 256.
1. Load a VST3 effect or instrument (ZamComp, Surge XT) and pick it so its parameters show in the params panel.
2. Drag a continuous parameter in the panel; the screen sends `SetPluginParam {commit: false}` about every 16.7 ms, then `commit: true` on release.
3. When the block that took v1 ends after the main loop's last `poll` and before `SetParam {v2}` arrives, the answer to v2 carries v1.

## Cause

`hear_audio` is meant to bring the controller up to date with what the audio thread gave the processor (automation). It does not know that the main thread has set a newer value for the same parameter after those values were queued, and `param()` calls it after `set_param`. The ring holds echoes of meloet's own `SetParam` values too (the block's `told` loop takes every `ToPlugin::Param`).

## Suggested fix

Answer `SetParam` from the controller without replaying the ring after the value was set: have `Vst3Controller::set_param` call `self.hear_audio()` before `setParamNormalized`, and read the answer with a form of `param()` that does not call `hear_audio()` (a flag or a small separate method used only by the `SetParam` arm). An echo of v1 from a block that was running at that moment can still pass over the controller later for a moment, but the answer and the panel row are right, and the echo of v2 follows. No protocol or file-shape change.

## Verification

Confirmed by reading (no test run). `Request::SetParam` (`control.rs:804-824`) calls `controller.set_param(id, v2)` (`vst3.rs:1459`, a plain `setParamNormalized`), then answers with `c.param(id)`, and `Vst3Controller::param` (`vst3.rs:1410-1414`) runs `hear_audio()` first, which applies every `(id, value)` the audio thread queued (`vst3.rs:987-997` queues the last value of every `ToPlugin::Param` of a block, meloet's own included). An echo of the value set before, still in the ring, lands over v2 and is what the answer reads. LV2's `param` reads the kept value and replays nothing. Severity `low` stands (needs a block that ends between the host's 10 ms poll and the next `SetParam`; the row is one step off until the next reading).

Corrections: none.

Fix plan: `crates/meloet-plughost/src/formats/vst3.rs`: in `Vst3Controller::set_param` call `self.hear_audio()` before `setParamNormalized`, and give the `SetParam` arm an answer that does not replay the ring after the set (a private `param_now(id)` without `hear_audio`, used by `param()` after its own `hear_audio()`); `crates/meloet-plughost/src/control.rs` uses it through the controller trait only if the trait needs a new method, else keep the change inside `vst3.rs` by remembering "just set" for the next `param` call. Test with the fake VST3-like controller if one exists in `control.rs` tests; else a unit test of the order in `vst3.rs`.

## Fix

- `crates/meloet-plughost/src/formats/vst3.rs`: `Vst3Controller::set_param` hears the ring first (values from before land before the new one), sets the value and keeps its id (`just_set`); the next `hear_audio` (the one inside the `param()` that answers `SetParam`) passes over that parameter's values in the ring, which were given before the set (a block that ran with the value before while `Handle::set_param` waited for the seat). Other parameters are heard as before. The trait and `control.rs` are unchanged.
- Test: `formats::vst3::tests::a_value_set_is_answered_over_the_one_before_it_heard_from_the_audio_thread` (`#[ignore]`, needs Surge XT (VST3)); written but not run in this fix (the request said not to run ignored tests). There is no fake VST3 controller to test it without a real plugin.
