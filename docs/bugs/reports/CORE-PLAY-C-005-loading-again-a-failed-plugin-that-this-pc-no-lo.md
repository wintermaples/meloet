---
id: CORE-PLAY-C-005
title: Loading again a failed plugin that this PC no longer has leaves its track silent, not on the sine stand-in
area: CORE-PLAY-C
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-PLAY-C-005: Loading again a failed plugin that this PC no longer has leaves its track silent, not on the sine stand-in

## Where

crates/meloet-core/src/play/plugs.rs:860-873 (`reload_plugin`, `Phase::Failed`: the return of `want_plug` is not looked at)
crates/meloet-core/src/play/plugs.rs:402-405 (`want_plug` → `Want::Missing` when `known` has no bundle for the key)
crates/meloet-core/src/instruments.rs:127 (`InstrumentRef::Plugin(_)` → `Silent`)

## What happens

「もう一度読みこむ」 on a track whose plugin instrument did not load (`Phase::Failed`, the track plays the sine wave) drops the failed entry and asks `want_plug` to load the plugin, then sends `EngineOp::Instrument { instrument: Plugin(..) }` whatever `want_plug` answered. When this PC no longer has the plugin (a later scan, e.g. 「もう一度スキャン」 after it was removed or moved, took it out of `known`), `want_plug` answers `Missing` and loads nothing, but the seat still gets the plugin stand-in, which is `Silent`. The track goes from the sine wave to silence and stays so; the screen shows the plugin as missing, which says the sine wave stands in.

## What should happen

docs/architecture.md 20-11 "Missing things": "No plugin → instrument sine"; `engine_instrument` (plugs.rs:343-350) gives the engine the sine for a plugin this PC does not have. The sibling `load_from_song` (aistate.rs:375-384) sends the instrument only when `want_plug(..) == Want::Loading`, and `seat_instrument` maps `Want::Missing` to `InstrumentRef::SINE` (plugs.rs:446-449).

## How to trigger

Read from the code, not run.
1. A song's track uses a plugin instrument that fails to load (`PluginUnreadable`): it plays the sine wave and shows 「もう一度読みこむ」.
2. The plugin is taken off the PC (or its bundle moved) and the scan runs again (「もう一度スキャン」): `set_known_plugins` drops the key.
3. Press 「もう一度読みこむ」 on the track: `reload_plugin` → `drop_plug` → `want_plug` = `Missing` → `EngineOp::Instrument { Plugin }` → the seat is `Silent`.

## Cause

In the `Phase::Failed` arm of `reload_plugin`, the `Want` that `want_plug` returns is thrown away and the plugin is always sent to the engine.

## Suggested fix

Look at the answer as `load_from_song` does: send `InstrumentRef::Plugin(plugin)` only for `Want::Loading`, and `InstrumentRef::SINE` for `Want::Missing` (`Kept` cannot happen after the drop). No protocol or file change.

## Verification

Confirmed by reading (no test run). In the `Phase::Failed` arm of `reload_plugin` (`play/plugs.rs:860-873`) the answer of `want_plug` is thrown away and `EngineOp::Instrument { instrument: InstrumentRef::Plugin(plugin) }` is always sent (outside a trial). `want_plug` answers `Want::Missing` and loads nothing when `plugs.known` has no bundle for the key (`plugs.rs:402-405`), and the engine's stand-in for a plugin is silent. `seat_instrument` (`plugs.rs:446-449`) maps `Want::Missing` to the sine. Severity `low` stands (the plugin must leave the PC between the failed load and the press).

Corrections: none.

Fix plan: `crates/meloet-core/src/play/plugs.rs` `reload_plugin`: `match self.want_plug(..)`: `Want::Loading` → the plugin stand-in as now; `Want::Missing` → `InstrumentRef::SINE` (the seat already plays the sine, so sending nothing is as good; choose the one that reads clearer). Test in the plugs tests: failed plug, the key leaves `known`, reload → no `EngineOp::Instrument { Plugin }`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
