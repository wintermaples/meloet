# UI-ROLL — 2 回目で直した点 (2026-11-25)

- ui/src/components/pianoroll/sounding.ts:137 — `soundingNotes` は、再生中、毎フレーム take の先頭(index 0)から歩きます(1 万ノートの take の終わり近くでは、1 フレームあたり約 1 万ステップ)。章 7 は、フレームごとの作業を、たまったものではなく変わったものに合わせるよう求めています。
- ui/src/components/pianoroll/shown.svelte.ts:299-306 — 拒まれた `takeNotes` を捕まえていません。前の take(別のクリップのものかもしれません)が、次の状態の変化まで表示されたままになります。
- ui/src/components/pianoroll/paint.ts:353-369 — ドラムの行では、キットにない鍵のノートにも、ベロシティのレーンが棒を描きます(`drag.ts:258-283` でドラッグもできます)。ノートの領域はそれを表示しません(仕様 3: そのようなノートは「ピアノロールに表示されない」)。
- ui/src/components/pianoroll/NoteInspector.svelte:135-140 — `bulkDrafts` は、`bulkSpecs` が作り直されるたびに設定し直されます(pianoRoll の新しい区間や take があるとき)。そのため、変更をまとめて行う行で入力中の文字は、途中で差分が届くと(AI の連携による編集や、別の場所からの選択など)失われます。
- ui/src/components/pianoroll/Roll.svelte:978-979 — `labelW` / `markerW` は、表示されたことのある拍のラベルとマーカーごとに一件ずつ保持します。スクロールすると増えていき、削られることがありません。
