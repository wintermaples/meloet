---
id: UI-TEXT-001
title: 英語の歌の録音の状態に、モニターが切れているのに「monitor on」と出る
area: UI-TEXT
kind: parity
severity: medium
status: fixed
found: 2026-11-25
---

# UI-TEXT-001: 英語の歌の録音の状態に、モニターが切れているのに「monitor on」と出る

## Where

ui/src/components/StatusLine.svelte:259
ui/src/components/StatusLine.svelte:264
ui/src/text/en.ts:1154 (`msg.recordingVocal`)
ui/src/text/en.ts:1567 (`msg.recordingVocals`)
ui/src/text/en.ts:1295 (`meter.monitorOn`)

## What happens

ページを英語にして、モニターを切ったままの歌のトラックを録音すると、状態行には今も「● Recording · 6 Vocal (In 1) · monitor on · Space to stop」と出る (2 本以上の歌のトラックでも同じ)。モニターが切れているのに、状態行はモニターが入っていると言う。日本語では、「モニター オン ・ 」の部分が意図どおり取り除かれる。

## What should happen

モニターが切れているとき、「monitor on · 」の部分は取り除かれるべきである。StatusLine.svelte:244 のコメント (モニターが切れているときは「モニター オン ・ 」を取り除く) のとおり。また、日本語のページのとおりでもある (仕様 6 「歌の録音」、「2 本以上の歌のトラックの録音」)。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。言語を英語にし、歌のトラックを追加し、その「モニター」を切り、録音を押す。`recordingVocalsParts(keys, vocals, monitor=false)` は `monitorOn = "Monitor on · "` を作り、`text.replace("Monitor on · ", "")` を "… · monitor on · …" に対して呼ぶ。一致せず、何も取り除かれない。

## Cause

コードは、取り除く部分を別のキーの文字で探している: `t("meter.monitorOn")` + `STATUS_JOIN`。日本語では、どちらも「モニター オン」と言うので問題ない。英語では、`meter.monitorOn` は "Monitor on" (M は大文字。ラベル) だが、文の中の文は "monitor on" (m は小文字。文の途中) である。`String.replace` は大文字小文字を区別する。

## Suggested fix

最も小さい方法: en.ts で、`msg.recordingVocal` と `msg.recordingVocals` の中の部分を、コードが取り除くものと一致させる。または (大文字小文字に縛られないほうがよいが): 別の方法で、モニターの部分がない文を作る。たとえば大文字小文字を区別しないで比べる、または、コードが、その前の穴と `STATUS_JOIN` の間の文字を、この用途だけに使う別のキー (`meter.monitorOnInline`: 「モニター オン」 / "monitor on") で取り除く。新しいキーには英語だけが要る (日本語は既存の「モニター オン」)。プロトコルは変えない。

## Verification

コードを読んで確かめた (実行はしていない)。`StatusLine.svelte:259-264` は、`t("meter.monitorOn") + STATUS_JOIN` を文から `String.replace` で取り除く。英語では、それは "Monitor on · " である (`en.ts:1295`)。一方 `msg.recordingVocal` / `msg.recordingVocals` は "monitor on · " を持つ (`en.ts:1154`、1567)。一致は大文字小文字を区別するので、何も取り除かれず、モニターが切れているのに状態行は "monitor on" と言う。日本語は両方に「モニター オン」を持つ。重大度 medium は正しい。使う人が聞こえているものについて、状態行が偽のことを言うからである。

修正計画 (新しい文言なし): `ui/src/components/StatusLine.svelte` で、部分を別のキーの文字で探さない。代わりに、文を自分の継ぎ目で分ける。穴 `{stopKey}` の前の最後の穴の後にある文字の部分は、「…・ モニター オン ・ 止めるには 」 / " · monitor on · " と、次の部分の始まりである。穴のない、2 つの `STATUS_JOIN` の間の部分を取り除く (モニターの部分は、両方の表で、この 2 つの文の中で唯一そのような部分である)。別の簡単な方法: 両方を大文字小文字を区別しないで比べる (`toLowerCase`、1 行)。ただし、同じ言葉を言う 2 つのキーに、まだ縛られたままである。`ui/tests/language.test.ts` にテストを追加する: 英語、モニターが切れている → "monitor on" がない。

## Fix

`ui/src/components/StatusLine.svelte`: モニターの部分を、`meter.monitorOn` の文字で探さなくなった。モニターが切れているとき、`withoutMonitorPiece` は、文の各文字の部分を継ぎ目の印 (`STATUS_JOIN` からその空白を除いたもの。日本語が「）・ 」と書くのと同じ) で分け、2 つの印の間にある部分を捨てる (両方の表で、どちらの側にも穴がない唯一の部分)。日本語の出力は変わらない。
テスト: `ui/tests/round7.vocal.test.ts` の「英語: モニターが切れているとき、「monitor on · 」も取り除かれ、入力は英語の括弧に入る」。
