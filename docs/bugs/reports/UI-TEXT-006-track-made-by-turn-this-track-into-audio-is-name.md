---
id: UI-TEXT-006
title: Track made by Turn this track into audio is named with a Japanese suffix in English
area: UI-TEXT
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-TEXT-006: Track made by Turn this track into audio is named with a Japanese suffix in English

## Where

crates/meloet-model/src/audio/bounce.rs:17 (`BOUNCED_SUFFIX = "（音）"`)
crates/meloet-model/src/audio/bounce.rs:158-168 (`bounced_name`)

## What happens

With the page in English, 「このトラックを音にする」 ("Turn this track into audio") on a track named "Piano" makes a new vocal track named "Piano（音）" (and "Piano（音） 2" when taken): a Japanese word and full-width brackets in an English screen, shown in the track head, the mixer strip, the history row and the status line "Put audio clips on 2 Piano（音）…".

## What should happen

Architecture 25-36 "Names made in Rust": the language is passed in where a name is made (`vocal_track_name`, `InstrumentRef::track_name`, …), and after `SetLanguage` what is made later follows it; spec 7-readings D4 "Names of new things" gives the English of each name made by meloet. The bounced track's name is made by meloet at that moment, like a new vocal track's "Vocal". The list of things that stay Japanese (folders in a song, file names, logs, the AI link) does not include it.

## How to trigger

Read from the code, not run. 「表示 ▸ ことば / Language ▸ English」; on a keys track with a clip, right-click ▸ "Turn this track into audio"; wait for it to finish.

## Cause

`bounce_track` names the track with the fixed `BOUNCED_SUFFIX` and takes no `Language`, unlike the other name makers of round 12 D4.

## Suggested fix

Pass `meloet_model::Language` into `bounce_track` (from `Play::names`, as `add_vocal_track` gets it) and pick the suffix with `language.pick("（音）", " (audio)")` or another English the owner approves. Needs new English wording (spec 3 has only 「（元の名前）（音）」; spec 7-readings D4 lists the English names, so the lead or owner should choose it). No protocol or song-file change. The work file name (`bounce_file_name`, core play/bounce.rs:71) follows the track name and may stay as it is (file names are out of D4).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
