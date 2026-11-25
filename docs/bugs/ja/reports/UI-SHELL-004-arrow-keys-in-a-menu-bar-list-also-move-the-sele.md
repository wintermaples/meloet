---
id: UI-SHELL-004
title: メニューバーの一覧で矢印キーを押すと、ピアノロールで選んだノートも動いてしまう
area: UI-SHELL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-SHELL-004: メニューバーの一覧で矢印キーを押すと、ピアノロールで選んだノートも動いてしまう

## Where

ui/src/components/menu/MenuList.svelte:90-118 (`onkeydown` は `stopImmediatePropagation()` ではなく `ev.stopPropagation()` で終わる)
ui/src/components/menu/MenuBar.svelte:31-34, 103 (一覧は `body` へ移される) と ui/src/components/menu/ContextMenu.svelte (同じ)
ui/src/keys/attach.ts:452-583 (`document` の `handleKeydown` は、`ev.defaultPrevented` も、フォーカスがメニューの中にあるかも見ない)
ui/src/keys/keys.ts:402-403 (↑ ↓ ← → は `nudge` になる)

## What happens

メニューバーの一覧と右クリックの一覧は `body` へ移される。Svelte 5.57.3 は、そのようなノードの委譲された `keydown` を、`document` に付けたリスナーで受け取る (node_modules/svelte/src/internal/client/render.js:230-250)。このリスナーは、`attachKeys` が `document` に自分のリスナーを付けるより前、マウント時に付けられている。MenuList の `stopPropagation()` は、同じノードにある別のリスナーを止めない。そのため、一覧の中で ↑ ↓ ← → と Enter を押すたびに `handleKeydown` も走る。
- ピアノロールが開いていて、最後に押したのがピアノロールで、ノートが選ばれているとき、一覧で ↑ ↓ を押すと選んだノートが半音ずつ動き、← → を押すと 1 グリッドずつ動く (動かすたびに `moveNotes` が 1 回送られ、動いたキーの試し弾きも鳴る)。
- 棚にキーがあるとき、↑ ↓ は試し弾きを変える。出口えらびが出ているとき、↑ ↓ はその強調を動かす。
- 音の設定が開いているとき、一覧の行で Enter を押すと `settingsApply` も送られる。

## What should happen

開いている一覧の中で使うキーは、その一覧のものである (仕様 2 「一覧」: ↑ ↓ で行を動かし、Enter で行の操作をし、← → で下の一覧を開いたり閉じたりする)。曲を変えてはならない。`body` にあるノードについて、その正しいやり方は MetronomeWindow.svelte:130-134 に示されている: `stopImmediatePropagation()` を使う。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。
1. ピアノロールを開く (E)。クリックしてから、ノートをいくつか選ぶ。
2. F10 を押す (または「ファイル」をクリックする)。↓ を 3 回押す。フォーカスは行を下へ動き、選んだノートは 3 半音下がる (取り消しの手が 3 つできる)。
3. → を押して次のメニューへ移る。ノートも 1 グリッド右へ動く。
ui/tests/menu.test.ts:581-630 のテストはこれを見つけられない。そこではピアノロールが閉じているので、矢印キーを押しても動かすための命令がない。

## Cause

`document` のリスナーの順序: Svelte の委譲ハンドラー (MenuList の `onkeydown` を呼ぶ。これは `stopPropagation` を呼ぶだけ) が先に走る。その後、keys/attach.ts の `handleKeydown` が、同じノードの上のリスナーなので、それでも走る。`handleKeydown` は `ev.defaultPrevented` を見ない。docs/progress.md には、`ChoiceMenu` と拡大の一覧について、同じ仕組みが「害があるかは確かめていない」と書かれている。ここでその害が出ている。メニューバーと右クリックの一覧である。

## Suggested fix

MenuList の `onkeydown` で、受け取るキーについて `ev.stopImmediatePropagation()` を呼ぶ (MetronomeWindow と同じ)。または、`handleKeydown` が `ev.defaultPrevented` が true のときすぐ戻るようにする (これで ChoiceMenu と拡大の一覧も入る)。プロトコル、文言、見た目は変えない。

## Verification

テストを実行して確かめた (作業用の worktree で、残していない): 実際のアプリと同じように `#app` の中に `MenuBar` をマウントした (一覧は自分で `body` へ移る)。`handleKeydown` は `document` に付いている。ピアノロールを開き、ノートを 1 つ選び、最後に押した状態にした。F10 を押してから ↓ を押すと、フォーカスは一覧の次の行へ動き、**そして** `moveNotes` の命令が送られた。既存のテスト (`ui/tests/menu.test.ts:581-630`) は `body` の中にマウントし、ピアノロールを閉じているので、どちらも見られない。

理由 (`ui/node_modules/svelte` 5.57.3 の `src/internal/client/render.js` を読んだ): 委譲されたイベントは、マウント先と `document` で受け取られる。マウント先の外にあるノードは `document` のリスナーで処理される。そこでの `stopPropagation()` は、同じノードにある `handleKeydown` を止められない。

これは `docs/progress.md` の既知の注意 (「body へ移した一覧 … stopPropagation でキーイベントを止めるが、同じノードの document のキーハンドラーは止まらない (R3 で見つけた。害があるかは確かめていない)」) より大きい。ここでの害は、矢印キーを 1 回押すごとに曲の中に取り消しの手が 1 つできることである。メニューバーと右クリックの一覧で起きる。修正は、この注意も閉じるべきである (`ChoiceMenu`、拡大の一覧)。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は同じ原因から出ている。前に出ている部品 (ダイアログ、開いている一覧、フォーカスされた部品) のためのキーが、`handleKeydown` (`ui/src/keys/attach.ts`) へ落ちてくる。handleKeydown は、その部品が前に出ていることを知らない。これらは、`handleKeydown` の 1 つの規則を使って、まとめて直す: (1) 「どのダイアログが前か」を調べる 1 つの処理を、すべての文脈 (`outputChooser`、`shelf`、`history`) と Enter と Esc の分岐で使う。順序は `App.svelte` でダイアログを描く順番に従う。(2) `ev.defaultPrevented` が true のときすぐ戻る (キーを受け取った部品がそう示す)。これで `body` へ移した一覧が入る。そうすれば、各部品は自分のキーだけを受け取ればよい。

このレポートの修正計画: `handleKeydown` は、`ev.defaultPrevented` が true のときすぐ戻る (`MenuList.svelte:117` は、受け取るキーについてすでに `preventDefault` を呼んでいる)。`typedKeys` の行とヒントの行の後に置く。ただし、文書のハンドラーがまだ見る必要のあるキーを、それより前に preventDefault で止めていないかを確かめる。`ui/src` の中の、ほかの `preventDefault` の呼び出し元も確かめる (キーダウンのハンドラーで `preventDefault` を検索する)。上のテストを `ui/tests/menu.test.ts` に追加する (`body` の子にマウントする)。

## Fix

`ui/src/keys/attach.ts`: `handleKeydown` は、キーがすでに `defaultPrevented` のとき、すぐ戻る (「キーで弾く」のキーとヒントの後)。これで、`body` へ移した一覧 (MenuList、ChoiceMenu、拡大の一覧。どれも受け取るキーを `preventDefault` する) が受け取ったキーを、もう一度使わない。`ui/src` の下のキーダウンのハンドラーで、ほかの `preventDefault` の呼び出し元も確かめた。どれもそのキーを `stopPropagation` でも止めている。`docs/architecture.md` 25-8 はこのことを書くようになった。テスト: `ui/tests/menu.test.ts` の「UI-SHELL-004: 一覧で使った ↑ ↓ ← → は、ピアノロールで選んだノートを動かさない」 (MenuBar を `body` の子にマウントし、ピアノロールを開いてノートを選んだ状態にする)。`ui/tests/keys.front.test.ts` の「UI-SHELL-004: 部品がすでに受け取ったキーは、もう一度使わない」。
