# fix-core-audio — 修正中に見つけたが、直していない点

- crates/meloet-core/src/vocalrec.rs `Side::start` — 録音が始まるときに入力が開いていないと（入力が消えることとの競合）、エラーは同じ `VocalsNotStarted` になるため、状態行には作業場所に書けないと出ます。入力自身の知らせ（`InputLost`）も一緒に出ます。
