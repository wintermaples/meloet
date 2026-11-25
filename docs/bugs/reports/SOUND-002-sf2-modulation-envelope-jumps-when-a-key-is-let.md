---
id: SOUND-002
title: SF2 modulation envelope jumps when a key is let go during its attack
area: SOUND
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# SOUND-002: SF2 modulation envelope jumps when a key is let go during its attack

## Where

crates/meloet-sound/src/sampler/shape.rs:222-227 (`Adsr::release_amount`)
crates/meloet-sound/src/sampler/shape.rs:198-208 (`Adsr::amount`, the convex attack it should continue from)

## What happens

For an SF2 zone whose modulation envelope has an attack (`attackModEnv` longer than one
frame) and moves the pitch or the filter (`modEnvToPitch`, `modEnvToFilterFc`), letting the key
go before that attack is over makes the pitch or the filter cutoff jump at once.

During the attack `amount()` is the convex curve `1 + 400/960 · log10(rise)`. On release,
`release_amount()` sets `fall = 1 - rise`, so the next `amount()` is `1 - fall = rise`, the
straight-line value. The two are not the same (computed, not run):

| rise at key up | amount before | amount after | jump with `modEnvToFilterFc` = 2400 cents |
|---|---|---|---|
| 0.10 | 0.583 | 0.10 | −1160 cents |
| 0.25 | 0.749 | 0.25 | −1198 cents |
| 0.50 | 0.875 | 0.50 | −899 cents |
| 0.75 | 0.948 | 0.75 | −475 cents |

The filter cutoff drops by about an octave inside one 32-frame chunk (the filter numbers are
moved in a straight line over that chunk, `Filter::run`), and with `modEnvToPitch` the pitch
drops by the same share of its cents in one chunk: heard as a click or a sudden darker or
lower sound at the key up.

## What should happen

The release goes on from where the envelope was. The volume envelope right beside it does so:
`release_level()` (shape.rs:214-220) turns the attack's level into the matching `fall`, with
the comment "The release goes on from the loudness the attack had reached". Architecture 20-7
says playback is to match FluidSynth; from memory (not checked in its source in this sweep)
FluidSynth also turns the convex attack value into the matching linear one at note-off, so the
verifier may want to read `fluid_voice.c` to be sure.

## How to trigger

Read from the code, not run. Load an SF2 preset that has a modulation envelope with an
attack of, say, 0.5 s and `modEnvToFilterFc` of a few thousand cents (a swept synth brass or
pad). Press a key and let it go after 0.1–0.3 s: the cutoff falls by about an octave at the
key up instead of going down smoothly from where it was.

## Cause

`release_amount` uses `1 - rise`, the value of a straight-line attack, while `amount()` gives
the convex value during the attack. The jump is `convex(rise) - rise`.

## Suggested fix

In `release_amount`, when `rising()`, set `self.fall = 1.0 - self.amount()` (taken before
`seg` changes), so the first released value equals the last attack value. Sound only; no
protocol, file shape, wording or looks change. A test like
`the_modulation_envelope_moves_the_pitch` in `crates/meloet-sound/tests/sf2.rs` with a key up in
the attack would watch it.
## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
