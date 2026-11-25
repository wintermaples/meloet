---
id: UI-PARITY-003
title: メニューバーの「編集 ▸ 消す」は、最後に選んだマーカー、テンポの変更、オートメーションの点を無視する。表示されている Delete キーとは違う
area: UI-PARITY
kind: parity
severity: medium
status: fixed
found: 2026-11-25
---

# UI-PARITY-003: メニューバーの「編集 ▸ 消す」は、最後に選んだマーカー、テンポの変更、オートメーションの点を無視する。表示されている Delete キーとは違う

## Where

ui/src/components/menu/menus.ts:145 (「消す」の行: `act("menu.delete", { kind: "deleteSelected" }, t("key.delete"), toArrange ? { usable: state.arrange.selectedClips.length > 0 && !busy } : {})`)
ui/src/keys/attach.ts:205-206 (`deleteSelected` → `{ type: "deleteSelected" }`。クリップだけ)
ui/src/keys/attach.ts:476-501 (Delete キーは、まず `laneDeleteCommand` と `timelineKeyCommand` を通る。メニューの行の `runKeyAction` は、それを一度も通らない)
ui/src/components/automation/laneKeys.ts:15-18, ui/src/components/timeline/keys.ts:62-66 (`lastPick` が "lane" / "timeline" のとき、キーが何をするか)
crates/meloet-core/src/play.rs:1132-1139 (`delete_selected` は、選ばれたクリップだけを消す)

## What happens

キーがアレンジに向いていて、最後に押したものがマーカー、テンポの点、拍子の旗、またはオートメーションの点 (`lastPick` が "timeline" か "lane") のとき:
- Delete キーは、そのマーカー、点、旗、またはその点たちを消す (`deleteTimeline`、`deleteAutoPoints`)。
- 「編集 ▸ 消す」は、右に「Delete」というキーを表示しているが、クリップが選ばれていないと使えない。マーカーや一覧を押してもクリップの選択は消えないので、クリップも選ばれたままのときは、この行は使え、マーカーや点ではなく選ばれたクリップを消す。

## What should happen

ui/src/components/menu/menus.ts:1 と attach.ts:2: 「キーのある行は、キーと同じように動く」。7-readings A-21: 「Delete は、最後に押して選んだものに働く (クリップ、テンポの点と旗、マーカー、オートメーションの点)」。メニューの行は、Delete キーが行うときにちょうど使え、ちょうど動くべきである。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。アレンジでクリップを選ぶ。次に、マーカーの行でマーカーを押す (それが選ばれ、クリップの選択はそのまま残る)。「編集」を開いて「消す」を押す: クリップが消え、マーカーは残る。代わりに Delete を押すと、マーカーが消える。クリップが選ばれていないと、Delete は働くのに「消す」の行は使えない。

## Cause

キーの経路は、`keyAction` より前に `laneDeleteCommand` と `timelineKeyCommand` (どちらも `lastPick` を見る) を呼ぶ。メニューの行は `runKeyAction({ kind: "deleteSelected" })` を直接呼ぶ。その `usable` は `state.arrange.selectedClips` だけを見る。

## Suggested fix

メニューの行に、キーと同じ選び方を与える。キーがアレンジに向いているときは、`laneDeleteCommand` / `timelineKeyCommand` から命令を組み立てる (普通の Delete の入力と、同じ文脈を使う)。そのあと `deleteSelected` に戻る。`usable` は、そのどれが当てはまるかに基づかせる (曲の先頭でない、選ばれたタイムラインの項目、選ばれたオートメーションの点、選ばれたクリップ)。attach.ts の `keyCommand` / `keyActionUsable` に置くのが一番よい。そうすれば、メニューとキーが同じものを使う。プロトコルや文言は変えない。

## Verification

実行して確かめたのではなく、コードを読んで確かめた。Delete キーは、まず `laneDeleteCommand` と `timelineKeyCommand` を通る (`keys/attach.ts:476-501`。どちらも最後に押したものを見る)。それから `keyAction` へ行く。メニューの行 (`menus.ts:145`) は `runKeyAction({ kind: "deleteSelected" })` を呼ぶ。これは `{ type: "deleteSelected" }` である (`attach.ts:205-206`。クリップだけ。`crates/meloet-core/src/play.rs` の `delete_selected`)。その `usable` は `selectedClips` だけを見る。7-readings A-21: 「Delete | 最後に押して選んだものに働く (クリップ、テンポの点と旗、マーカー、オートメーションの点)」。`menus.ts` / `attach.ts` のヘッダーの規則: キーのある行は、キーと同じように動く。重大度は medium で正しい。クリップが選ばれたままでマーカーを最後に押したとき、この行は、使う人が見ているマーカーではなく、クリップを消す (取り消し 1 回で戻せる)。

修正計画 (提案どおり): `ui/src/keys/attach.ts` で、「Delete が今何をするか」を決める 1 つの関数を作る (レーン → タイムライン → 従来の `deleteSelected` の経路)。これを `handleKeydown` と、メニューの行の `runKeyAction` / usable の判定の両方で使う。`ui/src/components/menu/menus.ts:145` は、`usable` をそこから取る。テスト `ui/tests/menu.test.ts`: マーカーを最後に押し、クリップも選ばれている → 行は `deleteTimeline` を送る。クリップが選ばれていない → 行は使える。

## Fix

`ui/src/keys/attach.ts`: `pickedDelete(state)` は、キーがアレンジに向いているとき、Delete キーと同じ規則 (`laneDeleteCommand`、次に `timelineKeyCommand`。普通の Delete を使う) を使う。`command()` は `deleteSelected` にこれを使う (そのためメニューの行は、キーと同じく `deleteAutoPoints` / `deleteTimeline` を送る)。`arrangeDeleteUsable(state)` は、行の `usable` を与える (点が選ばれている、曲の先頭でない項目、または選ばれたクリップがあり、使用中でない)。`ui/src/components/menu/menus.ts`: 「消す」は `usable` をそこから取る。テスト: `ui/tests/menu.test.ts` の「メニューバーの「消す」は、Delete と同じように動く (UI-PARITY-003)」 (3 つの場合)。
