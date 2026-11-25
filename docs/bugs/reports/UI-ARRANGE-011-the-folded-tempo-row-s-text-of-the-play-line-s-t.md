---
id: UI-ARRANGE-011
title: The folded tempo row's text of the play line's tempo and meter scrolls away with the song's head
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
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

Confirmed by reading (not run). `TempoBand.svelte:362` places the folded row's text at `left: -view.scrollX` (the song's head) plus `margin-left: space.1`; `now` is the value at the play line, kept up to date by `showNow` in `Arrange.svelte`. 7-readings A-20: "「96.00 ・ 4/4」 on the folded row | The values at the play line. | spec 3". A value that follows the play line is a label of the row, and at the song's head it leaves the view after any sideways scroll of more than its own width. The mocks show it at the row's left edge (they are not scrolled, so they do not tell the two apart; the reading above does). The only test of it (`ui/tests/round6.timeline.test.ts:270`) checks the text, not the place.

Fix plan (as suggested): in `ui/src/components/timeline/TempoBand.svelte` drop `style:left` from the `.now` span (it keeps `position: absolute; top; margin-left`, so it sits at the row's left edge). Then run `tools/measure/webkit-damage.sh` once with the song playing and the row folded, since the text now changes in place while the arrange pages.

## Fix

Not committed yet (the lead commits).

- `ui/src/components/timeline/TempoBand.svelte`: the folded row's 「96.00 ・ 4/4」 has no `style:left` any more; it sits at the row's left edge (`.now` keeps `margin-left: space.1`). `tools/measure/webkit-damage.sh` was not run (it needs the real app).

Test: `ui/tests/round6.timeline.test.ts` — "the folded row's tempo and time signature at the play line stay at the row's left edge when the arrange is scrolled".
