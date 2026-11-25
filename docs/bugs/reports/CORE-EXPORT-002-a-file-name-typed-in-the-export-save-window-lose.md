---
id: CORE-EXPORT-002
title: A file name typed in the export save window loses the text after its last dot
area: CORE-EXPORT
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# CORE-EXPORT-002: A file name typed in the export save window loses the text after its last dot

## Where

crates/meloet-core/src/export.rs:738-741 (`ExportNews::PlacePicked`: `path.file_stem()` becomes `self.name`)
crates/meloet-core/src/export.rs:896-899 (`file_name`: `format!("{name}.{}", self.format.extension())`)

## What happens

The name chosen in the save window of 「えらぶ…」 is kept as `Path::file_stem`, which cuts off everything after the last dot, whether or not that part is an audio ending. The format's ending is then added. A person who types 「デモ v1.2」 (no ending) gets the file 「デモ v1.wav」; 「Mix 2026.11.25」 becomes 「Mix 2026.11.wav」.
Worse, the save window asked about writing over the name that was typed (`set_do_overwrite_confirmation(true)`, crates/meloet-app/src/pcwindow.rs:132-135), not about the name that is really written. If 「デモ v1.wav」 is already in the folder (an earlier export), it is written over without any question.

## What should happen

Only the format's own ending is replaced ("The ending is the format's, whatever was typed", export/tests.rs, the test of `ChooseExportPlace`). Text that is not an audio ending stays part of the name. The sibling `split_save_as` (crates/meloet-core/src/state/songs.rs:565-571) does this right for 「名前をつけて保存…」: it strips only the `.meloet` suffix and keeps the rest of the name.

## How to trigger

Read from the code, not run.
1. Export once to 「~/音楽/デモ v1.wav」.
2. Open 「書き出し…」 again, press 「えらぶ…」, type 「デモ v1.2」 in the save window and press save (no question: that file does not exist).
3. The dialog's place shows 「~/音楽/デモ v1.wav」; 「書き出す」 writes over the first file.

## Cause

`file_stem()` treats any last dot as the start of an ending. Names with dots (versions, dates) are common.

## Suggested fix

In `PlacePicked`, take the whole file name and strip only an ending meloet writes (`wav`, `flac`, `mp3`, `mid`, any case), as `split_save_as` does for `.meloet`; otherwise keep the whole name. No protocol, wording or looks change. A test: 「デモ v1.2」 gives the place 「…/デモ v1.2.wav」, 「できた.FLAC」 gives 「…/できた.wav」 under wav.

## Verification

Confirmed by reading (no test run). `ExportNews::PlacePicked` (`export.rs:738-741`) keeps `path.file_stem()` as the name, and `file_name` (`export.rs:896-899`) adds the format's ending. `Path::file_stem` cuts at the last dot whatever follows it, so 「デモ v1.2」 becomes 「デモ v1.wav」. GTK's save window asks about writing over the name that was typed (`pcwindow.rs:132-135`), and the export itself moves the finished file over whatever has the final name (`export/job.rs` `move_to`, a plain rename), so the cut name can replace an earlier export with no question. `split_save_as` (`state/songs.rs:565-571`) strips only its own ending.

Severity changed from `low` to `medium`, as the finder also thought: names with a dot (a version, a date) are normal use, the file gets a name the person did not type, and an earlier export under the cut name is replaced without the question the window is there to ask. It is not `high`: what is lost is an exported file, not the song.

Corrections: none.

Fix plan: `crates/meloet-core/src/export.rs` `PlacePicked`: take `path.file_name()` and strip only an ending meloet writes (the endings of `ExportFormat`, any case: wav, flac, mp3, mid); keep everything else. Test in `export/tests.rs` next to the `ChooseExportPlace` test: 「デモ v1.2」 → 「デモ v1.2.wav」; 「できた.FLAC」 under wav → 「できた.wav」.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
