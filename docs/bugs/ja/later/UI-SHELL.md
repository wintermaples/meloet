# UI-SHELL: 後で見たい点(バグではない)

- ui/src/components/SettingsDialog.svelte:171-177 — 「くわしい設定」(「◂」のある 2 段目)で Esc を押すと、ダイアログ全体が閉じます(コアの `Back` は `self.dialog` を取ります。crates/meloet-core/src/state.rs:1702-1710)。仕様 4 の Esc の行には「2 段目では 1 段目へ」とありますが、それは About のために書かれたもので、くわしい設定も含むかどうかは分かりません。
- ui/src/keys/attach.ts:538-540 — くわしい設定の 2 段目では、明るいボタンは「閉じる」(`settingsCancel`、SettingsDialog.svelte:385-386)ですが、Enter は `settingsApply` を送ります。そのため、1 段目で選んだ出口の下書きは、Enter では適用され、ボタンでは捨てられます。
- ui/src/components/history/historyUi.svelte.ts:29 — ↓(古い方)を、何も選ばれておらず、すべて取り消された状態(current 0)で押すと、1 行目が選ばれます。これは「すべて取り消した」状態より新しい行です。
- ui/src/components/StatusLine.svelte:420-421 — `lengthMatched` は、曲の始めの拍子(`app.timeline.meterNum/Den`)で長さを書きます。拍子が途中で変わる曲では、音符の場所の拍子ではありません。
- ui/src/components/StatusLine.svelte:892-902 — 止まっているとき、押さえているキーの効果がテレメトリのメッセージのたびに走り、何も押さえていなくても 128 個のキーを調べます。
- ui/src/components/SoundHint.svelte:25-28 — 文は固定の `" "` でつながれていて、`SENTENCE_JOIN`(日本語では空)を使いません。モック F-Connect3 と照らし合わせてください。
- ui/src/keys/attach.ts:511-541 — ダイアログの中でフォーカスのあるボタン(例: 曲のダイアログの「キャンセル」)で Enter を押すと、明るいボタンが押されます。Space では演奏や停止が行われます。これは仕様 3「Enter は右端のボタン」に従っていますが、ほかのボタンをキーで押す方法がありません。
