---
id: UI-TEXT-003
title: English plural pair picks the plural for a signed -1 written with the minus sign
area: UI-TEXT
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-TEXT-003: English plural pair picks the plural for a signed -1 written with the minus sign

## Where

ui/src/text/index.ts:55 (`english()`: `Number(values?.[entry.by]) === 1`)
ui/src/components/pianoroll/drag.ts:408 (`pr.readMove` with `keys: signed(d.keys)`)
ui/src/components/arrangeDrag.ts:293 (`arrange.readMove` with `delta: barsDeltaText(...)`)
ui/src/components/pianoroll/geom.ts:441 (`signed` writes U+2212 "−")

## What happens

In English, moving notes one semitone down reads "+0.0000 · −1 semitones", and moving clips one bar left reads "… (−1 bars)". Moving up or right reads "+1 semitone" / "+1 bar" correctly, so the two directions disagree.

## What should happen

spec 7-readings, D4 "Numbers": a text whose noun follows a number has a form for 1; −1 is one semitone / one bar, as +1 is.

## How to trigger

Read from the code, not run. English page, drag a note down by one row (or press the down key with the drag reading shown), or drag a clip one bar to the left. `signed(-1)` is "−1" with U+2212; `Number("−1")` is NaN, so `english()` picks `other`. `Number("+1")` is 1, so the other direction is fine.

## Cause

`english()` reads the hole's value as a JavaScript number; the screen writes signed values with the typographic minus U+2212 (spec style), which `Number` does not read. Also, a plain "-1" would give -1, which is not `=== 1`.

## Suggested fix

In `english()`, read the value after replacing U+2212 with "-" and compare its absolute value with 1 (`Math.abs(Number(String(v).replace("−", "-"))) === 1`). One line in ui/src/text/index.ts; no wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
