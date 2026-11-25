---
id: CORE-PLAY-A-004
title: Keep what was just played: past 4096 events with one key in the phrase, the key's release is dropped and the note lasts to the phrase end
area: CORE-PLAY-A
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-PLAY-A-004: Keep what was just played: past 4096 events with one key in the phrase, the key's release is dropped and the note lasts to the phrase end

## Where

- crates/meloet-core/src/play/recent.rs:156-164 (`Phrase::push`: `self.events.remove(1)` when `notes <= 1`)

## What happens

A phrase of 「さっきのを残す」 that has one key press and then more than 4095 other events (pitch bend, modulation wheel, knobs sending CC) drops `events[1]` each time it is full. When the key was already let go, `events[1]` is its release (NoteOff, or NoteOn with velocity 0), so the release is the first thing dropped. The kept clip then has that note held until the end of the phrase (`keep_played` lets every key go at `placed.end`, recent.rs 465-471), for example a short note becomes many seconds long.

## What should happen

The comment at recent.rs 160-161 says the rule: "One key held under a long run of controller moves: the key stays, and the oldest of the moves goes." Only controller moves are meant to be dropped; the key's own release must stay so the note keeps the length it was played with (architecture 25-16: whole notes are dropped from the old side, not cut in two). The test `one_key_held_under_thousands_of_controller_moves_stays` (recent/tests.rs 590) only covers a key still held, where `events[1]` is a controller move.

## How to trigger

Read from the code, not run. Standing still: press and release C4 (two events), then move the pitch bend or modulation wheel for a few seconds (more than 4095 messages; a wheel sends one about every ms while it moves). Press 「さっきのを残す」: the clip's C4 lasts from its press to the end of the phrase. As a test: `still(0, S, on(60))`, `still(0, S + 1000, off(60))`, then 5000 `Cc { number: 1 }` items, keep, and the note length is not the 1000 µs played.

## Cause

With `notes <= 1` the code removes index 1 without looking at what it is. After the single press, index 1 is whatever came next; once the key is released, that is the release.

## Suggested fix

Drop the oldest controller move after index 0 instead of index 1 blindly: find the first `i >= 1` whose kind is `Cc` or `PitchBend` and remove it (dropping a middle controller move is what the code does today, so `head` needs no change); only when there is none fall back to `remove(1)`. No protocol, file or wording change.

## Verification

Confirmed by reading (no test run). `Phrase::push` (`play/recent.rs:152-169`): when the phrase is full and holds at most one key press, it calls `self.events.remove(1)` without looking at the event. With the press at index 0 and its release right after it, the release is the first event dropped, and `keep_played` then ends the key at the end of the phrase. The comment there says only controller moves are meant to go. Severity `low` stands (one key and more than 4095 controller moves in one phrase).

Corrections: none.

Fix plan: `crates/meloet-core/src/play/recent.rs` `Phrase::push`: in the `else` branch remove the first event at index ≥ 1 that is a `Cc` or `PitchBend`; fall back to `remove(1)` only when there is none. Test next to `one_key_held_under_thousands_of_controller_moves_stays` in `recent/tests.rs`: press, release after 1000 µs, 5000 CC moves, keep; the note keeps its played length.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
