---
id: CORE-PLAY-C-002
title: An AI state put whose plugin process stops after taking the bytes is loaded again with the put state, not the song's
area: CORE-PLAY-C
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# CORE-PLAY-C-002: An AI state put whose plugin process stops after taking the bytes is loaded again with the put state, not the song's

## Where

crates/meloet-core/src/plugins/hosts.rs:656-666 (`Ask::SetState`: on `Reply::Ok` the put state becomes `Slot::state`, the state `reload` uses)
crates/meloet-core/src/service.rs:3574-3591 (`PluginAsk::PutState`: `set_state`, then `state`; a `Gone` of the second one is told as `StatePut(Err(Gone))`)
crates/meloet-core/src/play/plugs/aistate.rs:186-190 (`Err(HostError::Gone)` → `revive(ticket, restored)`, `from_song` only after an undo or redo)
crates/meloet-core/src/play/plugs/aistate.rs:351-366 (`reload_at` → `Effect::ReloadPlugin`, which loads what `Hosts` remembers)
crates/meloet-core/src/play/plugs/aistate.rs:199-203 (the text the AI is given)

## What happens

When the AI link puts a state into a plugin (`set_plugin_state`, `set_plugin_state_text`, `load_preset`, the Surge XT tools) while the sound runs, the plugin host answers the set with `Ok` before the plugin has really taken the state: per docs/architecture.md 24-16 G, Surge XT takes it only in its next audio block, and the host makes the following `GetState` wait up to 3 blocks. If the process stops in that time (the state crashes the plugin as it is read in the audio block, or it hangs and is killed), the core gets `StatePut(Err(Gone))` and loads the plugin again with `Effect::ReloadPlugin`, and `Hosts` loads it with the state it remembers, which is now the state the AI put in, not the song's. The AI is told "it is loaded again with the song's state, and no step was made", and the song keeps its old state, but the plugin plays the AI's state; a later save asks the plugin and writes the AI's state into the file without a step. When that state is what crashed the plugin, the reload crashes again (`plugin_unreadable` leaves it `Stopped`), and every 「もう一度読みこむ」 the person presses loads the same state and crashes again: the plugin can only be removed.

## What should happen

docs/architecture.md 24-16 G: "The process stops (`Gone`) → `ReloadPlugin` with the state `Hosts` kept (a failed set does not replace it)", and `AiPutEnd::Stopped` (ai.rs:716-718): "The plugin's process stopped while it took it (the state may be broken): it is loaded again with the song's state." After a put that ends `Stopped`, the song and the plugin have the song's state (ai.rs:704-705: "after the others the song and the plugin have the song's state").

## How to trigger

Read from the code, not run.
1. A track with Surge XT (VST3) plays (or at least the output runs, so its blocks run).
2. Through the AI link, `set_plugin_state` with a container whose component part is broken (architecture 24-16 G: "A Surge XT VST3 state with the component part cut in half makes the plugin host crash (SIGSEGV)"; with the sound running the plugin reads it in its next block, after `SetState` was answered `Ok`).
3. `Hosts` stores the bytes as `Slot::state`; the `GetState` that follows gets `Gone`; the core reloads with `Effect::ReloadPlugin`; `Hosts::reload` puts the broken bytes in again.
The same without a crashing state: any stop of the process between the `Ok` of the set and the answer to the read (a hang killed after 5 s) reloads the plugin with the AI's state while the song keeps its own.

## Cause

`Run::answer` for `Ask::SetState` remembers a state as soon as the process answers the request, which for a plugin with running blocks only means the bytes were handed over. `ai_state_put` treats `Err(Gone)` as "the set failed" and revives through `ReloadPlugin` (from `Hosts`' memory) unless an undo or redo came in between (`restored`).

## Suggested fix

In `ai_state_put`, for `Err(HostError::Gone)` always revive the put plugin from the song: `self.revive(ticket, true, effects)` (the `from_song` path, `load_from_song`, already exists for the undo case). The other plugins of the same process that are revived keep using `Hosts`' memory. Alternatively, `Hosts` could remember a put state only after a later `GetState` of that slot answers. No protocol, wire or file change.

## Verification

Confirmed by reading (no test run; it needs a real plugin with the sound running, which I did not start). `Run::answer` for `Ask::SetState` (`hosts.rs:656-666`) stores the put bytes as `Slot::state` as soon as the host process answers `Ok`. `PluginAsk::PutState` (`service.rs:3574-3591`) sends `set_state` and then `state`; when the first is `Ok` and the second is `Gone`, the core gets `StatePut(Err(Gone))`. `ai_state_put` (`aistate.rs:186-190`) calls `revive(ticket, restored)`, and `from_song` is true only after an undo or redo; else `reload_at` pushes `Effect::ReloadPlugin`, and `Run::reload` loads `slot.state`, which is now the put state. Architecture 24-16 G says a state put while blocks run is taken by Surge XT only in its next block, after `SetState` was answered; so a crash there comes after the `Ok`. The same chapter promises "`ReloadPlugin` with the state `Hosts` kept (a failed set does not replace it)" and the answer text says "loaded again with the song's state". The case measured in 24-16 G (the half-cut state, "it is reloaded and the song is safe") was with no sound running, where the set itself fails; that case is right today. Severity `medium` stands (the song's plugin plays a state the song does not hold, and a state that crashes is put in again at every reload).

Corrections: none.

Fix plan: `crates/meloet-core/src/play/plugs/aistate.rs` `ai_state_put`: for `Err(HostError::Gone)` call `self.revive(ticket, true, effects)`, so the put plugin is loaded anew from the song (`load_from_song`); the other plugins of the same process keep the `ReloadPlugin` path. Test in the aistate tests: a put whose answer is `Gone` gives `Effect::LoadPlugin` with the song's state, not `ReloadPlugin`. If `crates/meloet-ai/tests/plugstate.rs` (ignored, real plugins) has the half-cut case, add the same with the sound running when someone can run it.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
