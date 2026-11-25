# UI-ARRANGE — 後で見たい点(バグではない)

- ui/src/components/automation/AutoLane.svelte:216-225 — 録音の枠は、書き込み中に毎フレーム `style.width` を書いて広がります（毎フレームのレイアウトです。architecture 7 の規則 (e) では、長さは `transform` で動かします）。`tools/measure/webkit-damage.sh` では計測していません。
- ui/src/components/Arrange.svelte:520-523 — `lanePicture` は `liveVersion` に依存するため、録音中に届く鍵のたびに、大きな面（すべてのクリップ）全体が作り直されます。確定した部分だけではありません。
