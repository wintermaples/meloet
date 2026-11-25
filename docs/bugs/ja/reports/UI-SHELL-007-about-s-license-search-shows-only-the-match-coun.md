---
id: UI-SHELL-007
title: 「meloet について」のライセンスの検索では、見出しに合う数しか出ず、合う数 / 全部にならない
area: UI-SHELL
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-SHELL-007: 「meloet について」のライセンスの検索では、見出しに合う数しか出ず、合う数 / 全部にならない

## Where

ui/src/components/about/AboutDialog.svelte:137（「音の素材」の見出しの数 `sounds.length`）
ui/src/components/about/AboutDialog.svelte:162（「書体」の見出しの数 `fonts.length`）
ui/src/components/about/AboutDialog.svelte:175（ライセンスの見出し `g.parts.length`）
ui/src/components/about/licenses.ts:97-107（`partGroups` は合う部品だけを残すので、見出しの全部の数が失われる）

## What happens

「meloet について」の二つ目のページで検索箱に入力している間、各見出しには合う行の数だけが出ます（「▾ MIT 1」）。何も入力していないときと同じです。

## What should happen

docs/spec/3-bands.md:131: 「検索中は、合うものがある見出しだけを開いて示し、見出しの数は「合う数 / 全部」になる。」棚の中の隣の検索は、すでにこれを行っています（ui/src/components/shelfList.ts:183、検索中の `shelf.matchCount` 「{shown} / {total}」）。仕様 3:141 も、棚は「『meloet について』の二つ目のページと同じ」に働くと書いています。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。ヘルプ ▸「meloet について」▸「オープンソースのライセンス ▸」で「tok」と入力すると、MIT の見出しには「MIT 1」と出ます。正しくは「MIT 1 / <MIT の部品すべて>」です。（ui/tests/about.test.ts:258 は今の形「▾ MIT 1」を確認しているので、テストも同じように変える必要があります。）

## Cause

見出しの数は常に、絞り込んだ後の一覧の長さです。絞り込む前の全部の数は `sectionHead` に渡されていません。

## Suggested fix

`searching` の間は、見出しの数として `t("shelf.matchCount", { shown, total })` を渡します。全部の数は、絞り込む前の `list.sounds.length`、`list.fonts.length`、およびライセンスごとの数から取ります（例: `partGroups(parts, "")` を空の検索で呼ぶ）。文面「{shown} / {total}」は既にあります（モック F5-ShelfSearch）。`about.` の下に専用の鍵が欲しい場合は、それは新しい文面なので、リードに任せます。プロトコルの変更はありません。

## Verification

コードを読んで確認しました（実行していません）。仕様 3 の 131 行目、そのままの文言: 「検索中は、合うものがある見出しだけを開いて示し、見出しの数は「合う数 / 全部」になる。」`AboutDialog.svelte:137, 162, 175` は、常に絞り込んだ後の一覧の長さを出します。`ui/tests/about.test.ts:258`（検索中は `["▾ MIT 1"]`）は今の形を固定していますが、仕様にも 7-readings にも、その形が意図されているとは書かれていません（7-readings には About の数についての行がありません）。そのため仕様が正しく、テストは修正に合わせて変えます。

文面: 「{shown} / {total}」の形は `shelf.matchCount` として既にあります（`ui/src/text/ja.ts:636`、出典 mock:F5-ShelfSearch。`en.ts` にもあります）。仕様 3 の 141 行目は、棚が「『meloet について』の二つ目のページと同じ」に働くと言うので、同じ鍵を使います。新しい文面はありません。

修正の計画（提案どおり）: `ui/src/components/about/AboutDialog.svelte` で、検索中は見出しの数として `t("shelf.matchCount", { shown, total })` を渡します。全部の数は、絞り込む前の一覧から取ります（`ui/src/components/about/licenses.ts`: `partGroups` にライセンスごとの全部の数を返させる、または空の検索で一度呼ぶ）。`ui/tests/about.test.ts:258` を新しい形に変えます。

## Fix

`ui/src/components/about/AboutDialog.svelte`: 検索中は、すべての見出しの数が `shelf.matchCount`「{shown} / {total}」になります。全部の数は、絞り込む前の一覧から取ります（`list.sounds`、`list.fonts`、ライセンスごとの `partGroups(parts, "")`）。検索していないときは、これまでどおり普通の数です。

テスト: `ui/tests/about.test.ts` の「the search box filters. Headings with a match open and count 「合う数 / 全部」 (spec 3)」（「▾ MIT 1」から「▾ MIT 1 / 1」に変更。「▾ 書体 1 / 2」を追加）。
