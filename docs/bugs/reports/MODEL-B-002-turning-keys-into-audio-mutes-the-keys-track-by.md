---
id: MODEL-B-002
title: Turning keys into audio mutes the keys track by hand only; a moving mute lane keeps it playing over its own audio
area: MODEL-B
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# MODEL-B-002: Turning keys into audio mutes the keys track by hand only; a moving mute lane keeps it playing over its own audio

## Where

crates/meloet-model/src/audio/bounce.rs:137-141 (`bounce_track`: only `muted: true` is set on the keys track)
crates/meloet-core/src/play/mix.rs:50-56 (`silent_tracks`: a mute lane that moves takes the place of the hand's mute)
crates/meloet-core/src/play/bounce.rs:120-123 (`bounce_doc` keeps the mute lane in the render)

## What happens

When the keys track has a mute lane that moves (not resting, with points), 「音のクリップにする」 / 「このトラックを音にする」 sets the track's hand mute, but the engine ignores the hand mute of a track whose mute lane moves. Wherever the lane says "not muted", the keys track keeps playing. The render kept the same mute lane, so the new vocal track has sound in exactly those places: the part is heard twice (about +6 dB, the same notes on top of each other). The song keeps `muted: true` on a track that is heard.

## What should happen

Spec chapter 3 (docs/spec/3-bands.md:455): "The keys track is muted (all of it)." The siblings that set a mute on a track whose mute lane moves rest the lane in the same step so the mute is heard: `Play::toggle_mute` (crates/meloet-core/src/play/mix.rs:21-26, docs/architecture.md 21-11) and `recall_mute_set` (crates/meloet-model/src/mute_sets.rs:192-202, "the lane rests in the same step, as when M is pressed on such a track, so that the set is heard").

## How to trigger

Read from the code, not run.
1. A keys track with notes in bars 1-8 and a mute lane with points 0.0 at bar 1 and 1.0 at bar 5 (plays bars 1-4, muted from bar 5).
2. Right-click the track head, 「このトラックを音にする」.
3. The render (`bounce_doc`) keeps the mute lane, so the audio has bars 1-4 and silence after. `bounce_track` sets `muted: true` on the keys track but leaves the lane moving.
4. Play: in bars 1-4 both the keys track (its lane says not muted, `silent_tracks` skips the hand mute because `laned` is true) and the new vocal track sound.

## Cause

`bounce_track` builds the keys track as `Track { muted: true, ..source }` and does not touch its `AutoTarget::Mute` lane. `silent_tracks` treats a track as silent by its hand mute only when `!laned` (no mute lane that moves), so the hand mute set by the step has no effect while the lane moves.

## Suggested fix

In `bounce_track`, when the keys track's mute lane moves, also put it to rest in the same step (`automation::set_resting(&mut new, from, AutoTarget::Mute, true)` after the track is replaced), as `recall_mute_set` does. Undo brings both back. No protocol or song-file change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
