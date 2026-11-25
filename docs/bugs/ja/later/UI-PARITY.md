# UI-PARITY — 後で見たい点(バグではない)

- ui/src/components/pianoroll/paint.ts:583-597 — 動かした CC の点は、元の場所に gray.7 の枠を残します。一方、オートメーションとテンポの点は accent の枠を残します(仕様 2「オートメーションの点」、7-readings A-20)。仕様 2 には CC の点について何も書かれていないため、小さな点すべてに一つの決まりを作るかどうかは、リードが決めるとよいかもしれません。
- ui/src/components/mixer/MuteSetBox.svelte:79-96 — 名前の入力枠は body に移され、キーは stopPropagation だけで止められます。そのため keys/attach.ts には Enter が届きます(例: 出口えらびが表示されているときの chooserConfirm)。MetronomeWindow.svelte:130-134 は同じ場合に stopImmediatePropagation を使っています(UI-SHELL-004 と同じ原因)。
- ui/src/components/** — どのドラッグの部品も lostpointercapture や window の blur を聞いていません。ドラッグの途中で pointerup が来ないまま(ウィンドウがフォーカスを失うなど)になると、次に押されるまでドラッグが開いたままになります。どの部品も同じなので、対等性のずれではありませんが、共通の決まりを一つ作る価値はあります。
- ui/src/components/LatencyDialog.svelte:182-185 — ダイアログの本文は `overflow: auto`(ブラウザの棒)でスクロールします。ほかのダイアログは `ScrollArea` を使います(仕様 2「スクロールの棒」)。内容が短いので、1024×640 ではあふれないかもしれません。
- ui/src/components/SongDialog.svelte:17, LatencyDialog.svelte:56, export/ExportDialog.svelte:146, plugins/ScanStoppedDialog.svelte:29 — `style:color-scheme={metrics.colorScheme}` がありません。SettingsDialog、AboutDialog、KeyListDialog、ExportedDialog はこれを設定しています(テキストボックスのネイティブ部品は、それらの中だけでテーマに合わせて変わります)。見える影響は確認されていません。
- ui/src/components/params/StretchNumber.svelte:117-120 — 無効になった「音の高さ」/「音程を寄せる」の数字欄は、`ns-resize` のカーソルのままです。無効なパラメータのバーは `cursor: default` になります(ParamRow.svelte:297-299)。
- ui/src/components/params/PresetMenu.svelte:57-69 — プリセットの一覧はどのキーも止めますが、行の間を ↑ ↓ で動けません。▾ の付く一覧(ChoiceMenu、ZoomMenu、WindowsMenu、MenuList)は ↑ ↓ で動きます。7-readings M1 では、パネルの中に描くと書かれているため、わざと違えているのかもしれません。
- ui/src/components/params/knobRows.ts:42 — `replace("−", "-")` は最初の U+2212 だけを置き換えます(マイナスの符号が一つだけなら害はありません)。
- ui/src/components/params/ClipPanel.svelte:124-129 — クリップの「ゲイン」の箱に読めない文字が入っても、何も送らず、何も知らせません。同じグループの位置、長さ、フェードの箱は `rejectValue` を送ります(ClipsPanel のコードのコメントには、これはゲイン用だと書かれています)。
- ui/src/components/ChoiceMenu.svelte:69-71, ZoomMenu.svelte:37-39, WindowsMenu.svelte:27-28 — フォーカスが行ではなく一覧の箱そのものにあるとき、↑ は最初の行に進みます。MenuList.svelte:101-102 は最後の行に進みます。最初のフォーカスが行に届かなかったときだけ起こります。
