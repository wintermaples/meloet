---
id: UI-ROLL-007
title: Showing a clip of another song with the same clip id keeps the old zoom, scroll and rows
area: UI-ROLL
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-ROLL-007: Showing a clip of another song with the same clip id keeps the old zoom, scroll and rows

## Where

- `ui/src/components/pianoroll/Roll.svelte:489-503` — the fit on opening (zoom, scroll to the clip, then the rows to the notes) runs only `if (prView.shownClip !== clip.id)`.
- `ui/src/components/pianoroll/view.svelte.ts:40` — `prView.shownClip` is module state kept while the app runs; nothing clears it when another song comes (`grep shownClip` finds only these two places).
- `crates/meloet-model/src/mixer.rs:354` — `NEW_SONG_NEXT_ID = 4`: every new song gives out the same ids in the same order, and ids are per song.
- Compare `ui/src/components/pianoroll/shown.svelte.ts:283-298` and `ui/src/components/pianoroll/ghosts.ts:25-28`, which keep songs apart by `song.serial`.

## What happens

After a new song (Mod N) or another song is opened, a clip shown in the piano roll that has the same id as the clip shown last in the earlier song is treated as "already open": the zoom, the sideways scroll and the top row stay as they were in the earlier song. The notes of the new clip can then be outside the view, so the piano roll looks empty, or the clip is shown at a zoom that does not fit. Clip ids repeat between songs often, because a new song always starts its ids at 4: the first clip recorded in two new songs made the same way gets the same id.

## What should happen

Spec 3: "When the piano roll opens, it scrolls to the start of that clip (with the loop on and overlapping the clip, to the start of the loop)." Architecture 16, "The view range": "On opening, the largest step that fits the clip is used, scrolled to the clip start, and vertically to where the notes are." The take shown (`shown.svelte.ts`) and the ghost list already treat another `song.serial` as another song.

## How to trigger

Read from the code, not run.
1. New song; record a short phrase high on the keyboard; open its clip in the piano roll and scroll far to the right.
2. Mod N; record a short phrase low on the keyboard the same way (it gets the same clip id); press its clip.
3. The piano roll keeps the earlier scroll, zoom and rows; the new notes are not in view.

## Cause

The check compares only the clip id, and clip ids are not unique across songs. `song.serial` is not part of it.

## Suggested fix

Keep the song serial with the shown clip (for example `prView.shownClip = { id, serial }` and compare both), or set `prView.shownClip = null` when `song.serial` changes. No protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
