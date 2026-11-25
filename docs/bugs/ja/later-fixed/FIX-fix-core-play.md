# FIX-fix-core-play — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/play/clips.rs:56 (`create_clip`) and play/shelf.rs:299 (棚から作る新しいトラック) — 選んだトラックを `selected_track_changed` を呼ばずに設定します。鍵のあるトラックだけを見るので、● が見逃されることはありません。ただし `last_keys` が残り、`create_clip` は `recent_follow_target` を呼びません。目に見える不具合は見つかっていません。
