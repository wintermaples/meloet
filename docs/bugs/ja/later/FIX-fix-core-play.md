# FIX-fix-core-play — 後で見たい点(修正中に見つけたが、直していない)

- crates/meloet-core/src/play/autosync.rs:187-208 — `all`(別の曲)のとき、`before` は常に空です。そのため、古い曲のレーンが動かしたパラメータを戻すものがありません。別の曲へ移るときも座席に残るプラグイン音源(`play/plugs.rs` の `seat_instrument` の `Want::Kept`、同じプラグインが同じ保存状態で残る場合)は、古いレーンが残した値のままになるかもしれません。コードを読んで判断したもので、実行はしていません。
- crates/meloet-core/src/play/clips.rs:56 (`create_clip`) and play/shelf.rs:299 (棚から作る新しいトラック) — 選んだトラックを `selected_track_changed` を呼ばずに設定します。鍵のあるトラックだけを見るので、● が見逃されることはありません。ただし `last_keys` が残り、`create_clip` は `recent_follow_target` を呼びません。目に見える不具合は見つかっていません。
