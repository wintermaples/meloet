---
id: SOUND-002
title: 鍵を、変調エンベロープのアタックの途中で離すと、SF2の変調エンベロープが飛ぶ
area: SOUND
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# SOUND-002: 鍵を、変調エンベロープのアタックの途中で離すと、SF2の変調エンベロープが飛ぶ

## Where

crates/meloet-sound/src/sampler/shape.rs:222-227 (`Adsr::release_amount`)
crates/meloet-sound/src/sampler/shape.rs:198-208 (`Adsr::amount`。アタック中の、なめらかに立ち上がる曲線で、ここから続くべき値)

## What happens

SF2 のゾーンで、変調エンベロープにアタックがあり (`attackModEnv` が1フレームより長い)、ピッチかフィルターを動かす (`modEnvToPitch`、`modEnvToFilterFc`) 場合、アタックが終わる前に鍵を離すと、ピッチかフィルターのカットオフが一度に飛びます。

アタックの間、`amount()` は凸の曲線 `1 + 400/960 · log10(rise)` です。離すと `release_amount()` が `fall = 1 - rise` を設定するため、次の `amount()` は `1 - fall = rise` になります。これは直線的な値です。二つは同じではありません (計算したもので、実行はしていません):

| 鍵を離したときの rise | 離す前の amount | 離したあとの amount | `modEnvToFilterFc` = 2400 セントのときの飛び |
|---|---|---|---|
| 0.10 | 0.583 | 0.10 | −1160 セント |
| 0.25 | 0.749 | 0.25 | −1198 セント |
| 0.50 | 0.875 | 0.50 | −899 セント |
| 0.75 | 0.948 | 0.75 | −475 セント |

フィルターのカットオフは、32フレームの一つのチャンクの中で約1オクターブ下がります (フィルターの数値は、そのチャンクの中で直線的に動かされる。`Filter::run`)。`modEnvToPitch` の場合、ピッチは同じ割合のセントを一つのチャンクで下げます。鍵を離したときに、クリックや、急に暗くなる、または低くなる音として聞こえます。

## What should happen

離したあとの解放は、エンベロープがいた所から続くべきです。隣の音量のエンベロープは、そうなっています。`release_level()` (shape.rs:214-220) は、アタックの到達したレベルを対応する `fall` に変換し、そこには「The release goes on from the loudness the attack had reached」(解放は、アタックが到達していた音量から続く) というコメントがあります。architecture 20-7 は、再生を FluidSynth に合わせると述べています。メモリによる推定ですが (今回の調査では、そのソースは確認していません)、FluidSynth も、鍵を離したときにアタックの凸の値を、対応する直線の値に変換するようです。検証する人は、確かめるために `fluid_voice.c` を読むとよいでしょう。

## How to trigger

コードを読んで確かめたもので、実行はしていません。アタックが例えば 0.5 秒で、`modEnvToFilterFc` が数千セントある変調エンベロープを持つ SF2 のプリセットを読みこむ (スイープするシンセのブラスやパッドなど)。鍵を押して、0.1〜0.3 秒後に離すと、カットオフは、今の値から滑らかに下がらず、鍵を離したときに約1オクターブ下がります。

## Cause

`release_amount` は `1 - rise` を使います。これは直線的なアタックの値です。一方、アタック中の `amount()` は凸の値を返します。飛びの大きさは `convex(rise) - rise` です。

## Suggested fix

`release_amount` で、`rising()` のとき `self.fall = 1.0 - self.amount()` を設定します (`seg` が変わる前に読む)。そうすると、解放の最初の値はアタックの最後の値と等しくなります。音だけの変更で、プロトコル、ファイルの形、文面、見た目は変えません。アタック中に鍵を離す場合を含む `crates/meloet-sound/tests/sf2.rs` のテスト (例: `the_modulation_envelope_moves_the_pitch`) のようなテストで確かめられます。

## Verification

コードを読んで確かめたもので、テストは実行していません。`Adsr::amount()` は、アタック中は凸の値 `1 + 400/960 · log10(rise)` を返します。`release_amount()` は `fall = 1 - rise` を設定するため、鍵を離したあとの最初の値は `rise` になります。これは直線的な値です。`rise = 0.25` では、amount が 0.749 から 0.25 に一度に変わります。報告の表は手で確かめました (0.10 → 0.583、0.50 → 0.875)。

根拠にしたもの: (1) 同じファイルの隣の `release_level()` です。これはアタックのレベルを対応する `fall` に変換します (「The release goes on from the loudness the attack had reached」)。(2) SF2 の仕様書にある、ジェネレーター 30 `releaseModEnv` の記述です。私の記憶によるもので (今回の作業ではファイルから読んでいないため、信頼度は C)、「the release phase linearly ramps toward zero from the current level」(解放は、現在のレベルからゼロへ直線的に下がる) です。これは、直線的な値ではなく、エンベロープが持っていたレベルを意味します。(3) FluidSynth です。これも記憶によるもので (信頼度 C)、変調エンベロープのアタックの中で note-off が来ると、凸の値を直線の値に変換するため、解放は同じ値から始まります。三つとも同じ方向を指しています。一次資料は開いて確かめていません。

訂正: なし。重大度 `low` は変えません (変調エンベロープにアタックがあり、その途中で鍵を離すという、SF2 のプリセットと操作が必要なため)。

修正計画: `crates/meloet-sound/src/sampler/shape.rs` の `release_amount`: `rising()` のとき `self.fall = 1.0 - self.amount()` (`seg` が変わる前に読む)。テスト: `crates/meloet-sound/tests/sf2.rs` の `the_modulation_envelope_moves_the_pitch` の隣、または `Adsr` の単体テスト: アタック中に `release_amount()` の前後で `amount()` が等しいこと。

## Fix

- `crates/meloet-sound/src/sampler/shape.rs` の `Adsr::release_amount`: アタック中は `fall = 1 - amount()` (凸の値) にして、解放がアタックの値から始まるようにする。
- テスト: `sampler::shape::tests::the_modulation_release_goes_on_from_the_attack`。SF2 のテストの期待値は変えていない (`tests/sf2.rs`、`tests/sf2_fluidr3.rs` は今までどおり通る)。
