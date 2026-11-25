---
id: UI-PARITY-013
title: 量子化の強さとスウィングのバーは、ドラッグ中に gray.7 の開始の印が出ない。パラメータとメトロノームのバーは出る
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-013: 量子化の強さとスウィングのバーは、ドラッグ中に gray.7 の開始の印が出ない。パラメータとメトロノームのバーは出る

## Where

ui/src/components/pianoroll/PercentSlider.svelte:47-53 (`ondown` は開始の値を保存しない) と :122-124 (バーは、溝、塗り、つまみだけを描く)
同じ仲間で印を持つもの: ui/src/components/params/ParamRow.svelte:62、:107、:228 (`from`。`{#if dragging}<div class="knob ghost" …>` で gray.7 の枠。:359-363)、ui/src/components/MetronomeWindow.svelte:44、:85、:267 (同じ。:401-404)

## What happens

「選んだノート」の量子化の 「強さ」 または 「スウィング」 のバーをドラッグしている間、動いているつまみだけが描かれ、ドラッグを始める前の値の位置は表示されません。パラメータの欄やメトロノームの窓にある同じ形のバーは、ドラッグの間ずっと、開始の値の位置に gray.7 の枠のつまみを表示します。

## What should happen

CLAUDE.md の規則: 「ドラッグ中は、まず画面だけが動き、離したときに意図を 1 つ送る … もとの位置は gray.7 の枠で示す。」 PercentSlider の見出しは、仕様 2 章 「パラメータの行」の 「なめらかに動く値」 だと書いています。7-readings の 79 行目 (「強さとスウィングのバーのホバーと動き」) は、このバーにパラメータの行と同じ動きの見た目を与えています。同じ形の他のバーである ParamRow と MetronomeWindow は、開始の印を描いています。

## How to trigger

コードを読んだ結果です。実行はしていません。
1. ピアノロールと 「選んだノート」 を開く。
2. 量子化の 「強さ」 のバーを押して横にドラッグする。つまみは動くが、古い値の位置に gray.7 の印は残らない。比べるなら、組み込みエフェクトのパラメータのバーをドラッグする。

## Cause

PercentSlider は、`from` の値を保持せず、ghost の要素も持っていません。

## Suggested fix

`ondown` で `from = value` を保存し、ドラッグ中は `{#if dragging}<div class="knob ghost" style:left="{from}%"></div>{/if}` を描く。スタイルは ParamRow と同じ `.knob.ghost` (枠は `line.hair` の gray.7、塗りなし)。既存のトークンだけを使い、プロトコルや文言は変えません。

## Verification

コードを読んで確認しました (実行はしていません)。`PercentSlider.svelte` は、`ondown` (47-53) で開始の値を保持せず、溝、塗り、つまみだけを描きます (`ghost` は grep で見つからない)。`ParamRow.svelte` と `MetronomeWindow.svelte` は、ドラッグ中に開始の値の位置へ gray.7 の枠のつまみを描きます。CLAUDE.md の規則: 「ドラッグ中は、まず画面だけが動き … もとの位置は gray.7 の枠で示す。」 7-readings の 79 行目は、これらのバーにパラメータの行のホバーと動きの見た目を与えており、開始の印を外すとは書いていません (わざと外している場合は、7-readings にそう書かれています。たとえば挿入の並べ替えは 299 行目)。

修正の計画 (提案どおり): `ui/src/components/pianoroll/PercentSlider.svelte` で、`ondown` に `from = value` を保持し、ドラッグ中は `{#if dragging}<div class="knob ghost" style:left="{from}%"></div>{/if}` を描く。スタイルは `ParamRow.svelte` の `.knob.ghost` (既存のトークンだけ)。

## Fix

`ui/src/components/pianoroll/PercentSlider.svelte`: 押したときの値 (画面に出ていた値) を `from` に保持し、ドラッグ中は `.knob.ghost` (枠は `line.hair` の gray.7、ParamRow と同じく塗りなし) を描く。テスト: `ui/tests/fix-ui-values.test.ts` の 「UI-PARITY-013: 強さとスウィングのバーは、ドラッグを始めた位置を示す」。
