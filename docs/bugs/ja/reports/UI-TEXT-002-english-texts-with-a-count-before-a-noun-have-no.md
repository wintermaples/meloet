---
id: UI-TEXT-002
title: 数の後ろに名詞が来る英語の文言に、1 のときの単数形がない（1 beats、1 steps、1 pts）
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-002: 数の後ろに名詞が来る英語の文言に、1 のときの単数形がない（1 beats、1 steps、1 pts）

## Where

ui/src/text/en.ts:34 (`msg.countingInRest`。:33 の `msg.countingIn` は今は使われていない)
ui/src/text/en.ts:1140 (`history.head`: "{total} steps" と "The {redo} above are undone")
ui/src/text/en.ts:1141 (`history.headLatest`)
ui/src/text/en.ts:1142 (`history.headAllUndone`)
ui/src/text/en.ts:1101 (`lane.now`: "{count} pts")
ui/src/text/en.ts:47 (`msg.scanDone`: "{stopped} stopped … and were left out")
呼び出し元: ui/src/components/StatusLine.svelte:451, ui/src/components/history/HistoryHead.svelte:14-16, ui/src/components/automation/LaneHead.svelte:119, ui/src/components/StatusLine.svelte:384

## What happens

英語では、これらの文言はただの文字列なので、数が 1 のとき英語が崩れます。「Recording starts in 1 beats. Space to stop」（カウントインの最後の拍のたびに出る）、「1 steps · now at step 1 (the newest)」（最初の変更の後の取り消しの帯の見出し）、「The 1 above are undone and can be redone」、「Now 50 % · 1 pts」（点が 1 つのレーン）、「1 stopped while being checked and were left out」（あるプラグインが止まったスキャン）。

## What should happen

仕様 7-readings、D4「数」: 「数の後ろに名詞が来る文言は、1 のときの形と、それ以外の数の形を持つ（en.ts の `n(by, one, other)`）」。en.ts の他の 60 の文言は、すでにこれをしています（たとえば `history.notes`、`msg.undoPending`、`transport.countInBars`）。

## How to trigger

コードを読んで確認したもので、実行はしていません。英語の画面で、カウントインつきで録音を始める（`countInBeatsLeft` が 1 まで下がる。`crates/meloet-engine/tests/tempo_map.rs:847`）。または、変更を 1 つしてから「取り消しの履歴」を開く。または、レーンにオートメーションの点を 1 つ加える。または、1 つのプラグインが止まるスキャンをする。

## Cause

これらのキーは、en.ts では `n(...)` の組ではなく、ただの文字列として書かれていました。ui/tests/text.test.ts のテストは、穴があるかは調べますが、単数形は調べません。

## Suggested fix

en.ts で、それぞれを `n(by, one, other)` の組にする: `msg.countingIn` / `msg.countingInRest` は `beats` で（"1 beat"）、`history.head` / `headLatest` / `headAllUndone` は `total` で（"1 step"。`history.head` は `redo` の部分も 1 のときに読める必要がある。たとえば "The {redo} above is undone and can be redone"。これには `redo` での組か、どちらの数でも読める言い換えが必要）、`lane.now` は `count` で（"1 pt"）、`msg.scanDone` は `stopped` で（"was left out"）。英語の文言だけで、日本語もプロトコルも変えません。`history.head` は 1 つの文言に数が 2 つあるので、リードは 1 組だけで済む言い換えを選んでもよいです。

## Verification

コードを読んで確認しました（実行はしていません）。挙げたキーは `ui/src/text/en.ts`（33-34、47、1101、1140-1142）の単なる文字列で、それぞれ数 1 になりえます（カウントインの最後の拍、最初の手、点が 1 つ、止まったプラグインが 1 つ）。7-readings D4: 「数 | 数の後ろに名詞が来る文言は、1 のときの形と、それ以外の数の形を持つ（en.ts の `n(by, one, other)`）。日本語は変えない。」 `n(...)` を使っている他の文言は 61 あります。

修正の計画（英語のみ。規則が求める形。日本語は変えない）: `ui/src/text/en.ts` で
- `msg.countingIn`、`msg.countingInRest`: `n("beats", "… in {beats} beat. …", "… in {beats} beats. …")`；
- `history.headLatest`、`history.headAllUndone`: `n("total", "{total} step · …", "{total} steps · …")`；
- `history.head` は数が 2 つあり、`n` は 1 つしか取れない。`total` で組にし、後半は数がどれでも読めるように言い換える。たとえば "… now at step {current}. Undone above: {redo}, can be redone"（リードかオーナーが別の言葉を選んでもよい。英語の表は、オーナーが承認した開発者の訳なので、修正の変更一覧でオーナーに伝える）；
- `lane.now`: `n("count", "Now {value} · {count} pt", "Now {value} · {count} pts")`；
- `msg.scanDone`: `n("stopped", "… {stopped} stopped while being checked and was left out. …", "… were left out. …")`。
`ui/tests/language.test.ts` に単数の場合のテストを加える。UI-TEXT-003 は先に、または一緒にやる（同じ `english()` を使う）。

## Fix

`ui/src/text/en.ts`: `msg.countingIn`、`msg.countingInRest`（`beats` で）、`history.headLatest`、`history.headAllUndone`（`total` で）、`history.head`（`redo` で。`total` はそこでは少なくとも 2 なので、1 のときに必要だったのは "The 1 above is undone" だけ。言い換えはしていない）、`lane.now`（`count` で。"1 pt"）、`msg.scanDone`（`stopped` で。"was left out"）を、`n(by, one, other)` の組にしました。日本語は変えていません。オーナーに伝える英語の文言の変更: "1 beat"、"1 step"、"The 1 above is undone and can be redone"、"1 pt"、"1 stopped … and was left out"。
テスト: `ui/tests/language.test.ts` の「英語: 数が 1 のときは 1 のときの形を取る（「−」が付いても同じ）」。
