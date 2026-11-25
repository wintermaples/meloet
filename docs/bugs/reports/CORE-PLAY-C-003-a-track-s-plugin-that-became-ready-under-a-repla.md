---
id: CORE-PLAY-C-003
title: A track's plugin that became ready under a replace audition loses its state when the sample rate changes
area: CORE-PLAY-C
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-003: A track's plugin that became ready under a replace audition loses its state when the sample rate changes

## Where

crates/meloet-core/src/play/plugs.rs:661-665 (`live` and `Phase::Ready` → `PlugState::Of(old.ticket)`)
crates/meloet-core/src/play/plugs.rs:674-676 (`Ready && !placed` → `Effect::DropPlugin { ticket: old.ticket }`, pushed BEFORE the `LoadPlugin`)
crates/meloet-core/src/play/plugs.rs:681-688 (the `LoadPlugin` with `PlugState::Of(old.ticket)`)
crates/meloet-core/src/service.rs:3338-3348 (`DropPlugin` removes `ready[old]` and `ids[old]` and unloads it)
crates/meloet-core/src/service.rs:3649-3652 (`PlugState::Of(other)` with no `ids[other]` → `StartState::Fresh`)

## What happens

When the output opens at another sample rate (`set_plug_rate` → `load_again(true)`) while a track's own plugin is loaded but not in the engine, the plugin is loaded again as it starts (`StartState::Fresh`), not with its state: the patch or settings the song has for it are not in the new instance. The song still holds its state, but the plugin plays its default sound; a save asks the plugin (`plugin_tickets`) and writes the default state into the file in place of the song's.
A track's plugin is "ready and not placed" exactly while the shelf auditions another sound on that track in 「入れかえる」 mode (`plugin_loaded`: `Spot::Seat(seat) if trial_seat == Some(seat) => plug.placed = false`).

## What should happen

A plugin loaded again for another rate keeps its sound: `load_again` says "`live`: with the state each has now", and the test `plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine` (state/added_tests.rs:1388) expects `PlugState::Of(ticket)`, "with the sound it has now". Plugin state in the song is never lost (CLAUDE.md, song file rules; architecture 20-6).

## How to trigger

Read from the code, not run.
1. Open a song whose track 1 has a plugin instrument with a saved patch (or have meloet start with no output chosen yet, so the plugin loads at 48 kHz).
2. While that plugin is still loading, open the shelf on track 1 in replace mode and try another row (a trial on its seat). The plugin finishes loading: `Ready`, `placed = false`.
3. Choose an output that runs at 44.1 kHz: `AudioOpened` → `set_plug_rate(44_100)` → `load_again(true)`: effects `DropPlugin { old }`, then `LoadPlugin { new, state: Of(old) }`. The service handles them in order; the second finds no `ids[old]` and loads `Fresh`.
4. Press Esc: the seat's own instrument comes back (`place_waiting`) with the default sound. Save: the file now holds the default state.
Also reached with two rate changes during one audition (the first one's reload lands under the trial and is `Ready && !placed` at the second).

## Cause

In `load_again`, the `DropPlugin` of an unplaced old instance is pushed before the `LoadPlugin` that asks for that instance's state by its ticket. `Effect::DropPlugin` removes the ticket from `PluginSide::ids`, so `load_plugin` cannot find the instance to read and falls back to `StartState::Fresh`.

## Suggested fix

Push the `DropPlugin { ticket: old.ticket }` after the `LoadPlugin` (the hosts' thread answers in order: it reads the old instance's state for the load, then unloads it). Or, for `!old.placed`, use `PlugState::Bytes(old.plugin.state)` (`Fresh` when empty), since an unplaced instance was never changed. No protocol or file change.

## Verification

Confirmed by reading (no test run). In `load_again` (`play/plugs.rs:659-690`) a plug that is `Ready` gets `PlugState::Of(old.ticket)` when `live`; when it is also `!old.placed`, `Effect::DropPlugin { ticket: old.ticket }` is pushed before the `Effect::LoadPlugin`. The service handles effects in order: `DropPlugin` (`service.rs:3338-3348`) removes `ready[old]` and `ids[old]`, then `load_plugin` (`service.rs:3649-3652`) finds no `ids[old]` for `PlugState::Of(old)` and loads `StartState::Fresh`. A seat's own plugin is `Ready` and not placed only while a replace audition runs on that seat (`plugin_loaded`, `plugs.rs:549`). The plugin then has its default state while the song still holds the saved one, and a save asks the plugin. Severity `medium` stands: it is a rare order of events (an output at another rate during a replace audition whose own plugin became ready under it), but the saved song loses the plugin's state.

Corrections: none.

Fix plan: `crates/meloet-core/src/play/plugs.rs` `load_again`: for `Ready && !placed` use the song's state instead of the instance's (`PlugState::Bytes(old.plugin.state.clone())`, `Fresh` when empty): an instance that was never placed was never played or changed, so both are the same, and the order of the two effects no longer matters. Keep the `DropPlugin`. Test in `state/added_tests.rs`: a rate change during a replace audition gives `LoadPlugin` with `Bytes(the song's state)`.

## Fix

`crates/meloet-core/src/play/plugs.rs` `load_again_at` (the body of `load_again`): only a plugin that is in the engine is loaded again with `PlugState::Of` its ticket; one that became ready under a trial and was never placed gets the song's state (`Bytes`, `Fresh` when empty). The `DropPlugin` stays.

Test: `play::plugs::tests::a_tracks_plugin_that_became_ready_under_a_trial_keeps_the_songs_state_at_another_rate`.
