---
id: CORE-PLAY-A-003
title: History rows of 「アルペジオを変える」 and 「音のつまみを変える」 show the track name twice, against spec 3 (94-6)
area: CORE-PLAY-A
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# CORE-PLAY-A-003: History rows of 「アルペジオを変える」 and 「音のつまみを変える」 show the track name twice, against spec 3 (94-6)

## Where

- crates/meloet-core/src/play/history.rs:1136-1142 (`StepKind::SetArp`: what `StepWhat::Track { track }`, place `on_track(track)`)
- crates/meloet-core/src/play/history.rs:1143-1160 (`StepKind::SetSoundKnob`: what `StepWhat::SoundKnob { track, .. }`, place `on_track(track)`)
- ui/src/components/history/stepText.ts:199-203 (`rowWhatText` drops the track only for `soundReplaced`)
- ui/src/bridge/fakeWave3.ts:27 (the fake copies the same row)

## What happens

In the history band, a 「アルペジオを変える」 row reads 「「アルペジオを変える」・ 1 ピアノ」 with 「1 ピアノ」 again in the right column, and a 「音のつまみを変える」 row reads 「「音のつまみを変える」・ 1 ピアノ ・ 明るさ 50 → 25 %」 with 「1 ピアノ」 again in the right column.

## What should happen

Spec 3 (94-6, 3-bands.md line 272): "for steps whose row shows the track name in the right column, 「（何を）」 does not include the track name." Spec 7 readings line 396 applies it to 「音源を入れかえる」 too (only the window row keeps its words twice, by the owner's choice). The sibling `soundReplaced` row already follows it: `rowWhatText` writes only 「グランドピアノ → オルガン」 when the place is the track.

## How to trigger

Read from the code, not run. Change the arpeggio of track 1 (「アルペジオ ▾」), or drag a knob of a built-in sound; open the history band and look at the newest row.

## Cause

Both step kinds put the track into 「（何を）」 and also name the track as the place. `rowWhatText` (stepText.ts 201) removes the track only for `what.kind === "soundReplaced"`; `track` (SetArp) and `soundKnob` pass through `stepWhatText` with the track label.

## Suggested fix

Screen side, as `soundReplaced` is done: in `rowWhatText`, when `row.place?.kind === "track"` and `row.what.kind === "track"` (the same track), give `history.rowBare`; for `soundKnob`, a form without the track (「明るさ 50 → 25 %」). The status line can keep the track (it has no right column). The `soundKnob` form without the track needs a wording key in `ja.ts` / `en.ts` (a lead decision, or reuse of an existing key); the SetArp half needs none. Alternatively Rust gives `StepWhat::None` for `SetArp`, but then the undo status line loses the track name too.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
