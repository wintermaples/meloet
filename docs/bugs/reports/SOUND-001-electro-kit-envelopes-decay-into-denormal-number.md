---
id: SOUND-001
title: Electro kit envelopes decay into denormal numbers and stay there for the rest of the hit
area: SOUND
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# SOUND-001: Electro kit envelopes decay into denormal numbers and stay there for the rest of the hit

## Where

crates/meloet-sound/src/synth/kit.rs:271-276 (`Voice::tick`: `env[0..3] *= coef[..]` with no flush)
crates/meloet-sound/src/synth/kit.rs:439 (`render`: the voice ends only on `env[0] + env[1] < SILENT`)
crates/meloet-sound/src/synth/kit.rs:125, 141, 180-185 (the short sub-envelopes: click `ring(0.0015)`, pitch `dc(0.030)` / `dc(0.012)`, cymbal hit `ring(0.06)` / `ring(0.025)`)

## What happens

On the audio thread, inside the per-sample loop of the electro kit, some sub-envelopes of a hit
(the click of kick and toms, the pitch fall of kick, toms and snare, the hit part of crash and
ride) keep being multiplied by their decay factor after they are far below anything audible.
They pass under the smallest normal f32 (1.18e-38) and then get stuck at a denormal number
that never reaches 0: once `x * coef` is less than half a step away from `x`, rounding gives `x`
back. Checked with numpy f32 at 48 kHz (computed, not run in meloet):

| Sub-envelope | coef | denormal from | stuck at |
|---|---|---|---|
| kick / tom click, `ring(0.0015)` | 0.98621 | 0.13 s | 5e-44 |
| snare pitch, `dc(0.012)` | 0.99827 | 1.05 s | 4.0e-43 |
| kick pitch, `dc(0.030)` | 0.99931 | 2.62 s | 1.0e-42 |
| ride hit, `ring(0.025)` | 0.99917 | 2.18 s | 8.4e-43 |
| crash hit, `ring(0.06)` | 0.99965 | 5.24 s | 2.0e-42 |

The voice itself lives on, because it ends only when `env[0] + env[1] < 1e-4`, and `env[0]`
rings longer: kick to 1.84 s, toms to 1.5–2.0 s, snare (velocity 127) to 1.29 s, ride to
5.1 s, crash (velocity 127) to 9.2 s, all four times longer with 「のび」 at 100 %. So for
every kick that rings past 0.13 s (on a beat of 0.5 s that is 0.37 s of each beat), every
crash that rings past 5.2 s, every ride hit that rings past 2.2 s, each sample does several
operations on denormal numbers (`env[k] *= coef[k]`, then `rng.bipolar() * env[1] * click` or
`hit * env[1]`). On CPUs that slow down on denormals this is tens to hundreds of cycles each.

## What should happen

CLAUDE.md rules: "Stop NaN where signals are summed, and set denormal small values to 0 in loops
(do not set FTZ or DAZ)". docs/architecture.md 19-2: "Each effect and instrument guards
denormals itself (values below 1e-18 in feedback and filter state become 0; envelopes stop at
−80 dB)". The sibling sound makers do it: the sampler ends every envelope at `SILENT`
(`sampler/player.rs:176, 181, 190`), the SF2 filter flushes at `TINY` (`sampler/shape.rs:339`),
the effects call `dsp::flush`.

## How to trigger

Read from the code, not run. Put the electro kit on a track, play one kick (key 36) or one
crash (key 49) and let it ring. From 0.13 s after the kick (5.2 s after the crash) every
sample of that voice works on denormal values until the voice ends. A song with a crash at
the start of each phrase or a kick on every beat does this all the time.

## Cause

`Voice::tick` multiplies the three envelopes every sample and never sets a tiny value to 0
(kit.rs:271-276). Only `env[0] + env[1]` decides when the voice ends (kit.rs:439), so a short
sub-envelope (`env[1]` of a kick, `env[2]` of a kick or snare, `env[1]` of crash and ride)
keeps decaying long after it is silent. The test `denormals_stay_finite_and_small` covers
only the effects; nothing watches the kit.

## Suggested fix

In `Voice::tick`, after the three multiplies, set each `env[k]` below about 1e-18 to 0 (the
same `flush` the effects use, `crate::effects::dsp::flush`, or a local copy). No change of
looks, wording, protocol or file shape. The sound changes only by values far under 1e-38 in
single samples; the stored hashes of the knob tests (architecture 25-27, "hashing the output of
all 31 built-in sounds") may need to be looked at in case one such tiny value was in them.
## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
