---
id: CORE-STATE-003
title: Closing the window or Quit while a save is being written is dropped without a word
area: CORE-STATE
kind: ui-ux
severity: low
status: confirmed
found: 2026-11-25
---

# CORE-STATE-003: Closing the window or Quit while a save is being written is dropped without a word

## Where

crates/meloet-core/src/state/songs.rs:454-457 (`close_requested`: `if self.files.exiting || self.files.waiting.is_some() { return; }`)
crates/meloet-core/src/service.rs:3716-3785 (`save_song`: a song with plugins or audio is written on the `meloet-save` thread; the answer comes later)

## What happens

While a song with plugins (up to `STATE_WAIT` = 3 s for their states) or with audio (copying the audio into `音/`, the status line says 「保存しています ・ 音のファイルを写しています …」, can take many seconds) is being written, the window's close button and 「meloet を終わる」 (`Command::Quit`) do nothing: the request is thrown away. When the save ends, meloet stays open; the person has to close it again and does not know why the first one did nothing.

## What should happen

A close is kept and done once the song is free, as for a recording: docs/architecture.md 14 "A close request stops recording, puts what was recorded into the song, and then decides" (`close_when_recorded`, songs.rs:447-452). The doc comment of `Waiting` (songs.rs:37) says "Song commands wait meanwhile", not that they are lost. After the save has answered, the usual flow applies: no unsaved changes → end; changes made during the save → 「保存していない変更があります」.

## How to trigger

Read from the code, not run.
1. Open a song with audio clips (or a plugin) and press Mod S: `Effect::SaveSong` starts the save thread, `files.waiting = Some(Waiting::Save(Stay))`.
2. While the status line still says 「保存しています …」, click the window's close button (`Input::CloseRequested`) or choose 「meloet を終わる」.
3. Nothing happens, then or after `Input::SongSaved` comes.

## Cause

`close_requested` returns at once when `files.waiting` is set, and nothing remembers the request (unlike the busy case right below it, which sets `files.close_when_recorded`).

## Suggested fix

When `files.waiting` is `Some(Waiting::Save(_))`, set a flag (or reuse `close_when_recorded`) and call `close_requested` again once the save has answered (`SongSaved` / `SongNotSaved` in `song_input`; after `SongNotSaved` the 「保存できない」 dialog may take precedence). No protocol, wording or file-shape change.

## Verification

Confirmed by reading (no test run). `close_requested` (`state/songs.rs:454-457`) returns at once when `files.waiting.is_some()`, and nothing keeps the request; the case right below it (a recording) keeps it in `files.close_when_recorded`. A save of a song with plugins or audio answers later (`service.rs` `save_song` on its own thread), so a close or 「meloet を終わる」 in that time is lost. Severity `low` stands (the way around is to close again).

Corrections: `files.waiting` is also set while an open or another file job waits; the fix below is for the save, and a close during the others can be kept the same way if it costs nothing more.

Fix plan: `crates/meloet-core/src/state/songs.rs`: in `close_requested`, when something waits, set a flag (a new `close_when_free`, or reuse `close_when_recorded` if its reader is called after the save's answer too) and return; where `SongSaved` / `SongNotSaved` (and the other answers that clear `files.waiting`) are handled, call `close_requested` again when the flag is set and no dialog came up from the answer (after `SongNotSaved` the 「保存できない」 dialog comes first and the flag is dropped). Test in `state/songs/tests.rs`: `CloseRequested` during a waiting save, then `SongSaved` → `Effect::Exit`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
