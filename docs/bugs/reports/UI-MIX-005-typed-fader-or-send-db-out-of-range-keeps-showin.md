---
id: UI-MIX-005
title: Typed fader or send dB out of range keeps showing the typed number when Rust clamps it to the value it already had
area: UI-MIX
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-MIX-005: Typed fader or send dB out of range keeps showing the typed number when Rust clamps it to the value it already had

## Where

ui/src/components/mixer/control.svelte.ts:219-226 (`submit` sets `keyed = { value: v }` with the typed value)
ui/src/components/mixer/control.svelte.ts:132-139 (`keyed` is cleared only when the song value changes)
ui/src/components/mixer/scale.ts:345-351 (`parseDb` leaves out-of-range values to Rust)

## What happens

Typing a dB value into a fader or send box (Enter or double click on the number) keeps the typed number as `keyed` until the song value changes. Rust brings an out-of-range value into range (`Gain::from_db` in crates/meloet-model/src/mixer.rs:29: above +6 is +6, below −60 is −∞). When that result is the value the strip already has, the song does not change (`unless_same`), no state comes back, and `keyed` stays: the strip shows the typed number and puts the handle by it, though the sound and the song have the old value. Examples: a fader at +6.0 and typing 12 shows 「12.0」; a send at −∞ and typing −80 shows 「−80.0」 with the bar empty.

## What should happen

The screen is a copy of Rust's state (CLAUDE.md; architecture 3, rule 5: only text being typed and the look during a drag may move first). After Enter the strip should show the song value, as it does when the clamped value differs from the old one.

## How to trigger

Read from the code, not run. Open the mixer, raise a fader to the top (+6.0), double click its number, type 12, Enter: the number stays 「12.0」 until the fader is changed again.

## Cause

`submit` stores the raw parsed value in `keyed`; it is dropped only by the `$effect` on `opts.value()`, which does not run when Rust's answer leaves the value as it was.

## Suggested fix

Do not keep the typed value in `keyed` after `send(opts.make(v, true))` (set `keyed = null`; the screen then shows the song value and Rust's answer), or clamp it the same way Rust does before keeping it (−60 floor to −∞, +6 top). No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
