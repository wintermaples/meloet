# UI-BRIDGE (ui/src/bridge, gl, state, time, lib, design, dev) — 後で見たい点(バグではない)

- ui/src/bridge/index.ts:72-74 — `pickTransport` は `createFakeTransport` をビルド時の制限なしに呼ぶので、偽のトランスポートとすべての場面(bridge/fake*.ts、約 9000 行)がリリースのページに入ります。アプリでは実行されません(`__TAURI_INTERNALS__` は常にあります)。大きさだけの問題です。
- ui/src/bridge/peaks.ts:152-175 — 伸びていく録音の `more()` は、`WAVE_POLL_MS`(1 秒に 10 回)の取得のたびに、持っている波の山をすべて新しい Int16Array にコピーします。そのため録音が長いほど、コピーが増えます(レベル 0 のステレオ 10 分のテイクで、取得ごとに約 0.9 MB です)。
- ui/src/gl/pool.ts:139-143 — プールに戻されたキャンバスは、別の領域がそれを使い、絵が上限を超えるまで、波のテクスチャを保ちます(キャンバスごとに最大 `WAVE_TEX_KEEP_BYTES`、16 MB の GPU メモリです)。
- ui/src/state/app.svelte.ts:70-72 — 接続のあとのエラー(もう一度取る状態の取得が失敗した、テレメトリが読めない、送信が断られた)は、`current.failure` を設定するだけです。App.svelte はそれを、状態がないときにだけ表示します。そのため、画面が古いかもしれないことは使う人に知らされません。使ったときに見えた不具合ではなく、設計の問いです。
- ui/src/dev/watch.ts:47 — ページのすべてのキャンバスに `getContext("webgl2")` を呼びます。まだ文脈のないキャンバスには、新しい WebGL の文脈が作られ、予算に数えられます。今はページのすべてのキャンバスに文脈があるので(WebGL か bitmaprenderer)、何も作られません。開発だけの問題です。
