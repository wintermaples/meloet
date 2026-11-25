# UI-ROLL — 後で見たい点(バグではない)

- ui/src/components/pianoroll/sounding.ts:137 — `soundingNotes` は、再生中、毎フレーム take の先頭(index 0)から歩きます(1 万ノートの take の終わり近くでは、1 フレームあたり約 1 万ステップ)。章 7 は、フレームごとの作業を、たまったものではなく変わったものに合わせるよう求めています。
- ui/src/components/pianoroll/Roll.svelte:595-596 — ベロシティのドラッグで値が一段動くたびに `rollDragKey` が変わるため、ベロシティのレーンしか変わらないのに、pianoRoll の領域全体(ノートの領域も含む)が描き直されます。
- ui/src/components/pianoroll/CcLanes.svelte:275-281 — CC のドラッグで、ポインターが変わって動くたびに `placeLive` が `getBoundingClientRect` を二回読みます(動きごとに強制レイアウト)。`Roll.svelte:334-337` の `at(e)` も、ノートの領域でホバーが動くたびに同じことをします。
- ui/src/components/pianoroll/Roll.svelte:894-910 — カウントイン中も `frame()` が動き、キーの列が録音の開始位置にあるノートを光らせます(`shownTick` が `recording.from` を持つため)。音は鳴っていません。仕様 E1 と readings は、カウントイン中はノートを光らせないと書いていますが、キーの列は「これまでどおり光る」とも書かれていて、どちらが優先かは分かりません。
- ui/src/components/pianoroll/shown.svelte.ts:299-306 — 拒まれた `takeNotes` を捕まえていません。前の take(別のクリップのものかもしれません)が、次の状態の変化まで表示されたままになります。
- ui/src/components/pianoroll/paint.ts:353-369 — ドラムの行では、キットにない鍵のノートにも、ベロシティのレーンが棒を描きます(`drag.ts:258-283` でドラッグもできます)。ノートの領域はそれを表示しません(仕様 3: そのようなノートは「ピアノロールに表示されない」)。
- ui/src/components/pianoroll/paint.ts:165-229 — ドラムのヒットの四角には、ホバーの見た目がありません。仕様 2 は「ドラムのヒットの四角」に、通常・選択・ホバーの三つを挙げています。
- ui/src/components/pianoroll/NoteInspector.svelte:135-140 — `bulkDrafts` は、`bulkSpecs` が作り直されるたびに設定し直されます(pianoRoll の新しい区間や take があるとき)。そのため、変更をまとめて行う行で入力中の文字は、途中で差分が届くと(AI の連携による編集や、別の場所からの選択など)失われます。
- ui/src/components/pianoroll/Roll.svelte:978-979 — `labelW` / `markerW` は、表示されたことのある拍のラベルとマーカーごとに一件ずつ保持します。スクロールすると増えていき、削られることがありません。
- ui/src/components/pianoroll/ChordRow.svelte:66 — コード進行の ▶ は、曲の先頭のテンポ(`timeline.tempoBpm`)で再生します。テンポが変わる曲では、コードが置かれる位置のテンポにはなりません(棚のパターンはそのテンポを使います。readings P4)。
