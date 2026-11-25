---
id: UI-TEXT-005
title: 英語の画面では、ピアノロールのドラムの行の名前から用語の説明が出ない
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-005: 英語の画面では、ピアノロールのドラムの行の名前から用語の説明が出ない

## Where

ui/src/components/pianoroll/drums.ts:111-115 (`DRUM_TERMS`、`drumTerm`)
ui/src/components/pianoroll/Roll.svelte:996 (`term: drumTerm(name)`)
ui/src/components/pianoroll/Roll.svelte:1181 (説明は `k.term` があるときだけ描かれる)

## What happens

ドラムのトラックでは、ピアノロールの行の名前（キック、スネア、クローズハイハット …）に、仕様 5 の用語の説明が日本語で付いています。英語では名前は「Kick」、「Snare」、「Closed hi-hat」…になり、`drumTerm` は、その中に日本語の言葉を見つけられず、null を返します。そのため、どのドラムの行にも説明がつきません。

## What should happen

仕様 7-readings、D4「用語の説明」: 「英語では、英語で示される名前（ドラムの名前など）も、その用語を見つける」。`termParts`（format.ts:79）は、英語の文で用語の英語の言葉をすでに見つけています。ただ、`drumTerm` が null を返すので、そこまで届いていないだけです。

## How to trigger

コードを読んで確認したもので、実行はしていません。英語の画面で、内蔵のキットか SF2 のキットがあるドラムのトラックでピアノロールを開き、「Kick」の上にマウスを置くと、説明が出ません。日本語では、「キック」の上に置くと「キック — いちばん低い、足で鳴らす大きな太鼓…」が出ます。

## Cause

`drumTerm(name)` は、表示される名前を日本語の言葉とだけ比べます（`name.includes(word)`）が、`drumName` は `t()` を通して名前を出すので、英語の画面では「Kick」などになり、何も一致しません。

## Suggested fix

ui/src/components/pianoroll/drums.ts の `drumTerm` で、英語の画面では、各日本語の言葉について `findTerm(word)?.term` と名前を比べる（大文字小文字を区別しない。`termParts` と同じ）。一致したら、日本語の言葉を返す（`use:term` が受け取るキー）。言葉の長いものを先に調べる。文言の変更もプロトコルの変更もありません。

## Verification

コードを読んで確認しました（実行はしていません）。`drumTerm`（`pianoroll/drums.ts:111-115`）は、表示される名前を日本語の言葉とだけ比べます。名前は `t()` を通るので、英語の画面では "Kick"、"Snare" などで、何も一致しません。そのため `Roll.svelte:996, 1181` は説明を描きません。7-readings D4: 「用語の説明 | 従来どおり日本語の言葉で探す。英語では、英語で示される名前（ドラムの名前など）も、その用語を見つける。」

修正の計画（提案どおり）: `ui/src/components/pianoroll/drums.ts` の `drumTerm` で、`textLanguage() === "en"` のとき、名前を `findTerm(word)?.term` と大文字小文字を区別せずに比べる（`components/format.ts:79-80` の `termParts` の規則）。日本語の言葉を返す（`use:term` が受け取るキー）。「長い言葉を先に」は従来どおり。8 個の言葉の英語の用語が、`en.ts` の英語のキットの名前と合うか確かめる（「Closed hi-hat」はハイハットを見つけ、「Side stick」はサイドスティックを見つける必要がある）。`ui/tests/language.test.ts` にテストを加える。

## Fix

文言の変更はありません。

- `ui/src/components/pianoroll/drums.ts`: `drumTerm` は、英語の画面では、用語の英語の言葉も名前の中から見つける（どの大文字小文字でも、単語として一致。`termParts` と同じ）。そして日本語の言葉を返す。言葉の長いものを先に調べるのは従来どおり。
- テスト: `ui/tests/language.test.ts` の「英語のキットの名前は、日本語の名前と同じ用語を見つける」。
