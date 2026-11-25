---
id: UI-MIX-002
title: オートメーションを書き込むドラッグの後、パラメータの欄の行がドラッグした値で止まったままになる
area: UI-MIX
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-MIX-002: オートメーションを書き込むドラッグの後、パラメータの欄の行がドラッグした値で止まったままになる

## Where

ui/src/components/params/ParamRow.svelte:60-72 (`held`、`shown = held ?? auto ?? value`、そして `value` が変わったときだけ `held` を消す effect)
ui/src/components/params/ParamRow.svelte:121-132 (onup は `commit: true` の後も `held = v` を残す)

## What happens

内蔵エフェクト（BuiltinList）または音源のつまみ（SoundKnobList）のパラメータの行で、そのオートメーションのレーンを再生中に「● 書く」で書き込みにしてあると、ドラッグはレーンを書き込みます。コアは、手を離したときの答えとして `LetGo::Written` を返し（crates/meloet-core/src/play/automation.rs:756-762）、レーンの点だけを変えて、曲の中のそのパラメータ自身の値は変えません。そのため、行の `value` の prop は変わらず、`held` を消す effect は動きません。`held` にはドラッグの最後の値が残ります。`shown` では `held` が `auto` より先に来るので、バー、つまみ、値の文字は、その値で止まったままになり、レーンはその間もサウンドを動かし続けます。曲のそのパラメータの値が別の理由で変わるまで、この状態が続きます。

## What should happen

architecture 21-8: 帯が動かしているものは、線の上の値で画面に示す（`auto`。ParamRow.svelte のヘッダーのコメントの「帯が動かしている間、値は線の上のもの」）。ミキサーはこれを正しく行っています。mixControl は、手を離したときにドラッグの値（`live`）を捨て、PluginParamRow は新しい値が来るたび（`tick`）に `held` を消します。

## How to trigger

コードを読んで確認したもので、実行はしていません。1. 内蔵のリバーブをトラックのインサートに置き、それを選ぶ。2. 行のオートメーションの印を押して、「ミックス」のレーンを加え、「● 書く」で書き込みにして、再生を始める。3. 「ミックス」のバーを 30 % から 80 % までドラッグし、手を離す。4. レーンには書き込まれた動きが入り、再生もされるが、行は 80 % のまま表示され、つまみは動かない。

## Cause

`held` は、`$effect(() => { void value; ... held = null })` でだけ消されます。オートメーションを書き込む手の離し方は、`value` の変化ではありません（同じ数になっても Svelte は effect を再実行しません）。そのため、held は決して消えません。

## Suggested fix

ドラッグの終わりで、答えが曲の値にはなりえないとき `held` を消す。または、`auto` が変わったときにも消す。例: effect が `auto` も読むようにする（`void value; void auto;`）、または、`auto !== null` のとき onup で `held = null` にする。プロトコルの変更も、新しい文言もありません。

## Verification

コードを読んで確認しました（実行はしていません）。`ParamRow.svelte:60-72`: `shown = held ?? auto ?? value` で、`held` は `value` についての effect でだけ消されます。`onup`（121-132）は `commit: true` の後も `held = v` を残します。Rust では、書き込みにしたレーンの上で再生中に手を離すと `LetGo::Written` になり（`crates/meloet-core/src/play/automation.rs:756-762`）、`play/params.rs:210` は `Written` に対して何もしないので、そのパラメータ自身の値は曲の中で変わらず、`value` の prop は同じ数のままで、effect は動きません。`PluginParamRow` と `mixControl` は、このように手を離した後も値を保つことはありません。

修正の計画: `ui/src/components/params/ParamRow.svelte` で、`value` が変わったときだけでなく、手を離したことへの答えが来たとき `held` を消す。`set(v, true)` は `send()` を通り、それは rev を返すので、その約束（promise）が解決し、新しいドラッグが始まっていないときに `held` を消す（Rust が拒んだ値の手の離し方もこれで扱える）。既存の effect は残す。もっと単純だが弱い方法: 消す effect が `auto` も読むようにする。テスト: `auto` を設定し、ドラッグして、`value` を変えずに手を離す → 行がまた `auto` を示す。

## Fix

`ui/src/components/params/ParamRow.svelte`: 行から送られた手（ドラッグの手の離し方、矢印キー、◂ ▸）は、その答えになる rev に画面の状態が達するまでだけ `held` を保つ（`commitStep`、`answer`）。そのため、レーンへの書き込みだけをした手の離し方は `auto` に戻ります。`value` での消去は残しています。テスト: `ui/tests/fix-ui-values.test.ts` の「ドラッグの答えが曲の値を変えなかった後、行はまたレーンに従う」。
