---
id: UI-ARRANGE-002
title: A loop range that is not on bar lines is drawn at the nearest bar lines, and its numbers differ from the status line
area: UI-ARRANGE
kind: bug
severity: medium
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
