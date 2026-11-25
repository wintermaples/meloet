---
id: UI-PARITY-008
title: Pressing outside a note list, CC list or selected-notes field box pulls the focus back to the row, unlike the number field
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-008: Pressing outside a note list, CC list or selected-notes field box pulls the focus back to the row, unlike the number field

## Where

ui/src/components/pianoroll/NoteFieldInput.svelte:65 (`onblur={oncancel}`: the blur and Esc call the same handler)
ui/src/components/pianoroll/NoteListPanel.svelte:173-175 (`cancel` → `close(true)`), :134-140 (`close(true)` focuses the row `[data-at]` after `tick()`)
ui/src/components/pianoroll/CcListPanel.svelte:157-159, :125-131 (the same)
ui/src/components/pianoroll/NoteInspector.svelte:347 (`oncancel={() => ... closeField(true)}`), :101-105 (`queueMicrotask(() => fieldButtons[name]?.focus())`)
The sibling that does it right: ui/src/components/NumberField.svelte:85 (`onblur={() => close(false)}`; only Enter and Esc call `close(true)`)

## What happens

While a field of the note list, the CC list or 「選んだノート」 is being typed in, pressing another text box (the shelf search, the params panel search, a box in the export dialog) does not leave the focus there. The blur of the note field closes it and, a microtask later, focuses the list row (or the field button in 「選んだノート」) again. That happens inside the focus change, so the pressed box does not get the focus (or loses it at once) and the person has to press it a second time. Keys typed meanwhile go to the list row and the document's key handler (one-letter keys act as shortcuts).

## What should happen

Esc puts the focus back on the row (spec 3 "Note list": Enter decides, Esc stops; the focus stays in the list for the next Enter), but a press outside should only close the box, as the transport's number field does: NumberField.svelte closes with `close(false)` on blur and `close(true)` only on Enter / Esc. The arrange and tempo boxes (timeline/ValueInput.svelte:66) and ValueField (params/ValueField.svelte:42-44) also do not take the focus back on blur.

## How to trigger

Read from the code, not run.
1. Open the piano roll with the note list, double-click a note's 「Vel」 field.
2. With the box open, press the search box of the params panel (or of the shelf).
3. The input's `blur` runs `oncancel` → `cancel(token)` → `close(true)`; after `tick()` the row button `[data-at]` gets `focus()`. In WebKit the microtasks run when the blur listener returns, so the focus move to the search box is blocked (or the row takes the focus right after). The search box does not have the caret.
The same in the CC list and in the four fields at the top of 「選んだノート」.

## Cause

NoteFieldInput has one `oncancel` for both Esc and blur, and all three parents answer it with "close and focus the row / field again". NumberField tells the two apart (`close(true)` for keys, `close(false)` for blur).

## Suggested fix

Give NoteFieldInput a separate blur path (for example `onblur={() => oncancel(false)}` with a `refocus` argument, Esc passing true), and in the three parents call `close(false)` / `closeField(false)` for the blur. No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
