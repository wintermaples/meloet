---
id: UI-PARITY-015
title: After Enter or Esc in a fader, pan, send or parameter value box the focus is lost, while the number field and note list give it back
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-015: After Enter or Esc in a fader, pan, send or parameter value box the focus is lost, while the number field and note list give it back

## Where

ui/src/components/params/ValueField.svelte:25-41 (Enter and Esc call `onsubmit` / `oncancel`; the input is then removed and nothing gets the focus)
ui/src/components/mixer/control.svelte.ts:88-98 (Enter on a focused fader, pan or send opens the box), :121-128 and :143-145 (`submit`, `stopTyping` only turn `typing` off)
ui/src/components/params/BuiltinList.svelte:83-88, SoundKnobList.svelte:51-54, PluginList.svelte:129-131 (Enter on the chosen row opens the box); params/paramUi.svelte.ts:15-19 (`stopTyping` only clears the row)
Siblings that do it right: ui/src/components/NumberField.svelte:41-47 and :55, :60 (`close(true)` focuses the reading again after Enter or Esc); pianoroll/NoteListPanel.svelte:134-140 and CcListPanel.svelte:125-131 (the row gets the focus back)

## What happens

Spec 3 (86-7): give a fader, pan or send the focus with Tab, move it with ↑ ↓ (← → for pan), Enter to type. After the typed value is decided with Enter (or the box is left with Esc), the box goes away and the focus is on nothing (the page body). The next ↑ ↓ do not move the fader any more; they go to the document's key handler and move the selected notes (or what else ↑ ↓ do there). The same in the params panel: ↑ ↓ choose a row, Enter types, Enter decides, and then ↑ ↓ no longer choose rows (the list lost the focus) but move notes.

## What should happen

After the box closes by a key, the focus goes back to where the keys were, as the transport's number field (`close(true)` → `shown?.focus()`) and the note and CC lists (`close(true)` → the row) do. Then the keys of spec 4 ("Move a fader / send ... when it has focus", "In the parameters: choose the row above / below") keep working without pressing Tab or the row again.

## How to trigger

Read from the code, not run.
1. Open the mixer, Tab to a track's fader, press ↑ once (it moves), then Enter.
2. Type -6 and Enter: `submit` sends `setGain`, `typing` becomes false, the ValueField and its input go away; `document.activeElement` is the body.
3. Press ↑: no `keydown` on the fader; `keys/attach.ts` gets it in the "main" context and runs `nudge` (moves the selected notes a semitone when the piano roll has a selection).
Params panel: choose a built-in effect's row, Enter, type a value, Enter, then ↓: the row does not change.

## Cause

ValueField's parents only switch `typing` off; none of them puts the focus back on the fader / knob / send element or on the list box, unlike NumberField and the note list which do it on key close.

## Suggested fix

When the box closes from Enter or Esc (not from blur), focus the control that opened it: in `mixControl` keep the element that got the Enter (`e.currentTarget`) and focus it after `typing = false`; in the three params lists focus `list` again after `stopTyping()` from a key. No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
