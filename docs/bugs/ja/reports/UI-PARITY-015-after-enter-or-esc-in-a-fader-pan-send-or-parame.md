---
id: UI-PARITY-015
title: フェーダー・パン・センド・パラメーターの値の欄で Enter または Esc を押したあと、フォーカスが失われる。数字の欄とノートの一覧は戻す
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-015: フェーダー・パン・センド・パラメーターの値の欄で Enter または Esc を押したあと、フォーカスが失われる。数字の欄とノートの一覧は戻す

## Where

ui/src/components/params/ValueField.svelte:25-41 (Enter と Esc は `onsubmit` / `oncancel` を呼び、入力はそのあと取り除かれ、何もフォーカスを受けない)
ui/src/components/mixer/control.svelte.ts:88-98 (フォーカスしたフェーダー、パン、センドで Enter を押すと欄が開く)、:118-125 と :139-141 (`submit`、`stopTyping` は `typing` を false にするだけ)
ui/src/components/params/BuiltinList.svelte:83-88、SoundKnobList.svelte:51-54、PluginList.svelte:129-131 (選んだ行で Enter を押すと欄が開く)。params/paramUi.svelte.ts:15-19 (`stopTyping` は行の状態を消すだけ)
正しく作られている兄弟: ui/src/components/NumberField.svelte:41-47 と :55、:60 (`close(true)` は、Enter または Esc のあと読み値にフォーカスを戻す)。pianoroll/NoteListPanel.svelte:134-140 と CcListPanel.svelte:125-131 (行にフォーカスが戻る)

## What happens

仕様 3(86-7)では、フェーダー、パン、センドに Tab でフォーカスを与え、↑ ↓(パンは ← →)で動かし、Enter で入力します。入力した値を Enter で決めた(または Esc で欄を出た)あと、欄は消え、フォーカスは何にもありません(ページの body)。次の ↑ ↓ はフェーダーを動かさず、文書のキーの処理に届いて、選んだノートを動かします(ほかに ↑ ↓ が何をするかは、その場所によります)。パラメーターの欄でも同じです。↑ ↓ で行を選び、Enter で入力し、Enter で決めると、あとの ↑ ↓ は行を選ばず(一覧がフォーカスを失った)、ノートを動かします。

## What should happen

キーで欄が閉じたあと、フォーカスはキーを受けていた場所に戻るべきです。トランスポートの数字の欄(`close(true)` → `shown?.focus()`)やノートと CC の一覧(`close(true)` → 行)がそうしています。そうすれば、Tab や行をもう一度押さなくても、仕様 4 のキー(「フェーダー・センドを動かす … フォーカスがあるとき」「パラメーターでは、上・下の行を選ぶ」)が働き続けます。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. ミキサーを開き、トラックのフェーダーに Tab で移動し、↑ を一度押す(動く)。それから Enter を押す。
2. -6 と入力して Enter を押す: `submit` は `setGain` を送り、`typing` は false になり、ValueField とその入力は消える。`document.activeElement` は body。
3. ↑ を押す: フェーダーには `keydown` が来ない。`keys/attach.ts` が "main" の文脈で受け取り、`nudge` を実行する(ピアノロールで選択があれば、選んだノートを半音動かす)。
パラメーターの欄: 組みこみエフェクトの行を選び、Enter、値を入力、Enter、それから ↓: 行は変わらない。

## Cause

ValueField の親たちは `typing` を切るだけで、フェーダー、つまみ、センドの要素や、一覧の枠のどれにもフォーカスを戻しません。キーで閉じると数字の欄やノートの一覧はフォーカスを戻しますが、それと違います。

## Suggested fix

Enter または Esc で欄が閉じたとき(blur ではなく)、欄を開いたものにフォーカスを戻します。`mixControl` では、欄を開いた Enter の `e.currentTarget` を覚えておき、`typing = false` のあとにフォーカスします。パラメーターの三つの一覧では、キーからの `stopTyping()` のあと、一覧(`list`)にフォーカスします。プロトコル、文面、見た目は変えません。

## Verification

コードを読んで確認しました(実行はしていません)。`ValueField.svelte:25-41` は `onsubmit` / `oncancel` を呼び、そのあと取り除かれます。`mixControl`(`mixer/control.svelte.ts:121-128`)と `paramUi.stopTyping` は入力状態を切るだけで、フェーダー、つまみ、センドや一覧に再びフォーカスを与えないため、`document.activeElement` は `body` に落ち、次の矢印は文書のハンドラーに届きます。`NumberField.svelte`(`close(true)`)とノートの一覧はフォーカスを戻します。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 と同じ元の原因です。前にある部品(ダイアログ、開いた一覧、フォーカスされた操作部)のためのキーが、文書のキーの処理(`ui/src/keys/attach.ts` の `handleKeydown`)に落ちます。その処理は、前にある部品を知りません。これらは、`handleKeydown` の一つの規則で一緒に直してください。(1) 「どのダイアログが前にあるか」の判定を一つ作り、すべての文脈(`outputChooser`、`shelf`、`history`)と Enter・Esc の分岐で使う。判定の順序は `App.svelte` でダイアログが描かれる順です。(2) `ev.defaultPrevented` のとき早く戻る(キーを受けた部品がそう示す)。これで、`body` に移った一覧も扱えます。そうすれば、各部品は自分のキーだけを受ければよくなります。

このレポートの修正案: 欄が Enter または Esc で閉じるとき(blur ではなく)、欄を開いたものにフォーカスを戻します。`ui/src/components/mixer/control.svelte.ts` では、欄を開いた Enter の `e.currentTarget` を覚え、`typing = false` のあと(`tick()` のあと)にフォーカスします。`ui/src/components/params/BuiltinList.svelte`、`SoundKnobList.svelte`、`PluginList.svelte` では、キーからの `stopTyping()` のあと、一覧の枠にフォーカスします。テスト: Enter、入力、Enter のあと、`document.activeElement` がもう一度その操作部になること。

## Fix

`ui/src/components/params/ValueField.svelte`: 欄は、開いたときにフォーカスを持っていたものを覚えておき、Enter または Esc で閉じたあと(blur ではなく)、フォーカスが何にもなくなっていれば、それにフォーカスを戻します(Enter を受けたフェーダー、パン、センド、またはパラメーターの一覧)。値の欄を使うすべての場所で一つの処理です。`mixer/control.svelte.ts` と一覧は、このためには変えていません。テスト: `ui/tests/round5.step3.test.ts` の「UI-PARITY-015: Enter または Esc のあと、入力の欄から戻ると、フェーダーやパンがまたキーを受ける」と、`ui/tests/params.test.ts` の「UI-PARITY-015: Enter または Esc のあと、入力の欄から戻ると、↑ ↓ がまた行を選ぶ」。
