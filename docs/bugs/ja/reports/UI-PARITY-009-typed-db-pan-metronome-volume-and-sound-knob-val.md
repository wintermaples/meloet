---
id: UI-PARITY-009
title: 打ち込んだ dB、パン、メトロノームの音量、音の つまみの値は、ほかの数字の欄と違い、全角の数字を読まない
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-009: 打ち込んだ dB、パン、メトロノームの音量、音の つまみの値は、ほかの数字の欄と違い、全角の数字を読まない

## Where

ui/src/components/mixer/scale.ts:63-68 (`parseDb`: `−` と `ー` しか変換せず、`\d` は ASCII だけ)
ui/src/components/mixer/scale.ts:71-78 (`parsePan`)
ui/src/components/metronome.ts:59-63 (`parseClickDb`)
ui/src/components/params/knobRows.ts:41-45 (`typedKnob`: `Number("３５")` は NaN)
これらを使う所: mixer/control.svelte.ts:172 (フェーダー)、mixer/SendBar.svelte:22、mixer/PanKnob.svelte:22、TrackHeads.svelte:49 (トラックの見出しの dB)、params/ClipPanel.svelte:124 と params/ClipsPanel.svelte:45 (クリップのゲイン)、MetronomeWindow.svelte:155、params/ParamRow.svelte:177 (音の四つのつまみ)

## What happens

日本語入力がオンのとき、数字は全角で入ります。フェーダー、センド、トラックの見出しの dB に「－３」、パンに「Ｌ２０」や「２０」、メトロノームの音量に「－１２」、クリップの「ゲイン」に「－２」、音の つまみ (「明るさ」など) に「３５」と打つと、読めません。ミキサーとメトロノームの欄は開いたままになり、クリップのゲインと音のつまみは何も送らず、何も言いません。同じ数字をテンポ、位置、ノートと CC の一覧、クリップの位置、長さ、フェード、音の高さ、「音程を寄せる」、遅延の欄、組み込みエフェクトの値に打つと読めます。

## What should happen

ほかの数字の欄は、まず文字を ASCII に直します。valueClamp.ts:23 と time.ts:64 (`normalize("NFKC")`)、pianoroll/noteFields.ts:27、params/clipFields.ts:23 (クリップの欄のほかの項目)、SettingsDialog.svelte:79、clipEdits.ts:42 (繰り返しの回数)、そして組み込みエフェクトの Rust の `ParamSpec::read` (architecture 20-13: 「full-width digits and 「−」 too」(全角の数字と「−」も))。7-readings には、位置の欄 (「full-width characters are fine」)、繰り返しの回数 (「full-width digits read」)、トラックの番号 (「Full-width digits are read as digits ... people type with the Japanese input on」) について同じ規則が書かれています。クリップの欄では、ゲインだけがそのグループでは読まない欄です。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。
1. 日本語入力をオンにする (全角の数字になる)。
2. ミキサーで、フェーダーの dB の文字をダブルクリックし、-3 と打つ。欄には「－３」が入る。
3. Enter: `parseDb` は「－３」を残す (U+FF0D と U+FF13 は変換されない)。パターン `^[+-]?(\d+...)$` は合わず、`undefined` になる。欄は開いたまま。
4. 音の組み込みのパラメータの欄で、「明るさ」の値をダブルクリックし、35 (「３５」) と打って Enter: `typedKnob` は null を返し、何も送らず、何も言わない。同じ文字をリバーブの「ミックス」の行に打つと、Rust が読む。

## Cause

この四つのパーサーは、それぞれ独自の小さな文字の対応表で書かれ、`normalize("NFKC")` を呼んでいない。ほかの欄が使う共通の `tidy` 関数とは違う。

## Suggested fix

`parseDb`、`parsePan`、`parseClickDb`、`typedKnob` のそれぞれを `text.normalize("NFKC")` で始める (`clipFields.ts` の `tidy` と同じ)。`−` を「-」にする変換は、その後に残す。NFKC は U+2212 をそのままにするため。プロトコル、文面、見た目の変更はなし。

## Verification

コードを読んで確かめた (実行していない)。`parseDb` と `parsePan` (`mixer/scale.ts:63-78`)、`parseClickDb` (`metronome.ts:59-63`)、`typedKnob` (`params/knobRows.ts:41-45`) は「−」(と「ー」) しか変換せず、そのあと `\d` / `Number` を使う。どちらも全角の数字を読まない。ほかの欄は、先に `normalize("NFKC")` を呼ぶ (`valueClamp.ts:23`、`time.ts`、`noteFields.ts`、`clipFields.ts`)。7-readings は、位置の欄、繰り返しの回数、トラックの番号について全角を読むと書いている (「people type with the Japanese input on」)。

修正案 (提案どおり): 四つのパーサーのそれぞれを `text.normalize("NFKC")` で始める (「−」から「-」への変換は、その後に残す。NFKC は U+2212 をそのままにするが、U+FF0D 「－」は「-」にする)。テストは既存のパーサーのテストの隣に置く: 「－３」、「Ｌ２０」、「－１２」、「３５」。UI-MIX-005 と同じ `submit` の経路に触れるので、一度にまとめて直す。

## Fix

`parseDb`、`parsePan` (`ui/src/components/mixer/scale.ts`)、`parseClickDb` (`ui/src/components/metronome.ts`)、`typedKnob` (`ui/src/components/params/knobRows.ts`) は `normalize("NFKC")` で始まる。テスト: `ui/tests/fix-ui-values.test.ts` の「UI-PARITY-009: full-width digits」。
