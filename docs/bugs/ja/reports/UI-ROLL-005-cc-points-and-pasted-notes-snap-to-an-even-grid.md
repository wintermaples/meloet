---
id: UI-ROLL-005
title: CC の点と貼りつけたノートが、曲の先頭から数えた等間隔のグリッドにそろう。拍子を変えた後は、グリッド線から外れる
area: UI-ROLL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-005: CC の点と貼りつけたノートが、曲の先頭から数えた等間隔のグリッドにそろう。拍子を変えた後は、グリッド線から外れる

## Where

- `ui/src/components/pianoroll/cc.ts:294-297` — CC の点を追加する: `snapTick(tick, step)`。
- `ui/src/components/pianoroll/cc.ts:408` — CC の点を動かす: `snapTick(target, step)`。
- `ui/src/components/pianoroll/commands.ts:40` — 再生の線の位置での Mod V (ノートの「貼りつけ」も): `snapTick(c.playTick, step)`。
- `ui/src/components/pianoroll/Roll.svelte:1106` — 何もない所を右クリックしての「貼りつけ」: `G.snapTick(G.xToTick(g, p.x), step)`。
- 比べる: `ui/src/components/pianoroll/Roll.svelte:283` (ノートのドラッグと描く道具) と `:1049` (目盛りを押す)。どちらも `gridTick(map, …)` を使う。また `ui/src/components/pianoroll/paint.ts:84-87` は、ノートの欄と CC のレーンの線を、地図から (`eachGridTick`、`eachLine`) 描く。

## What happens

拍子を変えた後、グリッド線は拍子の部分の先頭から数え直される (「1 小節」のときは小節ごと)。ノートはその線にそろう。しかし、追加または動かした CC の点と、再生の線や右クリックした所に貼りつけたノートは、グリッドの間隔の倍数に置かれる。その数え方は tick 0 から始まる。例: 3/4 の曲で、2 小節目 (tick 5760) から 4/4 に変わる。グリッドは 1/2 (3840 tick)。線は 3840、5760、9600、13440 … にある。tick 9000 に描いた CC の点は 7680 に置かれる。これは 2 本の線の間である (ノートのそろえ方では 9600 になる)。「1 小節」のときは、点と貼りつけは、最初の小節の長さの倍数に置かれ、小節の頭には置かれない。

## What should happen

読みの規則 A-7: 「何もない所を右クリックしたときの貼りつけの位置: いちばん近いグリッド線 (目盛りを押したときと同じそろえ方)」。目盛りは `gridTick(map, …)` を使う。アーキテクチャ 24-2: Mod V はノートを「再生の線の位置に (グリッドにそろえて)」置く。CC の点は、CC のレーンが描く線にそろうべきである。ノートがノートの欄の線にそろうのと同じである。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。3/4 の曲で、2 小節目に 4/4 への変更がある。グリッドは 1/2。「サステイン」を開き、2 小節の後の 2 本目の線の近くに点を描く。または、再生の線をそこに置き、ノートをコピーして Mod V を押す。

## Cause

この 4 か所は、単純な `snapTick(tick, step)` (`geom.ts:212`) を呼ぶ。これは tick 0 から `step` の倍数に丸める。`gridTick` (`time/bars.ts`) を使うべきところである。これは拍子の部分の先頭から数え、小節の刻みを知っている。`LaneInput` にはそろえる関数がまったくない (`RollInput.grid` にはある)。

## Suggested fix

これらの所で `gridTick(mapOf(timeline), tick, step, false, grid.division === 0)` を使う。`LaneInput` に `RollInput.grid` のようなそろえる関数を与える。`pianoRollCommand` (`state.timeline` を持つ) と、右クリックの貼りつけに地図を渡す。プロトコルは変えない。

## Verification

コードを読んで確かめた (実行はしていない)。`cc.ts:294-297` と 408、`commands.ts:40`、`Roll.svelte:1106` は、単純な `snapTick(tick, step)` (`geom.ts:212`。tick 0 から `step` の倍数) を使う。一方、ノートと目盛りは `gridTick(map, …)` (`Roll.svelte:283`、1049) を使い、線は地図から描かれる。拍子の変更の頭が、グリッドの間隔の倍数でない場合 (または「1 小節」のグリッドの場合)、この 2 つは食い違う。読みの規則 A-7: 「何もない所を右クリックしたときの貼りつけの位置 | いちばん近いグリッド線 (目盛りを押したときと同じそろえ方)」。

UI-ROLL-004 と UI-ROLL-006 と同じ原因である (ピアノロールは最初の拍子しか使わない)。まとめて直す。

このレポートの修正計画 (提案どおり): `LaneInput` (`cc.ts`) に `RollInput.grid` のようなそろえる関数を与える。地図 (または同じそろえる関数) を `pianoRollCommand` (`commands.ts`。`state.timeline` → `mapOf` を持つ) に渡す。`Roll.svelte:1106` で `gridTick` を使う。テスト: 3/4 で 2 小節目から 4/4、グリッド 1/2。tick 9000 の CC の点は 9600 にそろう。

## Fix

UI-ROLL-004 と UI-ROLL-006 と一緒に直した。プロトコルは変えない。

- `ui/src/components/pianoroll/cc.ts`: `LaneInput.grid()` (`RollInput.grid` のようなそろえる関数)。点の追加と移動は、それを使ってそろえる (`moveAmount` がそれを受け取る)。
- `ui/src/components/pianoroll/CcLanes.svelte`: レーンに `gridTick(map, …)` を与える。
- `ui/src/components/pianoroll/commands.ts`: 再生の線での Mod V は、`gridTick(mapOf(timeline), …)` でそろえる。
- `ui/src/components/pianoroll/Roll.svelte`: 何もない所の右クリックからの「貼りつけ」は、`gridTick(map, …)` でそろえる。
- テスト: `ui/tests/pianoroll.meters.test.ts` の「CC の点と貼りつけは、グリッド線にそろう (UI-ROLL-005)」 (3 つのテスト)。
