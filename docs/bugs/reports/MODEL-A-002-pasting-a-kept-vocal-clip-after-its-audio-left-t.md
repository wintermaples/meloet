---
id: MODEL-A-002
title: Pasting a kept vocal clip after its audio left the audio table makes a silent clip and a song file that cannot be opened
area: MODEL-A
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# MODEL-A-002: Pasting a kept vocal clip after its audio left the audio table makes a silent clip and a song file that cannot be opened

## Where

crates/meloet-model/src/arrange.rs:183-200 (`paste_clips`: the kept clips go in through `place` as they were kept)
crates/meloet-model/src/arrange.rs:261-270 (`place`: the new `Doc` keeps `doc.audio` as it is)
crates/meloet-core/src/play.rs:1196-1199, 2093 (`clip_board` is kept across undo; it is cleared only when the song is replaced)
crates/meloet-core/src/songfile.rs:1307 and 1447-1456 (the file's audio table is `used_audio(doc)`, taken from `doc.audio` only)
crates/meloet-core/src/songfile.rs:1627-1628 and 1501-1502 (`audio_take`: an audio take whose file is not in the table makes the song `None`)

## What happens

Mod C keeps `Arc<Clip>`s (`KeptClip`). A vocal clip's takes hold their `Arc<AudioSource>` directly. If the step that put that file into the audio table (`Doc::audio`) is undone after the copy — 「音のファイルを入れる…」 (`import_audio`), a vocal recording, 「逆にする」 — the table no longer has the file, but the clip board still has the clip. Mod V (or 「貼りつけ」 on a vocal lane) then puts a vocal clip into the song whose file is not in `Doc::audio`.
- The engine is told only the table (`EngineOp::AudioSources { sources: doc.audio }`, play/audiosync.rs:110-113), which does not hold that file.
- Saving writes the clip's audio take with `audio: <id>`, but the file's table is `used_audio(doc)`, a filter of `doc.audio`, so the file is not in it. Opening the song again fails at `audio.get(&t.audio)?` and the whole song is `ReadError::Broken`. The recovery file uses the same encoder, so it is broken too.

## What should happen

`Doc::audio` is "every audio file a take of the song plays" (crates/meloet-model/src/song.rs:368). Every other step that puts an audio take into the song also puts its file into the table in the same step (`import_audio`, audio/clips.rs:301; `add_vocal_recordings`, audio/record.rs:215; `reverse_clips`, audio/reverse.rs:260). Architecture 17 ("Paste: Rust remembers the clips … takes are only shared by `Arc`") expects a paste to give the same song as the clips had.

## How to trigger

Read from the code, not run.
1. On a vocal track, 「音のファイルを入れる…」 a WAV file (one step, the file goes into `Doc::audio`).
2. Click the new clip, Mod C.
3. Mod Z (the import is undone; the table goes back to before it).
4. Select the vocal track, Mod V.
5. Save, close, open the song: it is refused as broken. (The same with a vocal recording instead of the import.)

## Cause

`paste_clips` (through `place`) only places clips; it never looks at `Doc::audio`. Keys takes need no table, so this is right for keys clips, but a kept vocal clip can outlive its file's entry in the table, because `History::undo` brings back a `Doc` whose table does not have it while the core's clip board keeps the clip.

## Suggested fix

In `paste_clips`, for every placed vocal clip, add each `take.source` that `doc.audio` does not hold (by `AudioId`) to a copy of the table, kept in the order of id, and set `new.audio` to it, in the same step. Its id was given out by the same `next_id` and is never given again, so it cannot collide. No protocol or file-shape change. A test: import, copy, undo, paste, then `songfile::decode(songfile::encode(..))` reads the song back and `Doc::audio` holds the file.

## Verification

Confirmed by a test I ran (scratch worktree, `cargo test -p meloet-core --features dev-hooks --lib verify_model_a_002`): from the vocal song of `songfile/vocal_tests.rs`, one `History` step puts clip 43 and its file (audio 51) in; `arrange::copy_clips` of clip 43; `History::undo`; `arrange::paste_clips` on the vocal track. The pasted song's audio table is `[AudioId(50)]` only, and `songfile::decode(&songfile::encode(..))` gives `Err(Broken)`. The core keeps `clip_board` across undo (`play.rs:1198`, cleared only at `play.rs:2093` when the song is replaced), and `Play::paste_clips` (`play.rs:1245`) has no guard of its own. Severity `high` stands.

Corrections: none. The engine side is as written too: `EngineOp::AudioSources` carries `doc.audio` only, so until the save the pasted clip is silent.

Fix plan: in `crates/meloet-model/src/arrange.rs` `paste_clips` (after `place`), for each kept vocal clip add every `take.source` whose `AudioId` is not in `doc.audio` to a copy of the table (sorted by id, as the reader does) and set it on the new `Doc` in the same step. Tests: `crates/meloet-model/src/arrange/tests.rs` (the table holds the file after the paste) and `crates/meloet-core/src/songfile/vocal_tests.rs` (import, copy, undo, paste, `decode(encode(..))`). The file on disk: check while fixing that a recording / import undone and then pasted still has its file where `audiosync` looks for it (the file of an undone step is kept for redo, so it should; I did not run that part).

## Fix

`crates/meloet-model/src/arrange.rs` `paste_clips`: each file a kept vocal clip plays that is not in `Doc::audio` goes back into a copy of the table (in the order of id), set on the pasted `Doc` in the same step; when nothing is missing the table stays the same `Arc`. The core's `sync_audio_sources` sees the new table and sends `EngineOp::AudioSources` with the file; undoing the paste sends the table without it. The file itself stays in the work place or the song folder while the song is open (architecture 22: the in-memory table keeps unused files, the work place is cleared only when the song is let go of), so the store finds it. `docs/architecture.md` 17 (Paste) says so.

Tests: `arrange::tests::a_pasted_vocal_clip_brings_its_file_back_into_the_audio_table` (meloet-model) and `songfile::vocal_tests::a_vocal_clip_pasted_after_its_import_was_undone_keeps_its_file` (meloet-core: `Play` import, copy, undo, paste; the engine gets the table with the file; `decode(encode(..))` gives the song back; undo sends the table without it).
