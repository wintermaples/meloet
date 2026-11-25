# AI — 後で見たい点(バグではない)

- crates/meloet-ai/src/lib.rs:286 — `accept` のエラーは、すぐに `continue` されます。エラーが繰り返される場合（プロセスのファイルハンドルが尽きたときの EMFILE）、リンクのスレッドがビジーループになります。エラーの後に短く sleep すれば済みます。
- crates/meloet-ai/src/tools.rs:750, tools/notes.rs:514, tools/midi.rs:392, tools/render.rs:269 — `"id:7"` で与えられたトラックは、AI のスレッドでその番号に変換され、依頼には番号が入ります。コアが依頼を実行する前（バッチの外で）に使う人がトラックを足す、動かす、消すと、その番号は別のトラックを指します。トラックの id を取る依頼（`SetInstrument`、`SetPluginOutput` など）には、この隙間はありません。
- crates/meloet-ai/src/tools/automation.rs:324-373 — `move_automation` と `delete_automation` は、点を添字で指定し、ノートのテイク id のような保護がありません。レーンが変わった後（使う人の操作や別の呼び出しによって）、古い添字は別の点を動かしたり消したりします。
- crates/meloet-ai/src/tools/render.rs:133-149, tools/plugstate.rs:311 — `scratch.clean`（`render_audio` のフォルダを `remove_dir_all` することがあります）と `scratch.dir` は、ブロッキング用のプールではなく、リンクの一つのランタイムスレッドで動きます。大きなフォルダがあると、その間すべての接続が止まります。
- crates/meloet-ai/src/surge/sources.rs:1 — モジュールのコメントは、Surge XT 1.3.x にはソースが 41 個あると書いています。`SOURCES` は 40 個（id 1〜40）を持ちます。
- crates/meloet-ai/src/tools/midi.rs:458 — エラーは、名前が `.mid` で終わる必要があると言いますが、`.midi` も受け付けられています（`is_midi_name`）。
- crates/meloet-core/src/play/timeline.rs:405-420 — 使う人が K で作ったばかりの（まだ「新しい」状態の）マーカーに対して、AI の `rename_marker` を実行すると、その手を書き換えます。取り消しの数は変わらず、コアは `Unchanged` を返すため、名前は変わったのにツールは「変更なし」と言います。
