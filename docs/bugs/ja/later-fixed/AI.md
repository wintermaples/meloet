# AI — 2 回目で直した点 (2026-11-25)

- crates/meloet-ai/src/lib.rs:286 — `accept` のエラーは、すぐに `continue` されます。エラーが繰り返される場合（プロセスのファイルハンドルが尽きたときの EMFILE）、リンクのスレッドがビジーループになります。エラーの後に短く sleep すれば済みます。
- crates/meloet-ai/src/tools/render.rs:133-149, tools/plugstate.rs:311 — `scratch.clean`（`render_audio` のフォルダを `remove_dir_all` することがあります）と `scratch.dir` は、ブロッキング用のプールではなく、リンクの一つのランタイムスレッドで動きます。大きなフォルダがあると、その間すべての接続が止まります。
- crates/meloet-ai/src/surge/sources.rs:1 — モジュールのコメントは、Surge XT 1.3.x にはソースが 41 個あると書いています。`SOURCES` は 40 個（id 1〜40）を持ちます。
- crates/meloet-ai/src/tools/midi.rs:458 — エラーは、名前が `.mid` で終わる必要があると言いますが、`.midi` も受け付けられています（`is_midi_name`）。

## 見直したら本当ではなかった点 (2026-11-25)

- crates/meloet-core/src/play/timeline.rs:405-420 — 使う人が K で作ったばかりの（まだ「新しい」状態の）マーカーに対して、AI の `rename_marker` を実行すると、その手を書き換えます。取り消しの数は変わらず、コアは `Unchanged` を返すため、名前は変わったのにツールは「変更なし」と言います。
