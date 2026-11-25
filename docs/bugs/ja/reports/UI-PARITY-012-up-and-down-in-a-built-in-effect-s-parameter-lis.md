---
id: UI-PARITY-012
title: 組み込みエフェクトのパラメータの一覧で ↑ ↓ を押すと、見えていない行が選ばれることがある。プラグインの一覧は、その行までスクロールする
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-012: 組み込みエフェクトのパラメータの一覧で ↑ ↓ を押すと、見えていない行が選ばれることがある。プラグインの一覧は、その行までスクロールする

## Where

ui/src/components/params/BuiltinList.svelte:109-123 (`onkeydown`: ↑ ↓ は `pick(next)` を呼ぶだけ)
ui/src/components/params/BuiltinList.svelte:75-78 (`pick` は `preventScroll: true` で一覧にフォーカスし、`selectParam` を送る。スクロールはしない)
ui/src/components/params/BuiltinList.svelte:129 (行は `ScrollArea` の中にあるが、`viewport` を束ねていない)
正しく作られている兄弟: ui/src/components/params/PluginList.svelte:105-113 (`reveal`) と :167-171 (`pick(next); reveal(next);`)

## What happens

行の数が見える分より多い組み込みエフェクトのパラメータの欄では(4 バンドの EQ は、見出しを含めて 10 行ある。プリセットの一覧を開いているときや、窓が低いときは、見える行はさらに少ない)、↑ ↓ で選ばれている行(gray.4 の、アクセントの線)が、見えている部分の下や上を越えて動く。一覧はスクロールしないので、選ばれている行は見えず、使う人は Enter や ← → が何に効くのか分からない。プラグインのパラメータの一覧は、選ばれている行が見えるようにスクロールする。ノートの一覧、CC の一覧、履歴の帯も、行を見える所に保っている(NoteListPanel.svelte:126、CcListPanel.svelte:108、history/HistoryList.svelte:92)。

## What should happen

PluginList と同じにする。↑ ↓ のあと、選ばれている行を見える所までスクロールする。仕様 2「スクロールの棒」は、このようにスクロールする一覧の中にパラメータを挙げている。仕様 4 は、エフェクトが何であっても「パラメータの中」で同じ ↑ ↓ を定めている。

## How to trigger

コードを読んで確かめたもので、実行はしていない。

1. 窓を低くする(1024 × 640)。帯を一つ開いて右はしを短くするか、「プリセット ▾」を行の上に開く。
2. トラックに 4 バンドの EQ を置いて選ぶ。最初の行をクリックする。
3. ↓ を何回か押す。`sel.row` は見えている部分の下の行に動くが、ScrollArea の `scrollTop` は変わらない。

## Cause

BuiltinList には、PluginList が持つ `reveal` の手順がない(PluginList は ScrollArea の `viewport` を束ね、`scrollTop` を設定する。BuiltinList は束ねていない)。

## Suggested fix

BuiltinList で ScrollArea の `viewport` を束ね、キーで `pick(next)` のあとに、行 `[data-param="…"]` を見える所へ持ってくる(例: `el.scrollIntoView({ block: "nearest" })`。EndpointRow.svelte:32 がしているとおり。または PluginList のような `reveal`)。プロトコル、文言、見た目は変えない。

## Verification

読んで確かめた(実行はしていない)。`BuiltinList.svelte` の `onkeydown` は `pick(next)` で終わる。`pick` は `preventScroll: true` で一覧にフォーカスし、`selectParam` を送る。このファイルには `reveal` がない(検索結果)。一方 `PluginList.svelte:106` には `reveal` があり、`pick(next)` のあと(170 行目)に呼ぶ。そのため、↑ ↓ で選んだ行は、`ScrollArea` の見えている部分の外にあることがある。

修正計画(提案どおり): `ui/src/components/params/BuiltinList.svelte` で `ScrollArea` の viewport を束ね、キーで `pick(next)` のあとに、`PluginList` と同じ `reveal` で行を見える所へ持ってくる(小さな関数を一つ共有し、コピーを二つ作らない)。`SoundKnobList.svelte` は四行しかないので、何も要らないことを確かめる。

## Fix

`ui/src/components/params/BuiltinList.svelte`: ↑ ↓ で行を選んだあと、その行 `[data-param]` を `scrollIntoView({ block: "nearest" })` で見える所へ持ってくる(`EndpointRow.svelte` がしているとおり。PluginList の `reveal` は仮想の行を数えるので、共有はしない)。`SoundKnobList.svelte` は四行で、何も要らない。テスト: `ui/tests/params.test.ts` の「UI-PARITY-012: ↑ ↓ bring the chosen row into view」。
