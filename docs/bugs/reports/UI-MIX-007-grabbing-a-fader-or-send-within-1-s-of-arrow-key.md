---
id: UI-MIX-007
title: Grabbing a fader or send within 1 s of arrow keys starts from the old value and the handle jumps back
area: UI-MIX
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-007: Grabbing a fader or send within 1 s of arrow keys starts from the old value and the handle jumps back

## Where

ui/src/components/mixer/control.svelte.ts:49-57 (`down`: `const from = opts.value()`, the KeySteps wait is not flushed)
ui/src/components/mixer/control.svelte.ts:132-134 (`shown` shows `keyed` before the drag)

## What happens

After ↑ ↓ (fader, send) or ← → (pan), the strip shows the new value (`keyed`) and the step is sent with `commit: true` only 1 second after the key is let go. If the mouse grabs the same control inside that second, `down` starts the drag from the song value, not from the shown one: the handle jumps back to the old place at the press and the drag moves from there. Then the KeySteps timer fires during the drag and sends `commit: true` with the key value (one step, and the sound jumps to it) while the screen shows the drag value; the next `commit: false` of the drag moves the sound back.

## What should happen

The drag starts from where the handle is (spec 3 (86-7) for the keys; the header of control.svelte.ts: "It does not jump to the pressed point; only movement from the pressed point changes the value"). The keys' step should be made before a new hand starts on the control, as `startTyping` and Enter already do with `keys.flush()`.

## How to trigger

Read from the code, not run. Focus a fader with Tab, press ↑ three times (the value shows +3 dB over the old one), and within one second press the fader with the mouse: the handle goes back 3 dB at once.

## Cause

`down` neither flushes the pending key step nor reads `keyed`.

## Suggested fix

Call `keys.flush()` at the start of `down` and take `from` from the shown value (`keyed ? keyed.value : opts.value()`). No protocol change, no new wording.

## Verification

Confirmed by reading (not run). Lines corrected in `Where`. `down` (`control.svelte.ts:49-57`) takes `from = opts.value()` and does not call `keys.flush()`; `shown` is `keyed.value` before the press and `live.now = from` after it, so the handle jumps back by the keyed amount; the `KeySteps` timer (`mixDrag.ts:84`, `KEY_COMMIT_MS`) then fires during the drag and sends `commit: true` with the key value. `startTyping` and Enter already flush.

Fix plan (as suggested): in `down`, `keys.flush()` first and `from = keyed ? keyed.value : opts.value()` (`ui/src/components/mixer/control.svelte.ts`). Test: ↑ three times, press within the wait → one `commit: true` with the keyed value before the drag, `live.from` is the keyed value.

## Fix

`ui/src/components/mixer/control.svelte.ts` `down`: `keys.flush()` first, and the drag starts from `keyed` when there is one. Test: `ui/tests/fix-ui-values.test.ts` "UI-MIX-007: grabbing a fader right after arrow keys".
