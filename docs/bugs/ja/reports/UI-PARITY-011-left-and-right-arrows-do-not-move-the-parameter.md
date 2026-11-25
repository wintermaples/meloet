---
id: UI-PARITY-011
title: 上下で選んだパラメータの行を、左右の矢印で動かせない。矢印は選んだノートを動かしてしまう
area: UI-PARITY
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# UI-PARITY-011: 上下で選んだパラメータの行を、左右の矢印で動かせない。矢印は選んだノートを動かしてしまう

## Where

ui/src/components/params/BuiltinList.svelte:75-78 (`pick` は行の操作部ではなく、一覧の箱 `.list` にフォーカスを当てる) と :108-123 (`onkeydown` は ↑ ↓ Enter だけを扱い、← → は止めずに通す)
ui/src/components/params/SoundKnobList.svelte:43-46 と :48-66 (同じ)
ui/src/components/params/PluginList.svelte:116-119 と :156-172 (同じ)
ui/src/components/params/ParamRow.svelte:157-165 と params/PluginParamRow.svelte:105-116 (← → は、行自身のつまみや段のコントロールだけで扱う。フォーカスがそこにあるのは Tab を押したあとだけ)
ui/src/components/params/ParamRow.svelte:103-105 (バーを押すと `preventDefault` が呼ばれるので、バーを押してもフォーカスは移らない)
ui/src/keys/keys.ts:208-209 (ほかに取る者がいなければ、← → は `nudge` の動作になり、選んだノートを一目盛り動かす)

## What happens

パラメータの行を、名前を押して選ぶか、↑ ↓ で選びを動かすと、行は選ばれ (`SelectParam`)、フォーカスはパネルの一覧の箱にある。このとき ← → は選んだ行の値を動かさない。一覧のキーの処理は ← → を通し、それは文書のキーの処理に届き、「ノートを一目盛り動かす」になる。ピアノロールが開いていて、ノートが選ばれていると、ノートが動く (押すたびに取り消せる手が一つ)。Tab でスライダー自体にフォーカスが来てから、やっと ← → で値が動く。

## What should happen

仕様 3 (E3、内蔵の音のつまみ): 「← → で選んだ行を動かす、↑ ↓ で選ぶ、Enter で打ちこむ」。仕様 4: 「パラメータで: 上・下の行を選ぶ ↑ ↓」「値を動かす・細かく動かす ← → / Shift ← →」「値を打ちこむ Enter」は一つのまとまり。↑ ↓ と Enter は、一覧の箱から選んだ行に対して、すでに働いている (BuiltinList と PluginList の `enter()`、SoundKnobList の `paramUi.typing = knobs.row`)。← → だけが働いていない。

## How to trigger

実行せず、コードを読んで確かめた。
1. ピアノロールを開いて、ノートをいくつか選ぶ。トラックにコンプレッサーを載せ、パラメータの欄に出るように選ぶ。
2. 欄の「しきい値」という名前を押す (または、ある行を押してから ↓ を押す)。`pick` が `.list` にフォーカスを当てる。
3. → を押す: `BuiltinList.onkeydown` は 117 行目で、止めずに戻る。文書のハンドラ (`keys/attach.ts` の `handleKeydown`、文脈 "main") が `nudge` を受け、選んだノートが右へ一目盛り動く。しきい値は変わらない。
プラグインの行や、内蔵の音の「明るさ」などでも同じ。

## Cause

← → は、各行の操作部 (ParamRow / PluginParamRow) が扱う。しかし、行を選ぶと、フォーカスは行を囲む一覧に移る。一覧の処理は、そのグループのうち ↑ ↓ と Enter だけを実装している。

## Suggested fix

三つの一覧の `onkeydown` で、行が選ばれているとき ← → (と Shift ← →) を取り、その行の段へ送る。たとえば `pick` で、選んだ行の操作部 (`[data-param="…"] [role=slider]`) にフォーカスを移す。または、一覧が行と同じ `keyStep` / プラグインの段の処理を呼ぶ。どちらでもキーは止めて、文書へ届かないようにする。プロトコル、文言、見た目の変更はなし。

## Verification

テストを走らせず、読んで確かめた。`BuiltinList.svelte:75-78` の `pick` は一覧の箱にフォーカスを当てる。その `onkeydown` (108-123) は Enter と ↑ ↓ だけを取り、それ以外は止めずに戻る。そのため ← → は `handleKeydown` に届き、`nudge` になる (UI-SHELL-004 のテストが `moveNotes` を送ると示したのと同じ経路)。`SoundKnobList.svelte` と `PluginList.svelte` も同じ形をしている。仕様 3 (E3): 「← → で選んだ行を動かす、↑ ↓ で選ぶ、Enter で打ちこむ」。仕様 4 は、この三つを「パラメータで」という一つのまとまりとして挙げている。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は一つの原因を共有する。手前にある部品 (ダイアログ、開いた一覧、フォーカスされたコントロール) のキーが、文書のキーの処理 (`ui/src/keys/attach.ts` の `handleKeydown`) に落ちる。その処理は、手前に何があるかを知らない。一つの仕事として、`handleKeydown` に一つの規則を入れて直す: (1) 「どのダイアログが手前か」を見る一つの確認を、すべての文脈 (`outputChooser`、`shelf`、`history`) と Enter・Esc の枝で使う。順番は `App.svelte` で描く順番に従う。(2) `ev.defaultPrevented` ならすぐ戻る (キーを取った部品はそう示す)。これで `body` へ移された一覧も扱える。そのあと、各部品は自分のキーを取るだけでよい。
この報告に限れば、キーは誰も取っていないので、`defaultPrevented` での早い戻りだけでは直らない。一覧が ← → を取る必要がある。

この報告の修正案: `ui/src/components/params/BuiltinList.svelte`、`SoundKnobList.svelte`、`PluginList.svelte` の `onkeydown` で、行が選ばれているとき ← → と Shift ← → を取る (今は `shiftKey` で戻っている)。そして、その行の操作部が実行するのと同じ段の処理を実行する (`ParamRow.svelte:157-165` の `keyStep`、`PluginParamRow.svelte:105-116` の段の処理。二つ目の関数を書くのではなく、関数を共有する)。`preventDefault` と `stopPropagation`。テストは一覧ごとに用意する: 行を選び、→ を押すと `setParam` が送られ、`moveNotes` は送られないこと。

## Fix

`ui/src/components/params/rowKeys.ts` (新規): `arrowToChosenRow` は、行が選ばれているパラメータの一覧で ← → と Shift ← → を取って止め、その行自身の操作部 (`[data-param] [role=slider]`) に渡す。そのため、行自身の段の処理 (`keyStep` / プラグインの段) が実行される。`ui/src/components/params/BuiltinList.svelte`、`SoundKnobList.svelte`、`PluginList.svelte` の `onkeydown` の最初で呼ぶ。`ParamRow.svelte` と `PluginParamRow.svelte` は変えていない。テスト: `ui/tests/params.test.ts` の「UI-PARITY-011: with the focus on the list, ← → and Shift ← → …」、`ui/tests/e3.knobs.test.ts` の「UI-PARITY-011: …」、`ui/tests/round5.step3.test.ts` の「UI-PARITY-011: … plugin row …」。
