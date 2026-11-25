---
id: UI-TEXT-004
title: コード内で作られる範囲の印と括弧が、英語の状態行でも日本語のまま残る
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-004: コード内で作られる範囲の印と括弧が、英語の状態行でも日本語のまま残る

## Where

ui/src/components/format.ts:153-155 (`rangeText`: `${low}〜${high}`。pianoroll/noteFields.ts:96,237,258,285 と pianoroll/ccFields.ts:144,168 で使用)
ui/src/time/bars.ts:150 (`tickRangeText`: `0〜…`。valueClamp.ts:115、noteFields.ts:97,108 で使用)
ui/src/components/valueClamp.ts:79 (`1〜${max}`)
ui/src/components/valueClamp.ts:113 (`1〜${num}`)
ui/src/components/StatusLine.svelte:162 (`${min}〜${max}`。`params.valueClamped` 用)
ui/src/components/StatusLine.svelte:250 と :256 (`${track}（${short}）`。全角の括弧)
crates/meloet-core/src/play/params.rs:576 (`format!("{}〜{}", …)`: プラグインパラメータの範囲。Rust で作られ、StatusLine.svelte:158 がそのまま表示する)

## What happens

英語のページでは、状態行が英語の文に日本語の印を混ぜて表示します。「Beat is 1〜4, so it is now 4.」「Bars is 1〜64 …」「Tick is 0〜1919 …」「Ratio is 1〜20, so it is now 20.0 : 1.」、プラグインパラメータの「Mix is 0 %〜100 % …」。ノートの欄と CC の欄も同じです。2 本以上の歌のトラックを同時に録音しているときは、「● Recording · 6 Vocal（In 1）, 7 Vocal 2（In 1 · 2） · …」となります。en.ts から来る範囲は「1–127」「30.00–300.00」（`value.velocityRange`、`value.tempoRange`）と書かれているので、同じ文の形で、2 種類の範囲の印が出ます。

## What should happen

画面の文言と、それをつなぐ印は、すべてページの言語に従うべきです（CLAUDE.md: 文言は ui/src/text/ にある。architecture 25-36: 継ぎ目は index.ts の生きた束縛で、言語に合わせて変わる。spec 7-readings D4「継ぎ目」）。英語の範囲は en.ts と同じく「–」を使い、括弧は en.ts と同じく「(…)」にします（`msg.recordingVocal` の「{track} ({input})」）。

## How to trigger

コードを読んで確かめました。実行はしていません。英語のページで、拍の部分の位置の箱に 9 を入れる（4/4 で）、1919 より大きいティックを入れる、「くり返す…」の箱に 99 小節を入れる、組み込みエフェクトの行に大きすぎる値を入れる。または、歌のトラックを 2 本同時に録音する。

## Cause

これらの文字列は、コンポーネントのなかで、「〜」と「（」「）」を直接書いて組み立てられています。文字のキーや index.ts の継ぎ目を使っていないため、`useLanguage` で変わりません。

## Suggested fix

範囲の継ぎ目と括弧の形を、文言の表に加えます（例: `common.range` は「{low}〜{high}」と "{low}–{high}"、`common.inBrackets` は「{text}（{inner}）」と "{text} ({inner})"）。または index.ts に、`LIST_COMMA` の隣の継ぎ目として置きます）。そして、挙げた場所でそれを使います。プラグインの範囲は、Rust が両端をすでに文字として持っているので、画面がそれをつなぐのが望ましいです。ただし `ParamEnd` の変更になり、プロトコルの変更です。または、画面が継ぎ目を置き換えます（ただし壊れやすい）。日本語の出力は、そのまま変えません。英語の文言は 2 つ必要ですが、en.ts がすでに使っている形です。プロトコルは変えません。

## Verification

コードを読んで確かめました（実行はしていません）。印はコードに直接書かれています。`rangeText`（`components/format.ts:153-155`、「〜」）、`tickRangeText`（`time/bars.ts:150`）、`valueClamp.ts:79, 113`、`StatusLine.svelte:162`、`StatusLine.svelte:250, 256` の全角の括弧、そして、プラグインパラメータの範囲を作る Rust の `format!("{}〜{}", …)`（`crates/meloet-core/src/play/params.rs:576`）。どれも言語によって変わりません。一方、`en.ts` は範囲を「1–127」、括弧を「 (…)」と書いています。7-readings D4「継ぎ目」には「 ・ 」と「、」、そして文が挙がっていますが、範囲の印と括弧は挙がっていません。意図して残したのではなく、漏れていました。

新しい文言は要りません。`en.ts` がすでに使っている 2 つの印だけなので、`confirmed` とします。

修正案: 継ぎ目を 2 つ、`LIST_COMMA` / `STATUS_JOIN` の隣に加えます（`ui/src/text/ja.ts`、`en.ts` の `enJoins`、`index.ts`）。範囲の印（「〜」 / "–"）と、括弧の組（「（」「）」 / " (" ")"）です。それを `rangeText`、`tickRangeText`、`valueClamp.ts` の 2 か所、`StatusLine.svelte:162, 250, 256` で使います。日本語の出力は、1 文字も変えてはいけません（spec 6 の文言のテストが見張ります）。Rust で作るプラグインの範囲は、core が言語を知っています（architecture 25-36 の `Play::names`）。そのため `play/params.rs:576` が、言語に合わせて印を選びます（`language.pick("〜", "–")`）。プロトコルは変えません。（両端を別々に送る方がすっきりしますが、`ParamEnd` の変更になり、プロトコルの版が変わるので、必要ではありません。）7-readings D4 の「継ぎ目」に、2 つの印を 1 行足します。

## Fix

継ぎ目 `RANGE_MARK`（「〜」 / "–"）と `BRACKET_OPEN` / `BRACKET_CLOSE`（「（」「）」 / " (" ")"）を、`LIST_COMMA` の隣に加えました（`ui/src/text/ja.ts`、`en.ts` の `enJoins`、`index.ts` の生きた束縛）。使ったのは、`rangeText`（`ui/src/components/format.ts`）、`tickRangeText`（`ui/src/time/bars.ts`）、`ui/src/components/valueClamp.ts` の 2 つの範囲（`rangeText` を通して）、`ui/src/components/StatusLine.svelte` です（組み込みパラメータの範囲は `rangeText` を通し、歌の入力の括弧はそのまま）。Rust では、`crates/meloet-core/src/play/params.rs` がプラグインパラメータの両端を `self.names.pick("〜", "–")` でつなぎます。プロトコルは変えません。日本語の出力は変わりません。文書: architecture 25-36（継ぎ目、プラグインの範囲）、7-readings D4 の行「範囲の印と括弧」。
テスト: `ui/tests/language.test.ts` の「the range mark and the brackets follow the language」。`ui/tests/round7.vocal.test.ts` の「English: with monitor off, …」（括弧）。`crates/meloet-core/src/play/params/tests.rs` の `the_range_of_a_clamped_plugin_value_is_joined_in_the_language_of_the_page`。
