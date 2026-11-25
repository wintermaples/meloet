---
id: CORE-AUDIO-007
title: After 「さがす…」 takes a file with other contents, the clip keeps playing the old decoded copy and showing the old waveform
area: CORE-AUDIO
kind: bug
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-AUDIO-007: After 「さがす…」 takes a file with other contents, the clip keeps playing the old decoded copy and showing the old waveform

## Where

crates/meloet-core/src/audiofiles/keeper.rs:414-443 (`Work::found`: a file of the same rate and channels is copied in under the source's name, whatever it holds; nothing kept for the old contents is let go)
crates/meloet-core/src/audiofiles.rs:246-262 (`cached` / `playable`: the decoded copy `audio/<uid>-<rate>.wav` is taken as ready when it opens at the rate)
crates/meloet-core/src/audiofiles.rs:429-439 (`PcmStore::open`: the same copy is what plays)
crates/meloet-core/src/audiofiles/keeper.rs:465-467 (`watch`: the peaks file is kept when its frame count equals `source.frames`)

## What happens

「さがす…」 accepts a file with another SHA-256 when its rate and channel count match (docs/architecture.md 22-18 A2, B-94-30), and copies it into the work place under the missing source's name. The source keeps its `uid`, and everything meloet made from the old file is keyed by that uid and is still in the cache (a source in the table keeps its cache "used"):

- If the file is not a plain wav at the engine's rate (an mp3, a flac, an ogg, or a wav at another rate), `playable` and `PcmStore::open` find the old decoded copy `audio/<uid>-<rate>.wav` and use it. The clip plays the old sound, not the file that was found, and nothing decodes the found file.
- The old peaks file `peaks/<uid>.peaks` says `frames == source.frames` (the table's old count), so `watch` does not make peaks again: the waveform is the old file's, also for a plain wav that does play the new contents.

On a save the found file goes into `音/`; on another PC, or after the cache is emptied, the same song then plays the found file. So the same song sounds different by PC.

## What should happen

docs/architecture.md 22-18 A2: "The found file goes to the work place and moves to 音/ on the next save" — that file is the song's audio from then on, and what plays and what is drawn must come from it. 22-8: the decoded copy "can be rebuilt"; 22-9: "Missing cache files are rebuilt". The cache is a copy of the file, never a second source of truth.

## How to trigger

Read from the code, not run.

1. Bring an mp3 into a song and play it once (the keeper decodes it into `~/.cache/meloet/audio/<uid>-48000.wav` and writes its peaks). Save.
2. Close meloet. In another program change the mp3 (for example make it quieter, or cut a part out of its middle) and save it under another name at the same rate and channel count; move the original away.
3. Open the song: the clip is missing. Use 「さがす…」 and pick the changed file: the status line says it was found, but the clip plays and draws the old contents.

## Cause

`Work::found` only copies the file and calls `file_arrived`. The caches are looked up by uid alone (`cache_path(uid, rate)`, `peaks_path(uid)`), and the uid of brought-in audio is the hash of the old contents, so the old copies stay valid for the new file.

## Suggested fix

In `Work::found`, when the found file's SHA-256 differs from `source.sha256` (or `source.sha256` is `None`), remove the decoded copies of that uid (`audio/<uid>-*.wav`) and its peaks file before `file_arrived`, so `watch` makes them again from the found file; when the SHA-256 is the same, keep them as now. No protocol, file-shape or wording change. (Giving the found source a new uid would also work but changes the song's table.)

## Verification

Confirmed by reading (no test run). `Work::found` (`audiofiles/keeper.rs:414-443`) copies a file of the same rate and channel count into the work place under the missing source's name and calls `file_arrived`; nothing made from the old contents is let go. `AudioStore::playable` and `PcmStore::open` (`audiofiles.rs:251-262, 429-439`) take the decoded copy `audio/<uid>-<rate>.wav` whenever the found file is not itself a wav at the engine's rate, and `watch` (`keeper.rs:465-467`) keeps the peaks file while its frame count equals `source.frames`. The uid does not change, so the old decoded copy plays and the old waveform is drawn. Architecture 22-18 A2 accepts a file with another SHA-256 on purpose ("maybe the same song saved again by another app"), and 22-8 / 22-9 call the cache a copy that can be made again. Severity `low` stands (only after 「さがす…」 with a file of other contents, and for the decoded copy only when the file is not a wav at the engine's rate).

Corrections: none. I did not check when the cache of a missing source is cleaned; if it is already gone, only the peaks half is left.

Fix plan: `crates/meloet-core/src/audiofiles/keeper.rs` `Work::found`: after the copy, when the found file's SHA-256 is not `source.sha256` (or there is none), remove the decoded copies of that uid and its peaks file (a small method on `AudioStore`, which is the one place that knows the cache paths) and take the uid out of `peaks_made`, before `file_arrived`. Test in `audiofiles/tests.rs`: a cached copy and peaks for a uid, `found` with a file of other bytes → both are gone and `playable` says `NeedsCache`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
