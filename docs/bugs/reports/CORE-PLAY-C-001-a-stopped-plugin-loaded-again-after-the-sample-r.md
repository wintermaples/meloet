---
id: CORE-PLAY-C-001
title: A stopped plugin loaded again after the sample rate changed runs at the old sample rate
area: CORE-PLAY-C
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# CORE-PLAY-C-001: A stopped plugin loaded again after the sample rate changed runs at the old sample rate

## Where

crates/meloet-core/src/play/plugs.rs:668 (`load_again` passes over `Phase::Stopped | Phase::Reloading`)
crates/meloet-core/src/play/plugs.rs:855-858 (`reload_plugin`: `Effect::ReloadPlugin { ticket }` carries no rate)
crates/meloet-core/src/play/plugs/aistate.rs:362-365 (`reload_at`, the same effect)
crates/meloet-core/src/plugins/hosts.rs:1165-1172 (`Run::reload` loads with `slot.spec`, the spec of the first load)
crates/meloet-core/src/plugins/hosts.rs:1110-1117 (`Request::Load { sample_rate: spec.sample_rate, .. }`)
crates/meloet-core/src/service.rs:3364-3373 (`Effect::ReloadPlugin` → `Hosts::reload(id, block, ..)`)

## What happens

A plugin whose process stopped (or is being loaded again, `Phase::Reloading`) while the output runs at one sample rate, and that is loaded again with 「もう一度読みこむ」 (or by the AI link's revive) after the output was opened at another rate, is loaded at the OLD rate. Its plugin host is told the old rate in `Request::Load`, while the engine runs at the new one. An instrument then plays out of tune and out of time (told 48 kHz while the engine runs at 44.1 kHz: about 1.5 semitones low and slower; tempo-synced LFOs and delays wrong); an effect's delays, filters and reverb times are wrong. Nothing tells the person; the plugin shows as ready.

## What should happen

Every plugin runs at the rate of the stream: `Play::set_plug_rate` says "a plugin is made for one rate, so every one is loaded again when it is another than before" (plugs.rs:692-699), and `want_plug` / `load_again` always send `sample_rate: self.plugs.rate`. docs/architecture.md 20-10: "Reload: a new process, restoring the last known state plus parameters moved since" — with the rate the engine runs at now. The sibling path for a plugin that never loaded (`Phase::Failed` in `reload_plugin`) does it right: it goes through `want_plug`, which uses the present rate.

## How to trigger

Read from the code, not run.
1. A song with a plugin instrument (e.g. Surge XT) plays through an output at 48 kHz.
2. The plugin's process stops (crash, or a hang killed after 5 s): the track shows 「止まりました」 with 「もう一度読みこむ」.
3. Choose another output that runs at 44.1 kHz (`Input::AudioOpened { sample_rate: 44_100 }` → `set_plug_rate` → `load_again(true)`, which passes the stopped plugin over).
4. Press 「もう一度読みこむ」: `Effect::ReloadPlugin` → `Hosts::reload` → `place(&slot.spec, ..)` with `sample_rate: 48_000`. Play: the track sounds about 1.5 semitones too low.
The same for a plugin in `Phase::Reloading` at step 3 (its reload, already asked, answers at 48 kHz).

## Cause

`Hosts` keeps the `LoadSpec` of the first load in `Slot::spec` and `Run::reload` loads with it unchanged; only `SetOutputs` ever updates the spec (hosts.rs:752-759). The core's `load_again` reloads every `Ready`/`Loading` plugin at the new rate but `continue`s on `Stopped`/`Reloading`, and the later `Effect::ReloadPlugin { ticket }` has no rate to give, so nothing tells `Hosts` the rate changed.

## Suggested fix

Give the reload the present rate: `Effect::ReloadPlugin { ticket, sample_rate: self.plugs.rate }` and `Hosts::reload(id, sample_rate, block, done)`, which sets `slot.spec.sample_rate = sample_rate` before `place`. A plugin in `Phase::Reloading` when the rate changes can be asked to reload again (or marked so `plugin_loaded` reloads it once more). No protocol, wire or file change. See also CORE-PLAY-C-006 (the same `continue` in `load_again`, another fault).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
