---
id: UI-SHELL-006
title: ミキサーの帯札はいつも 0.0 dB を示し、マスターのフェーダーの値を示さない
area: UI-SHELL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-SHELL-006: ミキサーの帯札はいつも 0.0 dB を示し、マスターのフェーダーの値を示さない

## Where

ui/src/components/BandRow.svelte:35-43 (`count: t("band.masterDb", { db: t("common.db", { value: dbNumber(0) }) })`)

## What happens

帯札の行の帯札「ミキサー」は、マスターのフェーダーがどこにあっても (たとえば −6.0 dB、または −∞) いつも「マスター 0.0 dB」と出す。コメントには「今は 0.0 dB のまま」とあるが、これはマスターの欄にフェーダーがある前は正しかった。`SongView.master.gainDb` (ui/src/bridge/generated/MasterView.ts) が今は値を持っている。

## What should happen

docs/spec/3-bands.md:11 (下の帯の表): 「ミキサー | 帯札: マスターのフェーダーの値 (ラウドネスではない。新しい曲は 0.0 dB)」。仕様書 2「帯札」は「▸ ミキサー −6.2 dB M」を示す。docs/progress.md の「ミキサーの帯の帯札は、マスターの手の値を示す…」も、手の値を示すことを前提にしている。

## How to trigger

コードを読んで分かったこと。実行はしていない。ミキサーを開き (M)、マスターのフェーダーを −6 dB に動かして、ミキサーを畳む。帯札はまだ「マスター 0.0 dB」と言う。

## Cause

値は定数 `dbNumber(0)` で、BandRow.svelte では `app.song.master.gainDb` を一度も読まない。

## Suggested fix

`app.song.master.gainDb` から、マスターの欄が使うのと同じ数の形で埋める (ui/src/components/mixer/scale.ts の `faderText`、または null のときは −∞ の文字を付けた `dbNumber`)。プロトコルは変えない。「マスター {db}」の文言は残す。

## Verification

コードを読んで確認した (実行はしていない)。`BandRow.svelte:39` は定数 `dbNumber(0)` で帯札を埋める。`band.masterDb` はほかでは使われていない (grep で確認)。`SongView.master.gainDb` (`number | null`) がフェーダーの値を持つ。仕様書 3 の 11 行目: 「ミキサー | 帯札: マスターのフェーダーの値 (ラウドネスではない。新しい曲は 0.0 dB)」。`docs/progress.md` の注記 (「ミキサーの帯の帯札は、レーンが動かしている間も、マスターの手の値を示す」) は、どの値を示すかを言っているもので、定数のことではない。

修正案 (提案どおり): `ui/src/components/BandRow.svelte` で `app.song.master.gainDb` から埋める。マスターの欄が示す形 (`ui/src/components/mixer/scale.ts` の `faderText`。`null` には −∞ の文字がある) を使う。「stays at 0.0 dB for now」のコメントを消す。`ui/tests/screen.test.ts:90` は、帯札の幅を「マスター 0.0 dB」で測っている。一番長い値 (「−60.0 dB」、−∞) でも帯札に場所が残るか確かめる。

## Fix

`ui/src/components/BandRow.svelte`: ミキサーの帯札は、`app.song.master.gainDb` から「マスター {db}」を埋める (`common.db` と `dbNumber`。`null` のときは `common.dbSilent`。トラックの見出しと同じ)。帯札の幅は、表示された文字から `bandCountsShown` が測るので、長い値は既存の規則で数を減らす。直す固定の幅はない。

テスト: `ui/tests/followup.test.ts` 「ミキサーの帯札はマスターのフェーダーの値を示す (仕様書 3)、一番下まで下げたとき −∞」。
