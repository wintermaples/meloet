# UI-PARITY — 後で見たい点(バグではない)

- ui/src/components/pianoroll/paint.ts:583-597 — 動かした CC の点は、元の場所に gray.7 の枠を残します。一方、オートメーションとテンポの点は accent の枠を残します(仕様 2「オートメーションの点」、7-readings A-20)。仕様 2 には CC の点について何も書かれていないため、小さな点すべてに一つの決まりを作るかどうかは、リードが決めるとよいかもしれません。
- ui/src/components/** — どのドラッグの部品も lostpointercapture や window の blur を聞いていません。ドラッグの途中で pointerup が来ないまま(ウィンドウがフォーカスを失うなど)になると、次に押されるまでドラッグが開いたままになります。どの部品も同じなので、対等性のずれではありませんが、共通の決まりを一つ作る価値はあります。
- ui/src/components/params/PresetMenu.svelte:57-69 — プリセットの一覧はどのキーも止めますが、行の間を ↑ ↓ で動けません。▾ の付く一覧(ChoiceMenu、ZoomMenu、WindowsMenu、MenuList)は ↑ ↓ で動きます。7-readings M1 では、パネルの中に描くと書かれているため、わざと違えているのかもしれません。
- ui/src/components/params/ClipPanel.svelte:124-129 — クリップの「ゲイン」の箱に読めない文字が入っても、何も送らず、何も知らせません。同じグループの位置、長さ、フェードの箱は `rejectValue` を送ります(ClipsPanel のコードのコメントには、これはゲイン用だと書かれています)。
