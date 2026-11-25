---
id: MODEL-A-001
title: Joining clips keeps origin indexes on notes of a take with no origin, so the saved song cannot be opened
area: MODEL-A
kind: bug
severity: high
status: reported
found: 2026-11-25
---

# MODEL-A-001: Joining clips keeps origin indexes on notes of a take with no origin, so the saved song cannot be opened

## Where

crates/meloet-model/src/arrange.rs:574-581 (`join_clips`: `notes.push(Note { start, length, ..*n })` keeps `n.from`)
crates/meloet-model/src/arrange.rs:595-601 (the joined take is made with `origin: None`)
crates/meloet-core/src/songfile.rs:1540-1543 (`take_file` writes `NoteFile::From` for every note whose `from` is `Some`)
crates/meloet-core/src/songfile.rs:1896-1907 (`to_take`: a `From` note in a take without origin makes the whole song `None`)

## What happens

When one of the joined clips shows a quantized take, the joined take has no origin (`origin: None`), but its notes still carry the origin indexes (`from: Some(i)`) of the old quantized take. The song plays and edits normally. When it is saved (or the recovery file is written 2 s later), each such note is written as `NoteFile::From(start, length, key, velocity, i)`. Opening that file again fails: `to_take` accepts a `From` note only when `i < from_len`, and `from_len` is 0 for a take with no origin, so `to_song` returns `None` and the song is `ReadError::Broken`. The recovery file is read with the same `songfile::decode`, so it cannot bring the song back either. The whole song is lost from the person's point of view.

## What should happen

`Note::from` is "`None` for a note written, copied or pasted, and in a take with no origin" (crates/meloet-model/src/song.rs:33-35). Architecture 24-6 says the joined take keeps "other takes and the quantize origins are not kept". The sibling code that drops an origin does clear the indexes: `unquantize` sets `n.from = None` when it drops the origin (quantize.rs:164-168), and `overdub_recording` sets `from: None` when it merges clips and drops the origin (record/overdub.rs:104-110).

## How to trigger

Read from the code, not run.
1. Record notes into clip A, then press Q (quantize) on it, so its shown take has an origin and its notes have `from`.
2. Have a second keys clip B right after A on the same track.
3. Select A and B, 「クリップをつなげる」 (`Command::JoinClips`, also the AI link's join).
4. Save, close, and open the song again: it is refused as broken (also the recovery file after a crash).

## Cause

`join_clips` builds each note of the new take with `..*n`, which copies `from` from the shown take of each joined clip, while the new take is made with `origin: None`. The model never checks that `from` is only set under an origin, but the song file reader does, and turns the whole song away.

## Suggested fix

In `join_clips`, write `from: None` for every note put into the joined take (as `overdub_recording` does for its merged take). No protocol or file-shape change. A test: join a quantized clip with a neighbour and check that no note of the joined take has `from`, and that `songfile::decode(songfile::encode(..))` reads the song back.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
