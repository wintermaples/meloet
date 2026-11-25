---
id: UI-ARRANGE-012
title: The drop frame for a new vocal track is drawn right under the last track's row, over its lanes and bands
area: UI-ARRANGE
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-012: The drop frame for a new vocal track is drawn right under the last track's row, over its lanes and bands

## Where

ui/src/components/arrange/audioClipGeom.ts:179-183 (`dropFrameBox`: `top = rowTop(d, rows - 1) + rowHeight(d, rows - 1)` for a new track)
ui/src/components/arrange/DropFrame.svelte:40 (`dropFrameBox(view, dims, target, tracks.length, 2 * perBar)`)

## What happens

While audio files are dragged in and the drop would make a new vocal track (over a keyboard track, over an empty place, or 「別々のトラックに」), the gray.7 frame is put "just below the last row" by taking the bottom of the last track's **row**. When the last track has open automation lanes and its 「＋ 帯を足す ▾」 row, or the open 「いいとこ取り」 band, the frame is drawn on top of those lanes and bands, not below them where the new track will appear (a new vocal track is added at the end: `add_vocal_track(&new, None, …)` in `crates/meloet-model/src/audio/clips.rs:236`).

## What should happen

Spec 3 (29-A): 「持ってきている間、入る所に gray.7 の枠」 — the frame shows where the file will go. The comment of `dropFrameBox` says "A new track … is just below the last row". The bottom of all track blocks is `rows.tracksHeight` from `arrangeRows` (`automation/autoGeom.ts:96`).

## How to trigger

Read from the code, not run. Give the last track a volume lane (open), then drag a .wav file from the file manager over a keyboard track: the frame is drawn over the last track's volume lane.

## Cause

`dropFrameBox` only knows row tops and heights (`LaneDims`), not the height of the lanes, bands and the master row under the rows.

## Suggested fix

Pass the bottom of the track blocks (`rows.tracksHeight`, or `rows.bottoms[rows.length - 1]`) to `dropFrameBox` and use it as `top` for a new track. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
