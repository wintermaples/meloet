---
id: CORE-AUDIO-006
title: A long vocal track name makes the recorded file name longer than the file system allows, and the voice is not recorded
area: CORE-AUDIO
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# CORE-AUDIO-006: A long vocal track name makes the recorded file name longer than the file system allows, and the voice is not recorded

## Where

crates/meloet-core/src/vocalrec.rs:68-77 (`file_name`: the whole track name goes into the file name, with no limit on its length)
crates/meloet-core/src/vocalrec.rs:101 (`WavFile::create` fails on such a name)
crates/meloet-model/src/edit.rs:387-405 (`rename_track`: a track name has no length limit; the screen's name field has no `maxlength` either)

## What happens

A recorded file is named `<track name> <audio id>.wav`. Track names have no length limit, and a file name on Linux file systems (ext4, btrfs, xfs) is at most 255 bytes; a Japanese character takes 3 bytes in UTF-8. With a vocal track name of about 82 Japanese characters or more (about 245 ASCII characters), the name is too long: `WavFile::create` fails with "File name too long", `start_vocals` answers `VocalsNotStarted`, and the recording goes on without the voice (and without a word, CORE-AUDIO-003). The same function names the files of 「逆にする」 (play/reverse.rs:43) and 「音のクリップにする」 (play/bounce.rs:68-72, with a suffix added), which then fail too.

## What should happen

A track name the song accepts must not keep its track from being recorded. The other file names meloet makes from what a person typed are made safe for the file system: `vocalrec::file_name` already takes out `/`, `\`, NUL and control characters and handles an empty name (vocalrec.rs:70-75), and `songfile::folder_name` does the same for a song's folder; only the length is not handled.

## How to trigger

Read from the code, not run.

1. Rename a vocal track to a name of 90 Japanese characters (by typing it, or with the AI link's rename).
2. Arm it and record: the count-in and the recording run, no waveform grows, and no take is made for the voice.

## Cause

`file_name` joins the cleaned track name, a space, the id and `.wav` without looking at the byte length; `start_vocals` adds « (n)» for a taken name, which makes it longer still.

## Suggested fix

In `file_name`, cut the cleaned track name at a character boundary so that the whole name, with room for « (99)» or « 99» (the suffixes `start_vocals`, `bring_in` and `put_in_place` add), stays well under 255 bytes (for example at most 200 bytes for the track-name part). No protocol, file-shape or wording change; names of existing files are not touched.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
