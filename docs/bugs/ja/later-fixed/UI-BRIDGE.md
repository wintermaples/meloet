# UI-BRIDGE — 2 回目で直した点 (2026-11-25)

- ui/src/gl/Surface.svelte:72-76 — `$effect` が `current.state` を読むため、どの区分の更新のあとでも `engine.setScale` が走ります。`setScale` は `touch(null)` を呼ぶので、どの領域も汚れた扱いになります。そのため尺度が変わっていなくても、更新のたびにすべての枠の箱を測り直し(`getBoundingClientRect`)、絵をすべて作り直します。費用がかかるだけです。絵を比べるので、描き直しは止まります。
- ui/src/bridge/fakeAudio.ts:137 — `fakeAudioClip` は `takes: []` を入れていますが、Rust は表示中のテイクを必ず `AudioClipView.takes` に入れます(プロトコルの説明は「表示中のものも含む」)。今のところ画面のコードはこれに頼っていません。ただ、フェイクを使うテストでは、表示中のテイクを `takes` の中から探す部品の不具合を見逃します。
- ui/src/lib/HScrollbar.svelte:3-5 — ヘッダーのコメントでは、つまみは乗ったときが gray.8、動かしているときが gray.9 と書かれています。しかし CSS(と 7-readings の「場所ごとに地か墨(Fill or ink, per place)」)では accent と accent.ink を使います。古いコメントです。
