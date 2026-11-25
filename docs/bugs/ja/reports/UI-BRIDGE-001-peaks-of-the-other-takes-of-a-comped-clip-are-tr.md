---
id: UI-BRIDGE-001
title: いいとこ取りのクリップの、ほかのテイクのピークが使われていないものとして扱われ、表示中に手放されることがある
area: UI-BRIDGE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-BRIDGE-001: いいとこ取りのクリップの、ほかのテイクのピークが使われていないものとして扱われ、表示中に手放されることがある

## Where

ui/src/bridge/peaks.ts:220-227 (`songAudioIds`: 表示中のテイクのファイル `c.audio.audio` だけ)
ui/src/bridge/index.ts:150 (`song` 部分が変わるたびに `peakStore.keepOnly(songAudioIds(...))`)
ui/src/bridge/peaks.ts:186-201 (`PeakStore.keepOnly`: 使われていないものは `dropped` へ移り、4 MB まで保つ。古いものから手放す)

## What happens

いいとこ取り (round 12 A8) のボーカルのクリップは、選んだテイクのそれぞれの断片を、そのテイク自身のオーディオファイルから再生する。アレンジもそれを描く (ui/src/components/arrange/wave.ts:82-99 の `waveParts` は `AudioClipView.takes` の `take.audio` を使う)。いいとこ取りの帯 (ui/src/components/comp/CompBand.svelte:65、`b.take.audio`) は、クリップのすべてのテイクを描く。これらのファイルは、表示中のテイクのものを除いて `songAudioIds` に入らない。そのため、`song` 部分が変わるたびに、そのピークは `ready` から `dropped` へ移される。まるでどのクリップも使っていないかのように。

`peek` は、それらを `dropped` の中に今も見つける。`WaveBook.sourceFor` は `peek` だけで読み、取り戻さない (revive しない)。そのため、それらは `dropped` のまま残り、古くなっていく。`dropped` の量が `KEEP_DROPPED_PEAK_BYTES` (4 MB) を超えると、古いものから手放され、手放されるのはまさに画面にあるいいとこ取りのテイクである。すると `WaveBook.trim` がそれらのエントリを落とし、いいとこ取りのクリップの波形の断片と、いいとこ取りの帯の行は、全体のレベルが再び取得されるまで波形なしで描かれる。次の曲の変更で、それらはまた `dropped` へ移される。

## What should happen

`songAudioIds` の説明は「曲のクリップが再生するすべてのオーディオファイル」である。いいとこ取りのクリップは、その断片のファイルを再生し、いいとこ取りの帯はクリップのすべてのテイクを表示する。これらのファイルは使用中であり、表示中のテイクのファイルと同じく `ready` に残るべきである。どのクリップも使わないファイルだけが `dropped` へ行くべきである (peaks.ts:91 の説明と、take.ts の `keepOnly` の規則「使われているテイクと、新しい 16 個」)。

## How to trigger

実行せず、コードを読んで確かめた。
1. 長いテイクを多数 (たとえば 8 テイク、`COMP_ROWS_MAX`、各 5 分、ステレオ、48 kHz) 録音したボーカルのクリップを作り、いいとこ取り (「いいとこ取り」) を作る。いいとこ取りの帯を開く。
2. アレンジを拡大・縮小して、すべてのテイクの細かいピークのレベルを取得させる (`waveLevel` は、一画素あたり約 340 フレームより小さいとレベル 0、約 1365 より小さいとレベル 1 を選ぶ)。レベル 0 は 48000 × 60 / 256 ピーク × 2 チャンネル × 4 バイト = 一分あたり 90 KB。したがって 40 分のテイクは、レベル 0 で 3.6 MB、レベル 1 で 0.9 MB。合わせて 4 MB を超える。ほかに実際に使われなくなったファイルがある前に。
3. 曲に何か変更を加える (クリップを動かす、音量を変える)。変更のたびに `keepOnly` が走る。`dropped` が 4 MB を超えると、古いいいとこ取りのテイクが手放され、その波形はいいとこ取りのクリップといいとこ取りの帯から、再び取得されるまで消える。

## Cause

`songAudioIds(song, recording)` は `c.audio.audio` だけを集める。`AudioClipView.takes[].audio` (いいとこ取りの断片といいとこ取りの帯が読む、クリップのすべてのテイク) は集めない。そのため `PeakStore.keepOnly` は、それらのファイルを使われていないものとみなす。

## Suggested fix

`songAudioIds` で、`c.audio.takes` の各エントリの `audio` も集める (表示中のテイクもその中に入る)。プロトコル、ファイルの形式、文言、見た目の変更はなし。ui/tests/bridge.peaks.test.ts に、`audio.takes` が別のファイルを挙げているクリップの場合を加えると、これを見張れる。

## Verification

テストを走らせず、読んで確かめた。`songAudioIds` (`bridge/peaks.ts:220-227`) は `c.audio.audio` だけを集める (そのコメントは「each clip's shown take」)。`waveParts` (`arrange/wave.ts:82-99`) は、いいとこ取りの各断片を `a.takes` の `take.audio` から描く。いいとこ取りの帯は、すべてのテイクを描く。`PeakStore.keepOnly` (186-201) は、リストにないファイルのすべてのレベルを `dropped` へ移し、`dropped` が 4 MB (`KEEP_DROPPED_PEAK_BYTES`) を超えると古いものから手放す。そのため、`song` 部分が変わるたびに、いいとこ取りのクリップのほかのテイクのファイルは使われていないものとして扱われる。例のバイト数 (レベルの一分あたりの量) は計算し直していない。不具合はその数字に依存せず、いつ現れるかだけが数字で変わる。重大度 low は正しい。取得し直すと波形は戻る。

修正案 (提案どおり): `ui/src/bridge/peaks.ts` の `songAudioIds` で、`c.audio.takes` の各エントリの `audio` も集める (引数の型を広げ、説明のコメントを直す)。`ui/tests/bridge.peaks.test.ts` に一つ場合を加える。プロトコルは変えない。

## Fix

`ui/src/bridge/peaks.ts` の `songAudioIds` は、`c.audio.takes` の各エントリの `audio` も集める (引数の型を広げ、説明のコメントを直した)。`ui/src/bridge/index.ts` は変えていない (曲をすでに渡している)。
テスト: `ui/tests/bridge.peaks.test.ts` の「audio the song uses: every take of a comped clip, not only the shown one」。
