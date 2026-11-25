---
id: UI-ARRANGE-013
title: 録音中、アレンジの目盛のドラッグは、Rust が拒否する新しいループ範囲をまだ表示する
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-013: 録音中、アレンジの目盛のドラッグは、Rust が拒否する新しいループ範囲をまだ表示する

## Where

ui/src/components/Arrange.svelte:1263-1286 (`rulerDown` / `rulerMove` / `rulerUp`: `busy` を確かめていない)
crates/meloet-core/src/play.rs:1012-1015 (`set_loop`: `if self.busy() { return; }`)
crates/meloet-core/src/play.rs:928-931 (`locate`: 同じ)

## What happens

録音中またはカウントイン中に、アレンジの小節の目盛を押してドラッグすると、いつものようにループ範囲のドラッグが始まる。アクセントのループの印とループ線は新しい範囲へ動き、見出しはそれに対して「ループ a–b」を示す。離すと `setLoop` が送られるが、コアはそれを無視する (busy)。rev は来ず、プレビューは捨てられる (`rev === before`)。そのため印は元へ戻る。ふつうに押すと `locate` が送られるが、コアはそれも無視する。理由は何も使う人に知らされない。

## What should happen

アレンジの他のドラッグは、録音中またはカウントイン中には始まらない。クリップは選ばれるだけ (`Arrange.svelte:1381-1385`)、マーカーは動かない (`MarkerRow.svelte:65`、`moved = !busy && …`)、テンポの点と旗は選ばれるだけ (`TempoBand.svelte:99`、`:167`)、オートメーションの点は動かない (`AutoLane.svelte:131`)。目盛も同じにするべきである。`busy` の間はループのプレビューを出さない (architecture 10: カウントイン中と録音中に取らない操作)。

## How to trigger

コードから読んだもので、実行はしていない。録音を始める (R)。目盛を小節 3 から小節 6 までドラッグする: ドラッグ中はループの印が 3–6 を示す。離すと、元の範囲へ戻る。

## Cause

`rulerDown` は `busy` を見ずに `drag` を始める。`rulerMove` はそれから `preview` をセットする。

## Suggested fix

`busy` の間は `rulerDown` ですぐ戻る(または `busy` の間は `rulerMove` で `preview` をセットしない)。ピアノロールの目盛も同じハンドラを持つ (`pianoroll/Roll.svelte:1028-1039`、この範囲の外)。プロトコルや文面の変更はない。

## Verification

コードを読んで確認したもので、実行はしていない。`rulerDown` / `rulerMove` (`Arrange.svelte:1263-1273`) には `busy` の確認がないので、プレビューのループ範囲が表示される。`set_loop` と `locate` は `busy()` の間はすぐ戻る (`crates/meloet-core/src/play.rs:1012-1015`、928-931)。そのため返信の rev は `before` と等しく、プレビューは捨てられる。アレンジの他のドラッグは、busy の間は始まらないことをすでに拒否している。

UI-ROLL-008 (今は UI-PARITY-005 を含む)、UI-ARRANGE-009、UI-PARITY-010 と一つの根: 録音中またはカウントイン中に使えるように見えるが、Rust が拒否する部品。同じ規則で直す。アレンジがすでに持っている `busy` を使う。

修正計画: `ui/src/components/Arrange.svelte` で、`busy` の間は `rulerDown` ですぐ戻る (何も表示せず、何も送らない)。ピアノロールの目盛のハンドラ (`ui/src/components/pianoroll/Roll.svelte:1028-1039`) も同じにする(UI-ROLL-008 の修正が先に行われるなら、そちらでカバーされる)。プロトコルや文面の変更はない。

## Fix

アレンジの側。`ui/src/components/Arrange.svelte` の `rulerDown` は、`busy` (録音中またはカウントイン中) の間はすぐ戻る。ループのプレビューも、`setLoop` も、`locate` もなし。ピアノロールの目盛 (`pianoroll/Roll.svelte`) はピアノロールの担当者に任せた。
テスト: `ui/tests/arrange.test.ts` の「while recording or counting in, a drag or a press on the ruler shows no loop range and sends nothing」。
