# SOUND — 後で見たい点(バグではない)

- crates/meloet-sound/src/phrases.rs:229-251 — `PhrasePlayer` は `ask` と `latency` を渡さず、各ブロックを音のフレームで切ります。プラグインを使うサンプルの座席（`service.rs:3691`）では、各部分がプラグインホストとの同期的な往復になり、先読みされることがありません。
