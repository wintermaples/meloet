# fix-core-audio — 修正中に見つけたが、直していない点

- crates/meloet-core/src/state.rs `status` (with play.rs `record`) — `StatusMessage::NoInput` は、入力のない録音待ちの歌のトラックがあるときに R を押すと設定されますが、ただの知らせです。`moving_status`（カウントイン、録音）がその前に来るため、鍵の録音中には決して見えず、次の依頼（Space）で消えてしまいます。`VocalsNotRecorded` は、今は `moving_status` より前に専用の場所を持ちます。
- crates/meloet-core/src/made.rs:201 — クリップ用に作った音（のび縮みして別の音の高さにした `stretch/<key>.wav`）のキーは、中身ではなく元の uid を持っています。「さがす…」で中身の違うファイルを取り込んだ後も、のび縮みしたクリップは古いファイルから作られた音を鳴らし続けることがあります（CORE-AUDIO-007 が手放すのは、復号したコピーとピークだけです）。
- crates/meloet-core/src/vocalrec.rs `Side::start` — 録音が始まるときに入力が開いていないと（入力が消えることとの競合）、エラーは同じ `VocalsNotStarted` になるため、状態行には作業場所に書けないと出ます。入力自身の知らせ（`InputLost`）も一緒に出ます。
