---
id: UI-MIX-007
title: 矢印キーの直後 1 秒以内にフェーダーやセンドをつかむと、古い値から始まり、つまみが元に戻る
area: UI-MIX
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-007: 矢印キーの直後 1 秒以内にフェーダーやセンドをつかむと、古い値から始まり、つまみが元に戻る

## Where

ui/src/components/mixer/control.svelte.ts:49-57（`down`: `const from = opts.value()`。KeySteps の待ちを流さない）
ui/src/components/mixer/control.svelte.ts:132-134（`shown` は、押す前は `keyed` を表示する）

## What happens

↑ ↓（フェーダー、センド）または ← →（パン）を押すと、画面には新しい値（`keyed`）が出ますが、手は、キーを離してから 1 秒後に `commit: true` で送られます。その 1 秒の間にマウスが同じ部品をつかむと、`down` はドラッグを曲の値から始めます。つまみは押した時点で古い位置に戻り、ドラッグはそこから動きます。その後、ドラッグの途中で `KeySteps` のタイマーが動き、キーの値を `commit: true` で送ります（一つの手になり、音はその値に飛びます）。一方、画面はドラッグの値を示しています。ドラッグの次の `commit: false` で、音はまた戻ります。

## What should happen

ドラッグは、つまみのある所から始まるべきです（仕様 3（86-7）のキーについての決まり。control.svelte.ts の冒頭: 「押した点に飛ばない。押した点からの動きだけが値を変える」）。キーの手は、新しい手が部品に触れる前に作られるべきです。`startTyping` と Enter が既に `keys.flush()` でそうしています。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。Tab でフェーダーにフォーカスを合わせ、↑ を三回押します（表示は古い値より +3 dB）。1 秒以内にマウスでそのフェーダーを押すと、つまみは一度に 3 dB 戻ります。

## Cause

`down` は、保留中のキーの手を流さず、`keyed` も読みません。

## Suggested fix

`down` の最初で `keys.flush()` を呼び、`from` は表示している値から取る（`keyed ? keyed.value : opts.value()`）。プロトコルの変更、新しい文面はありません。

## Verification

コードを読んで確認しました（実行していません）。`Where` の行は訂正済みです。`down`（`control.svelte.ts:49-57`）は `from = opts.value()` を取り、`keys.flush()` を呼びません。`shown` は押す前には `keyed.value` ですが、押した後は `live.now = from` になるので、つまみはキーで動いた分だけ戻ります。その後、KeySteps のタイマー（`mixDrag.ts:84`、`KEY_COMMIT_MS`）がドラッグの途中で動き、キーの値を `commit: true` で送ります。`startTyping` と Enter は既に流しています。

修正の計画（提案どおり）: `down` で最初に `keys.flush()`、`from = keyed ? keyed.value : opts.value()`（`ui/src/components/mixer/control.svelte.ts`）。テスト: ↑ を三回押し、待ちの間に押すと、ドラッグの前に `commit: true` が一つ、キーの値で出る。`live.from` はキーの値。

## Fix

`ui/src/components/mixer/control.svelte.ts` の `down`: 最初に `keys.flush()` を呼び、`keyed` があればドラッグはそこから始まります。テスト: `ui/tests/fix-ui-values.test.ts` の「UI-MIX-007: grabbing a fader right after arrow keys」。
