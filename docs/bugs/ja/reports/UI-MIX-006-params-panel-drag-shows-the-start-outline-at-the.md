---
id: UI-MIX-006
title: パラメータの欄のドラッグは、オートメーションのレーンがつまみを置いていた所ではなく、曲の値で開始の枠を出す
area: UI-MIX
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-006: パラメータの欄のドラッグは、オートメーションのレーンがつまみを置いていた所ではなく、曲の値で開始の枠を出す

## Where

ui/src/components/params/ParamRow.svelte:103-113 (ondown の `from = value`)
ui/src/components/params/ParamRow.svelte:228 (gray.7 の開始の枠を `fractionOf(spec, from)` で描く)

## What happens

オートメーションのレーンが組み込みエフェクトのパラメータor音のつまみを動かしているとき (`auto` が設定されていて、つまみはレーンの線の値にある)、バーを押すと、`from` は `value` から取られます。これは曲の中のパラメータそのものの値で、表示している値ではありません。そのため、gray.7 の開始位置の枠は、ドラッグを始めたときのつまみの位置ではなく、レーンの下にある曲の値の位置に描かれます。動かしていないことを判定する `v === from && !previewed` も、その別の値と比べます。

## What should happen

CLAUDE.md: 「While dragging, only the screen moves first ... The original position is shown with a gray.7 border.」(ドラッグ中は、まず画面だけが動く。もとの位置は gray.7 の枠で示す)。もとの位置は、つまみがあった所です。ミキサーは正しくやっています。mixControl は `from = opts.value()` を取り、MixerStrips はそこへレーンの表示値 (`shownGain`、`shownPan`、`shownSends`) を入れる。PluginParamRow は、表示している行の値から取る。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。リバーブの「ミックス」にレーンを追加し、0 % から 100 % の線を引く。つまみが線の上を動くように再生する (曲の値は、たとえば 30 %)。つまみが 80 % にあるときにバーを押すと、開始の枠が 30 % に出る。

## Cause

ondown は、表示用の `shown` (`held ?? auto ?? value`) ではなく、`value` の prop を読む。

## Suggested fix

ondown で `from = shown` を取る (`held` を設定する前に読む)。プロトコルの変更なし、文面や見た目の変更もなし。

## Verification

コードを読んで確かめた (実行していない)。`ParamRow.svelte` の `ondown`: `from = value` (曲の値) である一方、つまみは `shown = held ?? auto ?? value` で描かれ、gray.7 の枠は `fractionOf(spec, from)` で描かれる。レーンがパラメータを動かしているとき `auto !== value` なので、枠はつまみがあった所に来ない。`mixControl` は `from = opts.value()` を取り、その値はストリップがレーンの表示値 (表示される値) から入れる。

修正案 (提案どおり): `ui/src/components/params/ParamRow.svelte` の `ondown` で `from = shown` とする。`held` を設定する前に読む。すると、動かしていないことを判定する `v === from && !previewed` も、表示している値と比べることになる。これが正しい「何も動いていない」の判定。

## Fix

`ui/src/components/params/ParamRow.svelte` の `ondown`: `from = shown` (つまみのある所。レーンがパラメータを動かしているときはレーンの値)。テスト: `ui/tests/fix-ui-values.test.ts` の「the start frame is where the handle was (the lane's value), not the song value」。
