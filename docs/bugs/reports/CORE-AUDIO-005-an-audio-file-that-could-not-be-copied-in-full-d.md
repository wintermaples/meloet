---
id: CORE-AUDIO-005
title: An audio file that could not be copied in (full disk, no work place) is reported as a kind meloet does not read
area: CORE-AUDIO
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# CORE-AUDIO-005: An audio file that could not be copied in (full disk, no work place) is reported as a kind meloet does not read

## Where

crates/meloet-core/src/audiofiles/keeper.rs:392-393 (`Work::bring`: an error of `bring_in` becomes `Unread::NotAudio`)
crates/meloet-core/src/audiofiles/keeper.rs:418-420, 431-441 (`Work::found`: no work place, or a copy that fails, becomes `Finding::Unreadable`)
crates/meloet-core/src/state/audio.rs:245, 291 (`Unreadable` / `NotAudio` → `StatusMessage::AudioUnreadable`)

## What happens

Bringing in a file first checks what it is (`decode::facts`), then copies it into the work place (`AudioStore::bring_in`). When the copy fails (the disk is full, the work place cannot be written, there is no state folder), the keeper answers `Brought::Unreadable(name, Unread::NotAudio)`, and the status line says 「「声.wav」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。」 about a wav file that reads well. 「さがす…」 does the same when its copy fails (`Finding::Unreadable` → the same message). The person is told to use another file kind, which does not help; the real cause (no room, no right to write) is only on stderr.

## What should happen

The message must match the cause (docs/bugs brief: no wrong or misleading wording). `facts` already tells "not a kind meloet reads" (`NotAudio`), "broken" (`Broken`) and "too many channels" apart; a failure to write is none of these. Spec 6 has a message that names the disk for a song that cannot be saved (「ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。…」), but none for audio that cannot be brought in.

## How to trigger

Read from the code, not run.

1. Fill the disk that holds `~/.local/state/meloet` (or make `work/音` unwritable).
2. Drag a good wav file onto the arrange.
3. The status line says the file cannot be read and lists wav as a kind that reads.

## Cause

`Work::bring` maps every error of `store.bring_in` to `Unread::NotAudio`, and `Work::found` maps a failed copy to `Finding::Unreadable`; `Unread` has no case for "could not be copied".

## Suggested fix

Add a case to `Unread` (and `Finding`) for "could not be written into the work place" and show its own message. Needs new wording (the lead or the owner decides) and a new `StatusMessage` variant (a protocol addition). A smaller step with no new wording is not possible, since every existing message names a different cause.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
