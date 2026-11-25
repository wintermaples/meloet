---
id: CORE-PLAY-A-001
title: Clicking a clip on a vocal track selects it without the vocal-track rules: the keys lose their target and the track gets no record arm
area: CORE-PLAY-A
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# CORE-PLAY-A-001: Clicking a clip on a vocal track selects it without the vocal-track rules: the keys lose their target and the track gets no record arm

## Where

- crates/meloet-core/src/play.rs:1070-1079 (`Play::click`: sets `session.play_target` directly)
- crates/meloet-core/src/play.rs:1061-1064 (`select_clip`) and 1096-1097 (`select_clips` with one clip) call `click`
- crates/meloet-core/src/play/vocal.rs:203-216 (`selected_track_changed`, the rules that are skipped)
- Same root cause, rarer: crates/meloet-core/src/play/tracks.rs:24-27 (`delete_track` moves the selection) and crates/meloet-core/src/play.rs:1414-1417 (`after_change` moves it to the first track) also set `play_target` without `selected_track_changed`.

## What happens

A click on an audio clip of a vocal track (`Command::SelectClip`, or `SelectClips` with `add`/`toggle` and one clip) makes that vocal track the selected track, but:
- `session.last_keys` is not set to the keys track that was selected before. `Song::keys_target()` then falls back to `last_keys`, which is `None` (or an older keys track). The input meter's 「弾くと鳴る先」 shows 「—」 (or a wrong track) and the keyboard stops sounding, although the person only clicked a clip.
- The vocal track does not get its ● (`armed_vocals` is not changed), so R does not record the voice (and with no keys target either, recording is not possible at all). A click on the same track's header (`SelectTrack`) does both.
- The `NoInput` notice that `SelectTrack` gives for a vocal track with no input (state.rs 1857) is not given either.

## What should happen

Spec 3 (3-bands.md line 99): "Pressing a clip also selects its track. The selected track becomes 「弾くと鳴る先」." Spec 3 line 256 (94-1, 35-A): "While a vocal track is selected: the keyboard keeps playing on the keyboard track selected last". Architecture 22-15 / 24-C: a vocal track that is selected gets its ● (`selected_track_changed` doc comment: "a vocal track that is selected gets its ●"). Selecting a track by its clip must follow the same rules as selecting it by its header (`Play::select_track`, play.rs 1043-1052), as `open_piano_roll` (clips.rs 73-76), `make_copy` (tracks.rs 126-129) and `place_pattern` (patterns.rs 150-154) already do. The fake (`ui/src/bridge/fake.ts` `settleSong`) also keeps the keys on the previous keys track when `selectClip` selects a vocal track.

## How to trigger

Read from the code, not run.
1. Open a saved song with keys track 1 and vocal track 2 that has an audio clip (`last_keys` and `armed_vocals` are not saved, so both start empty), and click track 1's header so the keys sound on track 1.
2. Click the audio clip on track 2 in the arrange view.
3. `song.keysTarget` is now `null`: pressing keys sounds nothing and the input meter shows 「—」. Track 2's ● stays off, so nothing can be recorded (`TransportView::can_record` is false; R gives `NoArmedTrack`).
Clicking track 2's header instead keeps the keys on track 1 (`last_keys = 1`) and turns its ● on. (When `last_keys` holds an older keys track from an earlier vocal selection, the keys go to that older track instead of track 1.)

## Cause

`Play::click` writes `self.song.session.play_target = Some(track.id)` (play.rs 1073) and never calls `selected_track_changed(before)`, which is the only place that sets `last_keys` and inserts into `armed_vocals` for a newly selected vocal track. `select_track` calls it; the clip path does not.

## Suggested fix

In `Play::click`, when not busy and the track differs, use the same path as `select_track`: `let before = self.song.session.play_target.replace(track.id); if before != Some(track.id) { self.selected_track_changed(before); }`. Consider the same in `delete_track` and the `after_change` fallback. If the `NoInput` notice should also be said on a clip click, `state.rs` would check `selected_vocal()` after `SelectClip` as it does after `SelectTrack` (lead's choice). No protocol, file or wording change.

## Verification

Confirmed by reading (no test run). `Play::click` (`play.rs:1070-1079`) writes `session.play_target = Some(track.id)` and nothing else; `selected_track_changed` (`play/vocal.rs:203-216`), the only place that sets `last_keys` and adds to `armed_vocals`, is called by `select_track`, `open_piano_roll` (`clips.rs:75`), `make_copy` (`tracks.rs:129`), `place_pattern` (`patterns.rs:153`) and `vocal.rs:245`, not by `click`. The screen sends only `selectClip` for a press on a clip (`Arrange.svelte:1378-1434`), no `selectTrack` with it. With `last_keys` empty, `Song::keys_target()` is `None` after the click, so the keys sound nowhere and the vocal track has no ●. Severity `medium` stands.

Corrections: when the selection moves from a keys track to a vocal track by a clip click, `last_keys` is not cleared either, so the second sentence of "What happens" holds as written (an older keys track can be the target).

Fix plan: `crates/meloet-core/src/play.rs` `Play::click`: `let before = self.song.session.play_target.replace(track.id); if before != Some(track.id) { self.selected_track_changed(before); }` inside the `!self.busy()` branch. The same two lines where `delete_track` (`play/tracks.rs:24-27`) and `after_change` (`play.rs:1414-1417`) move the selection, if a test shows they land on a vocal track (they are the same rule; keep the change to what a test proves). The `NoInput` notice on a clip click: give it the way `SelectTrack` does in `state.rs:1857` (the existing message, no new wording). Tests in `crates/meloet-core/src/play/clips_tests.rs` or `tests.rs`: after `SelectClip` of a vocal clip, `keys_target` is the keys track selected before and the vocal track is armed.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
