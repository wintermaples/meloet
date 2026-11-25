---
id: CORE-AUDIO-001
title: A new voice recording can take the name of a file the song's folder already has; the song then plays and saves that file, and the recording is lost
area: CORE-AUDIO
kind: bug
severity: high
status: confirmed
found: 2026-11-25
---

# CORE-AUDIO-001: A new voice recording can take the name of a file the song's folder already has; the song then plays and saves that file, and the recording is lost

## Where

crates/meloet-core/src/service/input.rs:213-223 (`start_vocals`: a name is free when `!dir.join(name).exists()`, `dir` being the work place only)
crates/meloet-core/src/audiofiles.rs:113-125 (`Places::folders` / `Places::find`: the song's `音/` and the earlier folders are looked in before the work place)
crates/meloet-core/src/audiofiles.rs:363-366 (`settle_until`: a name that is already in `<song>/音/` is not moved, whatever that file holds)
crates/meloet-core/src/service/input.rs:621-624 (`keep_vocals`: a recovered file whose name is already in the table is passed over)

## What happens

A recorded file is named `<track name> <audio id>.wav` (`vocalrec::file_name`), and `start_vocals` only checks that the work place (`work/音/`) has no file of that name. The song's own folder `<song>/音/` (and an earlier folder after 「別名で保存」) is not checked, nor are the names in the song's audio table whose files are missing.

When the song's folder already holds a different file of the same name (an audio file the person brought in earlier, which a save moved there), the recording is written to `work/音/<name>` and gets a fresh uid, but:

- `AudioStore::find` looks in the song's folder first, so `PcmStore::open` for the new uid opens the old file: the new clip plays the other file's sound (its waveform is the recording's, from the writer's peaks).
- On the next save, `settle_until` sees `<song>/音/<name>` already there and does not move the recording; the song file names the old file for both entries.
- When the song is closed or another song opened, the work place is emptied (`clear_work`) and the recording is gone for good.
- If meloet ends during such a recording, `keep_vocals` finds the name in the table and does not keep the file at all.

The same happens when the table has an entry of that name whose file is missing (for example a brought-in file the person moved away): the old entry then "finds" the recording and plays it.

## What should happen

Every other place that gives a new file a name in the work place checks all the places the song looks in, and the table's names: `AudioStore::bring_in` (audiofiles.rs:294-313, `self.find(&name)` and `taken`) and `clips::put_in_place` (audiofiles/clips.rs:273, `taken.contains(&name) || store.find(&name).is_some()`). docs/architecture.md 22-4: "a different file with the same name gets « 2»"; 22-16: "a taken name gets « (2)»". A recording must never be shadowed by, or saved as, another file.

## How to trigger

Read from the code, not run.

1. In song B (saved), on the vocal track 「歌」, bring in a wav file named like a recording of another meloet song, for example `歌 40.wav` from song A's `音/` folder, with a number larger than song B's next id. Save: the file moves to `B/音/歌 40.wav`.
2. Go on recording takes on track 「歌」 until a recording gets audio id 40 (ids rise by a few with every recording, clip and take; new songs start at 4).
3. That take plays the brought-in file, not the voice. Save, close the song, open it again: the take's voice is gone.

## Cause

`start_vocals` picks the name with `.find(|name| !names.contains(name) && !dir.join(name).exists())` where `dir` is `work_audio_dir(state_dir)`. It never asks `AudioStore::find`, which also looks in the song's `音/` and the earlier folders, and it does not get the table's names (`taken`). The id in the name is new to the song, but a file brought in keeps the name it had, so it can carry any `<name> <number>.wav`.

## Suggested fix

In `start_vocals`, treat a name as taken also when `self.audio_store.find(name).is_some()` or when the song's audio table already has that file name (the `taken` list the import uses), and go on to « (2)», « (3)» as now. No protocol, file-shape or wording change.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-core --features dev-hooks --lib verify_core_audio_001`), with one step taken from the code by reading. The test: an `AudioStore` with a song folder whose `音/` holds 「歌 40.wav」 (a file brought in and saved earlier) and an empty work place. The rule of `start_vocals` (`service/input.rs:219-223`, `!dir.join(name).exists()` on the work place only) says the name `vocalrec::file_name("歌", 40)` = 「歌 40.wav」 is free; the recording is written to `work/音/歌 40.wav`. Then, run on the real store: `AudioStore::find("歌 40.wav")` gives the song folder's file (content "the file brought in"), so the new clip plays the other file; `settle` (a save) does not move the recording, because the name is already in `音/`; `clear_work` (the song let go of) removes it. After that the only 「歌 40.wav」 is the brought-in file: the recording is gone. `start_vocals` itself was not called in the test (it needs the whole service and an input stream); its naming rule is the two lines quoted, and `Play::vocal_file_asks` gives `<track name> <audio id>.wav` with no look at the table. Severity `high` stands (a recording is lost, and until then another file plays in its place), though it needs a brought-in file whose name is `<track name> <a later audio id>.wav`, for example a take taken over from another meloet song.

Corrections: none. The half about a table entry whose file is missing, and `keep_vocals` passing over a recovered file of a name the table has (`input.rs:621-624`), I read only.

Fix plan: `crates/meloet-core/src/service/input.rs` `start_vocals`: a name is taken also when `self.audio_store.find(name).is_some()` (all the places the song looks in) or when the song's audio table has that file name; go on to « (2)» as now. The table's names must come with the ask: `crates/meloet-core/src/state.rs` `Effect::StartVocalRecording` gains `taken: Vec<String>` (the file names of `doc.audio`, as the import passes `taken`), filled in `play.rs:857`. Tests: `state/vocal_tests.rs` (the effect carries the table's names) and a service-level or store-level test of the name choice (a small free function `free_recording_name(store, dir, taken, names, ask)` makes it testable without a stream).

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
