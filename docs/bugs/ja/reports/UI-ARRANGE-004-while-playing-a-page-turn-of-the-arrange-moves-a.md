---
id: UI-ARRANGE-004
title: 再生中にアレンジのページがめくれると、つかんでいるクリップ・マーカー・点が、ポインターから 1 ページ分ずれる
area: UI-ARRANGE
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-004: 再生中にアレンジのページがめくれると、つかんでいるクリップ・マーカー・点が、ポインターから 1 ページ分ずれる

## Where

ui/src/components/Arrange.svelte:1080-1094 (`frame()`: `follow` がオンのとき、ドラッグ中でも `followScroll` でページを送る)
ui/src/components/arrangeDrag.ts:198 (`dragClipTo`: 動きは `(x - press.x) / pxPerTick`。押した位置からのピクセルで、ポインターの下の tick ではない)
ui/src/components/Arrange.svelte:465 と 414 (動いている絵は、現在の `view` と `dragShift` で置かれる)
ui/src/components/Arrange.svelte:1347-1351, 1421 (囲んで選ぶ操作は、始点の `x0` を古い表示のピクセルで保つ)
ui/src/components/timeline/MarkerRow.svelte:67, ui/src/components/automation/autoGeom.ts:204, ui/src/components/timeline/tempoGeom.ts:255 (マーカー、オートメーションの点、テンポの点のドラッグも、`(x - downX) / pxPerTick` を使う)

## What happens

クリップ、マーカー、オートメーションの点、テンポの点は、曲の再生中にもドラッグできます（architecture 17: 再生中も動かせる）。このドラッグの間も、`frame()` は再生線に合わせてアレンジのページを送ります（`follow` は再生の開始からオンのままで、使う人が手で横にスクロールするまで続きます）。ドラッグは「押してから動いたピクセル数」として動きを保つので、ページがめくれた後も、つかんでいるものは同じ tick のずれを保ちますが、表示は 1 ページ分の幅だけ進んでいます。そのためクリップの絵（`tickToX(view, first)` と `dragShift` に置かれたもの）は 1 ページ左へ飛び、たいていは画面の外に出ます。ポインターは空いている場所の上に残ります。手を離すと、クリップは絵のある場所に置かれ、ポインターの下には置かれません。マーカー、オートメーションの点、テンポの点も同じです。ページがめくれる前に始めた囲んで選ぶ操作は、最初の角をピクセルで保つので、始めた場所から 1 ページ離れたクリップを選びます。

コードは縦のホイールについて、この罠をすでに知っています。`wheelRows` は「クリップのドラッグや囲んで選ぶ操作の間は（つかんでいるものがポインターの下からずれてしまうので）スクロールしない」としています（`Arrange.svelte:1209-1211`）。

## What should happen

つかんでいるものは、ポインターの下に残るべきです（仕様 3: ドラッグ中は、画面だけがまず動き、元の場所は gray.7 の枠で示す。ドラッグはポインターに従う必要がある）。縦のホイールと同じように、何かをつかんでいる間はアレンジがページを送らないか、ドラッグがポインターの下の tick で動きを数えるべきです。

## How to trigger

コードを読んで確認したもので、実行はしていません。スペースキーを押して、アレンジの左端から曲を再生します。再生線が右端に届く前に、右寄りのクリップを押して、ゆっくり動かし続けます。再生線が右端を越えると、表示がページ送りされ、クリップの絵は左へ消えますが、ポインターはさっきの場所にあります。

## Cause

`frame()` は、クリップのドラッグ（`clipDrag`）、囲んで選ぶ操作（`boxSel`）、マーカーの行・テンポの帯・線のドラッグの状態を確かめずに、`G.followScroll` を呼びます。各ドラッグは、押してからのピクセル数で動きを測ります。

## Suggested fix

アレンジのドラッグや囲んで選ぶ操作の最中は、`frame()` でページを送らない（`clipDrag`、`boxSel`、および `MarkerRow`、`TempoBand`、`AutoLane`、`CompBand` が何かを持っている間に立てる、共有の小さな旗）。`wheelRows` と同じ考え方です。ページ送りは、手を離した後の最初のフレームから続けられます。ピアノロールも、ノートのドラッグの最中に同じようにページを送ります（`ui/src/components/pianoroll/Roll.svelte:894-901`。このまとまりの外）。仕様の文言やプロトコルは変えません。

## Verification

コードを読んで確認しました（実行はしていません）。`frame()`（`Arrange.svelte:1080-1094`）は `follow` が真のとき `followScroll` でページを送ります。`follow` は再生の開始（1124）で真になり、横のホイールやスクロールバーを動かしたとき（1184、1672）だけ偽になります。押したときには偽になりません。ドラッグは押してからのピクセル数を数え（`arrangeDrag.ts:198`、`MarkerRow.svelte:67`、`autoGeom.ts:204`、`tempoGeom.ts:255`）、つかんでいるものを現在の表示で置くので、ページがめくれた後は、つかんでいるものが 1 ページ分の幅だけポインターから離れ、手を離すとそこに置かれます。`wheelRows`（1209-1211）は、同じ理由で、クリップのドラッグ中はスクロールしないようにしています。重大度は medium のままです。「再生中に、ページのめくれをまたいでドラッグする」ことが必要ですが、そのときクリップ（またはマーカーや点）は、使う人が気づかないうちに 1 ページ分離れた、画面の外に置かれます。

修正の計画: `ui/src/components/Arrange.svelte` の `frame()` で、アレンジ内で何かをつかんでいる間は、ページを送らない。クリップのドラッグ、囲んで選ぶ操作、`MarkerRow`、`TempoBand`、`AutoLane`、`CompBand` のドラッグを、それぞれに旗を置かずに覆う最小の形は、アレンジのルートに置く「アレンジ内でポインターが押されている」という旗です（キャプチャ段階の `pointerdown` で立て、ウィンドウの `pointerup` / `pointercancel` で下ろし、`frame()` が読む）。ページ送りは、手を離した後の最初のフレームから続きます。timeline/ や automation/ には手を入れません。ピアノロールの半分（`pianoroll/Roll.svelte:894-901`。ノートや CC のドラッグの間の `prView.follow`）も、同じ変更で同じ規則で直します。

## Fix

`ui/src/components/Arrange.svelte`: `held` の旗を、アレンジのセクションでのキャプチャ段階の `pointerdown` で立て（クリップのドラッグ、囲んで選ぶ操作、目盛りの行、`MarkerRow`、`TempoBand`、`AutoLane`、`CompBand` を覆う）、ウィンドウのキャプチャ段階の `pointerup` / `pointercancel` と、ウィンドウの `blur` で下ろします。`frame()` は旗が立っている間はページを送らず、手を離した後の最初のフレームからページを送ります。`timeline/` や `automation/` には手を入れていません。ピアノロールの半分（`pianoroll/Roll.svelte`）は、ピアノロールの担当者に任せました。
テスト: `ui/tests/arrange.test.ts` の「ポインターがアレンジの中で押されている間（ドラッグ中）は、ページを送らない。手を離した後の最初のフレームからページを送る」。
