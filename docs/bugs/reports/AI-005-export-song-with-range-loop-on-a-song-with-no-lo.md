---
id: AI-005
title: export_song with range "loop" on a song with no loop writes the whole song and does not say so
area: AI
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# AI-005: export_song with range "loop" on a song with no loop writes the whole song and does not say so

## Where

crates/meloet-ai/src/tools/files.rs:480-482 (`RangeArg::Loop => doc.loop_range.or_else(|| TickRange::new(0, song_end))`)
crates/meloet-ai/src/tools/files.rs:50-51 (the argument's description: "The loop's bars.")

## What happens

`export_song {range: "loop"}` on a song that has no loop range writes the whole song (from the start to the last clip's end) and answers "Wrote 1 file(s), N s" with no word that the loop was not there. The AI believes it has the loop's bars in the file.

## What should happen

The AI is told clearly when what it asked for cannot be done (the task's focus: "errors told to the AI clearly"). The sibling `set_loop_on` refuses with "the song has no loop range; set one with set_loop" (timing.rs:294-296) in the same case.

## How to trigger

Read from the code, not run. New song with notes, no loop set; call `export_song {"folder": "/tmp", "range": "loop"}`.

## Cause

The fallback `or_else(|| TickRange::new(0, song_end))` turns a missing loop into the whole song silently.

## Suggested fix

For `RangeArg::Loop` with `doc.loop_range == None`, answer `failed("the song has no loop range; set one with set_loop, or give range \"song\"")`, as `set_loop_on` does. If the fallback is wanted, say it in the answer text and the argument's description instead. No protocol change.

## Verification

Confirmed by reading. `export_song` (`crates/meloet-ai/src/tools/files.rs:480-482`): `RangeArg::Loop => doc.loop_range.or_else(|| TickRange::new(0, song_end))`, so with no loop range the whole song is written and the answer does not say so; the argument's description says "The loop's bars." The sibling `set_loop_on` refuses in the same case ("the song has no loop range; set one with set_loop"). Severity `low`.

Fix plan: `crates/meloet-ai/src/tools/files.rs`: for `RangeArg::Loop` with no `loop_range`, `failed("the song has no loop range; set one with set_loop, or give range \"song\"")`. Check `crates/meloet-ai/tests/` for a test that relies on the fallback and change it with it; one line in `docs/development/ai-tools.md` under `export_song` errors.

## Fix

`crates/meloet-ai/src/tools/files.rs` (`export_song`): `range: "loop"` with no loop range answers `failed("the song has no loop range; set one with set_loop, or give range \"song\"")` and writes nothing. `docs/development/ai-tools.md`: an `export_song` line.

Test: `the_export_writes_over_a_file_only_when_asked` in `crates/meloet-ai/tests/link.rs` gained the case (refused, the text says "no loop range", no file).
