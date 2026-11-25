---
id: MODEL-B-003
title: エフェクトの値を打つとき、全角の数字は読めるが全角の単位の文字は読めず、全角の 500ms が 500 s になる
area: MODEL-B
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-B-003: エフェクトの値を打つとき、全角の数字は読めるが全角の単位の文字は読めず、全角の 500ms が 500 s になる

## Where

crates/meloet-model/src/effects.rs:122-130 (`ParamSpec::read`: ASCII に直すのは `０`-`９`、`−`、`－`、`＋`、`．` だけ)
crates/meloet-model/src/effects.rs:146-152 (単位は ASCII の `ms`、`s`、`k` とだけ比べる)
ui/src/components/params/ParamRow.svelte:174 (画面は打たれた文字をそのまま送る)

## What happens

組み込みエフェクトの値の欄に、日本語の入力方式を全角の英数字の状態で打つと、単位の文字が全角になります。リバーブの 「響きの長さ」 (秒) で 「５００ｍｓ」 と読むと、500 と読まれ (単位の 「ｍｓ」 は認識されない)、10 s に詰められ、状態行には値が範囲の端に置かれたと出ます。コンプレッサーのアタック (ms) で 「１.５ｓ」 と打つと、1.5 ms になり、1500 ms ではありません。EQ の周波数で 「２ｋＨｚ」 と打つと、2 Hz になり (最も低い値に詰められる)。同じ全角の文字でも単位がないもの、または単位が ASCII のものは正しく読めます。

## What should happen

単位は、どの幅で打たれても読むべきです。docs/architecture.md 20-13 (「組み込みエフェクトへの入力は … 「500 ms」、「1.5 s」、「2 kHz」 を変換する」)。また `read` は、全角の数字、符号、全角の点を既に読んでいます (そのコメント: 「１２０」)。画面の他の入力欄は、文字列全体を先に NFKC で正規化しているので、全角の英字も働きます (ui/src/components/valueClamp.ts:23、ui/src/components/time.ts:64、ui/src/components/params/clipFields.ts:23)。

## How to trigger

コードを読んだ結果です。実行はしていません。
1. トラックにリバーブを置き、パラメータの欄を開いて、「響きの長さ」 をダブルクリックする。
2. 入力方式を全角の英数字にして、500ms と打ち、Enter を押す。欄には 「５００ｍｓ」 が入り、`TypeParam { text }` で送られる。
3. `Play::type_param` (crates/meloet-core/src/play/mixer.rs:498) が `spec.read` を呼ぶ。数字は 500、`unit` は 「ｍｓ」 (全角のまま小文字にしても全角のまま)。`starts_with("ms")` は偽。倍率は 1。値は 10.0 s になり (詰められる)、`ParamClamped` が返る。

## Cause

`read` の文字の対応表は、数字といくつかの符号だけを扱い、全角の英字 (U+FF21..U+FF3A、U+FF41..U+FF5A) を扱っていません。そのため、`unit.starts_with("ms")`、`starts_with('s')`、`starts_with('k')` は全角の単位には一度も一致しません。

## Suggested fix

`ParamSpec::read` の文字の対応表に、全角の英字 (`'Ａ'..='Ｚ'`、`'ａ'..='ｚ'`) も ASCII の英字に直す (コードから 0xFEE0 を引く)。または、画面が `typed.normalize("NFKC")` を送るようにする (同じ仲間の欄がそうしているように。これなら、同じ箱に打ったプラグインの値も対象になります)。プロトコルや曲ファイルの変更はありません。「５００ｍｓ」 のような場合を、`a_typed_value_reads_with_or_without_its_unit` のテストで見張ることができます。

## Verification

コードを読んで確認しました (テストは実行していません)。`ParamSpec::read` は、全角の数字、`−`、`－`、`＋`、`．` だけを ASCII にします。単位は、`to_lowercase()` の後で ASCII の `ms`、`s`、`k` と比べられるため、全角の英字は全角のまま残ります。`ParamRow.svelte:174` は、打たれた文字をそのまま送ります。そのため、秒の値に 「５００ｍｓ」 と打つと、500 と読まれます。重大度 `low` はそのままです (入力方式を全角の英数字にしていて、単位が別の単位である場合だけ)。

Corrections: 訂正なし。

修正の計画: `crates/meloet-model/src/effects.rs` の `ParamSpec::read`: 文字の対応表に `'Ａ'..='Ｚ' | 'ａ'..='ｚ'` を加える (ASCII の英字へ、コードから 0xFEE0 を引く)。また、そのときに全角の空白 `'\u{3000}'` を普通の空白にする。Rust の側だけの変更なので、AI の連携が打つ値にも効きます。「５００ｍｓ」、「１．５ｓ」、「２ｋＨｚ」 の場合を、テスト `a_typed_value_reads_with_or_without_its_unit` に加える。

## Fix

`crates/meloet-model/src/effects.rs` の `ParamSpec::read`: 全角の英字 (`Ａ`-`Ｚ`、`ａ`-`ｚ`) は、単位を読む前に ASCII の英字になります (Rust の側なので、AI の連携が打つ値にも効きます)。計画にあった全角の空白の行は必要ありませんでした。`trim` と `trim_start` が既に全角の空白を空白として扱うためです (「５００　ＭＳ」 の場合がそれを示しています)。テスト: `effects::tests::a_typed_value_reads_with_or_without_its_unit` に、「５００ｍｓ」、「５００　ＭＳ」、「１．５ｓ」、「２ｋＨｚ」 の場合を加えた。
