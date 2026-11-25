---
id: UI-ARRANGE-008
title: The take switch tag's prev and next buttons have no style since the track head styles moved out of Arrange
area: UI-ARRANGE
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-008: The take switch tag's prev and next buttons have no style since the track head styles moved out of Arrange

## Where

ui/src/components/Arrange.svelte:1990 and 1996 (`<button class="tog step" …>◂</button>`, `…>▸</button>`)
ui/src/components/Arrange.svelte:2025-2395 (the style block: no `.tog` or `.step` rule)

## What happens

The ◂ and ▸ of the take switch tag (「◂ テイク 2 / 3 ▸ T」 above the chosen clip) use the classes `tog step`. Arrange.svelte has no rule for them, and Svelte scopes component styles, so the `.tog` rules in `TrackHeads.svelte` (which has no `.step` rule anyway) do not reach them, and `components/common.css` has no `.tog` or `.step`. The two buttons are drawn with the browser's own button look (its border, background, padding and system font) inside the 24 px tag.

Commit 7120b19 moved the track heads into `TrackHeads.svelte` and took the `.tog`, `.tog.step`, `.tog.step:hover` and `.tog.step:active` rules out of Arrange.svelte with them; the switcher buttons stayed in Arrange.svelte with the same class names.

## What should happen

Spec 2 "Take mark and switch": the switch tag is 「◂, 「テイク 2 / 3」, ▸, the key T」 on gray.4 with a gray.8 border, in the parts' looks (spec 2 "Parts and states": hover border gray.8 / text gray.10, pressed bg gray.2). The piano roll's copy of the same tag (`pianoroll/Toolbar.svelte:225-234`) uses `m-plain step` and its own styles.

## How to trigger

Read from the code, not run. Record two takes on a keyboard track, click the clip: the ◂ and ▸ in the tag above it look like default WebKitGTK buttons.

## Cause

The style rules for `.tog.step` were removed from Arrange.svelte when the track heads were split out (7120b19); the markup that uses them was not changed.

## Suggested fix

Give the two buttons a style in Arrange.svelte again (the old `.tog` / `.tog.step` rules), or use `m-plain` like the piano roll's tag. Check the look against mock F-Takes with `tools/design/shoot_ui.py`. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
