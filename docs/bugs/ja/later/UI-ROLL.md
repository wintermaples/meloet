# UI-ROLL — 後で見たい点(バグではない)

- ui/src/components/pianoroll/Roll.svelte:595-596 — ベロシティのドラッグで値が一段動くたびに `rollDragKey` が変わるため、ベロシティのレーンしか変わらないのに、pianoRoll の領域全体(ノートの領域も含む)が描き直されます。
- ui/src/components/pianoroll/CcLanes.svelte:275-281 — CC のドラッグで、ポインターが変わって動くたびに `placeLive` が `getBoundingClientRect` を二回読みます(動きごとに強制レイアウト)。`Roll.svelte:334-337` の `at(e)` も、ノートの領域でホバーが動くたびに同じことをします。
- ui/src/components/pianoroll/Roll.svelte:894-910 — カウントイン中も `frame()` が動き、キーの列が録音の開始位置にあるノートを光らせます(`shownTick` が `recording.from` を持つため)。音は鳴っていません。仕様 E1 と readings は、カウントイン中はノートを光らせないと書いていますが、キーの列は「これまでどおり光る」とも書かれていて、どちらが優先かは分かりません。
- ui/src/components/pianoroll/paint.ts:165-229 — ドラムのヒットの四角には、ホバーの見た目がありません。仕様 2 は「ドラムのヒットの四角」に、通常・選択・ホバーの三つを挙げています。
- ui/src/components/pianoroll/ChordRow.svelte:66 — コード進行の ▶ は、曲の先頭のテンポ(`timeline.tempoBpm`)で再生します。テンポが変わる曲では、コードが置かれる位置のテンポにはなりません(棚のパターンはそのテンポを使います。readings P4)。
