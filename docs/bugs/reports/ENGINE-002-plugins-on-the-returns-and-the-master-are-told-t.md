---
id: ENGINE-002
title: Plugins on the returns and the master are told the transport of the last stretch of a piece, not of its first frame
area: ENGINE
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# ENGINE-002: Plugins on the returns and the master are told the transport of the last stretch of a piece, not of its first frame

## Where

crates/meloet-engine/src/engine.rs:2964-2969 (`set_transport` is written for every stretch; only the built-in effects' `ctx` keeps the first one, `piece_ctx`)
crates/meloet-engine/src/engine.rs:3025-3039 (the returns' inserts run after the stretch loop)
crates/meloet-engine/src/engine.rs:3064-3065 (the master's inserts run after the stretch loop)
crates/meloet-engine/src/plugin.rs:211-216 (`submit` copies `self.block.transport()` as it is at that moment)

## What happens

`Engine::render` cuts a piece into stretches (at notes, clicks, tempo points, the loop's end, arpeggio steps, mute steps …) and, for each stretch, writes "where the song is at the first frame of this stretch" into the shared `PluginBlock` (line 2965). The tracks' instruments and inserts are run inside the stretch loop and read the right value. The returns and the master are run once for the whole piece after the loop (lines 3025-3065), so a plugin effect there submits the transport that the **last** stretch left in `PluginBlock`: its `position_quarters`, `position_frames`, `bar`, `bar_start_quarters`, tempo and flags describe a frame somewhere inside the piece, while the plugin processes the piece from its first frame.

- While the song plays, the told position jumps forward by 0 to one block (up to 1024 frames) from block to block, depending on where the last cut fell. A plugin on a return or the master that follows the host position (an LFO, gate, tremolo or delay synced to the song position; LV2 plugins get `time:Position` at frame 0 of every block from this value, docs/architecture.md 21-3) wobbles in phase with it.
- In a piece that holds the loop's end, the plugin is told the loop's start for a block whose first part is still before the loop's end.
- In the piece where a count-in ends, it is told "recording" from the first frame.

The built-in effects on the same strips get the tempo of the first stretch (`piece_ctx`), so the two kinds of effect disagree.

## What should happen

docs/architecture.md 21-3: "Built-in delay: `EffectContext::tempo_bpm` = `tempo_at` at the head of the cut. Returns and the master are not split by cuts, so they get the value at the head of the render (at most 1024 frames)." and "To plugins (`PluginTransport`): the values at the head of the cut". A strip that is not cut has one cut, the whole piece, whose head is the piece's first frame; the built-in effects already get that (`piece_ctx`).

## How to trigger

Read from the code, not run.

1. Put a plugin effect that follows the song position (for example a tempo-synced tremolo or an LFO-driven filter in sync mode) on return A or on the master.
2. Play a song with notes on a track, so that most blocks are cut into several stretches.
3. The plugin is told a position that is late by a different amount in each block; its synced modulation stutters. The same plugin on a track insert runs smoothly.

## Cause

`PluginBlock` holds one transport, overwritten at the head of every stretch. `PluginSlot::submit` reads it when the plugin's block is handed over, which for the returns and the master happens only after the last stretch.

## Suggested fix

Keep the transport of the first stretch (`told` of the first iteration, next to `piece_ctx`) and write it back with `self.plugin_block.set_transport(&first)` just before the returns' inserts run (line 3025). Nothing else changes; no protocol or file change. `tests/lookahead.rs` `plugins_are_told_where_the_song_is` could be grown with a fake plugin effect on the master that records what it is told.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
