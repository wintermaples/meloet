---
id: CORE-STATE-005
title: The PC's file windows start nowhere, not in the songs folder, when the song's folder is gone
area: CORE-STATE
kind: bug
severity: low
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
