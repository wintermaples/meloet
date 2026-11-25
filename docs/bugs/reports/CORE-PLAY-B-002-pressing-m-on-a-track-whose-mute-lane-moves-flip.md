---
id: CORE-PLAY-B-002
title: Pressing M on a track whose mute lane moves flips the hidden hand mute, not what is heard
area: CORE-PLAY-B
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# CORE-PLAY-B-002: Pressing M on a track whose mute lane moves flips the hidden hand mute, not what is heard

## Where

crates/meloet-core/src/play/mix.rs:20-23 (`toggle_mute`: `edit::toggle_mute` flips `t.muted`, then `rest_with`)
crates/meloet-core/src/play/automation.rs:916-917 (the sibling `mute_pressed` takes the opposite of the lane's value)

## What happens

A track has a mute lane that moves (not resting, with points). While stopped, or playing with 「● 書く」 off, the person presses M. What sounds and what the mixer's M shows is the lane's value at the play line (`ui/src/components/mixer/MixerStrips.svelte:99-103`, `shownMuted`), not the hand-set `muted` of the song, which the person cannot see. The press flips that hidden hand value and rests the lane in one step. When the hidden value already equals what the lane gives at the play line, the press changes nothing that is heard or shown, except that the lane goes to 「休み」:

- the lane says "muted" (1.0) at the play line, the hidden hand mute is off: M is lit and the track is silent. Pressing M to unmute sets the hand mute on and rests the lane: the track stays silent, M stays lit. A second press is needed.
- the lane says "playing" (0.0), the hidden hand mute is on (the lane was drawn after the track was muted): pressing M to mute sets the hand mute off: the track keeps playing.

The status line still says the mute was moved by hand and its automation was put to rest (`StatusMessage::AutomationRested`, `msg.automationRested`), although nothing changed.

## What should happen

Spec 3 (Y16, `docs/spec/3-bands.md:204`): when a parameter that has automation is moved by hand, the lane rests and "the value stays where the hand put it". For a mute button the hand puts it to the opposite of what it shows. The sibling path in the same feature does this right: with 「● 書く」 on while playing, `mute_pressed` (`automation.rs:916-917`) reads `lane.value_at(tick)` and writes the opposite. `docs/spec/7-readings.md:445` (mute sets) also describes resting "as pressing M does" only when the mute changes.

The test `a_mute_lane_takes_the_place_of_the_mute_put_by_hand` (`play/automation/tests.rs:229-233`) asserts the present result (hand mute on, lane at 0.0, press → hand mute off, `silent` unchanged), so it would need to change with the fix.

## How to trigger

Read from the code, not run.
1. Track 1: add a mute lane, put one point at 1.0 at tick 0 (the track goes silent; M in the mixer is lit). The hand mute stays off.
2. Stopped (or playing with 「● 書く」 off), press M on track 1 in the mixer.
3. The track stays silent and M stays lit; the lane is at rest. Undo count is +1.

## Cause

`toggle_mute` builds the edit with `edit::toggle_mute(self.doc(), track)`, which flips `Track::muted`, the hand-set value, whatever the lane makes heard. `rest_with` then rests the lane, so from then on the hand value is what counts. When the hand value was already equal to the lane's value at the play line, the new hand value is the opposite of what the person asked for.

## Suggested fix

In `toggle_mute`, when the lane moves, take the target value as `mute_pressed` does: `wanted = !(lane.value_at(self.play_line()).unwrap_or(hand) >= 0.5)`, and set `muted = wanted` (a set, not a toggle) together with the rest via `rest_with` (when the hand value is already `wanted`, the edit is unchanged and `apply_resting` makes the rest step alone). Needs a small model helper to set the mute to a value (or toggle only when `t.muted != wanted`). No protocol or file change; the test above changes.
## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
