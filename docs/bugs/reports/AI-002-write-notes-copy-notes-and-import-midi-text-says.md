---
id: AI-002
title: write_notes, copy_notes and import_midi text says "see changed" for shortened notes; the field is `cut`
area: AI
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# AI-002: write_notes, copy_notes and import_midi text says "see changed" for shortened notes; the field is `cut`

## Where

crates/meloet-ai/src/tools/notes.rs:260-262 (`notes_line`)

## What happens

When some notes were written shorter, the first line of the answer of `write_notes`, `copy_notes` and `import_midi` says "N written shorter (see changed)." The answer has no list called `changed`: the shortened notes are in `cut` (with `cut_count`, `cut_more`); `changed` is the boolean `true` of every write. An AI that follows the text looks in the wrong field.

## What should happen

The text names the field the answer has: `cut` (docs/development/ai-tools.md, "3 and 10": "`cut` | Up to 200 cut notes …"; `notes_value` in the same file writes `value["cut"]`).

## How to trigger

Read from the code, not run. `write_notes` into a new clip past bar 9999 or a clip end that cuts a note, e.g. a note near `MAX_TICK` with a long length; the text line says "(see changed)".

## Cause

The field was named `cut` in the JSON (`notes_value`, line 229 `value["cut"] = json!(changed)`; the local variable is still called `changed`), but the text in `notes_line` kept the old name.

## Suggested fix

Change "(see changed)" to "(see cut)" in `notes_line`. Text for the AI only (English, not screen wording); no protocol change.

## Verification

Confirmed by reading. `notes_line` (`crates/meloet-ai/src/tools/notes.rs:260-262`) writes "N written shorter (see changed)."; `notes_value` in the same file puts the shortened notes in `cut` (with `cut_count`, `cut_more`), and `changed` in an answer is the boolean of a successful write. The tool instructions (`tools.rs:106`) also say "Read not_written and cut". Severity `low`.

Fix plan: `crates/meloet-ai/src/tools/notes.rs`: "(see cut)". If a test pins the old text (`grep "see changed" crates/meloet-ai`), change it with it.

## Fix

`crates/meloet-ai/src/tools/notes.rs` (`notes_line`): "(see cut)". No test pinned the old text; none added (one line of text).
