# UI-PARITY — 2 回目で直した点 (2026-11-25)

- ui/src/components/mixer/MuteSetBox.svelte:79-96 — 名前の入力枠は body に移され、キーは stopPropagation だけで止められます。そのため keys/attach.ts には Enter が届きます(例: 出口えらびが表示されているときの chooserConfirm)。MetronomeWindow.svelte:130-134 は同じ場合に stopImmediatePropagation を使っています(UI-SHELL-004 と同じ原因)。
- ui/src/components/LatencyDialog.svelte:182-185 — ダイアログの本文は `overflow: auto`(ブラウザの棒)でスクロールします。ほかのダイアログは `ScrollArea` を使います(仕様 2「スクロールの棒」)。内容が短いので、1024×640 ではあふれないかもしれません。
- ui/src/components/SongDialog.svelte:17, LatencyDialog.svelte:56, export/ExportDialog.svelte:146, plugins/ScanStoppedDialog.svelte:29 — `style:color-scheme={metrics.colorScheme}` がありません。SettingsDialog、AboutDialog、KeyListDialog、ExportedDialog はこれを設定しています(テキストボックスのネイティブ部品は、それらの中だけでテーマに合わせて変わります)。見える影響は確認されていません。
- ui/src/components/params/StretchNumber.svelte:117-120 — 無効になった「音の高さ」/「音程を寄せる」の数字欄は、`ns-resize` のカーソルのままです。無効なパラメータのバーは `cursor: default` になります(ParamRow.svelte:297-299)。
- ui/src/components/params/knobRows.ts:42 — `replace("−", "-")` は最初の U+2212 だけを置き換えます(マイナスの符号が一つだけなら害はありません)。
- ui/src/components/ChoiceMenu.svelte:69-71, ZoomMenu.svelte:37-39, WindowsMenu.svelte:27-28 — フォーカスが行ではなく一覧の箱そのものにあるとき、↑ は最初の行に進みます。MenuList.svelte:101-102 は最後の行に進みます。最初のフォーカスが行に届かなかったときだけ起こります。
