---
id: UI-ARRANGE-002
title: A loop range that is not on bar lines is drawn at the nearest bar lines, and its numbers differ from the status line
area: UI-ARRANGE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-002: A loop range that is not on bar lines is drawn at the nearest bar lines, and its numbers differ from the status line

## Where

ui/src/components/arrangeGeom.ts:16 (`lineNear`: the nearest bar line)
ui/src/components/arrangeGeom.ts:296-302 (`loopBars`: both ends to the nearest bar line)
ui/src/components/Arrange.svelte:370 (`songLoop` from `loopBars`)
ui/src/components/Arrange.svelte:380 (`loopX`), 572-578 (loop lines on the clip area), 1722-1724 (「マーカー ・ ループ 6–7」), 1770-1772 (the accent loop mark on the ruler)
ui/src/components/Arrange.svelte:1267 (`startRulerDrag` grabs the rounded ends)
crates/meloet-core/src/play/timeline.rs:464-480 (`loop_from_marker`: the range is the marker ticks as they are)
crates/meloet-core/src/play.rs:757-766 (`bars`: the status line rounds outward, first bar touched to the bar line at or after the end)
(the piano roll does the same: ui/src/components/pianoroll/Roll.svelte:1015)

## What happens

The arrange turns the loop range into bar line numbers with "nearest", and draws the loop mark, the two loop lines over the clips and the heading 「マーカー ・ ループ a–b」 from those numbers. When the loop range is not on bar lines, all of these sit at the nearest bar lines, not where the loop really turns. The status line's 「ループ a–b」 comes from Rust, which rounds outward instead, so the two texts show different numbers for the same loop.

A loop that is not on bar lines is easy to make: 「ここから次のマーカーまでをループに」 uses the two markers' ticks as they are, and markers snap to beats (K at the play line, a double press on the marker row, a marker drag). It also happens when a time signature change is put in earlier: the loop keeps its ticks (architecture 21-2) while the bar lines move.

Worked example (4/4, read from the code): markers at 5.3.0000 and 7.2.0000, then 「ここから次のマーカーまでをループに」. The loop plays from bar line 5.5 to bar line 7.25. The arrange draws it from bar line 6 to bar line 7 and says 「ループ 6–7」 (5.5 is a tie, so the later line; 7.25 goes to 7). The status line says 「ループ 5–8」. Grabbing an end of the drawn mark and dragging it also moves the other end to its rounded bar line on release (`setLoop` with the rounded `fixed` bar).

## What should happen

What is drawn should be where the loop turns, and the two texts should agree. Spec 3 (ruler rules): "The loop range stays in whole bars"; spec 3 (Y18) offers 「ここから次のマーカーまでをループに」. Either the range made from markers is put on whole bars (then the drawing is right), or the screen draws the range at its real ticks and makes the numbers with the same rule as Rust's `bars`.

## How to trigger

Read from the code, not run. Stop at a place inside bar 5 and press K, move to a place inside bar 7 and press K, right-click the first marker, choose 「ここから次のマーカーまでをループに」, press Space: the loop turns at the markers, while the loop mark and lines are at the nearest bar lines and the heading and the status line show different numbers.

## Cause

`loopBars` rounds each end to the nearest bar line (`lineNear`), while `loop_from_marker` (and an earlier time signature change) gives ranges that are not on bar lines, and Rust's `bars` rounds outward. Three different readings of the same `TickRange`.

## Suggested fix

Needs a decision by the lead: (a) make `loop_from_marker` put the range on whole bars (the start bar's head and the bar line at or after the end), as spec 3 says the loop range stays in whole bars; or (b) keep the ticks and change the screen to draw the loop mark and lines at `loopRange.start` / `loopRange.end`, with the heading numbers made by the same rule as `Play::bars`. No protocol or wording change in either.

## Verification

Confirmed by reading (not run). `loopBars` (`arrangeGeom.ts:296-302`) rounds both ends to the nearest bar line, `loop_from_marker` (`play/timeline.rs:464-480`) keeps the marker ticks, and Rust's `bars` (`play.rs:757-766`) rounds outward; the worked example is right (5.5 → line 6 by the tie rule, 7.25 → line 7; Rust says 5–8).

Which side to follow is decided by the documents, so this is not `needs-lead`:
- architecture 21-2: "Notes, clips, automation points, markers, tempo points and the loop range **keep their ticks**" when a time signature change is added earlier. So a loop range that is not on bar lines exists whatever `loop_from_marker` does, and the screen has to draw it right.
- spec 3 (Y18): 「ここから次のマーカーまでをループに」 "(for the last marker, up to the end of the song)", and markers snap to beats, not bars: the range is meant to end at the marker. The doc comment of Rust's `bars` ("The bars the range touches … its first bar, and the bar line at its end or after it") shows Rust already expects such ranges.
- spec 3 line 52 / 92 ("Dragging on the ruler sets the loop range in whole bars", "The loop range stays in whole bars") is about what a ruler drag makes; the ruler drag still makes whole bars.

So choice (b): Rust stays, the screen changes.

Fix plan: in `ui/src/components/arrangeGeom.ts` give the loop two readings: the place to draw (`range.start` / `range.end` in ticks, as they are) and the numbers for the heading by the rule of `Play::bars` (the bar that holds the start, and the bar line at or after the end; the screen's tempo map has what is needed). In `ui/src/components/Arrange.svelte` use the ticks for `loopX`, the two loop lines and the accent mark (370, 380, 572-578, 1770-1772) and the outward numbers for 「マーカー ・ ループ a–b」 (1722-1724); the same in `ui/src/components/pianoroll/Roll.svelte:1015`. `startRulerDrag` (1267): when an end is grabbed, the end that stays keeps its tick; `SetLoop` takes bar lines, so check in `crates/meloet-protocol` whether the fixed end can be sent as it is; if it cannot, the fixed end goes outward to its bar line on release (say so in the fix), not to the nearest one. Add a case to `ui/tests/arrange.test.ts` (loop 5.3–7.2 in 4/4: drawn at the ticks, heading 5–8). No protocol or wording change expected.

## Fix

Screen only (Rust stays). `ui/src/components/arrangeGeom.ts`: `loopBars` now gives a `LoopRange`: the numbers by the rule of core's `Play::bars` (the bar that holds the start, the bar line at the end or after it) and the ticks `start` / `end` as they are; `barLoop` gives a ruler drag's bar lines with their ticks; `startRulerDrag` measures the grab at the drawn ticks, and the end that stays is its outward bar line number (`SetLoop` takes bar lines, so a range off the bar lines becomes whole bars, outward, when one end is dragged). `ui/src/components/Arrange.svelte`: the loop mark on the ruler and the two loop lines over the clips are drawn at the ticks; 「マーカー ・ ループ a–b」 uses the outward numbers (the same as the status line). `ui/src/components/pianoroll/Roll.svelte` (3 lines: `shownLoop`, `loopX`, the loop mark): the piano roll's ruler draws at the ticks too.
Tests: `ui/tests/arrange.geom.test.ts` "a loop range off the bar lines (markers 5.3 and 7.2 in 4/4) …" (new; the old "loop range (ticks) → bar line numbers" now also expects `start` / `end`), `ui/tests/arrange.test.ts` "a loop range off the bar lines (5.3–7.2): the mark at its ticks, 「ループ 5–8」 …".
