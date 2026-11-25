---
id: UI-PARITY-010
title: The transport's tempo and time signature boxes open and take Enter while counting in or recording, while the tempo band's boxes do not
area: UI-PARITY
kind: parity
severity: low
status: fixed
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

Confirmed by reading (not run). `Transport.svelte:446-470`: the tempo and time signature `NumberField`s get no busy state, and `NumberField.svelte:90` always has `m-typable` and opens; `set_tempo` / `set_meter` return `None` at once while `busy()` (`crates/meloet-core/src/play/timeline.rs:192-195`, 222-224), and `commitClamped` (`Transport.svelte:294-297`) still sends `clampValue`, so for an out-of-range text the status line says the value was set though nothing was. The tempo band's boxes are not typable while busy. 7-readings line 77 (hover of typable numbers): not while the number cannot be typed.

One root with UI-ROLL-008 (which now holds UI-PARITY-005), UI-ARRANGE-009 and UI-ARRANGE-013 (parts that look usable while recording or counting in though Rust refuses).

Fix plan (as suggested): a `disabled` prop on `ui/src/components/NumberField.svelte` (no `m-typable`, `open` does nothing, the reading looks as now), passed as the transport's recording / count-in state for the tempo and the time signature in `ui/src/components/Transport.svelte`. Check the position box (`locate` is refused while busy too, `play.rs:928-931`) and give it the same prop if it opens today. No protocol or wording change.

## Fix

Not committed yet (the lead commits).

- `ui/src/components/NumberField.svelte` (another fixer's file, kept small): a `disabled` prop — pressing does not open the box, no `m-typable`, no hover border, `cursor: default`, and `aria-disabled="true"` (the transport's way of marking parts that cannot be used now). Without it the markup is as before.
- `ui/src/components/Transport.svelte`: `disabled={recordingNow}` for the tempo and the time signature, and for the position too (`locate` is refused while busy as well, as the verification asked).

Test: `ui/tests/transport.test.ts` — "while recording or counting in, tempo, time signature and position do not open and do not look typable (core takes none of them, as in the tempo band)".
