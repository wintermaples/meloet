---
id: CORE-STATE-002
title: Save as opens in the current song's own folder, so the new song folder is made inside the old one
area: CORE-STATE
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-STATE-002: Save as opens in the current song's own folder, so the new song folder is made inside the old one

## Where

crates/meloet-core/src/service.rs:3235-3240 (`Effect::PickSaveAs` uses `self.start_folder()`)
crates/meloet-core/src/service.rs:3798-3806 (`start_folder`: the parent of the song file, which is the song folder `<songs>/<name>/` itself)

## What happens

For a song that was saved before, 「名前をつけて保存…」 opens the PC's save window in the song folder `<songs>/<name>/` (where `<name>.meloet` and `音/` are). A person who only types a new name and presses save gets `<songs>/<name>/<new name>/<new name>.meloet`: the new song folder sits inside the old song's folder, next to its `音/`. Deleting or moving the old song folder later takes the new song with it.

## What should happen

docs/spec/3-bands.md 89: 「名前をつけて保存」 "makes a folder named 「名前」 in the chosen place". docs/architecture.md 14: a song is a song folder in `<music>`, one folder per song. The place the window offers first should be the place that holds song folders: the folder that holds the current song folder (or the songs folder), not the inside of a song folder. The first save already goes to `<songs>/<name>/` without asking.

## How to trigger

Read from the code, not run.
1. Save a new song: it is `<songs>/はじめての曲/はじめての曲.meloet`.
2. Menu 「ファイル ▸ 名前をつけて保存…」: `ShellRequest::PickSaveAs { folder: Some(<songs>/はじめての曲) }`; the shell calls `set_current_folder` with it (crates/meloet-app/src/pcwindow.rs:124-126).
3. Type 「夏の曲」 and press save: `split_save_as` gives that folder, and `save_new_with` makes `<songs>/はじめての曲/夏の曲/夏の曲.meloet`.

## Cause

`start_folder` was written for 「ひらく…」 and the folder window of 「保存できない」 (it is the folder of the song file in use), and `Effect::PickSaveAs` reuses it. For a save that makes a new song folder in the chosen place, the folder of the song file is one level too deep.

## Suggested fix

For `Effect::PickSaveAs` start in the parent of the song folder (`song_path().parent().parent()`), else the songs folder; keep `start_folder` as it is for the open window. The folder window of 「保存できない」 (`Effect::PickFolder`) also makes a new song folder in the chosen place, so the same start fits it; the lead may decide that one. No protocol, wording or file-shape change.

## Verification

Confirmed by reading (no test run). `Effect::PickSaveAs` (`service.rs:3235-3240`) starts the save window in `start_folder()`, the parent of the song file, which for a saved song is its own song folder `<songs>/<name>/`. `split_save_as` (`state/songs.rs:565-571`) takes the parent of the path the window gives as the place and makes the new song folder there, so a new name typed without changing the folder lands in `<songs>/<name>/<new name>/`. Architecture 18 ("Save as") and 14 (one folder per song in the songs folder) and spec 3 ("makes a folder named 「名前」 in the chosen place") do not say where the window starts; nothing says the inside of a song folder is meant. Severity `medium` stands (the plain use of the command gives a song inside another song's folder).

Corrections: none.

Fix plan: `crates/meloet-core/src/service.rs`: a second helper for windows that make a song folder in the chosen place: the folder that holds the current song folder (`song_path().parent().parent()`), else the songs folder, each only when it is a directory (see CORE-STATE-005 for the order of the filter). `Effect::PickSaveAs` uses it; `Effect::PickFolder` (「保存できない」 → 「場所をえらぶ」, which also makes a new song folder in the chosen place) uses it too. 「ひらく…」 keeps `start_folder`. A small unit test of the helper with a temp folder.

## Fix

`crates/meloet-core/src/service.rs`: a second helper `Worker::place_folder` (the folder that holds the song's folder, else the songs folder, each only when it is a directory; the shared filter is `first_folder`). `Effect::PickSaveAs` and `Effect::PickFolder` (「保存できない」 → 「場所をえらぶ」) use it; 「ひらく…」 keeps `start_folder`. `docs/architecture.md` 18 says where the windows start.

Test: `service::shell_tests::the_pcs_windows_start_among_the_song_folders_or_in_the_songs_folder` (`service/shell_tests.rs`, a new test file of the core with a shell that records what it is asked).
