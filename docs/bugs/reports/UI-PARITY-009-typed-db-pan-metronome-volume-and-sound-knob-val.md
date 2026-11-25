---
id: UI-PARITY-009
title: Typed dB, pan, metronome volume and sound knob values do not read full-width digits, unlike the other number boxes
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-009: Typed dB, pan, metronome volume and sound knob values do not read full-width digits, unlike the other number boxes

## Where

ui/src/components/mixer/scale.ts:63-68 (`parseDb`: only `−` and `ー` are mapped; `\d` is ASCII only)
ui/src/components/mixer/scale.ts:71-78 (`parsePan`)
ui/src/components/metronome.ts:59-63 (`parseClickDb`)
ui/src/components/params/knobRows.ts:41-45 (`typedKnob`: `Number("３５")` is NaN)
Users of these: mixer/control.svelte.ts:172 (fader), mixer/SendBar.svelte:22, mixer/PanKnob.svelte:22, TrackHeads.svelte:49 (track head dB), params/ClipPanel.svelte:124 and params/ClipsPanel.svelte:45 (clip gain), MetronomeWindow.svelte:155, params/ParamRow.svelte:177 (the four sound knobs).

## What happens

With the Japanese input on, digits come in full width. 「－３」 typed into a fader, a send or the track head dB, 「Ｌ２０」 or 「２０」 into the pan, 「－１２」 into the metronome volume, 「－２」 into a clip's 「ゲイン」, and 「３５」 into a sound knob (「明るさ」 …) cannot be read: the mixer and metronome boxes stay open, the clip gain and the knob send nothing and say nothing. The same digits typed in the transport's tempo, the position, the note and CC lists, the clip's position, length, fade, pitch and 「音程を寄せる」, the latency box and a built-in effect's value are read.

## What should happen

The other number boxes turn the text to ASCII first: valueClamp.ts:23 and time.ts:64 (`normalize("NFKC")`), pianoroll/noteFields.ts:27, params/clipFields.ts:23 (the clip panel's own other fields), SettingsDialog.svelte:79, clipEdits.ts:42 (the repeat count), and Rust's `ParamSpec::read` for built-in effects (architecture 20-13: "full-width digits and 「−」 too"). 7-readings writes the same rule for the position box ("full-width characters are fine"), the repeat count ("full-width digits read") and track numbers ("Full-width digits are read as digits ... people type with the Japanese input on"). In the clip panel the gain is the only field of its group that does not read them.

## How to trigger

Read from the code, not run.
1. Turn the Japanese input on (full-width digits).
2. Double-click a fader's dB text in the mixer, type -3: the box holds 「－３」.
3. Enter: `parseDb` keeps 「－３」 (U+FF0D and U+FF13 are not mapped), the pattern `^[+-]?(\d+...)$` fails, `undefined`; the box stays open.
4. In the params panel of a built-in sound, double-click 「明るさ」's value, type 35 (「３５」), Enter: `typedKnob` gives null, nothing is sent and nothing is said; the same text in a reverb's 「ミックス」 row is read by Rust.

## Cause

These four parsers were written with their own small character maps and never call `normalize("NFKC")`, unlike the shared `tidy` helpers of the other fields.

## Suggested fix

Start each of `parseDb`, `parsePan`, `parseClickDb` and `typedKnob` with `text.normalize("NFKC")` (as `clipFields.ts` `tidy` does; keep the `−` mapping after it, since NFKC leaves U+2212 as it is). No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
