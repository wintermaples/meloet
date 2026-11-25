---
id: UI-SHELL-005
title: 書き出しダイアログの上に「キーの一覧」と「meloet について」が開き、そこで Enter を押すと書き出しが始まる
area: UI-SHELL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-SHELL-005: 書き出しダイアログの上に「キーの一覧」と「meloet について」が開き、そこで Enter を押すと書き出しが始まる

## Where

ui/src/components/menu/menus.ts:452-458 (`noDialog` は songDialog、settings、停止のダイアログは確認するが、`export.dialog` も「書き出しました」も確認しない)
ui/src/keys/attach.ts:511-541 (`state.export.dialog` があるときの素の Enter は `startExport` を送る。キーの一覧や About が前にあるときも同じ)

## What happens

書き出しダイアログ(または「書き出しました」)が開いている間も、「ヘルプ ▸ キーの一覧」と「meloet について」は使えたままで、ダイアログはその上に開きます。すると、素の Enter(たとえばキーの一覧の検索の欄や、「閉じる」ボタンで)は、キーの一覧に届きません。`handleKeydown` は `state.export.dialog` を見て `startExport` を送るため、キーの一覧の後ろで書き出しが始まります。「書き出しました」の間の Enter は `closeExport` を送ります。

## What should happen

docs/spec/7-readings.md の「「meloet について」を持つもの」では、ほかのダイアログが開いている間は、入口が使えないことになっています。menus.ts:452 のコメントも同じことを言っています(「ほかのダイアログ … が開いている間は開かない(Esc の順番を一つに保つため)」)。書き出しダイアログと「書き出しました」も、ダイアログです(App.svelte:108-118)。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. Mod Shift E を押す: 書き出しダイアログが開く。
2. 「ヘルプ」▸「キーの一覧」をクリックする: キーの一覧がその上に開く。
3. その検索の欄に入力して Enter を押す: `startExport` が送られ、書き出しが始まる。

## Cause

`topMenus` の `noDialog` は、書き出しダイアログと「書き出しました」ができる前に書かれたもので、それらを含みません。attach.ts の素の Enter の処理は、画面だけのダイアログ(About、キーの一覧)が前にあることを知らず、どの場合も Enter を処理します。

## Suggested fix

menus.ts の `noDialog` に `state.export.dialog === null && exportedOpen(state) === null` を加えます(exportedOpen は ui/src/components/export/exported.ts にあります)。プロトコル、文面、見た目は変えません。

## Verification

コードを読んで確認しました(実行はしていません)。`menus.ts:306`: `noDialog = !songDialog && !settings && !stoppedOpen`。書き出しダイアログと「書き出しました」は含まれないため、その上でも「キーの一覧」と「meloet について」は使えたままです。`attach.ts` は、何が前にあっても、素の Enter に `startExport` / `closeExport` で答えます(Enter の分岐は `about.open` / `keyList.open` を見ません)。7-readings:「「meloet について」を持つもの | … ほかのダイアログが開いている間は、メニューの行は使えない」。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 と同じ元の原因です。前にある部品(ダイアログ、開いた一覧、フォーカスされた操作部)のためのキーが、文書のキーの処理(`ui/src/keys/attach.ts` の `handleKeydown`)に落ちます。その処理は、前にある部品を知りません。これらは、`handleKeydown` の一つの規則で一緒に直してください。(1) 「どのダイアログが前にあるか」の判定を一つ作り、すべての文脈(`outputChooser`、`shelf`、`history`)と Enter・Esc の分岐で使う。判定の順序は `App.svelte` でダイアログが描かれる順です。(2) `ev.defaultPrevented` のとき早く戻る(キーを受けた部品がそう示す)。これで、`body` に移った一覧も扱えます。そうすれば、各部品は自分のキーだけを受ければよくなります。

このレポートの修正案: `ui/src/components/menu/menus.ts` の `noDialog` に `state.export.dialog === null && exportedOpen(state) === null`(と、遅れのダイアログ。ただし遅れのダイアログは、音の設定の中でしか開かないため、もう含まれている)を加えます。`attach.ts` では、About またはキーの一覧が開いている間、素の Enter の分岐は何もしません(前にあるかを見る共通の判定で、自動的にそうなります)。

## Fix

`ui/src/components/menu/menus.ts`: `noDialog` にも、書き出しダイアログと「書き出しました」が必要です(遅れのダイアログは、音の設定の中でしか開かないため、もう含まれています)。`ui/src/keys/attach.ts`: About またはキーの一覧が前にあるとき、素の Enter は何もしません(`dialogInFront`)。テスト: `ui/tests/keys.front.test.ts` の「UI-SHELL-005: キーの一覧と About は、…の上には開けない」と「UI-SHELL-005: 書き出しの上のキーの一覧や About で Enter を押しても、書き出さない」。
