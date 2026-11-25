---
id: CORE-AUDIO-004
title: A recording that a full disk or the 4 GB wav limit stops ends with no notice, unlike a recording the sound system stops
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: needs-lead
found: 2026-11-25
---

# CORE-AUDIO-004: A recording that a full disk or the 4 GB wav limit stops ends with no notice, unlike a recording the sound system stops

## Where

crates/meloet-core/src/vocalrec.rs:959-962 (`Job::write`: a file without room sets `said.full`)
crates/meloet-core/src/vocalrec.rs:684-686, 988-990 (a failed write or `fdatasync` sets `said.error`)
crates/meloet-core/src/service/input.rs:423-427 (`poll_input`: `full` or `error` → `Input::VocalsCannotGoOn`)
crates/meloet-core/src/state.rs:1409-1413 (`VocalsCannotGoOn` only calls `stop_recording`; no notice is set)

## What happens

When the disk is full, a write or sync of a voice file fails, or a file reaches the size a wav can hold (about 4 h 8 min at 48 kHz, 24-bit stereo; 93 min at 96 kHz, 32-bit stereo), the disk writer marks the status and the core stops the recording exactly as if the person had pressed stop. The take is kept, but the status line says nothing about why the recording stopped. Only stderr gets "the recording was not written whole" (for an error; nothing at all for the 4 GB limit). The person sees the recording end by itself and may press record again into the same full disk.

## What should happen

docs/architecture.md 22-7: "4 GB limit …: stop recording before the limit and keep what was recorded (same path as "the sound stopped", 4-2)". 4-2: "If the sound stops while recording, the recording is closed …, what was recorded is kept as a take, and a notice is shown." The sibling for a stream that went away does show one: `cut_notice` (state/vocal.rs:65-73, `RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`, spec 6 「…録音を止めました。そこまでの演奏は テイク 2 に残っています。」).

## How to trigger

Read from the code, not run.

1. Put the state folder on a nearly full file system (or fill the disk while recording).
2. Record a vocal track until a write fails with "no space".
3. The recording stops; the take is there, and nothing tells why.

(The 4 GB case: record a stereo vocal track at 96 kHz with 32 bits for 93 minutes.)

## Cause

`Input::VocalsCannotGoOn` carries neither the reason nor the take, and its arm in state.rs sets no `StatusMessage`; `vocals_written` returns a `Cut` only for the stream cases.

## Suggested fix

Keep the reason (`full` or an error) with the recording, as `Cut` is kept for a stream that went away, and set a notice when the take is in, like `cut_notice`. Needs wording for "the disk is full" and "the file reached its largest size" (spec 6 has neither), so the lead or the owner decides the text; a new `StatusMessage` variant is a protocol addition.

## Verification

Real, checked by reading (no test run). `Job::write` sets `said.full` when a file has no room left (`vocalrec.rs:959-962`) and `said.error` on a failed write or sync; `poll_input` (`service/input.rs:423-427`) turns either into `Input::VocalsCannotGoOn`; its arm (`state.rs:1409-1413`) only calls `stop_recording`. The take is kept, and no notice is set; a stream that went away gets one through `cut_notice` (`RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`). Architecture 22-7 says the 4 GB stop goes the "same path as the sound stopped (4-2)", and 4-2 says "a notice is shown".

Looked for an existing message: `RecordingStoppedAudioSystem` names PipeWire, `RecordingStoppedOutputMissing` names the output, `InputLost` names the input that was unplugged (「…録音を止めました。録れた所までは残しています。」), `RecordingGaps` and `DiskTooSlow` are about skips. None can be used without saying a wrong cause. `ja.ts` has `pack.diskFull` (about the sound pack) and nothing for a recording.

Why `needs-lead`: new wording for two causes (the disk cannot be written; the file reached the largest size a wav can hold) and a new `StatusMessage` variant (protocol addition). Choices:
1. One new variant with a reason (`full` / `error`), said when the take is in, as `cut_notice` does: for example 「ディスクに書けなくなったので、録音を止めました。録れた所までは残しています。」 and 「ファイルがいちばん大きい所まで来たので、録音を止めました。録れた所までは残しています。」 (the second half is the wording of `msg.inputLost`). The reason travels with the recording from `Input::VocalsCannotGoOn { full }`. Recommended; the owner decides the text.
2. One text for both causes (shorter, less exact).
3. Leave it and list it in `docs/progress.md`.

Severity `medium` stands (the take is kept, but the person is not told why the recording ended, and can record again into the same full disk).

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
