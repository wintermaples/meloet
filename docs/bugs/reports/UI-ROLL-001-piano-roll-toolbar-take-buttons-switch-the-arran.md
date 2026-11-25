---
id: UI-ROLL-001
title: Piano roll toolbar take buttons switch the arrange's chosen clip, not the clip the piano roll shows
area: UI-ROLL
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-ROLL-001: Piano roll toolbar take buttons switch the arrange's chosen clip, not the clip the piano roll shows

## Where

- `ui/src/components/pianoroll/Toolbar.svelte:77-81` — `takeTag` is made from `shown.clip` (the clip the piano roll shows, `arrange.pianoRollClip`).
- `ui/src/components/pianoroll/Toolbar.svelte:225` and `:234` — ◂ ▸ send `{ type: "stepTake", delta }`, which names no clip; they are disabled only by `takeTag.recording`.
- `crates/meloet-core/src/play.rs:1295-1299` — `Play::step_take` acts on `self.selected` (`arrange.selectedClip`).
- `crates/meloet-core/src/play.rs:1055-1060` — `select_clip(None)` (a press on an empty place of the arrange) clears `selected` but keeps `piano_roll`.
- `crates/meloet-protocol/src/lib.rs:2810-2816` — `selected_clip` and `piano_roll_clip` are kept apart on purpose ("kept after it is unselected").

## What happens

The take tag 「◂ テイク 2 / 3 ▸ T」 on the piano roll toolbar shows the take of the clip the piano roll shows, but its ◂ ▸ switch the take of the clip chosen in the arrange. The two are often not the same clip:
- After a press on an empty place of the arrange, no clip is chosen but the piano roll keeps its clip and its tag. ◂ ▸ then do nothing at all.
- After a box in the arrange that leaves one other clip chosen (core makes it the clicked clip, but not the shown one; see the test in `crates/meloet-core/src/play/arrange_tests.rs:289-295`), ◂ ▸ switch the take of that other clip: its sound changes and an undo step is made, while the piano roll and its tag stay as they were.
- During the count-in, or while recording on another track, ◂ ▸ look usable but core does nothing (`step_take` returns at `self.busy()`); the arrange's 「次のテイク」「前のテイク」 rows are disabled then.

## What should happen

Spec 3, "Rules for round 3": "When a clip with 2 or more takes is opened in the piano roll, the same switching tab (「◂ テイク 2 / 3 ▸ T」) is shown next to the track name on the toolbar." The tab belongs to the clip the piano roll shows, so ◂ ▸ should switch that clip's take (the arrange's own tab acts on the clip it is drawn on, `Arrange.svelte` `switcher` from `selected`). Readings A-8: next take / previous take are disabled while recording.

## How to trigger

Read from the code, not run.
1. Record two takes into clip A on track 1. Press A in the arrange: the piano roll shows A and 「◂ テイク 2 / 2 ▸ T」.
2. Press an empty place of the arrange lanes (no drag). `selectClip { clip: null }` is sent (`Arrange.svelte:1424`); the piano roll still shows A with the tag.
3. Press ▸ on the piano roll toolbar: nothing changes.
Variant: in step 2 draw a box around only clip B (2 or more takes) on track 2, then press ▸ in the piano roll: B's take changes, A does not.

## Cause

The toolbar decides what it shows from `shown.clip` but sends `StepTake`, which has no clip id; core reads `self.selected`, which is not the piano roll's clip after a press on an empty place, a box, Mod A or a box of one clip. The buttons' disabled state looks only at `takeTag.recording` (recording into this clip), not at the count-in or a recording elsewhere.

## Suggested fix

Make ◂ ▸ act on the shown clip. Either (a) give `StepTake` an optional clip id (or add a command with the clip, like `DeleteTake { clip }`) and send `shown.clip.id` from the toolbar — a protocol change; or (b) without a protocol change, disable ◂ ▸ (and drop the 「T」 hint) unless `arrange.selectedClip === shown.clip.id`, and also while counting in or recording. (b) leaves the tag unusable in the cases above. Which one is a decision for the lead.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
