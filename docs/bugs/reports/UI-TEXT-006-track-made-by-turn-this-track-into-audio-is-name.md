---
id: UI-TEXT-006
title: Track made by Turn this track into audio is named with a Japanese suffix in English
area: UI-TEXT
kind: parity
severity: low
status: fixed
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

Real (read, not run): `bounce_track` names the new track with the fixed `BOUNCED_SUFFIX = "（音）"` (`crates/meloet-model/src/audio/bounce.rs:17`, 158-168) and takes no `Language`, while architecture 25-36 "Names made in Rust" passes the language in wherever meloet makes a name (`vocal_track_name`, `add_vocal_track`, `import_audio` …) and its list of things that stay Japanese (folders in a song, file names, logs, the AI link) does not hold this name. So an English page gets "Piano（音）".

`needs-lead` because the English suffix is new wording. I looked for an existing text: spec 3 (A7) has only 「（元の名前）（音）」 (「ピアノ（音） 2」 when taken); 7-readings D4 "Names of new things" lists the approved English names and has none for this; `ui/src/text/en.ts` has the action ("Turn this track into audio", `menu.bounceTrack`; "Turn into audio clips") but no name form. Rust names are not in `ui/src/text/` anyway (they are picked with `language.pick` in the model).

Recommended wording: Japanese stays 「（音）」; English " (audio)" (a space, half-width brackets), so "Piano (audio)", and "Piano (audio) 2" when taken. Where the words come from: "audio" is the word `en.ts` and `docs/spec/glossary.md` use for 音 in this feature ("Turn this track into audio", "audio clips"); the bracket form is the one of `msg.recordingVocal` "{track} ({input})".

Choices: (a) pass `Language` into `bounce_track` (from `Play::names`, as `add_vocal_track` gets it) and pick the suffix; add the name to 7-readings D4 "Names of new things"; tests of `bounced_name` in both languages. No protocol or song-file change; old songs keep their names (a name never changes by itself). (b) keep 「（音）」 in both languages and add it to the "not changed" list of architecture 25-36. I recommend (a). The work file name (`bounce_file_name`, `crates/meloet-core/src/play/bounce.rs`) follows the track name and needs no change.

## Fix

Lead's decision (a): `bounce_track` takes a `Language` (`crates/meloet-model/src/audio/bounce.rs`; `bounced_name` picks `BOUNCED_SUFFIX` 「（音）」 or " (audio)", same clash rule); `crates/meloet-core/src/play/bounce.rs` passes `self.names`. Songs already saved keep their names. The work file name (`bounce_file_name`) keeps 「（音）」 (file names are outside D4). Documents: architecture 25-36 "Names made in Rust" and A7 line, 7-readings D4 row "The name of a track turned into audio" (decided by the lead, 2026-11-25).
Test: `crates/meloet-model/src/audio/bounce/tests.rs` `in_english_the_name_ends_in_audio_with_the_same_numbers` (the other callers in tests pass `Language::Ja`).
