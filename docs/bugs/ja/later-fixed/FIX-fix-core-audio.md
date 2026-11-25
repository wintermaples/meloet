# FIX-fix-core-audio — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/state.rs `status` (with play.rs `record`) — `StatusMessage::NoInput` は、入力のない録音待ちの歌のトラックがあるときに R を押すと設定されますが、ただの知らせです。`moving_status`（カウントイン、録音）がその前に来るため、鍵の録音中には決して見えず、次の依頼（Space）で消えてしまいます。`VocalsNotRecorded` は、今は `moving_status` より前に専用の場所を持ちます。
- crates/meloet-core/src/made.rs:201 — クリップ用に作った音（のび縮みして別の音の高さにした `stretch/<key>.wav`）のキーは、中身ではなく元の uid を持っています。「さがす…」で中身の違うファイルを取り込んだ後も、のび縮みしたクリップは古いファイルから作られた音を鳴らし続けることがあります（CORE-AUDIO-007 が手放すのは、復号したコピーとピークだけです）。
