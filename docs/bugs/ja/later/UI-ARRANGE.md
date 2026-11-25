# UI-ARRANGE — 後で見たい点(バグではない)

- ui/src/components/automation/AutoLane.svelte:226-233 — 録音の枠の `$effect` は `placeRec()` を呼び、それが `playTick()` を呼んで `current.telemetry` を読みます。レーンへ書き込んでいる間は、テレメトリのメッセージ（毎秒約 60 回）が来るたびに effect が動き直し、その都度 `everyGlFrame` を外して付け直し、`drawnWidth` をリセットします。そのため、幅が変わっていなくても、幅が書き直されます。
- ui/src/components/automation/AutoLane.svelte:216-225 — 録音の枠は、書き込み中に毎フレーム `style.width` を書いて広がります（毎フレームのレイアウトです。architecture 7 の規則 (e) では、長さは `transform` で動かします）。`tools/measure/webkit-damage.sh` では計測していません。
- ui/src/components/automation/WriteLine.svelte:35-52 — 書き込み中にループが折り返すと、`sample()` はもう値を取りません（`tick > last.tick` が偽になるためです）。そのため、そのパスの残りの間、枠の中の線は変わらなくなります（画面だけの問題です。コアは値を自分で置きます）。
- ui/src/components/arrange/wavePeaks.svelte.ts:145-147 — `trim()` は、ほかの曲のエントリを捨てますが、`failed` と `tries` は残します。音声 id は曲ごとに一から始まるため、新しい曲の最初のピークは、古い曲の再試行までの待ち時間（バックオフ、最大 8 秒）を待つことがあります。
- ui/src/components/Arrange.svelte:1031-1041 — 歌の録音のラベルの全幅（`labelFullPx`）は、テイルごとに一度だけ測られます。後のラップでテイク番号の桁が増えると、ラベルは一文字短く切られます（鍵のラベルは、新しいテイクのとき測り直します。928 行目）。
- ui/src/components/Arrange.svelte:405-415, 449-465 — クリップ移動中の動く絵は、移動先の行に対して作り直されません。160 の行へ移したクリップは、古い高さのまま描かれます。複数の行にまたがるグループでは、どの行も掴んだクリップの px ぶん動くため、トラックの間にレーンがあると、ドラッグ中にほかのクリップが自分の行からずれて描かれます。
- ui/src/components/Arrange.svelte:520-523 — `lanePicture` は `liveVersion` に依存するため、録音中に届く鍵のたびに、大きな面（すべてのクリップ）全体が作り直されます。確定した部分だけではありません。
- ui/src/components/TrackHeads.svelte:141-148 — 見出しの並べ替えのドラッグ中にホイールを回すと、持ち上げた見出しの下の行がスクロールします（`wheelRows` が止めるのは、クリップのドラッグと範囲選択だけです）。そのため、差し込みの線がポインタと合わなくなります。
- ui/src/components/pianoroll/Roll.svelte:894-901 — ノートのドラッグ中も、ピアノロールは再生の線に合わせてページを送ります（UI-ARRANGE-004 と同じ罠です。この範囲の外の話です）。
