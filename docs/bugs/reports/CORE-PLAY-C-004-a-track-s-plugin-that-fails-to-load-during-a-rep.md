---
id: CORE-PLAY-C-004
title: A track's plugin that fails to load during a replace audition leaves the track silent after Esc, not on the sine stand-in
area: CORE-PLAY-C
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# CORE-PLAY-C-004: A track's plugin that fails to load during a replace audition leaves the track silent after Esc, not on the sine stand-in

## Where

crates/meloet-core/src/play/plugs.rs:607-624 (`plugin_unreadable`: under a trial on the seat, no `EngineOp::Instrument { SINE }`)
crates/meloet-core/src/play/plugs.rs:509-520 (`place_waiting` only places a `Ready` plugin)
crates/meloet-core/src/play/shelf.rs:364-375 (`end_audition` in replace mode: `EndTrial { keep: false }`, then `place_waiting`)
crates/meloet-core/src/play/plugs.rs:446-459 (`seat_instrument`: a plugin that is loading is sent to the engine as `InstrumentRef::Plugin`, which `instruments::make` makes `Silent`)

## What happens

If a track's plugin instrument is still loading when the person starts trying other sounds on that track in 「入れかえる」 mode, and the load then fails, the track is silent after the audition is cancelled (Esc, or the shelf closed): the engine's own instrument of the seat is still the silent stand-in for a plugin that loads. The status line said 「曲で使っている … を読みこめませんでした。1 本のトラックはサイン波で鳴らしています。」 (`PluginUnreadable`), and the instrument row says the sine wave stands in, but nothing sounds when it is played.

## What should happen

Spec 3-bands "A plugin that could not be loaded (89-12)": "When a plugin of the song is there but could not be loaded, that track plays with a sine wave, and the instrument row says 「サイン波で代わり」." `plugin_unreadable`'s own doc: "A track's instrument that did not load sounds as the sine wave (spec 3, 89-12)". The sibling `sync_plugs` (plugs.rs:724-731) keeps this under a trial because the sine is what was set aside there; here what was set aside is the silent stand-in.

## How to trigger

Read from the code, not run.
1. Open a song whose track 1 has a plugin instrument that takes long and then fails to load (its plugin host does not answer `Load` within 30 s, or the load is refused late).
2. While it loads (the track is silent), open the shelf on track 1 to replace its sound and choose a row (a trial on the seat).
3. The load fails: `plugin_unreadable` → `Phase::Failed`, `trial_seat == Some(seat)` so no sine is sent; the status line says the track plays a sine wave.
4. Press Esc: `end_audition` → `EndTrial { keep: false }` brings back the seat's own `Silent`; `place_waiting` does nothing for `Failed`. Play track 1: silence.

## Cause

`plugin_unreadable` leaves out the sine while a trial is on the seat (the engine would take it as the tried instrument), and nothing gives it later: `end_audition` → `place_waiting` handles only a `Ready` plugin.

## Suggested fix

In `place_waiting` (or in `end_audition` for replace mode), when the seat's own plugin is `Phase::Failed`, push `EngineOp::Instrument { seat, instrument: InstrumentRef::SINE }`. No protocol, wording or file change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
