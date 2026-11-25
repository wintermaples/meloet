---
id: CORE-STATE-002
title: Save as opens in the current song's own folder, so the new song folder is made inside the old one
area: CORE-STATE
kind: ui-ux
severity: medium
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
