---
id: UI-ROLL-006
title: 「1 小節」のグリッドで、描いたノート・矢印の移動・Mod D・ドラムのマスが、別の拍子の小節でも最初の小節の長さを使う
area: UI-ROLL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-006: 「1 小節」のグリッドで、描いたノート・矢印の移動・Mod D・ドラムのマスが、別の拍子の小節でも最初の小節の長さを使う

## Where

- `ui/src/components/pianoroll/Roll.svelte:117` — `step = G.gridTicks(pr.grid, meter)`。`meter` は曲の始めの拍子 (`Roll.svelte:75`)。
- `ui/src/components/pianoroll/geom.ts:207-210` — 分割 0 (「1 小節」) のとき、step は `ticksPerBar(meter)`。
- `ui/src/components/pianoroll/drag.ts:242`, `:251` — 描いたノートは 1 つの `step` の長さで始まる。`drag.ts:340` と `geom.ts:392-394` — 描いている間、それより短くならない。
- `ui/src/components/pianoroll/geom.ts:358` — 長さを変えたノートは、1 つの `step` (またはそのノートの長さ) より短くできない。
- `ui/src/components/pianoroll/commands.ts:33-34`, `:67`, `:74` — ← → は `step` ずつ動かす。Mod D は `step` の整数倍でコピーする (同じく、先頭の拍子の `step`)。
- `ui/src/components/pianoroll/Roll.svelte:124` — ドラムのマスは `step * ppt` の幅。

## What happens

グリッドが「1 小節」のとき、すべての長さと移動は、曲の始めの拍子の 1 小節の長さを使う。曲の途中で拍子が変わっても同じ。例: 始めは 4/4、小節 5 から 3/4。小節 6 で、「書く」で描いたノートは 7680 ティック (4/4 の 1 小節) の長さになり、小節 7 の 1920 ティックまで伸びる。← → はノートを 7680 ティックずつ動かすので、小節線から外れる。Mod D はコピーを 7680 ティック後ろに置く。ドラムのマスは 4/4 の 1 小節と同じ幅になる。ノートは 7680 ティックより短く長さを変えられない。始まりの位置を揃える処理は正しい (`gridTick(…, bar)` はその小節の頭へ行く) ので、狂うのは長さと距離だけである。コアのクオンタイズは、そのノートの小節を使う (`crates/meloet-model/src/quantize.rs:22-23`、`grid.ticks(meter)` の拍子は `meter_stretch(tick)` から取る)。

## What should happen

仕様書 3: 「描く道具: 何もない所を押すと、1 グリッドの長さのノートができる」。仕様書 4: 「ノートを 1 グリッドずつ動かす」。ドラム: 「幅は 1 グリッドのマスから 2 を引いたもの」。アーキテクチャ 16: 「分割 0 は 1 小節 (拍子から)」。「1 小節」では、1 グリッドの長さはその場所の小節である。位置を揃える処理 (`time/bars.ts` の `gridTick` に `bar` を渡す) とコアのクオンタイズは、すでにそう数えている。

## How to trigger

コードを読んで分かったこと。実行はしていない。始めは 4/4、小節 5 から 3/4 の曲。グリッドは「1 小節」。小節 6 の頭でノートを描き、ノートの一覧でその長さを見る (3/4 の拍では 1 つの 4/4 の小節、「4.0000」と読める)。そのノートを選んで → を押す。

## Cause

`gridTicks` には先頭の拍子しか渡されない (`Roll.svelte:75`、`commands.ts:33`)。そのため、分割 0 のとき step は、いつも最初の小節の長さになる。

## Suggested fix

分割 0 では、step をその場所の小節から取る。描く・長さを変えるときは描き始め、← → と Mod D では最も前に動かしたノート、ドラムのマスでは表示の左端 (または、各マスの小節) を使う。たとえば `ticksPerBar(meterAtOf(map, tick))`。プロトコルは変えない。

## Verification

コードを読んで確認した (実行はしていない)。`Roll.svelte:117` は `step = G.gridTicks(pr.grid, meter)` で、最初の拍子 (75) を使う。`gridTicks` (`geom.ts:207-210`) は分割 0 で `ticksPerBar(meter)` を返すので、「1 小節」では、どの長さも距離も (描いたノート、最も短い長さ変え、← →、`commands.ts:33` の Mod D、ドラムのマス幅) すべて最初の小節の長さになる。ほかのグリッドは拍子に依存しないので、「1 小節」かつ拍子の変わる曲だけに当たる (低)。Rust 側のグリッドは、その場所の小節を使う (`crates/meloet-core/src/play/automation.rs` の `on_grid`、`meloet-model/src/quantize.rs`)。

UI-ROLL-004 と UI-ROLL-005 と同じ原因である。まとめて直す (最初の拍子ではなく、地図から)。

この報告の修正案: 分割 0 では、step を場所の関数にする。`ticksPerBar(meterAtOf(map, tick))`。`ui/src/components/pianoroll/drag.ts` (描いたときと長さを変えるときの描き始め)、`commands.ts` (← → と Mod D では最も前に動かしたノート)、`Roll.svelte:124` (ドラムのマス: 各マスの小節。描画で表示ごとに 1 つの幅が必要なら、表示の左端の小節)。送られた `ticks` を Rust がどう扱うかを確かめる (送られたとおりに受け取るので、決めるのは画面だけ)。4/4 から 3/4 の曲でテストする。

## Fix

UI-ROLL-004 と UI-ROLL-005 と一緒に直した。プロトコルは変えない。

- `ui/src/components/pianoroll/drag.ts`: `RollInput.step(tick)` はその場所の 1 グリッドの長さを返す。描く道具は描き始め (長さと影) で、長さを変えるときは掴んだノートで、それを使う。
- `ui/src/components/pianoroll/Roll.svelte`: マウス用の `stepAt(tick)` = `gridTicks(grid, meterAtOf(map, tick))`。ドラムのマスは、表示の左端の小節の 1 マス (`DrumGeom.hitW` は表示ごとに 1 つの幅)。
- `ui/src/components/pianoroll/commands.ts`: → と Mod D は、最も前に選んだノートの step を使う。← は、その直前の step (小節の頭からなら、前の小節の長さ) を使う。そのため、小節線上のノートは小節線上に止まる。
- テスト: `ui/tests/pianoroll.meters.test.ts` の「別の拍子の小節での「1 小節」のグリッド (UI-ROLL-006)」(2 件)。ドラムのマス幅にはテストがない。
