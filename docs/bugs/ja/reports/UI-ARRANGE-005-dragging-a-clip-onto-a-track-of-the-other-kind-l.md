---
id: UI-ARRANGE-005
title: 違う種類のトラックへクリップをドラッグすると許されているように見えるが、Rust は何も言わずに断る
area: UI-ARRANGE
kind: parity
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-005: 違う種類のトラックへクリップをドラッグすると許されているように見えるが、Rust は何も言わずに断る

## Where

ui/src/components/arrangeDrag.ts:149-158 (`groupPlaceable`: 確かめるのは「トラックの中にあるか、0より前でないか、重なっていないか」だけ)
ui/src/components/arrangeDrag.ts:260-265 (`clipDragCursor`: `ok` のとき `move` / `copy`)
crates/meloet-model/src/arrange.rs:235-236 (`place`: `if doc.tracks[to].kind.is_vocal() == c.is_keys() { return Err(EditError::WrongKind) }`)
crates/meloet-core/src/play.rs:1257-1261 (`apply_placed`: `Overlap` 以外のエラーは `None` になり、状態行の文字は出ない)

## What happens

鍵盤のクリップを、歌のトラックへ上下にドラッグする (または、音声のクリップを鍵盤のトラックへ) と、通常の移動として表示されます。動く絵はそのトラックの上にあり、カーソルは `move` (Alt で `copy`) で、読み値には新しい位置が出ます。離すと `moveClips` が送られ、モデルは `WrongKind` を返します。コアは状態行の文字を出さずに捨て、クリップは元の位置に戻ります。使う人には理由が伝わりません。違う種類のトラックの空いている所への「貼りつけ」でも同じです (`Arrange.svelte:1653`: クリップボードがあれば、その行は使えるようになっている)。

## What should happen

architecture 22-15: 「違う種類のトラックへクリップを移動または貼りつけると、`EditError::WrongKind` が返る」。architecture 17 / B-71: 移動できない場所 (「トラックの外」) では、「何も起きず、何も言わない (画面には「できない」のカーソルが出る)」。画面も違う種類の行を同じに扱うべきです。つまり `ok: false`、`not-allowed` のカーソル、何も送らない。

## How to trigger

コードを読んで確かめたもので、実行はしていません。クリップのある鍵盤のトラックと、その下の歌のトラックを作る。鍵盤のクリップを歌のトラックの行へ下にドラッグして離す。絵はそこに移動の カーソルとともに表示され、それから戻ります。状態行には何も出ません。

## Cause

`groupPlaceable` は、クリップの種類 (`clip.audio !== null`) と、対象のトラックの種類 (`track.kind.kind`) を比べていませんが、モデルはその組み合わせを断ります。

## Suggested fix

`groupPlaceable` で、クリップが違う種類のトラックに入るとき (`(clip.audio !== null) !== (track.kind.kind === "vocal")`) は false を返す。そうすると `ok` が false になり、カーソルは `not-allowed` になります。任意の追加として、クリップボードに違う種類のクリップしかないとき、違う種類のトラックの空いている所の「貼りつけ」の行を使えなくすることもできます。ただし、そのためにはクリップボードの種類を知る必要があり、今は `clipboardClips` (数) だけが送られているので、この半分にはプロトコルのフィールドが要り、リードの決定が必要です。ドラッグの半分では、文面は変えません。

## Verification

コードを読んで確かめたもので、ドラッグの半分については実行していません。`groupPlaceable` (`arrangeDrag.ts:149-158`) は「トラックがあるか、0より前でないか、重なっていないか」だけを確かめます。モデルの `place` は、鍵盤のクリップを歌のトラックに置くこと、およびその逆を `EditError::WrongKind` で断ります (`crates/meloet-model/src/arrange.rs:235-236`)。`apply_placed` (`crates/meloet-core/src/play.rs:1257-1261`) は、`Overlap` 以外のエラーをすべて「何もしない、文字もなし」にします。そのため、画面は `move` / `copy` を出し、Rust が断る場所を示すことになります。architecture 17 は、移動できない場所についての規則を示しています。「できない」のカーソルで、何も送らない、というものです。

修正計画: `ui/src/components/arrangeDrag.ts` の `groupPlaceable` で、`(clip.audio !== null) !== (track.kind.kind === "vocal")` のとき false を返す。アレンジのドラッグのテストに一つ場合を加える。プロトコルや文面の変更はありません。

貼りつけの半分 (違う種類のトラックの空いている所の「貼りつけ」、`Arrange.svelte:1653`) も実際にありますが、この報告の修正からは外します。画面が知っているのは `clipboardClips` (数) だけなので、その行を使えなくするには新しいプロトコルのフィールドが要ります。`docs/bugs/later/VERIFY-UI.md` に回します。

## Fix

ドラッグの半分だけ。`ui/src/components/arrangeDrag.ts` の `groupPlaceable`: クリップが違う種類のトラックに入るとき (`(clip.audio !== null) !== (track.kind.kind === "vocal")`) は false にする。そうすると `ok` が false になり、カーソルは `not-allowed` になり、何も送られません。貼りつけの半分は `docs/bugs/later/VERIFY-UI.md` に残します。
テスト: `ui/tests/arrange.group.test.ts` の「a keyboard clip on a vocal track, or an audio clip on a keyboard track, is 「できない」 …」。
