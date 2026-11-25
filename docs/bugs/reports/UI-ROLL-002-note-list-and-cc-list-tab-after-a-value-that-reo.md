---
id: UI-ROLL-002
title: Note list and CC list: Tab after a value that reorders the take opens the next field on another note and edits it
area: UI-ROLL
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# UI-ROLL-002: Note list and CC list: Tab after a value that reorders the take opens the next field on another note and edits it

## Where

- `ui/src/components/pianoroll/NoteListPanel.svelte:149-164` — `finish` sends the decided value, then opens the next field at row `e.at + next.row` with `open()`, whose `draft` is read from the take shown before the answer.
- `ui/src/components/pianoroll/NoteListPanel.svelte:118-123` — the box is kept by row number (`editing.at`); `editSpec` is made again from whatever note is at that row in the new take.
- `ui/src/components/pianoroll/NoteListPanel.svelte:153` — the command goes to `indexes[e.at]` of the take shown at that moment.
- `ui/src/components/pianoroll/noteFields.ts:315-317` — `commitCommand` compares the typed value with `spec.current` of the note now at that row, so an unchanged box still sends a change.
- `ui/src/components/pianoroll/CcListPanel.svelte:102-106`, `:132-146` and `ui/src/components/pianoroll/ccFields.ts:176-182` — the same in 「CC の一覧」, where the box is kept by the index in the take.

## What happens

Read from the code. The note list has row 0 C3 at 1.1.0000, row 1 D3 at 1.2.0000, row 2 E3 at 1.3.0000. Open 「位置」 of row 0, type `1.4`, press Tab.
1. `finish` sends `setNotesField` (start) for index 0 and at once opens 「音」 of row 0 with the text of the old take: `C3`.
2. Rust answers with a new take in start order: D3, E3, C3. Row 0 is now D3 (its position, length and velocity are shown in the row), and the moved C3 is selected on row 2. The open box on row 0 still says `C3`.
3. Pressing Enter or Tab without typing sends `setNotesField` key = 48 for index 0 of the new take: D3 becomes C3. Typing a new note name for the moved note changes D3 instead.
The same happens with Shift Tab, and with 「音」 among notes that start together (the order is start, then key). In 「CC の一覧」: two sustain points, index 0 at 1.1 (127) and index 1 at 1.2 (0); type a later position for index 0 and press Tab: the 「値」 box opens for index 0, which after the answer is the other point, on the same row, with `127` in it; Enter sends `moveControls` value +127 for that point, so the pedal never comes up there.

## What should happen

Spec 3, "Note list": "Tab goes to the field on the right" — the field on the right of the same note. A box that was not changed must not change anything (`commitCommand` is meant to return null then). Architecture 16: notes are pointed to by take id and index; an index taken from an earlier take must not edit another note.

## How to trigger

As above, read from the code, not run. Any typed 「位置」 that moves a note past the start of another note, or a typed 「音」 among notes with the same start, followed by Tab or Shift Tab, then Enter.

## Cause

The box remembers a row number (note list) or a take index (CC list), and its `draft` is filled from the take before the answer. When the answer's take arrives, `editSpec` follows whatever note or point is now at that row or index, but the text in the box does not; deciding then aims at the note now at that place. The panel's header comment ("The row being typed in is remembered by its row number in the list (deciding changes the take id)") covers the new id but not the new order.

## Suggested fix

Follow the edited note, not the row: Rust selects the edited notes after an edit (architecture 16), so the note list can open the next field on the row of the selected note once the new take is there, with its text from the new take. Simpler, and also for the CC list: close the box after a value that can move the note or point (position, note; position in the CC list) instead of moving on with Tab. No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
