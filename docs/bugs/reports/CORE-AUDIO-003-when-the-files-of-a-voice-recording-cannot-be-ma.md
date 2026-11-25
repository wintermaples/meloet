---
id: CORE-AUDIO-003
title: When the files of a voice recording cannot be made, the recording goes on without the voice and nothing tells the person
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: needs-lead
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

Real, checked by reading (no test run). `Side::start` (`vocalrec.rs:591-615`) fails on `create_dir_all`, `WavFile::create` or the folder sync (and when no input is open); `start_vocals` (`service/input.rs:257-263`) prints to stderr and answers `Input::VocalsNotStarted`; its arm in `state.rs:1394` only calls `Play::vocals_not_started` (`play/vocal.rs:398-403`), which drops the vocal tracks from the recording. No notice is set anywhere, and the recording goes on.

Looked for an existing message, as the lead asked: `StatusMessage` has `NoInput`, `InputLost` (「…が抜けました。録音を止めました。…」), `RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`, `RecordingGaps`, `DiskTooSlow` (reading, while playing), `BounceFailed` (`msg.bounceFailedWrite` 「音にできませんでした。作業の場所に書けません。」), `ClipWorkFailed` (about clips). None says "the voice is not being recorded because its file could not be made"; each names another cause, so using one would mislead. `ui/src/text/ja.ts` has no other fitting key (the save dialog's 「ここには保存できませんでした（書きこむ権利がないか、空きが足りません）…」 is about the song's place).

Why `needs-lead`: the fix needs new wording (spec 6 has none) and a new `StatusMessage` variant (a protocol addition: decide the version). Choices:
1. A new variant (for example `VocalsNotStarted`) with a text from the owner, set in the `Input::VocalsNotStarted` arm; when the recording had only vocal tracks, also stop it (nothing would be recorded). Cost: protocol + `ja.ts` / `en.ts` + the status line's table + a fake scene. Recommended. A text to offer the owner, in the words of `msg.bounceFailedWrite`: 「声を録れません。作業の場所に書けません。」 (and for a recording with keys: the keys go on).
2. No new wording: stop the recording at once with no take and say nothing. Still silent; not recommended.
3. Leave it and list it in `docs/progress.md`.

Severity `medium` stands: a whole take can be sung into nothing; it needs a work place that cannot be written.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
