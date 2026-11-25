---
id: UI-TEXT-003
title: 英語の単数・複数の組が、マイナス記号で書いた符号つきの -1 に複数形を選ぶ
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-003: 英語の単数・複数の組が、マイナス記号で書いた符号つきの -1 に複数形を選ぶ

## Where

ui/src/text/index.ts:55 (`english()`: `Number(values?.[entry.by]) === 1`)
ui/src/components/pianoroll/drag.ts:408 (`pr.readMove` に `keys: signed(d.keys)`)
ui/src/components/arrangeDrag.ts:293 (`arrange.readMove` に `delta: barsDeltaText(...)`)
ui/src/components/pianoroll/geom.ts:441 (`signed` は U+2212 の「−」を書く)

## What happens

英語では、ノートを半音下へ動かすと「+0.0000 · −1 semitones」と読める。クリップを一小節左へ動かすと「… (−1 bars)」と読める。上や右へ動かすと「+1 semitone」「+1 bar」と正しく読めるので、二つの向きが食い違う。

## What should happen

仕様 7-readings、D4「数字」: 数字のあとに名詞が続く文字は、1 の形を持つ。−1 は one semitone / one bar で、+1 と同じ。

## How to trigger

実行せず、コードを読んで確かめた。英語のページで、ノートを一行下へドラッグする (または、ドラッグの読み値が出ている状態で下のキーを押す)。またはクリップを一小節左へドラッグする。`signed(-1)` は、U+2212 の「−1」である。`Number("−1")` は NaN なので、`english()` は `other` を選ぶ。`Number("+1")` は 1 なので、もう一方の向きは正しい。

## Cause

`english()` は、穴の値を JavaScript の数として読む。画面は符号つきの値を、タイポグラフィーのマイナス U+2212 で書く (仕様の書き方)。それを `Number` は読めない。また、ふつうの「-1」だと -1 になり、`=== 1` にはならない。

## Suggested fix

`english()` で、値の U+2212 を「-」に置き換えてから読み、その絶対値を 1 と比べる (`Math.abs(Number(String(v).replace("−", "-"))) === 1`)。ui/src/text/index.ts の一行で済む。文言の変更はなし。

## Verification

テストを走らせず、読んで確かめた。JavaScript の事実は明らかである: U+2212 の `Number("−1")` は `NaN` になり、`Number("+1")` は 1 になる。`english()` (`ui/src/text/index.ts:53-56`) は `Number(v) === 1` のときだけ `one` を選ぶ。`signed` (`pianoroll/geom.ts:441`) は U+2212 で「−1」を書く。`pr.readMove` / `arrange.readMove` は `n("keys", …)` / `n("delta", …)` の組である (`en.ts:914`、721)。そのため、「−1 semitones」/「(−1 bars)」と、「+1 semitone」/「(+1 bar)」が並ぶ。

修正案 (提案どおり): `english()` で、U+2212 を「-」に置き換えた値を読み、その絶対値を 1 と比べる。`barsDeltaText` (`arrangeDrag.ts`) が、その後に `Number` で読める形を返すか確かめる (穴の中に空白や単位がないこと)。テストは `ui/tests/language.test.ts` に置く: `t("pr.readMove", { keys: "−1", … })` が "semitone" で終わること。

## Fix

`ui/src/text/index.ts` (`english()`): 値は、「−」(U+2212) を "-" に置き換えてから読み、その絶対値を 1 と比べる。そのため、「−1」は「+1」と同じく 1 の形をとる。`barsDeltaText` は「−1」/「−1.5」を返す (穴の中に単位はない)。置き換えのあとに読める。
テスト: `ui/tests/language.test.ts` の「English: a count of 1 takes the form for 1, also when signed with 「−」」(`pr.readMove` に「−1」、`arrange.readMove` に「−1」と「−1.5」)。
