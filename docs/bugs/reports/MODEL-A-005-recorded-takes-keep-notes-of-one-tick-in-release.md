---
id: MODEL-A-005
title: Recorded takes keep notes of one tick in release order, so an edit that changes nothing still makes a step
area: MODEL-A
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# MODEL-A-005: Recorded takes keep notes of one tick in release order, so an edit that changes nothing still makes a step

## Where

crates/meloet-model/src/record.rs:301 (`Recorder::take`: `notes.sort_by_key(|n| n.0)`, by start only; `lap.notes` is filled in the order keys are let go, record.rs:359)
crates/meloet-model/src/notes.rs:873 and 876-885 (`put_take_reaching` sorts the new notes by `(start, key)`, then calls the edit `Unchanged` only when they equal the old notes in the old order)

## What happens

A recorded take holds the notes that start on one tick in the order the keys were let go (stable sort by start only). A chord whose key events reach the PC together (one tick is 0.26 ms at 120.00; events of one USB MIDI packet are read microseconds apart) can get one tick for all of them; the arpeggio and 「さっきの演奏を残す」 go through the same `Recorder`. Then C-E-G released G, E, C is stored G, E, C.

Every note edit builds its new notes and sorts them by `(start, key)` (C, E, G), then compares them with the old notes in the take's order (G, E, C). They differ, so an edit that changes no note is not `Unchanged`: it makes a new take and an undo step that does nothing to the song. Examples: 「ベロシティをまとめて変える」 「割合」 100 %, 「次の音までのばす」 when the notes already reach the next start, typing the velocity a note already has, a velocity step up at 127. (A quantized take is already sorted, so it is only a recorded take not yet edited.) The person sees a history row (「ベロシティを割合で変える」 …) and needs one more Mod Z.

## What should happen

Architecture 13: "An action that changes nothing makes no step"; architecture 25-10: for the velocity and length tools "no change → no step". Architecture 16: "a take's notes are ordered 'start → key → earlier first' (stable sort)". Every other maker of keys takes follows that order (`write_in_new_clip` notes.rs:354, `join_clips` arrange.rs:592, `overdub_recording` record/overdub.rs:127, `put_take_reaching`).

## How to trigger

Read from the code, not run.
1. Record a chord whose notes get the same tick and are let go top key first (or any recorded take with two notes on one tick, higher key first).
2. Select the chord, 「ベロシティをまとめて変える」 → 「割合」 100 → apply.
3. A step 「ベロシティを割合で変える」 appears in the history; the notes are unchanged.

## Cause

`Recorder::take` sorts by start only, so the take breaks the `(start, key)` order the edits assume; `put_take_reaching` compares the sorted new list with the unsorted old list.

## Suggested fix

In `Recorder::take`, sort by `(start, key)` (`notes.sort_by_key(|n| (n.0, n.2))`, stable), as the other makers of takes do. Optionally also sort `live_notes` the same way so the live picture matches. No protocol or file-shape change; takes already saved keep their order and behave as now.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-model --lib verify_model_a_005`): three keys pressed on one tick and let go top first give a take with keys `[67, 64, 60]`. `put_take_reaching` (`notes.rs:873-885`) sorts the new notes by `(start, key)` and compares them with the old notes in the take's order, so for such a take an edit that changes nothing is not `Unchanged` (read, not run end to end). Severity `low` stands: it needs two keys on the same tick, and the cost is one empty history row.

Corrections: none.

Fix plan: `crates/meloet-model/src/record.rs` `Recorder::take`: `notes.sort_by_key(|n| (n.0, n.2))`. Test in `record/tests.rs` (the order of a same-tick chord). Leave `live_notes` alone unless the screen's indexes into a live take must match the final take; check that while fixing.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
