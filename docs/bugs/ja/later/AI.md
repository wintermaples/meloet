# AI — 後で見たい点(バグではない)

- crates/meloet-ai/src/tools.rs:750, tools/notes.rs:514, tools/midi.rs:392, tools/render.rs:269 — `"id:7"` で与えられたトラックは、AI のスレッドでその番号に変換され、依頼には番号が入ります。コアが依頼を実行する前（バッチの外で）に使う人がトラックを足す、動かす、消すと、その番号は別のトラックを指します。トラックの id を取る依頼（`SetInstrument`、`SetPluginOutput` など）には、この隙間はありません。
- crates/meloet-ai/src/tools/automation.rs:324-373 — `move_automation` と `delete_automation` は、点を添字で指定し、ノートのテイク id のような保護がありません。レーンが変わった後（使う人の操作や別の呼び出しによって）、古い添字は別の点を動かしたり消したりします。
