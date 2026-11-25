---
id: UI-PARITY-002
title: ドラッグしたマーカー・テンポの点・拍子の旗が、離したあとしばらく元の場所へ戻る
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-002: ドラッグしたマーカー・テンポの点・拍子の旗が、離したあとしばらく元の場所へ戻る

## Where

ui/src/components/timeline/MarkerRow.svelte:73-79 (`up`: `moveMarker` の答えの前に `drag = null` にする)
ui/src/components/timeline/TempoBand.svelte:116-121 (`laneUp`: `pointDrag = null` にしてから `moveTempoPoint`)
ui/src/components/timeline/TempoBand.svelte:183-188 (`rowUp`: `flagDrag = null` にしてから `moveMeterChange`)
答えまでドラッグの見た目を保つ兄弟: ui/src/components/automation/AutoLane.svelte:68-79, 153-164 (答えの `current.rev >= rev` まで `pending`)、ui/src/components/Arrange.svelte:1436-1439 (`clipPending`)、ui/src/components/pianoroll/Roll.svelte:375-386 と CcLanes.svelte:160-172 (`pending`)、ui/src/components/comp/CompBand.svelte:46-54, 116-118、ui/src/components/arrange/AudioClipHandles.svelte:50-61, 243。

## What happens

マーカー、テンポの点、拍子の旗を離すと、ドラッグの状態がすぐに消され、命令が送られる。コアの答え (rev 付きの新しい時間軸) が戻るまで、その部分は古い時間軸から描かれる。そのため、マーカー・点・旗はその時間の間は元の場所に描かれ、それから新しい場所へ飛ぶ。ほかのドラッグの部品は、答えが届くまでドラッグの見た目を保つ。

## What should happen

兄弟の部品はわざとそうしている。AutoLane.svelte:68 のコメントは次のとおり: 「離したあと、命令を送って答えが届くまで見た目を保つ (点が一瞬元の場所へ戻らないように)」。テンポとマーカーの部品も同じ種類のドラッグ (CLAUDE.md: 画面が先に動き、離すと命令が 1 つ) なので、同じように一瞬戻ってはならない。

## How to trigger

コードを読んで分かったこと。実行はしていない。マーカーを右へ 2 拍ドラッグして離す。離してからコアの次の状態が届くまでの間 (1 フレーム以上。コアが忙しいとき、たとえばプラグインのある曲が鳴っている間は長くなる) は、タグが古い tick に描かれる。テンポの点と拍子の旗も同じ。

## Cause

`up`、`laneUp`、`rowUp` は、送る前にドラッグを null にし、何も残さない。表示される位置 (MarkerRow の `tags`、TempoBand の `T.draggedPoints(points, pointDrag)` と `flagsShown`) は、答えが届くまで動かす前の `timeline` に戻る。

## Suggested fix

離したドラッグを `pending` として残し、`send(...)` が返す rev に `current.rev` が達するまで、表示する位置にそれを使う (AutoLane.svelte:153-164 と、その `$effect` (74-77) と同じ型)。新しく押したときと、マーカー・点・旗がなくなったときに消す。プロトコルや見た目は変えない。

## Verification

コードを読んで確認した (実行はしていない。飛ぶ長さは測っていない)。`MarkerRow.svelte:73-79` の `up`、`TempoBand.svelte:116-121` の `laneUp`、183-188 の `rowUp` は、ドラッグを null にしてから送る。表示される場所は、答えが届くまで `timeline` のままなので、そのフレームの間、その物は元の場所に描かれる。ほかのドラッグの部品は答えまで見た目を保つ (`AutoLane` の `pending`、`Arrange` の `clipPending`、`Roll` / `CcLanes` の `pending`、`CompBand`)。

修正案: `ui/src/components/timeline/MarkerRow.svelte` と `TempoBand.svelte` で、離したあとも同じ見た目を保つ: `current.rev` が `send()` の返す rev に達するまで、離したドラッグを保つ。UI-ARRANGE-001 と同じ失敗をしないように書く。保つ見た目は、その物の名前 (マーカーの id、テンポの点の tick、旗の小節) に結び付け、その物がまだ `timeline` に元のとおりあるときだけ使う。index では決して使わない。Rust は送られた tick をそろえ (アーキテクチャ 21-15)、動かした点を選び直すことに注意する。UI-ARRANGE-006 を直したあとは、保つ場所が答えと同じになる。

## Fix

まだ commit していない (commit は lead が行う)。

- `ui/src/components/timeline/MarkerRow.svelte`、`ui/src/components/timeline/TempoBand.svelte`: 離したドラッグを (`pending`、`pointPending`、`flagPending`)、`current.rev` が `send()` の返す rev に達するまで保つ。`AutoLane` / `CompBand` と同じ型。物は名前で結び付けられ、`timeline` の中でその物がまだ元のとおりのときだけ使う。マーカーは元の tick の marker id、テンポの点は元のテンポでの tick、旗は小節。新しく押すとこれを消す。Esc では消さない (兄弟と同じ。命令はもう送られている)。

テスト: `ui/tests/round6.timeline.test.ts` — 「離したあと、マーカーはコアの答えまで動かした場所を保つ。答え、または変化のない rev で終わる」、「離したあと、テンポの点と拍子の旗は、コアの答えまで動かした場所を保つ」。
