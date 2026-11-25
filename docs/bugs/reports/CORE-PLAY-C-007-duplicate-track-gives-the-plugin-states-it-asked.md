---
id: CORE-PLAY-C-007
title: Duplicate track gives the plugin states it asked for to the copy by slot number, not by plugin
area: CORE-PLAY-C
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-007: Duplicate track gives the plugin states it asked for to the copy by slot number, not by plugin

## Where

crates/meloet-core/src/play/plugs/copies.rs:54-72 (`ask_copy_states` keeps only the place, after checking the key once)
crates/meloet-core/src/play/plugs/copies.rs:106-132 (`copy_state_said` hands back `(place, state)`; the key the state belongs to is not kept)
crates/meloet-core/src/play/tracks.rs:109-122 (`make_copy` puts each state on whatever plugin the copy has at that place now, under that plugin's key)

## What happens

「トラックを複製」 on a track whose plugins sound first asks each of them for its state and makes the copy when all have answered (round 12, A3). The answers are matched to the copy by place only (instrument, or insert slot number). If the track's inserts were reordered (`MoveInsert`) or an insert was replaced by another plugin (`ReplaceEffect`, 「入れかえる」) after the asks went out and before the last answer came, the state of plugin A is given to the plugin B that is in that slot now: `set_plugin_state(doc, at, &key_of_B, state_of_A, ..)`. The copy's B then holds A's bytes in the song (and in the file when saved); its load gives B a state that is not its own, which the plugin host refuses (the slot then stays empty: `plugin_unreadable` → `Failed`) or which B reads as something else.

## What should happen

copies.rs doc: "the copy's plugins then have the state the song has for them" — the state of the same plugin. A plugin state belongs to its key (docs/architecture.md 20-5: "Parameters and state differ between formats"; 24-16 G: "The two formats of one plugin cannot take each other's state"). `ask_copy_states` itself checks `q.key == p.plugin.key` when it asks; the answer should be held to the same check.

## How to trigger

Read from the code, not run.
1. A track with two different plugin effects, Zam (slot 1) and LSP (slot 2), both sounding.
2. 「トラックを複製」: two `StateAsk::Copy` asks go out. Their answers come later through the plugin hosts' one thread (behind, for example, a slow `Load` of another plugin, or a plugin window being made on the process's main thread: 10–300 ms).
3. Before they come, drag LSP above Zam in the mixer (`MoveInsert`). The intents are also served back to back when both were held behind an AI batch (state/ai.rs `ai_hold`): duplicate, then the reorder, then the answers.
4. The copy is made from the song as it is now: its slot 1 is LSP, and gets Zam's state under LSP's key.

## Cause

`CopyAsk` keeps `place` and `puts` but not the plugin's key; `copy_state_said` and `make_copy` do not check that the plugin now at that place of the source track is still the one that was asked. `forget_copy` is called only for another song or a history jump, not for other steps.

## Suggested fix

Keep the key (or the ticket) in `CopyAsk` / `CopyStates`, and in `make_copy` give a state only where the copy's plugin at that place has the same key (better: find the place by the ticket's present spot, so a moved plugin still gets its own state); a state with no such plugin is dropped and the copy keeps the song's state. No protocol or file change.

## Verification

Confirmed by reading (no test run). `ask_copy_states` (`play/plugs/copies.rs:46-90`) checks the key once when it asks and keeps only `ticket`, `place`, `puts`. `copy_state_said` hands back `(place, state)`, and `make_copy` (`play/tracks.rs:109-122`) puts each state on whatever plugin the copy has at that place now, under that plugin's key. `forget_copy` runs only for another song or a history jump, so a `MoveInsert` or a replace on the source track between the asks and the last answer is not noticed. Severity `low` stands (the two intents must fall inside the wait for the answers, tens to hundreds of milliseconds).

Corrections: none.

Fix plan: `crates/meloet-core/src/play/plugs/copies.rs`: keep the plugin's key in `CopyAsk` and hand it back with the state (`CopyStates` rows of `(place, key, state)`); `crates/meloet-core/src/play/tracks.rs` `make_copy`: give a state only where the copy's plugin at that place has that key, else leave the song's state. (Following a moved plugin by its ticket is more than the fix needs.) Test in the copies tests: asks out, `MoveInsert`, answers in → no state lands on the other plugin.

## Fix

`crates/meloet-core/src/play/plugs/copies.rs`: `CopyAsk` keeps the key of the plugin asked; when the last answer comes, a state is kept only where the plugin of its ticket is still at the place it was asked at and the song's plugin there has that key. Any other state is dropped, and the copy keeps the song's state there. `play/tracks.rs` (`make_copy`) is unchanged.

Test: `play::plugs::tests::a_copy_gets_a_state_only_where_the_plugin_that_said_it_is` (asks out, `MoveInsert`, answers in).
