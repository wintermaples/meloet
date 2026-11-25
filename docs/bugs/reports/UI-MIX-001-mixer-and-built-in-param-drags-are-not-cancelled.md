---
id: UI-MIX-001
title: Mixer and built-in param drags are not cancelled when their part goes away, so the preview sound and the held lane stay
area: UI-MIX
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-MIX-001: Mixer and built-in param drags are not cancelled when their part goes away, so the preview sound and the held lane stay

## Where

ui/src/components/mixer/control.svelte.ts:80-86 (the onMount cleanup of mixControl: `off(); keys.flush();`, no `cancel()`)
ui/src/components/params/ParamRow.svelte:143-149 (the onMount cleanup: `off(); stopFrame();`, no `cancel()`)
ui/src/components/params/PluginParamRow.svelte:91-97 (the sibling that does it right: `if (drag) cancel();`)

## What happens

When a fader, pan knob or send bar (mixControl) or a row of a built-in effect or a sound knob (ParamRow) is unmounted while its mouse drag is going on, nothing is sent: neither `commit: true` nor `cancelMixPreview`. Core keeps the last `commit: false` value in `Mixer::previews`, so the engine keeps playing the dragged value while the song (and the screen, after the part comes back) has the old one. If the control has an automation lane, the `commit: false` intents made a grip (`hand_moved` in crates/meloet-core/src/play/automation.rs:715) and held the lane; the grip is never let go, so the lane stays held and the control no longer follows the automation line. `mixer_changed` (play/mixer.rs:738) clears the previews on the next song change, but nothing clears `auto.grips` except `CancelMixPreview` (`cancel_hands`) or another drag of the same control. The screen-side hand value (`noteHand` in automation/handValue) is also never forgotten.

## What should happen

CLAUDE.md rules and the comment of mixer/mixDrag.ts: a mixer drag ends with one `commit: true` on release or `CancelMixPreview` when it is cancelled ("Cancelled (Esc, or the pointer was taken)"). A drag cut short by the part going away is a cancel. PluginParamRow.svelte already cancels its drag in its unmount cleanup.

## How to trigger

Read from the code, not run. 1. Open the mixer band. 2. Hold the mouse button on a track's fader and move it (the sound gets louder). 3. While still holding, press M (one-letter key `toggleBand mixer`; keys/keys.ts:119; handleKeydown has no guard for a mouse drag). The mixer closes, the Strip unmounts, and no intent is sent. The track stays at the dragged loudness although its fader value in the song did not change. Same with U (params panel, `toggleBand params`) or P (the shelf takes the right side in front of the params panel) during a drag of a built-in effect row, or any other way the part unmounts mid-drag. With a volume lane on the track and playback running, the lane stays held after step 3 and the volume stops following the line.

## Cause

mixControl keeps the drag in `press` (with its MixPreview) and ParamRow in `dragging`, and both clean up on unmount without calling their own `cancel()`. The pending requestAnimationFrame of MixPreview can even send one more `commit: false` after the unmount (MixPreview.stop is never called).

## Suggested fix

In the unmount cleanup, cancel a running drag first, as PluginParamRow does: in control.svelte.ts `return () => { off(); cancel(); keys.flush(); };` and in ParamRow.svelte `return () => { off(); cancel(); stopFrame(); };` (`cancel()` already does nothing when no drag runs). No protocol change, no new wording.

## Verification

Confirmed by reading (not run). Lines corrected in `Where` (`control.svelte.ts:80-86`). The unmount cleanup of `mixControl` is `off(); keys.flush();` and that of `ParamRow.svelte:143-149` is `off(); stopFrame();`; neither calls its own `cancel()`, while `PluginParamRow.svelte:91-97` does (`if (drag) cancel()`). One-letter keys are taken while a mouse button is held (`handleKeydown` has no guard), so M / U / P during a drag unmounts the part. Rust side: `commit: false` values stay in `Mixer::previews` (`crates/meloet-core/src/play/mixer.rs:245-251`) until the same control is set again, `CancelMixPreview`, or the next mixer change, and a grip made by `hand_moved` (`play/automation.rs:715`) is removed only by `hand_let_go` or the cancel path; so the heard value and the song differ, and a lane stays held.

Fix plan (as suggested): `ui/src/components/mixer/control.svelte.ts` cleanup → `off(); cancel(); keys.flush();`; `ui/src/components/params/ParamRow.svelte` cleanup → `off(); cancel(); stopFrame();` (`cancel()` does nothing with no drag). Check that `MixPreview.cancel()` also drops its pending animation frame (`ui/src/components/mixer/mixDrag.ts`). Tests: mount a strip, press and move, unmount → `cancelMixPreview` was sent.

## Fix

`ui/src/components/mixer/control.svelte.ts` and `ui/src/components/params/ParamRow.svelte`: the unmount cleanup calls `cancel()` first (sends `cancelMixPreview` for a running drag; `MixPreview.cancel()` already drops its pending frame). Test: `ui/tests/fix-ui-values.test.ts` "UI-MIX-001: a drag whose part goes away is cancelled".
