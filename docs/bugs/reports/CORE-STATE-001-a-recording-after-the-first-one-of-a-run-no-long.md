---
id: CORE-STATE-001
title: A recording after the first one of a run no longer writes the waiting change to the recovery file first
area: CORE-STATE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-STATE-001: A recording after the first one of a run no longer writes the waiting change to the recovery file first

## Where

crates/meloet-core/src/state/recovery.rs:208-217 (`recovery_tick`: `let started = busy && !std::mem::replace(&mut self.recovery.was_busy, busy);`)
crates/meloet-core/src/state/recovery.rs:54-55 (`was_busy`, never set back to false)

## What happens

Only the first recording (or count-in) of a run writes a change that still waits for its 2 seconds into the recovery file at once. From the second recording on, a change made less than 2 s before Record is not written when the recording starts; it waits until the recording has ended. If meloet ends in the middle of that recording (crash, SIGKILL, power cut), the next start brings back the song as it was before that change, and a vocal recording kept from `work/rec.json` is put on that older song (another tempo map, a track that is not there any more, and so on).

## What should happen

The comment in the same function: "A recording starts over a change that still waits: the song is written now. The voice recorded from here on is placed on this song, and a start after meloet ended in the middle must find it, not the song before the change." This holds for every recording, not only the first one of a run. docs/architecture.md 15 (written 2 s after a change; "Not during recording or count-in; after what was recorded is in the song") relies on this write at the start.

## How to trigger

Read from the code, not run.
1. Start meloet, set the count-in to 0, record a short take and stop (any recording; now `was_busy` is true for good).
2. Change something (for example `SetTempo`, or add a vocal track) and press Record within 2 s.
3. The effects of that Record hold no `Effect::WriteRecovery` (with the same steps as the first recording of a run, they do; see the test `nothing_is_written_while_recording_and_the_take_is_written_after`).
4. Kill meloet during the recording: the recovery file still holds the song before step 2.

The existing test `a_recording_over_a_song_that_is_written_already_writes_nothing_more` does a second recording too, but its change was written already, so it passes with the fault.

## Cause

`busy && !std::mem::replace(&mut self.recovery.was_busy, busy)`: `&&` does not run its right side when `busy` is false, so `was_busy` is only ever written while busy, and it stays `true` after the first recording ends. At every later start of a recording `replace` returns `true`, so `started` is `false`, and the early `return` in the `if busy` block skips the write.

## Suggested fix

Always update `was_busy`, for example:
`let was_busy = std::mem::replace(&mut self.recovery.was_busy, busy); let started = busy && !was_busy;`
Add a test: record once, then change and record again within 2 s, and expect a `WriteRecovery` from the second Record. No protocol or file-shape change.

## Verification

Confirmed by reading (no test run; the line decides it alone). `recovery_tick` (`state/recovery.rs:209`): `let started = busy && !std::mem::replace(&mut self.recovery.was_busy, busy);`. `&&` does not run its right side when `busy` is false, so `was_busy` is written only while busy and is never set back to false after the first recording of a run. At every later start `replace` returns `true`, `started` is false, and the `if busy { … return; }` block writes nothing. The comment in the block says the write is meant for every recording that starts over a waiting change. Severity `medium` stands (after an end in the middle of a recording, the song before the last change comes back, and the kept voice is placed on it).

Corrections: none.

Fix plan: `crates/meloet-core/src/state/recovery.rs` `recovery_tick`: `let was_busy = std::mem::replace(&mut self.recovery.was_busy, busy); let started = busy && !was_busy;`. Test in `state/recovery/tests.rs` next to `nothing_is_written_while_recording_and_the_take_is_written_after`: record and stop once, then a change and Record inside 2 s gives `Effect::WriteRecovery`.

## Fix

`crates/meloet-core/src/state/recovery.rs` `recovery_tick`: `was_busy` is now set after every input (`let was_busy = std::mem::replace(..); let started = busy && !was_busy;`), so every recording or count-in that starts over a waiting change writes it at once, not only the first one of a run.

Test: `state::recovery::tests::every_recording_over_a_waiting_change_writes_it_first` (`state/recovery/tests.rs`).
