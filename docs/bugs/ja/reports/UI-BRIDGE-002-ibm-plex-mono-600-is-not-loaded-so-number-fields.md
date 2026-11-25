---
id: UI-BRIDGE-002
title: IBM Plex Mono の 600 が読みこまれていないため、ウェイト 600 の数字の欄に作られた太字が付く
area: UI-BRIDGE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-BRIDGE-002: IBM Plex Mono の 600 が読みこまれていないため、ウェイト 600 の数字の欄に作られた太字が付く

## Where

ui/src/design/fonts.ts:6-7 (読みこんでいるのは `@fontsource/ibm-plex-mono/400.css` と `500.css` だけ)
600 の Mono を使っている部品:
ui/src/components/arrange/FadeBox.svelte:102-116 (`.fade-field`。「曲の終わりにフェードアウトをつける…」の箱)
ui/src/components/Arrange.svelte:2344-2358 (`.repeat-field`。クリップの繰り返しの回数の箱)
ui/src/components/timeline/MarkerRow.svelte:379-393 (`.space-field`。「この前に空きを入れる…」の箱)

## What happens

ページは 'IBM Plex Mono' の `@font-face` を 400 と 500 の二つだけ宣言しています。上の三つの数字を入れる箱は `font-family: var(--font-mono)` と `font-weight: var(--text-s-weight-600)` を設定しています。このファミリーには 600 の面がないため、ブラウザは 500 の面を使い、600 は太字扱いなので、作られた太字 (「偽の」太字) を描きます (CSS Fonts の `font-synthesis-weight: auto`、既定の設定)。これらの箱の数字は、400 と 500 の数字に使われている本物の Plex Mono の字形と比べて、太く、にじんで見えます。箱から 600 を受け継ぐほかの Mono の文字も同じになります。

## What should happen

仕様 1「文字」: 数字は IBM Plex Mono で、600 は `text.s` の許される太さです。CLAUDE.md: フォントは IBM Plex で、@fontsource の woff2 ファイルをそのまま使う。画面が使う太さには、その本物の面を読みこむべきです。fonts.ts が IBM Plex Sans JP の 400、500、600 でそうしているのと同じです。

## How to trigger

コードを読んで確かめたもので、実行はしていません。「曲の終わりにフェードアウトをつける…」の箱、クリップの繰り返しの回数の箱、またはマーカーの「この前に空きを入れる…」の小節の箱を開き、そこに打ちこんだ数字を、ほかの Mono の数字と比べて見る。

## Cause

fonts.ts は `@fontsource/ibm-plex-mono` の 400 と 500 を読みこみますが、部品は Mono の 600 (`--text-s-weight-600`) を求めています。`ui/node_modules/@fontsource/ibm-plex-mono/600.css` はありますが、取りこまれていません。

## Suggested fix

ui/src/design/fonts.ts に `import "@fontsource/ibm-plex-mono/600.css";` を加える (同じパッケージで、ライセンスも同じ OFL-1.1)。`node licenses/check.mjs` は通ったままのはずです。フォントの数を減らすことをリードが望むなら、別案として、その三つの箱を 500 で描く方法があります。これは見た目の決定です。

## Verification

コードを読んで確かめたもので、見た目は実際のアプリでは見ていません。確かめた事実は次のとおりです。
- `ui/src/design/fonts.ts` は `@fontsource/ibm-plex-mono/400.css` と `500.css` だけを取りこんでいます。`ui/node_modules/@fontsource/ibm-plex-mono` のパッケージには 100 から 700 まで入っています (`600.css` もあります)。
- 数字の入った箱のうち三つが、Mono の 600 を求めています (`FadeBox.svelte:112-114`、`Arrange.svelte:2356`、`MarkerRow.svelte:391`)。これらは仕様 2 の「トラック名の入力の箱」 (「… | (背景 gray.0、枠 gray.10、ウェイト 600)」) に続くもので、中に数字が入ります。
- ウェイトはトークンにあります。仕様 1 の 142行目: 「`text.s` | 12 / 15 | 16 / 20 | 400 / 500 / 600 | 本文、ラベル、ボタン、表の数字」、144行目: 「`text.l` | … | 600 | 文書の見出し、カウントインの数字」。138行目: 「数字、位置、値は IBM Plex Mono」。`ui/src/design/tokens.ts:59-61` にも同じウェイトがあります。そのため、Mono の 600 は仕様そのものが求めています (`text.l` の数字や、600 の見出しの中の `m-num` の数字も 600 を受け継ぎます)。三つの箱はトークンにないウェイトを使っていません。修正は、報告にある第一の方法であり、逆ではありません。
- 500 の面から作られた太字がブラウザで描かれることは、CSS Fonts の規則 (ウェイトの照合は 500 にフォールバックする。`font-synthesis-weight: auto`) と、WebKit の規則 (600 以上の要求で、その下の面しかないと合成の太字になる) の私の記憶によるものです。インストールされた WebKitGTK では調べておらず、画面でも見ていません。合成がなくても、表示される面は 600 ではなく 500 です。

修正計画: `ui/src/design/fonts.ts` に `import "@fontsource/ibm-plex-mono/600.css";` を加える (ファイルはそのまま使う。OFL-1.1)。`node licenses/check.mjs` を実行し、`ui/src/components/about/` がウェイトごとには何も並べていないことを確かめる。`tools/design/shoot_ui.py` で、三つの箱と、数字の入った 600 の見出しを一つ見る。

## Fix

`ui/src/design/fonts.ts`: `import "@fontsource/ibm-plex-mono/600.css";` (ファイルはそのまま。OFL-1.1)。`node licenses/check.mjs` は通ります。バージョン情報のダイアログはパッケージを並べ、ウェイトは並べません。画像では見ていません (フォントの読みこみなので、テストはありません)。
