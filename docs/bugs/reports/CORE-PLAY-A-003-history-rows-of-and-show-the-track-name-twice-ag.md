---
id: CORE-PLAY-A-003
title: History rows of 「アルペジオを変える」 and 「音のつまみを変える」 show the track name twice, against spec 3 (94-6)
area: CORE-PLAY-A
kind: ui-ux
severity: low
status: fixed
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

Confirmed by reading (no test run). `play/history.rs:1136-1160` gives `StepWhat::Track` (SetArp) and `StepWhat::SoundKnob` with the place `on_track(track)`; `rowWhatText` (`ui/src/components/history/stepText.ts:199-203`) drops the track only for `soundReplaced`, so both rows name the track in 「（何を）」 and again in the right column. Spec 3 (94-6): "for steps whose row shows the track name in the right column, 「（何を）」 does not include the track name"; `7-readings.md` line 396 keeps the name only for rows whose right column is 「アレンジ」 or 「ミキサー」. Severity `low` stands.

On the wording: no new text key is needed, so this is `confirmed`, not `needs-lead`. 「アルペジオを変える」 with the track as its place becomes the bare row (`history.rowBare`). The knob row without the track is 「明るさ 50 → 25 %」, which existing keys already say: `history.changeAt` (`{place} {from} → {to}`, spec:3) with the knob's name as `place`, `from` the number, and `to` through `param.value.percent` (`{value} %`, spec:3). Both keys exist in `ja.ts` and `en.ts`.

Fix plan (screen only): `ui/src/components/history/stepText.ts` `rowWhatText`: when `row.place?.kind === "track"`: for `what.kind === "track"` return `null` (bare row); for `what.kind === "soundKnob"` return the `history.changeAt` form above. The status line keeps `stepWhatText` with the track. `ui/src/bridge/fakeWave3.ts:27` follows. Tests in the history text tests next to the `soundReplaced` case.

## Fix

`ui/src/components/history/stepText.ts` `rowWhatText`: with the track as the place, a `track` 「（何を）」 of the same track gives the bare row, and `soundKnob` gives `history.changeAt` with the knob name and `param.value.percent` (「明るさ 50 → 25 %」). The status line keeps `stepWhatText`. The fake (`fakeWave3.ts`) gives the same row as Rust and needed no change. Test: `ui/tests/fix-ui-values.test.ts` "CORE-PLAY-A-003: rows whose right column is the track do not name it again".
