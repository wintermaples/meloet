# CORE-EXPORT — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/export/job.rs:279-293 — 最後の `move_to` の後、`Finished` が読まれる前に届いた CancelExport は、ファイルを残したまま、実行結果には「途中までのファイルは消しました」と出ます（とても短い区間です）。
- crates/meloet-core/src/export.rs:979-984 — MIDI の `can_export` は、ダイアログが開いていることも `!running()` も必要としません。二回目の StartExport（素早い二度押し）は MIDI ファイルを書き直し、音声の実行中に届いたものは、`Drop` なしに `Run::Going` を置き換えてしまいます。
- crates/meloet-core/src/export/job.rs:259-264 — 「トラックごと」は、選んだフォルダの同じ名前のファイルを、確認なしに上書きします（一つのファイルとして保存するときの保存ウィンドウは確認します）。
