---
id: UI-ARRANGE-004
title: While playing, a page turn of the arrange moves a held clip, marker or point a whole page away from the pointer
area: UI-ARRANGE
kind: ui-ux
severity: medium
status: reported
found: 2026-11-25
---

# UI-ARRANGE-004: While playing, a page turn of the arrange moves a held clip, marker or point a whole page away from the pointer

## Where

ui/src/components/Arrange.svelte:1080-1094 (`frame()`: pages with `followScroll` whenever `follow` is on, also during a drag)
ui/src/components/arrangeDrag.ts:198 (`dragClipTo`: the move is `(x - press.x) / pxPerTick`, px from the press, not ticks under the pointer)
ui/src/components/Arrange.svelte:465 and 414 (the moving picture is placed with the current `view` plus `dragShift`)
ui/src/components/Arrange.svelte:1347-1351, 1421 (box select keeps `x0` in px of the old view)
ui/src/components/timeline/MarkerRow.svelte:67, ui/src/components/automation/autoGeom.ts:204, ui/src/components/timeline/tempoGeom.ts:255 (marker, automation point and tempo point drags: also `(x - downX) / pxPerTick`)

## What happens

Clips, markers, automation points and tempo points can be dragged while the song plays (architecture 17: moves are taken while playing). During that drag `frame()` still pages the arrange with the play line (`follow` is on from the start of play until the person scrolls sideways by hand). The drag keeps its amount as "px moved since the press", so after a page turn the held thing keeps the same tick offset while the view has moved one whole width: the clip picture (placed at `tickToX(view, first)` + `dragShift`) jumps one page to the left, usually off the screen, and the pointer is left over empty space. Releasing puts the clip where the picture is, not under the pointer. Markers, automation points and tempo points do the same. A box select started before the page turn keeps its first corner in px, so it selects clips one page away from where it was started.

The code already knows this trap for the vertical wheel: `wheelRows` refuses to scroll "during a clip drag or a box select (what is held would slip from under the pointer)" (`Arrange.svelte:1209-1211`).

## What should happen

What is held stays under the pointer (spec 3: while dragging, only the screen moves first and the original place keeps its border; the drag must follow the pointer). As for the vertical wheel, the arrange should not page while something is held, or the drag should count in ticks under the pointer.

## How to trigger

Read from the code, not run. Press Space so the song plays from the left edge of the arrange, and before the play line reaches the right edge, press a clip near the right side and keep moving it slowly. When the play line crosses the right edge, the view pages: the clip picture disappears to the left while the pointer stays where it was.

## Cause

`frame()` calls `G.followScroll` with no check for `clipDrag`, `boxSel` or the drags of the marker row, the tempo band and the lanes; the drags measure their move in px since the press.

## Suggested fix

Skip the follow paging in `frame()` while a drag or box select of the arrange is going on (`clipDrag`, `boxSel`, and a small shared flag set by `MarkerRow`, `TempoBand`, `AutoLane`, `CompBand` while they hold something), the same idea as `wheelRows`. Paging can go on at the next frame after release. The piano roll pages the same way during note drags (`ui/src/components/pianoroll/Roll.svelte:894-901`, outside this area). No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
