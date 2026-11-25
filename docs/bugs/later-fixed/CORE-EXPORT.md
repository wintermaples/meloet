# CORE-EXPORT — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/export/job.rs:279-293 — CancelExport that lands after the last `move_to` but before `Finished` is read leaves the files in place while the run says 「途中までのファイルは消しました」 (tiny window).
- crates/meloet-core/src/export.rs:979-984 — `can_export` for MIDI does not need the dialog open nor `!running()`: a second StartExport (fast double press) writes the MIDI files again, and one arriving while an audio run goes would replace `Run::Going` without a `Drop`.
- crates/meloet-core/src/export/job.rs:259-264 — 「トラックごと」 writes over files of the same name in the chosen folder without asking (the single-file save window asks).
