---
id: UI-PARITY-008
title: ノート一覧・CC 一覧・「選んだノート」の欄の外を押すと、数字の欄と違って、フォーカスが行に戻される
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-008: ノート一覧・CC 一覧・「選んだノート」の欄の外を押すと、数字の欄と違って、フォーカスが行に戻される

## Where

ui/src/components/pianoroll/NoteFieldInput.svelte:65 (`onblur={oncancel}`: ぼけ（blur）と Esc が同じ処理を呼ぶ)
ui/src/components/pianoroll/NoteListPanel.svelte:173-175 (`cancel` → `close(true)`), :134-140 (`close(true)` は `tick()` の後に行の `[data-at]` にフォーカスする)
ui/src/components/pianoroll/CcListPanel.svelte:157-159, :125-131 (同じ)
ui/src/components/pianoroll/NoteInspector.svelte:347 (`oncancel={() => ... closeField(true)}`), :101-105 (`queueMicrotask(() => fieldButtons[name]?.focus())`)
正しく動いている隣の部品: ui/src/components/NumberField.svelte:85 (`onblur={() => close(false)}`。Enter と Esc だけが `close(true)` を呼ぶ)

## What happens

ノート一覧、CC 一覧、または「選んだノート」の欄に入力中に、別のテキストの箱（棚の検索、パラメーターの欄の検索、書き出しの画面の箱）を押しても、フォーカスはそこに留まりません。ノートの欄のぼけ（blur）で欄が閉じ、一マイクロタスク後に一覧の行（または「選んだノート」の欄のボタン）にもう一度フォーカスが移ります。これはフォーカスの移動の中で起きるので、押された箱はフォーカスを受け取らない（または、すぐに失う）ため、使う人は二度押す必要があります。その間に打ったキーは、一覧の行と文書のキーの処理に届き（一文字のキーはショートカットとして働く）ます。

## What should happen

Esc は、フォーカスを行に戻します（仕様 3「ノート一覧」: Enter で決め、Esc でやめる。次の Enter のためにフォーカスは一覧に残る）。しかし、外を押した場合は、箱を閉じるだけであるべきです。トランスポートの数字の欄がそうしているように。NumberField.svelte は、ぼけでは `close(false)`、Enter と Esc でだけ `close(true)` を使います。アレンジとテンポの箱（timeline/ValueInput.svelte:66）と ValueField（params/ValueField.svelte:42-44）も、ぼけのときにフォーカスを戻しません。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. ノート一覧のあるピアノロールを開き、ノートの「Vel」の欄をダブルクリックします。
2. 箱が開いたまま、パラメーターの欄の検索箱（または棚の検索箱）を押します。
3. 入力の `blur` が `oncancel` → `cancel(token)` → `close(true)` を呼び、`tick()` の後に行のボタン `[data-at]` の `focus()` が呼ばれます。WebKit ではぼけのリスナーが戻るときにマイクロタスクが動くので、検索箱へのフォーカス移動はブロックされる（または、行がすぐに取る）。検索箱にはカーソルがありません。
同じことが CC 一覧と「選んだノート」の上の四つの欄でも起きます。

## Cause

NoteFieldInput は、Esc とぼけに一つの `oncancel` を使い、三つの親はどれも「閉じて、行（または欄）にもう一度フォーカスする」で答えています。NumberField は両者を区別しています（キーでは `close(true)`、ぼけでは `close(false)`）。

## Suggested fix

NoteFieldInput にぼけ用の別の経路を与えます（例: `onblur={() => oncancel(false)}` と `refocus` の引数。Esc は true を渡す）。そして三つの親で、ぼけのときは `close(false)` / `closeField(false)` を呼びます。プロトコル、文面、見た目の変更はありません。

## Verification

コードを読んで確認しました（実行はしていません。WebKitGTK でのブラウザーのフォーカスの順序は試していません）。`NoteFieldInput.svelte:65` は、Esc と同じく blur にも `oncancel` を使います。`NoteListPanel.svelte:173-175` は `close(true)` で答えます。これは `tick()` の後に行にフォーカスします。`CcListPanel.svelte` と `NoteInspector.svelte`（`queueMicrotask(() => fieldButtons[name]?.focus())`）も同じです。ブラウザーのフォーカスの順序がどうであっても、行の `focus()` はぼけの後に走るので、使う人が押した箱はフォーカスを保てません（移動が止められるか、行がすぐに取るかのどちらか）。`NumberField.svelte` は両者を区別しています（ぼけでは `close(false)`、Enter と Esc では `close(true)`）。

修正の計画（提案どおり）: `ui/src/components/pianoroll/NoteFieldInput.svelte` で、ぼけでは `oncancel(false)`、Esc では `oncancel(true)` を呼びます（または二つ目の prop）。`NoteListPanel.svelte`、`CcListPanel.svelte`、`NoteInspector.svelte` では、それを `close(refocus)` / `closeField(refocus)` に渡します。テスト: ぼけで箱が閉じ、`document.activeElement` が行ではないこと。

## Fix

プロトコル、文面、見た目の変更はありません。

- `ui/src/components/pianoroll/NoteFieldInput.svelte`: `oncancel(refocus)`。Esc は `true`、ぼけは `false` を渡す。
- `ui/src/components/pianoroll/NoteListPanel.svelte`、`ui/src/components/pianoroll/CcListPanel.svelte`、`ui/src/components/pianoroll/NoteInspector.svelte`: それを `close(refocus)` / `closeField(refocus)` に渡す。外を押すと、箱を閉じるだけになる。
- テスト: `ui/tests/pianoroll.panels.test.ts` の「pressing outside a field box leaves the focus there (UI-PARITY-008)」（ノート一覧、選んだノート）、`ui/tests/pianoroll.r5-notes.test.ts` の「pressing another text box closes the box and leaves the focus there (UI-PARITY-008)」。
