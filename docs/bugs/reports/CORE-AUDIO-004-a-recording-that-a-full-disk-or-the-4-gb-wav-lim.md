---
id: CORE-AUDIO-004
title: A recording that a full disk or the 4 GB wav limit stops ends with no notice, unlike a recording the sound system stops
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
