---
id: UI-SHELL-002
title: 書き出しダイアログや「書き出しました」が開いていても、キーが棚に残る
area: UI-SHELL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-SHELL-002: 書き出しダイアログや「書き出しました」が開いていても、キーが棚に残る

## Where

ui/src/components/tracks.svelte.ts:58-67 (`shelfHasKeys` は songDialog、settings、スキャン停止のダイアログだけを見ている)
ui/src/keys/attach.ts:469-470 (「棚」の文脈は `shelfHasKeys` から来る)
ui/src/keys/attach.ts:530-537 (書き出しダイアログと「書き出しました」の Enter の分岐は、keyAction が null を返したときだけ動く)
ui/src/keys/attach.ts:618-619 (ダイアログの中の押下は `shelfUi.keys` を消さない。ダイアログは `.app` の外にある)

## What happens

棚がキーを持っているとき (棚の行をクリックした後) に、Mod Shift E で書き出しダイアログを開くと、キーは棚に行ったままになります。
- Enter は `shelfConfirm` を送ります (試聴中の音から新しいトラックを作る、または選んだエフェクトを入れる)。書き出しの「書き出す」にはなりません。棚で何も選んでいなければ、Enter は何もしません。
- ↑ ↓ はダイアログの後ろで棚の試聴を変えます。Space は棚のサンプルを鳴らします。F はお気に入りに入れます。
「書き出しました」ダイアログでも同じことが起きます。Enter で閉じられません。
ダイアログの中をクリックしても直りません。`handlePointerdown` が `shelfUi.keys` を消すのは `.app` の中の押下だけで、ダイアログは外にあるためです。

## What should happen

手前にあるダイアログがキーを受け取ります。仕様 4 章: 「書き出しました」は Esc か Enter で閉じる。attach.ts:530-533 は、書き出しダイアログでの Enter は 「書き出す」 だと書いています。同じ仲間の確認 `historyKeys` (attach.ts:424-435) は、`export.dialog` と `exportedOpen(state)` を除外しています。`shelfHasKeys` も同じようにすべきです。

## How to trigger

コードを読んだ結果です。実行はしていません。
1. 棚 (P) を開き、楽器の行をクリックする。試聴が始まり、棚がキーを持つ。
2. Mod Shift E を押す。書き出しダイアログが開く (コアは OpenExport で試聴を止めない。crates/meloet-core/src/export.rs:413)。
3. Enter を押す。`shelfConfirm` が送られ、ダイアログの後ろで新しいトラックができる。書き出しは始まらない。

## Cause

`shelfHasKeys` は、`export.dialog` か書き出しダイアログが開いているあいだ true を返すため、`handleKeydown` は「棚」の文脈を使います。`keyAction` は Enter を `shelfConfirm` にマップします (コマンドが null でも、それは操作です)。そのため、書き出しダイアログの Enter の分岐が飛ばされます。

## Suggested fix

`shelfHasKeys` で、`state.export.dialog !== null` または `exportedOpen(state) !== null` のときにも false を返す (`historyKeys` と同じ)。文言、見た目、プロトコルは変えません。About とキーの一覧は、docs/progress.md で既知として挙げられています (「一文字のキーはまだ働く」)。これらにも同じ確認を入れられます。

## Verification

テストを実行して確認しました (作業用の worktree。残していない)。`shelfUi.keys = true`、棚が手前、`export.dialog` を設定したとき、Enter は `startExport` を送りませんでした。`shelfUi.keys = false` のときは送りました。コードを読んだ結果: `shelfHasKeys` (`tracks.svelte.ts:58-67`) は、`export.dialog` と書き出しましたダイアログを除外していません。一方 `historyKeys` (`attach.ts:424-435`) は除外しています。`handlePointerdown` が `shelfUi.keys` を消すのは `.app` の中の押下だけで、ダイアログは外にあります (`App.svelte`)。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は、ひとつの根から来ています。手前の部品 (ダイアログ、開いている一覧、フォーカスされた操作部品) のキーが、文書全体のキー処理 (`ui/src/keys/attach.ts` の `handleKeydown`) に流れてしまいます。handleKeydown は、手前にある部品を知りません。これらは、`handleKeydown` のひとつの規則でまとめて直すべきです: (1) 「どのダイアログが手前か」の確認をひとつにし、すべての文脈 (`outputChooser`、`shelf`、`history`) と Enter と Esc の分岐で使う。確認の順は `App.svelte` でダイアログが描かれる順。(2) `ev.defaultPrevented` のとき早く戻る (キーを取った部品がそう示す。これで `body` に移された一覧も対象になる)。そうすれば、各部品は自分のキーだけを処理すればよくなります。

このレポートの修正の計画: `ui/src/components/tracks.svelte.ts` の `shelfHasKeys` で、`state.export.dialog !== null`、`exportedOpen(state) !== null`、遅れのダイアログ、About、キーの一覧のときにも false を返す (共通の「手前のダイアログ」の確認)。`docs/progress.md` は、About とキーの一覧の「一文字のキーはまだ働く」を既知として挙げています。その注記は文字のキーについてであり、棚の Enter と矢印についてではありません。

## Fix

`ui/src/components/tracks.svelte.ts`: どのダイアログが手前にあっても、`shelfHasKeys` は false になる (`ui/src/keys/front.ts` の `dialogInFront`。書き出しダイアログ、書き出しましたダイアログ、遅れのダイアログ、About、キーの一覧も含む)。`ui/src/keys/attach.ts` の `historyKeys` と `auditionToCancel` も、同じ確認を使う。テスト: `ui/tests/keys.front.test.ts` の 「UI-SHELL-002: 書き出しダイアログや「書き出しました」の後ろにある棚は、Enter や ↓ を取らない」。
