---
id: UI-ARRANGE-003
title: Track name box: any change of the song section while typing puts the old name back and selects it
area: UI-ARRANGE
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-ARRANGE-003: Track name box: any change of the song section while typing puts the old name back and selects it

## Where

ui/src/components/TrackHeads.svelte:68-81 (the `$effect` that fills `draft` and focuses the box)

## What happens

The `$effect` that opens the track name box reads `trackEdit.renaming` and also `song.tracks` (to find the track), so it runs again every time the `song` section is replaced while the box is open. Each run sets `draft = track.name` again and calls `focus()` and `select()`. What the person has typed so far is thrown away, the old name is back in the box, and it is all selected, so the next key replaces it.

The `song` section is sent again without the person editing the song in several normal cases, for example:

- Mod S while the box is open (Mod keys work while typing, `WITH_MOD` in `ui/src/keys/attach.ts:387-399`): the save turns `song.file` from 「changed」 to 「saved」;
- a plugin instrument or SF2 kit finishing its load and naming its keys (`TrackView.drumKeys` / `keyNames`), the sound pack arriving (`standIn` cleared): typical right after adding a track and going to name it;
- an edit made by the AI link.

## What should happen

Text being typed stays until Enter, Esc or a press outside (spec 3 / 7-readings "Track name input box": decided with Enter and when pressing outside, Esc stops). The sibling boxes already guard this: the clip name box (`Arrange.svelte:716-734`, `nameFor`), the marker name box (`timeline/MarkerRow.svelte:135-147`, `openFor`) and the repeat box (`Arrange.svelte:1489-1501`, `repeatFor`) fill their draft only when the box opens.

## How to trigger

Read from the code, not run. Double click a track name, type 「ベース」 (do not press Enter), press Ctrl S. The box shows the old name again, selected; pressing Enter now keeps the old name.

## Cause

The effect has `song` as a dependency and no "only when `renaming` changes" guard, so `draft = track.name` and `select()` run on every new `song` object.

## Suggested fix

Guard it like the clip name box: remember the id the draft was made for (`let draftFor: number | null = null;`), return early when `trackEdit.renaming === draftFor`, and read the track inside `untrack` (or only on the first run for that id). Keep the existing "track gone → close" rule. No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
