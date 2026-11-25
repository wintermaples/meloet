---
id: CORE-STATE-004
title: The keyboard fader's waiting step is made in a song that replaced the one it was moved in
area: CORE-STATE
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# CORE-STATE-004: The keyboard fader's waiting step is made in a song that replaced the one it was moved in

## Where

crates/meloet-core/src/state/buttons.rs:110-115 (`keyboard_fader` keeps `Pending { track, db, at_us }` by track id only)
crates/meloet-core/src/state/buttons.rs:128-146 (`fader_due` / `commit_fader` send `SetGain { commit: true }` for that id, whatever song is in place by then)
crates/meloet-core/src/state/songs.rs:536-541 (`replace_song` does not let go of `self.fader`)

## What happens

The fader (CC 7) of a keyboard sets the selected track's level as a preview, and its step is made 0.5 s after the fader rests (`FADER_REST_US`). If the song is replaced within that half second (「新しい曲」 or 「最近ひらいた曲 ▸」 with nothing unsaved, a file the PC hands over, the AI link's new or open song), the preview goes with the old song (`Play::replace_song` → `forget_mixer`), but the pending step stays. On the next `Input::Tick` it is made in the new song: the track there that has the same id (track ids start again for every song, so usually track 1) gets the old fader level as a 「音量を変える」 step. The new song shows as changed and its level jumped, though nobody touched it.

## What should happen

A step belongs to the song it was made in. docs/architecture.md 13: "Esc (`CancelMixPreview`), another step, undo, or replacing the song ends the preview" — the fader's level is such a preview until it rests; replacing the song should end it, not turn it into a step of the next song.

## How to trigger

Read from the code, not run.
1. A song with nothing unsaved; track 1 selected. Move the keyboard's fader (`Input::KeyboardFader`): `fader.pending = Some(Pending { track: 1, .. })`.
2. Within 0.5 s, choose 「新しい曲」 (no question, nothing unsaved): `then(Then::NewSong)` puts a new song in place.
3. 0.5 s after the last fader move, `Input::Tick` → `fader_due` → `commit_fader` → `SetGain { strip: Track { id: 1 }, commit: true }` → `Play::mix` makes the step on the new song's track 1 (it makes the step even with no preview, play/mixer.rs:251-260).

## Cause

`Model::fader` (`buttons::Fader`) is not tied to the song: neither `replace_song` nor anything else clears `pending` (or compares a serial) when another song takes the place of this one.

## Suggested fix

Keep the song's serial in `Pending` and drop the pending step in `commit_fader` when `self.play.serial()` differs, or reset `self.fader` to its default in `Model::replace_song`. No protocol, wording or file-shape change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
