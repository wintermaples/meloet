---
id: MODEL-A-004
title: After close_laps, a late key or pedal of the ended pass goes back into that pass when the next pass has no event yet
area: MODEL-A
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-A-004: After close_laps, a late key or pedal of the ended pass goes back into that pass when the next pass has no event yet

## Where

crates/meloet-model/src/record.rs:249-281 (`close_laps` ends the passes but never opens the next one)
crates/meloet-model/src/record.rs:136-140 (a key is held in `self.laps.len() - 1`, the last pass the recorder has)
crates/meloet-model/src/record.rs:370-379 (`control`: a late controller move goes to `last`)
crates/meloet-model/src/record.rs:335-349 (`enter_lap` is the only place a pass is added, and only `push` and `finish` call it)

## What happens

With a loop and 「重ねる」, the core calls `Recorder::close_laps(upto)` when the heard position enters the next pass (`Play::pass_over`, crates/meloet-core/src/play.rs:1857). The recorder adds a pass only when an event of that pass comes in. At the moment of the close, usually no key of the new pass has come yet (a person's keys are stamped with the heard position), so the last pass in `laps` is the one just closed.

A key pressed in the last moment of the closed pass whose event reaches the core after that close (the engine reports it a block later) then goes into the closed pass, not into "the pass that is going on":
- A late key: the closed pass gets notes again, and the next `close_laps` gives it as a take of its own. With 「重ねる」 each take is its own step, so the person gets an extra 「録音」 step holding that one note (or an extra take in the clip, when 「重ねる」 was just turned on and `add_recording` takes the passes).
- A late pedal up (or bend, modulation) with no late note: it goes into the closed pass's `controls`; that pass has no notes, so the next `close_laps` gives no take and clears it. The pedal-up point is lost from the song (the pass-0 take has no pedal up in its last moments). The next pass still starts with the pedal up, because the recorder's held values take the move.

When some event of the new pass came before the close, the same late key goes into the new pass, as the test `closed_passes_are_given_once_and_late_keys_go_on` expects. So the result depends on whether the person happened to play in the first 15 ms of the new pass.

## What should happen

The doc of `close_laps`: "Keys that come late for an ended pass go into the pass that is going on." The pedal test `a_pedal_of_a_pass_goes_there_though_notes_of_the_next_pass_came_first` ("Once the pass was ended, what comes late for it goes into the pass going on") expects the same for controller moves.

## How to trigger

Read from the code, not run. In the model:
```
let mut r = Recorder::new(BAR, Some(bars_2_3), HeldControls::default());
r.push(on(B + 100, 0, 60)); r.push(off(B + 200, 0, 60));
r.close_laps(0);                     // laps = [pass 0], all closed
r.push(on(2 * B - 20, 0, 65));       // late key of pass 0
r.push(off(2 * B - 10, 0, 65));
r.push(on(B + 10, 1, 64)); r.push(off(B + 60, 1, 64));
r.close_laps(1)                      // gives 2 takes: [65 alone], [64]; expected 1 take [64, 65]
```
In the app: loop recording with 「重ねる」 on, press a key a few ms before the loop end, play nothing else in the first moment of the next pass.

## Cause

`close_laps` clears the closed passes but leaves them as the last entries of `laps`. `push` puts every key into `self.laps.len() - 1`, and `control` puts a move whose pass is closed into `last`, without checking that this pass is still open (`number >= open_from`).

## Suggested fix

At the end of `close_laps`, when the last pass in `laps` has `number <= upto`, call `self.enter_lap(upto + 1)` so that a pass going on exists (it then starts with the held controllers, as any new pass). Then late keys and moves land in it as documented. No protocol or file-shape change. Add the case above to the recorder tests.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-model --lib verify_model_a_004`): the sequence of "How to trigger" gives two takes at `close_laps(1)`: `[(7660, 10, 65)]` and `[(10, 50, 64)]`, where the doc of `close_laps` and the test `closed_passes_are_given_once_and_late_keys_go_on` promise one take holding both. The core calls `close_laps` from `Play::pass_over` (`play.rs:1844`) as soon as a heard position of the next pass comes, so the order is reachable.

Correction: the pedal half is smaller than written. A late pedal-up of the closed pass is dropped from that pass, but `HeldControls` still takes it, so the next pass does not start with the pedal down (my test `verify_model_a_004_pedal` passed: the head of pass 1 has no `CC 64 = 127`). What is lost is only the pedal-up point in the last moments of the closed pass's take. Severity `low` stands.

Fix plan: in `crates/meloet-model/src/record.rs` `close_laps`, after the loop, when the last entry of `laps` has `number <= upto`, call `self.enter_lap(upto + 1)`. Add the sequence above to `record/tests.rs`. Check `finish` afterwards: an empty pass that was entered this way must not give an empty take or count as a pass with notes.

## Fix

Keys half fixed, controller half left as it is. `crates/meloet-model/src/record.rs` `Recorder::push`: a key pressed late for an ended pass first enters the pass going on (`enter_lap(open_from)`), so it goes there also when nothing of that pass came before the close. The verifier's plan (`enter_lap(upto + 1)` at the end of `close_laps`) was not used: it writes the next pass's head while a pedal-up of the ended pass may still be on its way, so a late pedal-up would leave the whole next pass with the pedal down (Fable agreed). Controller moves are as before: with no pass going on yet, a late move stays with the ended pass and its point is lost, while the next pass starts from the moved value; with a pass going on, the move lands there at its tick and that pass's head keeps the old value (asserted as meant in `a_pedal_of_a_pass_goes_there_though_notes_of_the_next_pass_came_first`). Which of these is right is left to the lead (`docs/bugs/later/FIX-fix-model-a.md`).

Test: `record::tests::late_keys_go_on_when_nothing_of_the_next_pass_came_first` (meloet-model; also checks that a late pedal-up does not leave the next pass with the pedal down at its head).
