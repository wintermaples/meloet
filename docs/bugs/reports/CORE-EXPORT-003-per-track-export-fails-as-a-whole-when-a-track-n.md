---
id: CORE-EXPORT-003
title: Per-track export fails as a whole when a track name is longer than a file name can be
area: CORE-EXPORT
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# CORE-EXPORT-003: Per-track export fails as a whole when a track name is longer than a file name can be

## Where

crates/meloet-core/src/export/job.rs:299-313 (`free_name`)
crates/meloet-core/src/export/job.rs:259-264, 286 (the name joined to the folder, then `move_to`)
crates/meloet-core/src/export/midi.rs:342-348 (MIDI 「トラックごと」 uses the same `free_name`)

## What happens

「トラックごと」 names each file after its track with `free_name`, which takes out `/`, `\`, NUL and control characters but does not limit the length. A track name has no length limit (`meloet_model::edit::rename_track` only trims it). On Linux file systems a file name is at most 255 bytes; a Japanese character is 3 bytes in UTF-8, so a track name of about 84 Japanese characters or more makes `move_to` (audio) or `write_beside` (MIDI) fail with ENAMETOOLONG. The whole export then ends as 「書き出せませんでした」 (`ExportFailure::CannotWrite`, "the disk is full, or the place does not take it"), the files already moved are taken away again, and nothing tells the person that one track's name is the reason.

## What should happen

`free_name` says it gives `<name>.<ending>` "with what a file name cannot hold taken out"; a name too long for a file is such a thing. Every track should get a file (spec 3 "Per track makes one file for each track, named after the track").

## How to trigger

Read from the code, not run.
1. Rename a track to a name of 90 Japanese characters.
2. 「書き出し…」, 「トラックごと」, any folder, 「書き出す」.
3. The run ends with the failure 「書き出せませんでした」; no file is left.

## Cause

No byte length limit in `free_name`; the format's ending and the " 2" number are added after the name.

## Suggested fix

In `free_name`, cut `clean` at a character boundary so that `clean`, the number part and `.<ending>` together stay within 255 bytes (for example cut `clean` to 200 bytes before adding the number and the ending). Both the audio and the MIDI 「トラックごと」 use it. No protocol, wording or looks change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
