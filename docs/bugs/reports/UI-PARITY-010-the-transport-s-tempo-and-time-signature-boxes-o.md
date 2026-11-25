---
id: UI-PARITY-010
title: The transport's tempo and time signature boxes open and take Enter while counting in or recording, while the tempo band's boxes do not
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-010: The transport's tempo and time signature boxes open and take Enter while counting in or recording, while the tempo band's boxes do not

## Where

ui/src/components/Transport.svelte:446-470 (the tempo and time signature `NumberField`s get no busy check)
ui/src/components/Transport.svelte:294-297 (`commitClamped` sends `ClampValue` after the set command, whatever Rust does with it)
ui/src/components/NumberField.svelte:89-98 (the reading is always `m-typable` and always opens)
The sibling that does it right: ui/src/components/timeline/TempoBand.svelte:281-287 and :333-340 (`class:m-typable={!busy}`, `if (!busy) editing = ...`; its own comment at :29 "While recording or during count-in: only chooses (the core accepts only that)")
Rust side: crates/meloet-core/src/play/timeline.rs:192-195 and :222-224 (`set_tempo` / `set_meter` return at once when `busy()`)

## What happens

While counting in or recording, the tempo and time signature numbers in the transport still light up on hover as a place to type (`m-typable`), open the input box when pressed, and take Enter. Rust drops the `SetTempo` / `SetMeter`, so nothing changes. If the typed value was out of range (「400」), the screen also sends `ClampValue` and the status line says the tempo was set to 300.00, though nothing was set. The same values in the tempo band (a tempo point's number, a time signature flag) do not look typable and do not open at that time.

## What should happen

The same value should behave the same in both places: not typable while counting in or recording, as TempoBand.svelte does and as Rust enforces. 7-readings line 77: the typable look is "Not while the number cannot be typed".

## How to trigger

Read from the code, not run.
1. Press ● to record (or be in the count-in).
2. Press the tempo number in the transport: the box opens.
3. Type 400 and Enter: `readTempo` clamps to 300 and `commitClamped` sends `setTempo` (dropped by `set_tempo` because `busy()`) and `clampValue` (Rust shows `ValueClamped`, the "set to the nearest end" text of spec 6).
4. Compare: double-click a tempo point's number in the tempo band during the recording: nothing opens.

## Cause

Transport passes no "busy" state to its NumberFields; NumberField has no way to be shown as not typable.

## Suggested fix

Give NumberField a `disabled` prop (no `m-typable`, `open` does nothing; the reading looks as now) and pass `recordingNow` for the tempo and the time signature in Transport.svelte, as TempoBand does with `busy`. No protocol or wording change; the reading's look while busy is that of a plain readout (what TempoBand shows).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
