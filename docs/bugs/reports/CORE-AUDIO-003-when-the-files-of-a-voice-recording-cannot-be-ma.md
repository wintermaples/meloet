---
id: CORE-AUDIO-003
title: When the files of a voice recording cannot be made, the recording goes on without the voice and nothing tells the person
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: reported
found: 2026-11-25
---

# CORE-AUDIO-003: When the files of a voice recording cannot be made, the recording goes on without the voice and nothing tells the person

## Where

crates/meloet-core/src/vocalrec.rs:591-615 (`Side::start`: `fs::create_dir_all`, `WavFile::create`, the folder sync; any error ends the start)
crates/meloet-core/src/service/input.rs:257-263 (`start_vocals`: the error goes only to stderr, then `Input::VocalsNotStarted`)
crates/meloet-core/src/state.rs:1394 (`Input::VocalsNotStarted => self.play.vocals_not_started()`, no notice)
crates/meloet-core/src/play/vocal.rs:398-403 (`vocals_not_started`: the vocal tracks are dropped from the recording)

## What happens

When the disk writer cannot make the files of a recording (no room on the disk, no right to write in the work place, no state folder, a file name the file system refuses), `start_vocals` writes `meloet: cannot record into …` to stderr and answers `VocalsNotStarted`. The rules then take the vocal tracks out of the recording, and the recording goes on (the engine was already told to record, play.rs:855-900). Nothing is shown: no status line, no dialog. The armed vocal track does not play its clips during the recording (it is in `muted`), no waveform grows, and after the stop no take is made for the voice. A person who sang a whole take learns only afterwards that nothing was kept, and never why.

## What should happen

docs/bugs brief: an error must not be swallowed so the person never learns. The siblings tell the person when a recording cannot do what it should: the `StatusMessage::NoInput` notice (play.rs:838) when an armed track has no input, and the messages of spec 6 for a sound system or input that went away during recording (「…録音を止めました。そこまでの演奏は テイク 2 に残っています。」). docs/architecture.md 4-2: when recording cannot go on, "a notice is shown".

## How to trigger

Read from the code, not run.

1. Make the work place unwritable (for example `chmod a-w ~/.local/state/meloet/work/音`), or fill the disk.
2. Arm a vocal track that has an input and press record.
3. The count-in and the recording run as usual; nothing says the voice is not being recorded. After the stop there is no vocal take.

## Cause

`Input::VocalsNotStarted` carries no reason, and its handler only clears `rec.vocal.tracks`; no `StatusMessage` is set.

## Suggested fix

Set a notice in the `Input::VocalsNotStarted` arm of state.rs (and, if wanted, stop the recording when it had only vocal tracks). Needs new wording (spec 6 has none for this), so the text is the lead's or the owner's decision; no protocol change if an existing `StatusMessage` shape with a new variant is acceptable (that is a protocol addition).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
