---
id: UI-ROLL-004
title: ピアノロールのドラッグ中の読み値が、曲の最初の拍子で小節・拍・長さを数える
area: UI-ROLL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-004: ピアノロールのドラッグ中の読み値が、曲の最初の拍子で小節・拍・長さを数える

## Where

- `ui/src/components/pianoroll/Roll.svelte:75` — `meter` は `{ num: timeline.meterNum, den: timeline.meterDen }`。曲の始めの拍子だけ。
- `ui/src/components/pianoroll/Roll.svelte:285` と `:1258` — その `meter` を、`RollDrag` と `CcLanes` (`LaneMouse`) が文字の読み値に使う。
- `ui/src/components/pianoroll/drag.ts:404` — 描く: `positionText(d.start, meter)`、`lengthText(d.length, meter)`。
- `ui/src/components/pianoroll/drag.ts:408-409` — 動かす: `signedLength(d.ticks, meter)`。
- `ui/src/components/pianoroll/drag.ts:415` — 長さを変える: `lengthText(…, meter)`。
- `ui/src/components/pianoroll/cc.ts:380` と `:383` — CC の点: `positionText(…, meter)`。

## What happens

拍子が途中で変わる曲では、ノートを描いているとき、または変化のあとに CC の点を追加・動かしているとき、「その場の読み値」と状態行は、誤った小節と拍を示す。長さや動きは、最初の拍子の拍で数えられる。例: 始めは 4/4、5 小節目から 3/4 (5 小節目は tick 30720 から始まる)。7 小節目の 1 拍目 (tick 42240) で描くと「6.3.0000」と読める。放すとノートの一覧と「選んだノート」は、同じノートについて「7.1.0000」を示す。4/4 の曲の 6/8 の部分では、八分音符一つ分の長さのノートは「長さ 0.0960」と読めるが、ノートの一覧は「1.0000」を示す。

## What should happen

仕様 3、「round 3 の規則」: 「動かしている値 (位置、音の高さ、長さ、ベロシティ、CC) は、「その場の読み値」(カーソルの近く) と状態行の両方に出す」。ほかのすべての場所では、位置はテンポマップに従い、長さはその場所の拍子を使う。ノートの一覧と「選んだノート」(`NoteListPanel.svelte:36`、`noteFields.ts:217`、`notes.ts:88`)、アレンジのドラッグの読み値 (`arrangeDrag.ts:290`)、マーカーとテンポの読み値 (`MarkerRow.svelte:302`、`TempoBand.svelte:78`) がそうである。

## How to trigger

実行せず、コードを読んで確かめた。4/4 の曲に、5 小節目で 3/4 への変更を加える。7 小節目に届くクリップをピアノロールで開く。「書く」を選び、7 小節目の 1 拍目で押したまま、読み値を読む。

## Cause

`positionText(tick, bars)` を、ふつうの拍子で呼ぶ (`barsMap(meter)` → 曲全体に一つの拍子) と、その一つの拍子の小節を tick 0 から数える。`lengthText` / `signedLength` は、その拍子の拍で数える。ピアノロールは、すでに持っているテンポマップ (`map`、`Roll.svelte:77`) ではなく、先頭の拍子を渡している。

## Suggested fix

`RollDrag` と `LaneMouse` に、読み値のためのテンポマップを渡す: `positionText(tick, map)`。長さは、そのノートの拍子で数える (`meterAtOf(map, tick)`)。ノートの一覧と同じように (ノートの一覧と同じ形)。プロトコルの変更はなし。新しい文言もなし。

## Verification

テストを走らせず、読んで確かめた。`Roll.svelte:75` は `timeline.meterNum` / `meterDen` (曲の最初の拍子) から `meter` を作り、`RollDrag` (285) と `CcLanes` に渡す。`drag.ts:404-415` と `cc.ts:380-383` は `positionText(tick, meter)` / `lengthText` / `signedLength` で読み値を作る。ふつうの拍子で `positionText` を呼ぶと、その一つの拍子の小節を tick 0 から数える。ノートの一覧と「選んだノート」はマップを使う。拍子が変わる曲にだけ当たる (そのため重大度は low)。

この不具合は UI-ROLL-005 と UI-ROLL-006 と一つの原因を共有する。ピアノロールは、その部品に先頭の拍子を渡している (`Roll.svelte:75`、`commands.ts:32-33`)。本当は、テンポマップ (`map`、`Roll.svelte:77`) が必要である。三つを一緒に直す: `RollInput` / `LaneInput` / `PianoRollKeyContext` にマップを渡し、そのテックの拍子をマップから取る。

この報告の修正案: `ui/src/components/pianoroll/drag.ts` と `cc.ts` は、位置を `positionText(tick, map)` で読み、長さはそのノートの拍子 (`meterAtOf(map, tick)`) で読む。`NoteListPanel.svelte:36` / `noteFields.ts:217` と同じように。テスト: 5 小節目から 3/4 の 4/4 の曲で、tick 42240 の描く読み値が「7. 1. 0000」と言うこと。

## Fix

UI-ROLL-005 と UI-ROLL-006 と一緒に直した。プロトコルの変更はなし。

- `ui/src/components/pianoroll/drag.ts`、`ui/src/components/pianoroll/cc.ts`: `RollInput.meter()` / `LaneInput.meter()` はテンポマップ (`Bars`) を返す。位置は `positionText(tick, map)` で読み、長さはそのノートの拍子 (`meterAtOf`) で読む。
- `ui/src/components/pianoroll/Roll.svelte`、`ui/src/components/pianoroll/CcLanes.svelte`: 最初の拍子ではなく、テンポマップ (`mapOf(timeline)`) を渡す。
- テスト: `ui/tests/pianoroll.meters.test.ts` の「readings while dragging follow the time signature changes (UI-ROLL-004)」(3 件) と「the piano roll hands the tempo map to its readings (mount)」。
