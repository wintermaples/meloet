---
id: CORE-STATE-005
title: The PC's file windows start nowhere, not in the songs folder, when the song's folder is gone
area: CORE-STATE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-STATE-005: The PC's file windows start nowhere, not in the songs folder, when the song's folder is gone

## Where

crates/meloet-core/src/service.rs:3798-3806 (`start_folder`)

## What happens

When the song has a file but its folder is not there any more (a USB stick pulled out, the folder moved or deleted in the file manager), `start_folder` returns `None`, so 「ひらく…」, 「名前をつけて保存…」 and the folder window of 「保存できない」 open wherever GTK starts by itself, not in the songs folder. This is exactly the moment 「保存できない」 comes up (the save over the file failed because the folder is gone), and the person is left to find the songs folder alone.

## What should happen

Its own doc comment: "The folder of the song file in use, or else the songs folder." With the song's folder gone, the songs folder (`<music>`, docs/architecture.md 14).

## How to trigger

Read from the code, not run.
1. Save a song, then remove its folder `<songs>/<name>/` outside meloet (or save it on a USB stick and pull the stick out).
2. Press Mod S: `SaveTo::File` fails, 「保存できない」 comes up.
3. 「場所をえらぶ」: `Effect::PickFolder` → `start_folder()` is `None` → `ShellRequest::PickFolder { folder: None }`, so the shell does not call `set_current_folder` (crates/meloet-app/src/pcwindow.rs:124-126).

## Cause

`.or_else(|| self.songs_dir.clone())` comes before `.filter(|d| d.is_dir())`: the song's folder is taken when the song has a path at all, and is then thrown away by the filter, with nothing to fall back to.

## Suggested fix

Filter the song's folder before falling back: `self.model.song_path().and_then(|p| p.parent()).filter(|d| d.is_dir()).map(PathBuf::from).or_else(|| self.songs_dir.clone().filter(|d| d.is_dir()))`. No protocol, wording or file-shape change.

## Verification

Confirmed by reading (no test run). `start_folder` (`service.rs:3798-3806`): `song_path().and_then(parent).map(PathBuf::from).or_else(|| songs_dir.clone()).filter(|d| d.is_dir())`. When the song has a path, the `or_else` is never taken, and a folder that is gone is then thrown away by the filter: `None`, not the songs folder its own comment promises. Severity `low` stands.

Corrections: none.

Fix plan: `crates/meloet-core/src/service.rs` `start_folder`: filter each choice before falling back (`…parent().filter(|d| d.is_dir()).map(PathBuf::from).or_else(|| self.songs_dir.clone().filter(|d| d.is_dir()))`). Do it together with CORE-STATE-002, which adds a second helper with the same shape.

## Fix

`crates/meloet-core/src/service.rs` `start_folder`: each choice is filtered before the fall back (`first_folder`: the song's folder when it is a directory, else the songs folder when it is one). Done together with CORE-STATE-002 (`place_folder` uses the same filter).

Test: `service::shell_tests::the_pcs_windows_start_among_the_song_folders_or_in_the_songs_folder` (the part after the song's folder is removed: 「ひらく…」, 「名前をつけて保存…」 and 「保存できない」 → 「場所をえらぶ」 start in the songs folder).
