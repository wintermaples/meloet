---
id: UI-MIX-003
title: Right arrow does nothing at the bottom of compressor attack, delay interval and limiter release
area: UI-MIX
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-MIX-003: Right arrow does nothing at the bottom of compressor attack, delay interval and limiter release

## Where

ui/src/components/params/paramRows.ts:103-113 (`keyStep`: `valueAt(p, fractionOf(p, value) + dir / 20)`)
ui/src/components/params/paramRows.ts:13-35 (`decimals`, `roundTo`)

## What happens

On three log-scale rows, → (without Shift) at the lowest value gives back the same value, so the key does nothing: compressor 「おさえるまで」 (attack) at 0.1 ms, delay 「間隔」 (ms, sync off) at 1 ms, limiter 「もどるまで」 (release) at 1 ms. One twentieth of the bar from the bottom is ×1.41 or ×1.46 (0.141 ms, 1.41 ms, 1.46 ms), and rounding to the shown places (1, 0 and 0 decimals) brings it back to the bottom value. `change()` then sees `v === shown` and sends nothing. I checked every continuous parameter of EFFECT_PARAMS with a small script (python, the same math): these are the only three values where an arrow key does not move inside the range.

## What should happen

Spec 4 "In the parameters: move the value / move it finely" (← →, Shift ← →): each press moves the value. Every other value of every parameter moves with →.

## How to trigger

Read from the code and checked with a small python copy of fractionOf / valueAt / roundTo, not run in the app. Put a built-in limiter on an insert, select 「もどるまで」, press ← until it shows 1 ms, then press →: the value stays 1 ms. Shift → moves it to 2 ms.

## Cause

keyStep steps by a share of the bar and then rounds to the shown places; at the bottom of these ranges the step is smaller than one unit of the last shown place.

## Suggested fix

In keyStep, when the stepped value rounds back to the same value, move by one unit of the last shown place instead (as Shift does): e.g. `const next = valueAt(...); return next !== roundTo(p, value) ? next : roundTo(p, value + dir / 10 ** decimals(p));`. No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
