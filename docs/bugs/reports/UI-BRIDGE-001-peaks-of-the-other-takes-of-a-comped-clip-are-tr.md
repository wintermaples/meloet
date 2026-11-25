---
id: UI-BRIDGE-001
title: Peaks of the other takes of a comped clip are treated as unused and can be let go of while shown
area: UI-BRIDGE
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-BRIDGE-001: Peaks of the other takes of a comped clip are treated as unused and can be let go of while shown

## Where

ui/src/bridge/peaks.ts:220-227 (`songAudioIds`: only `c.audio.audio`, the file of the shown take)
ui/src/bridge/index.ts:150 (`peakStore.keepOnly(songAudioIds(...))` on every change of the `song` section)
ui/src/bridge/peaks.ts:186-201 (`PeakStore.keepOnly`: what is not in use goes to `dropped`, kept up to 4 MB, oldest let go of first)

## What happens

A vocal clip with 「いいとこ取り」 (round 12 A8) plays, and the arrange draws, a piece of each chosen take, each from its own audio file (`waveParts` in ui/src/components/arrange/wave.ts:82-99 uses `take.audio` of `AudioClipView.takes`). The comp band draws every take of the clip (ui/src/components/comp/CompBand.svelte:65, `b.take.audio`). None of these files except the shown take's is in `songAudioIds`, so at every change of the `song` section their peaks are moved from `ready` to `dropped`, as if no clip used them.

`peek` still finds them in `dropped`, and `WaveBook.sourceFor` only reads with `peek` (it does not revive them), so they stay in `dropped` and grow old there. When the levels in `dropped` pass `KEEP_DROPPED_PEAK_BYTES` (4 MB), the oldest are let go of first, and those are exactly the comp takes that are on the screen. Then `WaveBook.trim` drops their entries, the pieces of the wave of the comped clip and the rows of the comp band are drawn without a wave until the whole level has been fetched again, and the next song change moves them to `dropped` again.

## What should happen

`songAudioIds` says it gives "every audio file the song's clips play"; a comped clip plays the files of its comp pieces, and the comp band shows every take of the clip. These files are in use and should stay in `ready`, the same as the shown take's file. Only files no clip uses should go to `dropped` (the comment at peaks.ts:91 and the take rule of `keepOnly` in take.ts: "takes in use and the 16 newest").

## How to trigger

Read from the code, not run.
1. Record a vocal clip with many long takes (for example 8 takes, `COMP_ROWS_MAX`, of 5 minutes, stereo, 48 kHz) and make a comp of them (「いいとこ取り」), open the comp band.
2. Zoom the arrange in and out so that the fine peak levels are fetched for every take (`waveLevel` picks level 0 below about 340 frames a px, level 1 below about 1365). Level 0 is 48000 × 60 / 256 peaks × 2 channels × 4 bytes = 90 KB a minute, so the 40 minutes of takes are 3.6 MB at level 0 and 0.9 MB at level 1: together over 4 MB, before any other file that really went out of use.
3. Make any change to the song (move a clip, change a volume): each change runs `keepOnly`. Once `dropped` is over 4 MB, the oldest comp takes are let go of, and their waves disappear from the comped clip and the comp band until they are fetched again.

## Cause

`songAudioIds(song, recording)` collects `c.audio.audio` only. `AudioClipView.takes[].audio` (every take of the clip, which the comp pieces and the comp band read) is not collected, so `PeakStore.keepOnly` takes those files for unused.

## Suggested fix

In `songAudioIds`, also collect the `audio` of every entry of `c.audio.takes` (it holds the shown take too). No protocol, file-shape, wording or look change. A test in ui/tests/bridge.peaks.test.ts with a clip whose `audio.takes` names other files would watch it.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
