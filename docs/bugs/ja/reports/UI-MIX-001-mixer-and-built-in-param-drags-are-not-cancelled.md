---
id: UI-MIX-001
title: 部品が消えたとき、ミキサーと組み込みエフェクトのパラメータのドラッグが取り消されず、試し聞きの音と保持されたレーンが残る
area: UI-MIX
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-MIX-001: 部品が消えたとき、ミキサーと組み込みエフェクトのパラメータのドラッグが取り消されず、試し聞きの音と保持されたレーンが残る

## Where

ui/src/components/mixer/control.svelte.ts:80-86 (mixControl の onMount の後始末: `off(); keys.flush();`。`cancel()` はない)
ui/src/components/params/ParamRow.svelte:143-149 (onMount の後始末: `off(); stopFrame();`。`cancel()` はない)
ui/src/components/params/PluginParamRow.svelte:91-97 (正しく作られている兄弟: `if (drag) cancel();`)

## What happens

フェーダー、パンのつまみ、センドのバー(mixControl)や、組み込みエフェクトの行、音のつまみの行(ParamRow)が、マウスのドラッグの途中で外されると、何も送られない。`commit: true` も `cancelMixPreview` も送られない。コアは最後の `commit: false` の値を `Mixer::previews` に残すので、エンジンはドラッグされた値を鳴らし続ける。一方で、曲(と、部品が戻ったあとの画面)は古い値のままになる。その部品にオートメーションのレーンがあると、`commit: false` のインテントが掴み(`hand_moved`、crates/meloet-core/src/play/automation.rs:715)を作り、レーンを保持していた。その掴みは離されず、レーンは保持されたままになり、部品はオートメーションの線に従わなくなる。`auto.grips` を消せるのは、`CancelMixPreview`(`cancel_hands`)か、同じ部品の別のドラッグだけである。`mixer_changed`(play/mixer.rs:738)は次の曲の変更で previews を消すが、auto.grips は消さない。画面側の手の値(automation/handValue の `noteHand`)も忘れられない。

## What should happen

CLAUDE.md の決まり、と mixer/mixDrag.ts のコメント: ミキサーのドラッグは、離したときに `commit: true` を一つ送って終わる。取り消すときは `CancelMixPreview` を送る(「取り消し(Esc か、ポインタが奪われた場合)」)。部品が消えて途中で切れたドラッグも、取り消しとして扱う。PluginParamRow.svelte は、すでに後始末でドラッグを取り消している。

## How to trigger

コードを読んで確かめたもので、実行はしていない。 1. ミキサーの帯を開く。 2. トラックのフェーダーをマウスで押さえて動かす(音が大きくなる)。 3. 押さえたまま M を押す(一文字のキー `toggleBand mixer`。keys/keys.ts:119。handleKeydown はマウスのドラッグを見ていない)。ミキサーが閉じ、Strip が外れ、インテントは何も送られない。トラックは、曲のフェーダーの値は変わっていないのに、ドラッグされた音量のままになる。同じことは、組み込みエフェクトの行のドラッグ中に U を押す(パラメータの欄、`toggleBand params`)か、P を押す(棚が右側でパラメータの欄の前に出る)か、部品がドラッグの途中で外れるほかの方法でも起きる。トラックにボリュームのレーンがあって再生中なら、手順 3 のあとレーンが保持されたままになり、ボリュームは線に従わなくなる。

## Cause

mixControl はドラッグを `press`(MixPreview つき)で保ち、ParamRow は `dragging` で保つ。どちらも外れるときに、自分の `cancel()` を呼ばずに後始末する。MixPreview の、まだ待っている requestAnimationFrame は、外れたあとでもう一度 `commit: false` を送ることさえある(MixPreview.stop は呼ばれない)。

## Suggested fix

外れるときの後始末で、動いているドラッグを先に取り消す。PluginParamRow と同じ形にする。control.svelte.ts では `return () => { off(); cancel(); keys.flush(); };`、ParamRow.svelte では `return () => { off(); cancel(); stopFrame(); };`(ドラッグがないときは `cancel()` は何もしない)。プロトコルの変更はなく、新しい文言もない。

## Verification

読んで確かめた(実行はしていない)。`Where` の行は修正して合わせた(`control.svelte.ts:80-86`)。`mixControl` の後始末は `off(); keys.flush();` で、`ParamRow.svelte:143-149` の後始末は `off(); stopFrame();` である。どちらも自分の `cancel()` を呼ばない。一方、`PluginParamRow.svelte:91-97` は呼ぶ(`if (drag) cancel()`)。マウスボタンを押している間も一文字のキーは受け付けられる(`handleKeydown` は確認をしていない)ので、ドラッグ中の M / U / P で部品が外れる。Rust 側: `commit: false` の値は、同じ部品がもう一度設定されるか、`CancelMixPreview` が来るか、次のミキサーの変更が来るまで `Mixer::previews` に残る(`crates/meloet-core/src/play/mixer.rs:245-251`)。`hand_moved`(`play/automation.rs:715`)が作った掴みは、`hand_let_go` か取り消しの経路でしか消えない。そのため、聞こえている値と曲の値は食い違い、レーンは保持されたままになる。

修正計画(提案どおり): `ui/src/components/mixer/control.svelte.ts` の後始末を `off(); cancel(); keys.flush();` にする。`ui/src/components/params/ParamRow.svelte` の後始末を `off(); cancel(); stopFrame();` にする(ドラッグがないときは `cancel()` は何もしない)。`MixPreview.cancel()` が、待っている動画フレームも捨てることを確かめる(`ui/src/components/mixer/mixDrag.ts`)。テスト: 帯をマウントし、押して動かし、外す → `cancelMixPreview` が送られたこと。

## Fix

`ui/src/components/mixer/control.svelte.ts` と `ui/src/components/params/ParamRow.svelte`: 外れるときの後始末で、最初に `cancel()` を呼ぶ(動いているドラッグがあれば `cancelMixPreview` を送る。`MixPreview.cancel()` は待っているフレームをすでに捨てる)。テスト: `ui/tests/fix-ui-values.test.ts` の "UI-MIX-001: a drag whose part goes away is cancelled"。
