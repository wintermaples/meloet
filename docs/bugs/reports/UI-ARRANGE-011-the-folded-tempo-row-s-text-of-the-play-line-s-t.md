---
id: UI-ARRANGE-011
title: The folded tempo row's text of the play line's tempo and meter scrolls away with the song's head
area: UI-ARRANGE
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-011: The folded tempo row's text of the play line's tempo and meter scrolls away with the song's head

## Where

ui/src/components/timeline/TempoBand.svelte:362 (`<span class="m-num m-lbl now" style:left="{-view.scrollX}px" …>{now}</span>`)
ui/src/components/Arrange.svelte:1096-1119 (`now`: the tempo and meter at the play line, rewritten up to 10 times a second while playing)

## What happens

When the 「テンポ ・ 拍子の変更」 row is folded, its time side shows 「96.00 ・ 4/4」, the tempo and time signature where the play line is (Arrange keeps it up to date while playing, `showNow`). The text is placed at `left: -view.scrollX`, that is at the song's head, so it moves left with every sideways scroll and is out of view as soon as the arrange is scrolled more than its width (about 80 px). While playing with the view following the play line, it is out of view after the first page turn, which is exactly when the value changes.

## What should happen

7-readings (round 6 tempo band table): 「96.00 ・ 4/4」 on the folded row shows "the values at the play line". Mock F6-AutoRest (and the other mocks with this row, e.g. F-History) put it at `left: 4px` of the row, as a label of the row, not as a mark at a song position. A value that follows the play line has no place in the song to stick to.

## How to trigger

Read from the code, not run. With the band folded, scroll the arrange sideways by a few bars (Shift + wheel), or play past the first page: the 「96.00 ・ 4/4」 text is gone from the row.

## Cause

The text is positioned with the song's head (`-view.scrollX`) instead of the row's left edge.

## Suggested fix

Place it at the row's left edge (drop the `style:left`, keep `.now`'s `margin-left: space.1`), as in the mocks. Check that the WebKit damage stays small (`tools/measure/webkit-damage.sh`; the text changes at most 10 times a second). No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
