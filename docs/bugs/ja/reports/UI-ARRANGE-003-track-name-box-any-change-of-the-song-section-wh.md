---
id: UI-ARRANGE-003
title: トラックの名前の入力欄: 入力中に曲の部分が変わると、打った文字が消えて元の名前に戻り、選ばれる
area: UI-ARRANGE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-003: トラックの名前の入力欄: 入力中に曲の部分が変わると、打った文字が消えて元の名前に戻り、選ばれる

## Where

ui/src/components/TrackHeads.svelte:68-81 (入力欄の `draft` を埋め、欄にフォーカスする `$effect`)

## What happens

トラックの名前の入力欄を開く `$effect` は、`trackEdit.renaming` と、トラックを探すための `song.tracks` の両方を読みます。そのため、欄が開いている間に `song` の部分が置き換わるたびに、動き直します。そのたびに `draft = track.name` となり、`focus()` と `select()` が呼ばれます。使う人が打った文字は消え、元の名前が欄に戻って全部選ばれた状態になるので、次に打つ字がそれを置き換えてしまいます。

使う人が曲を編集していなくても、`song` の部分は、次のような普通の場合に送られ直します。

- 欄が開いている間に Mod S を押す (Mod のキーは入力中も働く。`ui/src/keys/attach.ts:387-399` の `WITH_MOD`)。保存で `song.file` が「変更あり」から「保存済み」に変わる。
- プラグインの音源や SF2 のキットが読み込みを終え、キーの名前を付ける (`TrackView.drumKeys` / `keyNames`)。音の素材が届く (`standIn` が消える)。トラックを追加した直後に名前を付けに行くと、よく起きる。
- AI のリンクによる曲の編集。

## What should happen

入力中の文字は、Enter か Esc か、枠の外を押すまで残る (仕様 3 と 7-readings 「Track name input box」: Enter と枠の外を押したときに決めた。Esc で止まる)。同じ仲間の欄は、これを防いでいる。クリップの名前の入力欄 (`Arrange.svelte:716-734`、`nameFor`)、マーカーの名前の入力欄 (`timeline/MarkerRow.svelte:135-147`、`openFor`)、繰り返しの入力欄 (`Arrange.svelte:1489-1501`、`repeatFor`) は、欄が開いたときだけ draft を埋める。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。トラックの名前をダブルクリックし、「ベース」と打つ (Enter は押さない)。Ctrl S を押す。欄には元の名前が選ばれた状態で戻る。今 Enter を押すと、元の名前が残る。

## Cause

この効果は `song` を依存に持ち、「`renaming` が変わったときだけ」という保護がない。そのため、新しい `song` のオブジェクトが来るたびに、`draft = track.name` と `select()` が動く。

## Suggested fix

クリップの名前の入力欄と同じく保護する。埋めた先の id を覚える (`let draftFor: number | null = null;`)。`trackEdit.renaming === draftFor` なら早く戻る。トラックは `untrack` の中で読むか、その id の最初の実行のときだけ読む。「トラックがなくなったら閉じる」という既存の規則は残す。プロトコル、文面、見た目の変更はなし。

## Verification

コードを読んで確かめた (実行していない)。`TrackHeads.svelte:68-81`: この効果は `trackEdit.renaming` と `song.tracks` を読む (`song` は `$derived(current.state!.song)`。状態の `song` の部分が置き換わるたびに新しいオブジェクトになる)。そして、動くたびに `draft = track.name` を設定し、`focus()` / `select()` を呼ぶ。「欄が開いたときだけ」という保護はない。Mod S は入力中も動く (`keys/attach.ts` の `handleKeydown`: `isTyping` の間も `WITH_MOD` の操作は取られる)。保存で曲の部分が変わる。報告に挙げた仲間の欄は、これを防いでいる (`Arrange.svelte` の `nameFor`、`MarkerRow.svelte` の `openFor`)。

修正案 (提案どおり): `ui/src/components/TrackHeads.svelte` で、draft を埋めた先のトラックの id を覚える。`trackEdit.renaming` が新しい id に変わったときだけ、`draft` を埋めて欄にフォーカスする。「トラックがなくなったら閉じる」は、どの実行でも残す。`renaming` が null に戻ったとき、覚えた id を消す。`ui/tests/` にテストを追加する (入力し、曲の部分を新しいオブジェクトに置き換え、打った文字が残ることを確かめる)。

## Fix

`ui/src/components/TrackHeads.svelte`: 効果は、draft を埋めた先のトラックの id を覚え (`draftFor`)、`trackEdit.renaming` が新しい id になったときだけ、`draft` を埋めてフォーカスする。「トラックがなくなったら閉じる」は、どの変化でも動く。欄が閉じると id を忘れる。
テスト: `ui/tests/tracks-shelf.test.ts` の「name input box: a new song part while typing (a save, an AI edit) keeps the typed text」。
