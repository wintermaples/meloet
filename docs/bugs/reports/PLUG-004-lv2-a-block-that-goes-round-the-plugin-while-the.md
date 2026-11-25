---
id: PLUG-004
title: LV2: a block that goes round the plugin while the main thread holds the gate loses its notes, so a note-off can be lost and a note keeps sounding
area: PLUG
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# PLUG-004: LV2: a block that goes round the plugin while the main thread holds the gate loses its notes, so a note-off can be lost and a note keeps sounding

## Where

crates/meloet-plughost/src/formats/lv2.rs:1728-1736 (`Lv2Processor::process`: when `gate.enter()` fails the block is passed by and `block.events` are never looked at)
crates/meloet-plughost/src/rt.rs:178 (`self.events.clear()` after `process`: what the block did not use is gone)
crates/meloet-plughost/src/formats/lv2.rs:2264-2289 (`save_state` holds the gate through `state.to_string` and the copy, longer than the plugin needs it)
crates/meloet-plughost/src/formats/lv2.rs:2248-2262 (`set_param` takes the gate for every `SetParam` of a control port)
crates/meloet-plughost/src/formats/vst3.rs:970-980 (the same pass-by for VST3 while buses are switched or the plugin is switched off and on)

## What happens

While the host's main thread holds an LV2 plugin's gate (taking its state, putting one back, setting a control port from meloet, saving a preset), a block that the engine asks of that plugin "goes round" it: an instrument gives silence for that block, an effect gives the dry input. The block's events, the MIDI note-ons and note-offs and the automation points, are dropped with it: `process` returns before it reads `block.events`, and `SlotRun::serve` clears them after.

A dropped note-off leaves the note sounding in the plugin (the engine believes it is off, so stopping the song does not end it either); a dropped note-on is a note that is not heard. With a pad or organ sound the note hangs until the same key is played again.

The gate is taken in normal use while the song plays: every song save with plugins asks each plugin's state (docs/architecture.md 20-11), every visit to a plugin's window asks it on entering and on leaving (20-13, 15-C), a panel step asks it after `commit: true`, and every `SetPluginParam` of a control port takes it. `save_state` holds it also while it writes the state as Turtle text, which does not touch the plugin.

## What should happen

A block that cannot run the plugin may lose its sound (that is the gate's purpose, lv2.rs:9-11, and architecture 24-16 notes the gap), but the notes of that block must still reach the plugin, or at least no note-off may be lost: a key let go must end its note.

## How to trigger

Read from the code, not run.
1. Put Surge XT (LV2) or another LV2 synth with a held sound on a track with notes, and play the song.
2. While it plays, save the song (Ctrl+S), or click into and out of the plugin's window, or drag one of its parameters in the params panel.
3. When a block whose note-off falls in the time the gate is held is passed by, that note keeps sounding after its end.

## Cause

`Lv2Processor::process` treats a busy gate like "no plugin this block" and returns at once, without keeping `block.events`. Nothing carries them to the next block. `save_state` keeps the claim to the end of the function, so the gate is held for the Turtle writing too, which makes the window wider.

## Suggested fix

Two small changes in `formats/lv2.rs`:
1. In `process`, when the gate is busy, keep the block's MIDI events for the next block instead of dropping them (a fixed-size buffer in `Lv2Processor`, made at load, pushed into the MIDI port at frame 0 of the next block that runs; when it is full keep at least the note-offs). The same can be done in `Vst3Processor::process` for its rarer pass-by.
2. In `save_state`, let the claim go right after `instance.state()` (as `save_preset` does at lv2.rs:2364-2369); `to_string` and the copy need no gate.

No protocol or file-shape change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
