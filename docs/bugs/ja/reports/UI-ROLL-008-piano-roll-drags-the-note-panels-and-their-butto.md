---
id: UI-ROLL-008
title: 録音中・カウントイン中でも、ピアノロールのドラッグとノートの欄のボタンは使えるように見えるが、コアは無視する
area: UI-ROLL
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-008: 録音中・カウントイン中でも、ピアノロールのドラッグとノートの欄のボタンは使えるように見えるが、コアは無視する

## Where

- `ui/src/components/pianoroll/NoteInspector.svelte:46`, `:95`, `:180`, `:264-265` — `live`、`specOf`、`toolUsable`、`canQuantize`、`canUnquantize` は `pr.take === take.id` だけを見ていて、トランスポートは見ていない。
- `ui/src/components/pianoroll/NoteListPanel.svelte:134-139` と `ui/src/components/pianoroll/CcListPanel.svelte:117-122` — 欄は、いつでも開いて `setNotesField` / `moveControls` を送る。
- `ui/src/components/pianoroll/Roll.svelte:280` と `ui/src/components/pianoroll/CcLanes.svelte:85` — ドラッグと描く道具は、録音中のクリップ(`liveMode`)のためにだけ止められていて、カウントイン中や別のクリップの録音中は止められていない。
- `ui/src/components/pianoroll/rollMenu.ts:32` と `Roll.svelte:1103` を比べる(右クリックの一覧は、録音中やカウントイン中に編集の行を使えなくする)。`ui/src/components/menu/menus.ts:230`(「もとの位置へ戻す」は `!busy` が必要)。`ui/src/components/Arrange.svelte:1381-1385`(busy の間は、クリップのドラッグを始めない)。
- `crates/meloet-core/src/play.rs:2349-2354`(busy の間は `aimed` が None を返す)と `:2444`(busy の間は `add_note` が戻る)。

## What happens

カウントイン中と、表示中のクリップ以外のクリップを録音している間(別のトラック上、または同じトラックの新しいクリップ)、ピアノロールでノートを編集するものは、すべて使えるように見える。「選んだノート」のボタン(「かける」、「もとの位置へ戻す」、「そろえる」、「割合」「だんだん」「ばらつき」 「かける」、「のばす」)、その欄の入力、「ノートの一覧」と「CC の一覧」の入力、そして、ノートの領域、ベロシティのレーン、CC のレーンでのドラッグと描く操作も使える。ドラッグすると、ノートが動いて見えることさえある。離すか Enter を押すと、コアは何もしない(busy のため)。状態行にも何も出ず、ノートは元の位置に戻ってしまう。

## What should happen

読み方 A-7、"Right-click during recording and count-in": "Rows that change the song (cut, paste, duplicate, delete, quantize, back to the original position) are disabled." 曲を変える行(切り取り、貼りつけ、複製、削除、クオンタイズ、もとの位置へ戻す)は使えなくする。
メニューの行は、busy の間「もとの位置へ戻す」を使えなくし、アレンジは busy の間クリップのドラッグを始めない。ピアノロールの残りも同じ決まりに従うべきである。欄のボタンと入力は使えないときの見た目にし、busy の間はノートや点のドラッグや描く操作をしない。

## How to trigger

コードを読んで確かめたもので、実行はしていない。キーボードのトラックを 2 本作り、トラック 1 のクリップをピアノロールで開き、トラック 2 を選んで R を押す。カウントイン中か録音中に、「クオンタイズ」の下の「かける」を押すか、表示中のクリップのノートをドラッグして離す。

## Cause

これらの部品は、カウントインや録音について `transport.state` を一度も読まない。扱っているのは、右クリックの一覧(`busy`)と、録音中のクリップ(`liveMode`)だけである。

## Suggested fix

ほかで使っている busy の確認(`transport.state === "recording" || transport.state === "countIn"`。`keys/attach.ts` の `songBusy` と同じ)を、`live` / `toolUsable` / `canQuantize` / `canUnquantize` と、一覧の欄を開くところに加える。busy の間は、移動・長さ変更・描く・消す・ベロシティ・CC の点のドラッグを始めない(押して選ぶことや箱で選ぶことは残してよい。右クリックの一覧でも、コピーと全部選ぶは使えるままだから)。プロトコルは変えない。使えないときの見た目は、すでにある。

## Verification

読んで確かめた(実行はしていない)。Rust 側: `busy()` の間、`aimed` は `None` を返し(`crates/meloet-core/src/play.rs:2349-2354`)、ノートのインテントはこれを通る。`add_note` は busy の間、戻る。画面側: `Roll.svelte:280` は `liveMode`(135 行目: 表示中のクリップが録音中のクリップかどうか)のときだけドラッグを止める。そのため、カウントイン中と別のクリップの録音中は、ドラッグ、描く操作、欄のボタン、一覧の欄がすべて画面で動き、コアは何も知らせずに捨てる。右クリックの一覧(`rollMenu.ts:32`)とメニューの行は、すでに `busy` を使っている。7-readings の A-7「Right-click during recording and count-in」が決まりを示す(曲を変える行は使えなくする)。

同じ原因を持つ報告は、UI-ARRANGE-009、UI-ARRANGE-013、UI-PARITY-010(録音中やカウントイン中に使えるように見えるが、Rust が拒否する部品)である。また UI-PARITY-005(このバグのドラッグの半分を、パリティ側から見たもの。アレンジのドラッグはすべて busy の間は選ぶだけになる)もあった。この報告にまとめ、そちらは削除した。

ここから、さらに次の場所も見つかった。`ui/src/components/pianoroll/Roll.svelte:346-360`(`down`)、`drag.ts:191-250`(`RollDrag.down` は busy の確認なしに、移動・長さ変更・ベロシティ・消す・描くを始める)、`CcLanes.svelte:137-145` と `cc.ts:255-270`(CC の点)。busy の間だけ選ぶようになっている同類は、`Arrange.svelte:1376-1385`、`TempoBand.svelte:99, 166`、`MarkerRow.svelte:65`、`CompBand.svelte:90`、`AudioClipHandles.svelte:174, 268`。busy の間、`down` は今の選択だけを送り、ドラッグは始めない。

Fix plan(提案どおり): ピアノロールに `busy` を一つ作る(`transport.state === "recording" || transport.state === "countIn"`。`ui/src/keys/attach.ts` の `songBusy` と同じ決まり)。次で使う。`NoteInspector.svelte`(`live`、`toolUsable`、`canQuantize`、`canUnquantize`。使えないときの見た目はある)、`NoteListPanel.svelte` / `CcListPanel.svelte`(欄を開かない。行は選べる)、`Roll.svelte` / `CcLanes.svelte` / `drag.ts` / `cc.ts`(移動、長さ変更、描く、消す、ベロシティ、CC の点のドラッグを始めない。押して選ぶことと箱で選ぶことは残す)、ピアノロールの目盛のループのドラッグ(UI-ARRANGE-013 と対になるもの)。プロトコルも文言も変えない。

## Fix

プロトコルと文言は変えない。使えないときの見た目は、すでにあるものを使う。

- `ui/src/components/pianoroll/drag.ts`、`ui/src/components/pianoroll/cc.ts`: `RollInput` / `LaneInput` に `busy()` を置く。busy の間、押すと今の選択だけを送る(ノート、端、CC の点。箱は選べる)。移動、長さ変更、描く、消す、ベロシティ、CC の点のドラッグは始めない。
- `ui/src/components/pianoroll/Roll.svelte`、`ui/src/components/pianoroll/CcLanes.svelte`: `busy`(録音中かカウントイン中)を渡す。ピアノロールの目盛は、busy の間はループのドラッグを始めず、locate も送らない(UI-ARRANGE-013 と対になる)。
- `ui/src/components/pianoroll/NoteInspector.svelte`: 欄、全部に変える行、「割合」「だんだん」「ばらつき」、「のばす」、「かける」、「もとの位置へ戻す」は `editable`(`live` かつ `songBusy` でない)で決める。値は表示したままにし、欄のボタンは使えなくする。
- `ui/src/components/pianoroll/NoteListPanel.svelte`、`ui/src/components/pianoroll/CcListPanel.svelte`: busy の間は、どの欄も開かない(行は選べる)。
- テスト: `ui/tests/pianoroll.drag.test.ts` の "while recording or counting in (UI-ROLL-008)"(4 件)。`ui/tests/pianoroll.cc.test.ts` の "… while recording, a press on a point only selects it …"。`ui/tests/pianoroll.panels.test.ts` の "while recording or counting in, the panels do not edit (UI-ROLL-008)"(2 件)。`ui/tests/pianoroll.r5-notes.test.ts` の "while recording or counting in, no field opens (UI-ROLL-008)"。`ui/tests/pianoroll.hold-mount.test.ts` の "while counting in or recording (UI-ROLL-008, UI-ARRANGE-013)"(2 件)。
