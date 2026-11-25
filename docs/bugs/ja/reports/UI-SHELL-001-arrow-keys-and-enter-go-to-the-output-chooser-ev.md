---
id: UI-SHELL-001
title: ダイアログが開いていても、矢印キーと Enter が出口えらびに届く
area: UI-SHELL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-SHELL-001: ダイアログが開いていても、矢印キーと Enter が出口えらびに届く

## Where

ui/src/keys/attach.ts:467-473 (`state.workspace.chooser` がセットされていれば、ダイアログを見る前に文脈を "outputChooser" にする)
ui/src/keys/keys.ts:376-380 (その文脈で ↑ ↓ Enter は chooserUp / chooserDown / chooserConfirm になる)
ui/src/keys/attach.ts:511-541 (ダイアログのための素の Enter の分岐は、keyAction が null を返したときだけ動く)
crates/meloet-core/src/state.rs:3018-3020 (`chooser_shown` はダイアログを見ないので、ダイアログが開いていても出口えらびは状態に残る)

## What happens

出口えらび(まだ出口が選ばれておらず、鍵盤がある状態。流れ 2)が出ている間も、使う人は音の設定 (Mod ,)、曲のダイアログ (Mod N / Mod O で変更があるとき)、書き出しのダイアログ、スキャン停止のダイアログを開ける。そのどれが開いていても:
- Enter は、ダイアログの明るいボタンではなく `chooserConfirm` を送る。音の設定では、ダイアログで選んだ出口ではなく、後ろの出口えらびで光っている出口から音が鳴り、ダイアログは開いたままになる。曲のダイアログ(「保存しますか」…)では、Enter はダイアログに答えない。
- ↑ ↓ は、ダイアログの後ろにある出口えらびの光る位置を動かす。

## What should happen

仕様書 3 (保存のダイアログ): 「Enter は右はしのボタン」。音の設定: Enter = 「OK」(settingsApply, attach.ts:538-540)。ダイアログは前にあるので、そのキーが先に来る(仕様書 3 の Esc の順は、後ろの部品より前にダイアログを置く)。同じ文脈の他のものは、すでにこれをしている: `shelfHasKeys` (ui/src/components/tracks.svelte.ts:58-67) と `historyKeys` (attach.ts:424-435) は、songDialog、settings、スキャン停止のダイアログのどれかが開いていると false を返す。

## How to trigger

コードから読んだもので、実行はしていない。
1. 出口がまだ決まっておらず、鍵盤が一つある状態で始める(または `meloet-vkeys appear`)。出口えらびが出る。
2. Mod , を押す。音の設定が開く。出口えらびで光っている出口以外の出口の行をクリックする。
3. Enter を押す: `chooserConfirm` が送られ、出口えらびで光っていた出口が選ばれ、ダイアログは開いたままになり、ダイアログで選んだ出口は使われない。
また: 出口えらびが出ている間に変更を加え、Mod N を押し、Enter を押す: 「保存しますか」のダイアログに答えられない。

## Cause

`handleKeydown` は、`songDialog`、`settings`、`export.dialog`、`exportedOpen(state)`、`plugins.stoppedOpen` を見ずに、`context = state.workspace.chooser ? "outputChooser" : …` を選ぶ。すると `keyAction` は ↑ ↓ Enter に出口えらびの操作を返すので、`action` は null にならず、素の Enter のダイアログの分岐 (attach.ts:511-541) には決して届かない。

## Suggested fix

"outputChooser" の文脈は、どのダイアログも開いていないときだけ使う (`historyKeys` と同じ確認: songDialog、settings、export.dialog、exportedOpen、plugins.stoppedOpen、そして about.open と keyList.open)。プロトコル、文面、見た目は変えない。

## Verification

実行したテストで確認した(作業用の scratch worktree。残していない): `workspace.chooser` と `workspace.settings` をセットした状態の `handleKeydown` は、Enter で `chooserConfirm` を、↓ で選択の移動を送った。代わりに `songDialog` をセットすると、Enter は `answerSongDialog` を送らなかった。コードを読んだ確認: `attach.ts:467-473` は `state.workspace.chooser` だけから "outputChooser" の文脈を選ぶ。コアの `chooser_shown` (`crates/meloet-core/src/state.rs:3018-3020`) はダイアログを見ない。実際のアプリで、出口えらびが出ている間に挙げたダイアログがすべて開けるかは確認していない(私が読んだ限り、メニューの行と Mod , は出口えらびで無効にはならない)。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は一つの根による: 前にある部品(ダイアログ、開いた一覧、フォーカスのある操作部品)のためのキーが、`handleKeydown` (`ui/src/keys/attach.ts`) の文書のキー処理へ落ち、そこは前にある部品を知らない。これらは一つの作業として、`handleKeydown` の一つの規則で直す: (1) すべての文脈(`outputChooser`、`shelf`、`history`)と Enter / Esc の分岐が使う「どのダイアログが前にあるか」の確認を一つにし、`App.svelte` でダイアログを描く順に並べる。(2) `ev.defaultPrevented` ならすぐ戻る(キーを取った部品がそう言う)。これは `body` へ動かされた一覧もカバーする。そうすれば、各部品は自分のキーだけを扱えばよい。

この報告の修正計画: `ui/src/keys/attach.ts` で、前にダイアログがないときだけ "outputChooser" の文脈を使う(songDialog、settings、latencyDialog、export.dialog、`exportedOpen`、`plugins.stoppedOpen`、`about.open`、`keyList.open`)。ケースを `ui/tests/keys*.test.ts` に足す。

## Fix

`ui/src/keys/front.ts` (新規): `dialogInFront(state)` は、キーを持つダイアログの名前を返す (About、キーの一覧、スキャン停止のダイアログ、曲のダイアログ、書き出しのダイアログ、「書き出しました」、遅れのダイアログ、音の設定。Esc の順: 画面自身のダイアログが先、その後にコアの `Back` が閉じる順)。`ui/src/keys/attach.ts`: `handleKeydown` は、前にダイアログがないときだけ "outputChooser" (および "shelf"、"history") の文脈を使う。素の Enter の分岐は `dialogInFront` で決める。テスト: `ui/tests/keys.front.test.ts` の「UI-SHELL-001: …」(二つのケース)。`ui/tests/screen.test.ts` の「↑ ↓ Enter in the output chooser, Enter in sound settings」は変えずに通る。
