---
id: UI-PARITY-001
title: オートメーションの点、マーカー、テンポ・拍子のドラッグは、その場の読み値は出るが、状態行には出ない
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-001: オートメーションの点、マーカー、テンポ・拍子のドラッグは、その場の読み値は出るが、状態行には出ない

## Where

ui/src/components/automation/AutoLane.svelte:72, 100-115 (`reading` はローカルの状態だけで、`dragReading` は設定されない)
ui/src/components/timeline/MarkerRow.svelte:43, 61-72 (マーカーのドラッグ: ローカルの `reading` だけ)
ui/src/components/timeline/TempoBand.svelte:75-79 (`readAt`: テンポの点と拍子の旗のドラッグ。ローカルの `reading` だけ)
正しく作られている兄弟: ui/src/components/pianoroll/Roll.svelte:339-343、ui/src/components/pianoroll/CcLanes.svelte:121-124、ui/src/components/Arrange.svelte:1409(クリップのドラッグ)、ui/src/components/arrange/AudioClipHandles.svelte:218。どれも `dragReading.text` を設定し、StatusLine.svelte:958 がそれを表示する。

## What happens

オートメーションの点を動かしているとき(または、点を足すためにポインタを押さえているとき)、マーカーを動かしているとき、テンポの点や拍子の旗を動かしているとき、その場の読み値はカーソルの近くに出るが、状態行はいつもの文字のままである。ピアノロール(ノート、ベロシティ、CC の点)とアレンジ(クリップ、歌のクリップの取っ手、繰り返しの取っ手)では、同じ読み値が状態行にも出る。

## What should happen

仕様 2、「その場の読み値」の部分: 「同じ値は状態行にも出る」。仕様 3(ピアノロールの決まり): 「動かしている値は、カーソルの近くの「その場の読み値」と状態行の両方に出る」。タイムライン・オートメーションの三つの部品は、Roll.svelte、CcLanes.svelte、Arrange.svelte と同じことをすべきである。7-readings A-20(「点や旗をドラッグしている間の読み値: 位置だけ」)は、読み値が何を持つかを言うだけで、状態行に出ないとは言っていない。

## How to trigger

コードを読んで確かめたもので、実行はしていない。アレンジの「テンポ ・ 拍子の変更」の行を開き、テンポの点を横にドラッグする。カーソルの横に「9. 2. 0000」が出るが、状態行は変わらない。マーカー、拍子の旗、オートメーションのレーンの点でも同じになる。比べると、クリップをドラッグすると状態行に読み値が出る。

## Cause

この三つの部品は、その場の箱のために自分の `reading` の状態を持ち、`dragReading.text`(ui/src/components/pianoroll/view.svelte.ts:152)には一度も書かない。読み値が状態行に届く道は、dragReading.text を通るもの(StatusLine.svelte)だけである。

## Suggested fix

各部品で、その場の読み値が変わるたびに、同じ文字を `dragReading.text` に設定する。離したとき、取り消したとき(`onDragCancel` のコールバック)、破棄されたときに null に戻す。Arrange.svelte:1409 / 1432 / 1451 がクリップのドラッグでしているとおりにする。MarkerRow の文字は `positionText(drag.toTick, map)` である。プロトコル、文言、見た目は変えない。

## Verification

読んで確かめた(実行はしていない)。`dragReading.text`(`pianoroll/view.svelte.ts:152`)は、読み値が状態行に届く唯一の道である。`AutoLane.svelte`、`TempoBand.svelte`、`MarkerRow.svelte` のマーカーのドラッグは、ローカルの `reading` だけを持つ(検索結果: `dragReading` が `MarkerRow.svelte` に出るのは「この前に空きを入れる…」の箱のためだけで、ほかの二つには出ない)。仕様 2 の「その場の読み値」: 「ドラッグしている間だけ表示する。カーソルの左上へ 6 離れる。画面の端では内側へ動かす。同じ値は状態行にも出る」。7-readings A-20 は、読み値が何を持つかだけを言う。

修正計画(提案どおり): `ui/src/components/automation/AutoLane.svelte`、`ui/src/components/timeline/TempoBand.svelte`、`MarkerRow.svelte` で、ローカルの読み値が変わるたびに、同じ文字を `dragReading.text` に書く。離したとき、取り消したとき、部品が消えたときに null に戻す(`Arrange.svelte` がクリップのドラッグでしているとおり)。すると、これらのドラッグが動くとき状態行も変わる。点のドラッグを一回、`tools/measure/webkit-damage.sh` で確かめる(状態行とアレンジはページ上で離れているため)。

## Fix

まだコミットしていない(リードがコミットする)。

- `ui/src/components/automation/AutoLane.svelte`、`ui/src/components/timeline/TempoBand.svelte`、`ui/src/components/timeline/MarkerRow.svelte`: その場の読み値の文字も `dragReading.text` に書く(点の移動と、押すと点が足される場所。テンポの点と旗。マーカー)。離したとき、Esc のとき、部品が消えたときに null に戻す。 `tools/measure/webkit-damage.sh` は実行していない(実際のアプリが要るため)。

テスト: `ui/tests/automation.lane-drag.test.ts` の「moving a point, and holding an empty place, put the in-place reading's text on the status line; release and Esc take it away」。`ui/tests/round6.timeline.test.ts` の「the reading in place is on the status line too; it goes on release and on Esc (spec 2 「その場の読み値」)」。
