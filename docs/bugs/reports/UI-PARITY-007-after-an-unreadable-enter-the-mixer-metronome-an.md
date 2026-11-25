---
id: UI-PARITY-007
title: After an unreadable Enter, the mixer, metronome and percent typing boxes no longer close when focus leaves
area: UI-PARITY
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-007: After an unreadable Enter, the mixer, metronome and percent typing boxes no longer close when focus leaves

## Where

ui/src/components/params/ValueField.svelte:27-31 (`done = true` is set before `onsubmit`, whatever `onsubmit` does)
ui/src/components/params/ValueField.svelte:42-44 (`onblur` cancels only when `!done`)
ui/src/components/mixer/control.svelte.ts:121-124 (`submit` returns false and keeps `typing` on for text that cannot be read; fader, pan and both sends)
ui/src/components/TrackHeads.svelte:48-50 (track head dB: returns early, box stays)
ui/src/components/MetronomeWindow.svelte:153-156 (metronome volume: "Text that cannot be read keeps the box open")
ui/src/components/pianoroll/PercentSlider.svelte:83-85 (quantize strength and swing: returns early, box stays)

## What happens

In the value box of a fader, pan, send, the track head dB, the metronome volume or the quantize strength / swing, type text that cannot be read (for example 「あ」) and press Enter: the box stays open, as meant (7-readings A-18 "Text that cannot be read keeps the box open"). From then on the box never closes by leaving it: pressing anywhere else moves the focus away, but the box stays on the screen, with the typed text, and the number under it is hidden. The only ways out are to press the box again and use Esc or a readable Enter. The same box closes on a press outside when Enter was never pressed.

## What should happen

ValueField's own header says "Leaving the box by pressing elsewhere also stops (only Enter decides)". The sibling boxes close on blur in every case: NumberField.svelte:85 (`onblur={() => close(false)}`), timeline/ValueInput.svelte:66 (`onblur={close}`), pianoroll/NoteFieldInput.svelte:65 (`onblur={oncancel}`), even after an unusable Enter (they never set a "done" flag).

## How to trigger

Read from the code, not run.
1. Open the mixer, double-click a fader's dB text (or Tab to the fader and press Enter).
2. Type 「あ」 and press Enter: `ValueField.onkeydown` sets `done = true`, calls `onsubmit`; `parseDb("あ")` is undefined, `submit` returns false, `typing` stays true.
3. Press somewhere else in the mixer: `onblur` runs, `done` is true, so `oncancel` is not called. The box stays.
The same with the pan text, a send text, the dB in a track head, the metronome volume number and the quantize strength / swing value.

## Cause

`done` is meant to stop the blur that follows a decided Enter from also cancelling. It is set before the parent says whether the text was taken, and it is never set back when the parent keeps the box open. Parents that always close on submit (ParamRow, ClipPanel, ClipsPanel, SettingsDialog latency) do not meet this; the five parents above keep it open for unreadable text.

## Suggested fix

Let `onsubmit` return whether it closed the box (the mixer's `submit` already returns a boolean; make the others return one too) and set `done` only when it returns true; or set `done = false` again after an `onsubmit` that leaves the box mounted. No protocol, wording or look change.

## Verification

Confirmed by reading (not run). `ValueField.svelte:27-31` sets `done = true` before calling `onsubmit`, and `onblur` (42-44) cancels only when `!done`; the parents that keep the box open for unreadable text (`mixControl.submit` returns false and leaves `typing` on, `control.svelte.ts:118-125`; `TrackHeads.svelte:48-50`; `MetronomeWindow.svelte:153-156`; `PercentSlider.svelte:83-85`; 7-readings A-18: "Text that cannot be read keeps the box open") leave the same `ValueField` mounted with `done` stuck at true, so a later blur closes nothing. The header of `ValueField.svelte` says leaving the box stops.

Severity lowered to low: it needs an unreadable Enter first, and Esc or a readable Enter still closes the box; nothing in the song is touched.

Fix plan: in `ui/src/components/params/ValueField.svelte` set `done` back to false after `onsubmit` returns when the field is still mounted (the smallest change, no parent changes), or let `onsubmit` return whether it closed. Test: unreadable Enter, then blur → `oncancel` is called.

## Fix

`ui/src/components/params/ValueField.svelte`: after Enter, when the box is still there after the next tick (text that could not be read), `done` goes back to false, so leaving the box stops it. No parent changed. Test: `ui/tests/fix-ui-values.test.ts` "UI-PARITY-007: after an Enter that could not be read, leaving the box closes it".
