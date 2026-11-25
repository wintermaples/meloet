---
id: UI-ARRANGE-008
title: トラック見出しのスタイルが Arrange から移ったあと、テイク切りかえの札の前後ボタンに見た目がない
area: UI-ARRANGE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-008: トラック見出しのスタイルが Arrange から移ったあと、テイク切りかえの札の前後ボタンに見た目がない

## Where

ui/src/components/Arrange.svelte:1990 と 1996 (`<button class="tog step" …>◂</button>`、`…>▸</button>`)
ui/src/components/Arrange.svelte:2025-2395 (スタイルのブロック。`.tog` と `.step` の規則はない)

## What happens

テイク切りかえの札(クリップの上の「◂ テイク 2 / 3 ▸ T」)の ◂ と ▸ は、`tog step` のクラスを使っています。Arrange.svelte には、その規則がありません。Svelte はコンポーネントのスタイルを範囲に限るため、`TrackHeads.svelte` の `.tog` の規則(そこには `.step` の規則もありません)は、このマークアップに届きません。`components/common.css` にも `.tog` と `.step` はありません。そのため二つのボタンは、ブラウザ自身のボタンの見た目(枠、背景、余白、システムのフォント)で、24 px の札の中に描かれています。

コミット 7120b19 でトラック見出しが `TrackHeads.svelte` に移されたとき、`.tog`、`.tog.step`、`.tog.step:hover`、`.tog.step:active` の規則も Arrange.svelte から取り除かれました。切りかえボタンは Arrange.svelte に残り、同じクラス名のままでした。

## What should happen

仕様 2「テイクの印と切りかえ」: 切りかえの札は、「◂」、「テイク 2 / 3」、「▸」、キー T で、gray.4 の地に gray.8 の枠です。部品の見た目は、仕様 2「部品と状態」に従います(マウスが乗ったときは枠 gray.8・文字 gray.10、押したときは地 gray.2)。ピアノロールの同じ札(`pianoroll/Toolbar.svelte:225-234`)は `m-plain step` と、その独自のスタイルを使っています。

## How to trigger

コードを読んで確認したもので、実行はしていません。2 つのテイクがある鍵盤トラックを録音し、そのクリップをクリックする。クリップの上の札の ◂ と ▸ が、WebKitGTK の既定のボタンに見える。

## Cause

`.tog.step` の規則は、トラック見出しが分けられたときに Arrange.svelte から取り除かれました(7120b19)。それを使うマークアップは変えられていませんでした。

## Suggested fix

Arrange.svelte の二つのボタンに、もう一度スタイルを与えます(以前の `.tog` / `.tog.step` の規則)。または、ピアノロールの札と同じく `m-plain` を使います。モック F-Takes と、`tools/design/shoot_ui.py` で見た目を比べます。プロトコルや文面は変えません。

## Verification

コードを読んで確認しました(実行はしていません。画像も撮っていません)。`Arrange.svelte:1990, 1996` は `class="tog step"` を使っています。`Arrange.svelte` のスタイルのブロックには `.tog` も `.step` の規則もありません(grep: なし)。`.tog` は `TrackHeads.svelte` と `mixer/Strip.svelte` にしかなく(範囲に限られるため、このマークアップには届きません)、`ui/src` には `components/common.css` の `:focus-visible` 以外に、`button` へのグローバルな規則もありません。`git show 7120b19 -- ui/src/components/Arrange.svelte` では、`.tog`、`.tog.step`、`.tog.step:hover`、`.tog.step:active` の規則が取り除かれています。そのため、二つのボタンはブラウザの既定のボタンの見た目になっています。

修正案: `ui/src/components/Arrange.svelte` で、二つのボタンに、ピアノロールの札が使っているクラス(`m-plain step`、`pianoroll/Toolbar.svelte:225-234`)を与え、同じ小さな `.step` の規則を付けます。または、取り除かれた `.tog.step` の規則を戻します。モック F-Takes(`tools/design/shoot_ui.py`)と比べます。新しい色やサイズは作りません。UI-ARRANGE-009 と一緒に直します(同じ二つのボタン)。

## Fix

`ui/src/components/Arrange.svelte`: `.tog.step` の規則をスタイルのブロックに戻しました(大きさは `part.toggle-size`、枠は gray.6、地は gray.1、文字は gray.8。マウスが乗ったときは枠 gray.8・文字 gray.10。押したときは地 gray.2・枠 gray.8)。UI-ARRANGE-009 のための使えない状態の見た目(枠 gray.5、文字 gray.7)も加えました。新しい値はありません。モック F-Takes と画像では比べていません(CSS だけの変更で、テストもありません)。
