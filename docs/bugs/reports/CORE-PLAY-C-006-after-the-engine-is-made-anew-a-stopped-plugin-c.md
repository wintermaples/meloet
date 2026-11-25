---
id: CORE-PLAY-C-006
title: After the engine is made anew, a stopped plugin can no longer be loaded again
area: CORE-PLAY-C
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-PLAY-C-006: After the engine is made anew, a stopped plugin can no longer be loaded again

## Where

crates/meloet-core/src/play/plugs.rs:701-707 (`plugs_lost` → `load_again(false)`)
crates/meloet-core/src/play/plugs.rs:668 (`Phase::Stopped | Phase::Reloading | Phase::Failed => continue`)
crates/meloet-core/src/service.rs:2587-2598 (`unload_gone`: the stopped slot handed back with the lost engine is taken out of `ids` and unloaded in `Hosts`)
crates/meloet-core/src/service.rs:3364-3373 (`Effect::ReloadPlugin` with no `ids[ticket]` → `Input::PluginUnreadable`)
crates/meloet-core/src/play/plugs.rs:603-604 (`Phase::Reloading` → back to `Phase::Stopped`)

## What happens

When the engine is lost with its stream and made anew (`service.rs` `Audio::open` → `rebuild` → `Input::PluginsLost`), every plugin that sounds is loaded again under a new ticket, but a plugin that had stopped is passed over. The lost engine's slots are dropped, and each one's `Gone` makes `unload_gone` take the stopped plugin's ticket out of `PluginSide::ids` and unload its slot in `Hosts` (with the state and parameters `Hosts` remembered for its reload). The core still lists the plugin as stopped and the screen still offers 「もう一度読みこむ」, but pressing it does nothing: `ReloadPlugin` finds no id, answers `PluginUnreadable`, and the plugin goes back to `Stopped`. An instrument stays silent, an effect stays passed by, until the person removes it or opens the song again. The AI link's revive (`reload_at`) ends the same way.

## What should happen

docs/architecture.md 20-10: "Reload: a new process, restoring the last known state plus parameters moved since." `stopped_plugins` / `plugin_is_down` keep offering 「もう一度読みこむ」 for such a plugin, so it should work (or the plugin should be loaded again with the others, from what the song has).

## How to trigger

Read from the code, not run.
1. A plugin of the song stops (its process crashed): 「…が止まりました」 with 「もう一度読みこむ」.
2. The output is lost in a way that loses the engine with the stream (cpal's failure paths, `recover` gets nothing back), and an output opens again: `rebuild`, `PluginsLost`.
3. Press 「もう一度読みこむ」: `Effect::ReloadPlugin` → no `ids[ticket]` → `PluginUnreadable` → still stopped.

## Cause

`load_again` does not load stopped plugins, and the reload path depends on the old slot still being known to `PluginSide::ids` and `Hosts`, which the drop of the lost engine removes. `ReloadPlugin` has no way to fall back to a load from the song.

## Suggested fix

In `load_again(false)`, load a `Stopped` plugin anew like the others (`PlugState::Bytes(old.plugin.state)` or `Fresh`, new ticket, removed from `plugs.stopped`), or in the service turn a `ReloadPlugin` whose ticket has no id into `Input::PluginUnreadable` only after trying a load from the song (the rules could do this in `plugin_unreadable` for `Phase::Reloading`: `load_from_song(spot)`). Related, but another fault: CORE-PLAY-C-001 (the same `continue` keeps the old sample rate for a later reload). No protocol or file change.

## Verification

Confirmed by reading (no test run). `plugs_lost` (`play/plugs.rs:701-707`) calls `load_again(false)`, which passes over `Phase::Stopped` (`plugs.rs:668`). The engine lost with its stream held the stopped plugin's slot (`HousedInstrument` / `HousedEffect`, `plugins/housed.rs`); when it is dropped its `Gone` tells the service, and `unload_gone` (`service.rs:2587-2598`) takes the ticket out of `ids` and unloads the slot in `Hosts`. A later `Effect::ReloadPlugin` finds no id (`service.rs:3364-3373`) and answers `Input::PluginUnreadable`, which puts `Reloading` back to `Stopped` (`plugs.rs:603-604`). 「もう一度読みこむ」 stays offered and does nothing. Severity `low` stands (it needs a stopped plugin and an engine lost with its stream, which the code itself treats as a failure path: "the engine was lost with an earlier stream; making a new one").

Corrections: none.

Fix plan: `crates/meloet-core/src/play/plugs.rs` `load_again`: when not `live` (the engine was made anew), load a `Stopped` plug anew like the others, from the song's state (`PlugState::Bytes(old.plugin.state)` or `Fresh`), with a new ticket, and take its old ticket out of `plugs.stopped`. `Reloading` and `Failed` stay as they are. What `Hosts` remembered of parameters moved since the last state is lost in this corner; that is acceptable against a plugin that cannot come back. Test next to `plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
