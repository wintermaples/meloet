---
id: UI-ROLL-009
title: The draw tool sounds its note or chord while the song plays, records or counts in
area: UI-ROLL
kind: ui-ux
severity: low
status: confirmed
found: 2026-11-25
---

# UI-ROLL-009: The draw tool sounds its note or chord while the song plays, records or counts in

## Where

- `ui/src/components/pianoroll/drag.ts:241` (a chord stamp) and `:250` (one note) — the draw branch of `RollDrag.down` calls `i.audition.play(…)` without asking `i.mayAudition()`.
- Compare `ui/src/components/pianoroll/drag.ts:227` and `:317` (pressing or moving a note: only `if (i.mayAudition())`), and `ui/src/components/pianoroll/audition.ts:20-21` (`notesMayAudition`: only when stopped).

## What happens

Pressing an empty place with 「書く」 sends `audition` with the note (or every note of the chosen chord) also while the song plays, records or counts in, and it sounds over the song until the button is let go. Pressing or moving an existing note does not sound at those times. While recording or counting in, core does not even write the note (edits are refused while busy), but it still sounds.

## What should happen

Spec 3, "Playing sounds in the piano roll": "… Nothing sounds when selecting with a box, on Mod A, when choosing a row in the list, or during play and recording." Only the keyboard column on the left is said to sound during recording. `RollInput.mayAudition` is documented as "Whether to play when a note is pressed or moved (only when stopped)".

## How to trigger

Read from the code, not run: play the song (Space), choose 「書く」 in the piano roll, press an empty place of the note area and hold.

## Cause

The two `audition.play` calls of the draw tool were left without the `mayAudition()` check that the other note sounds have.

## Suggested fix

Call `i.audition.play(…)` in the draw branch only when `i.mayAudition()` is true (writing the note stays as it is). No protocol change.

## Verification

Confirmed by reading (not run). `drag.ts:241` and 250 call `i.audition.play(…)` in the draw branch with no `i.mayAudition()` check; the press and move branches have it (227, 317), and `mayAudition` is `notesMayAudition(app) && !liveMode` (`Roll.svelte:230`, only when stopped). Spec 3 line 96: "Notes: a note sounds when pressed (until released; stopped at the length of the note), each time its pitch changes while dragging, and when it is moved with ↑ ↓ (for about 0.3 s). Nothing sounds when selecting with a box, on Mod A, when choosing a row in the list, or during play and recording." Only the keyboard column is said to sound during recording.

Fix plan (as suggested): in `ui/src/components/pianoroll/drag.ts` wrap the two `i.audition.play` calls of the draw branch in `if (i.mayAudition())`; check that the pitch-change audition while a drawn note is dragged (around line 340) has the same check. Test: transport playing, draw → no `audition` intent.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
