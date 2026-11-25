---
id: UI-ROLL-001
title: Piano roll toolbar take buttons switch the arrange's chosen clip, not the clip the piano roll shows
area: UI-ROLL
kind: bug
severity: medium
status: fixed
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

Confirmed by reading (not run). The tag is made from the shown clip (`Toolbar.svelte:77-81`), its buttons send `stepTake` with no clip (225, 234), and `step_take` acts on `self.selected` (`crates/meloet-core/src/play.rs:1299`); `select_clip(None)` clears `selected` and keeps the piano roll's clip (1055-1060). So after a press on an empty place of the arrange the buttons do nothing, and after a box that leaves another clip chosen they change that other clip. The buttons are also not disabled while counting in or recording elsewhere (`disabled={takeTag.recording}` only), where `step_take` returns at `busy()`; that half is the same root as UI-ROLL-008 / UI-ARRANGE-009.

A fix without a protocol change exists, so this is `confirmed`, not `needs-lead`:

Fix plan: in `ui/src/components/pianoroll/Toolbar.svelte`, when `current.state.arrange.selectedClip !== clip.id`, send `{ type: "selectClip", clip: clip.id }` before `stepTake` (both through `send`, which keeps the order; `selectClip` is not an undo step and the piano roll already shows that clip, so only the arrange's choice moves to the clip whose take is being switched, which is also what the arrange's own tag needs to show the result). Disable ◂ ▸ also while `transport.state` is `recording` or `countIn`. Add a test: shown clip ≠ chosen clip → the two intents in this order.

The other way, if the lead does not want the arrange's choice to move: give `StepTake` a clip id (protocol version change, the T key and the menu rows then send the chosen clip). Not needed for the fix.

## Fix

No protocol change.

- `ui/src/components/pianoroll/Toolbar.svelte`: ◂ ▸ send `selectClip` for the shown clip first when the arrange has another clip (or none) chosen, then `stepTake`; they are disabled while recording or counting in (`songBusy`), the rule of 「次のテイク」「前のテイク」 in the menu.
- Tests: `ui/tests/pianoroll.band.test.ts` "◂ ▸ switch the take of the shown clip: when the arrange chose another clip or none, the shown one is chosen first (UI-ROLL-001)" and "◂ ▸ cannot be used while counting in or recording, as the menu's 「次のテイク」「前のテイク」 (UI-ROLL-001)".
