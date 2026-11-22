# meloet デザイン仕様書

This is the design spec of meloet. It is the source of truth: the code and the tests follow it.

The mocks in [`docs/design/mock/`](../design/mock/) are sample pictures of the screen. When a mock and this spec do not agree, this spec is right.

## Chapters

1. [1 数値の一覧](1-tokens.md)
2. [2 部品と状態](2-parts.md)
3. [3 帯の決まり](3-bands.md)
4. [4 キーの一覧](4-keys.md)
5. [5 用語の説明](5-terms.md)
6. [6 エラーと案内の文面](6-messages.md)
7. [7 この版で変えた所](7-changes.md)

Other files:

- [`samples.css`](samples.css): the style of the original spec page. The font lists (`--sans`, `--mono`) and the sizes of the sample parts in chapter 2 are written here.
- [`glossary.md`](glossary.md): Japanese words of the spec and the screen, and the English names used for them.

Text that is shown on the screen, and the names of the values (`color.gray.4`, `space.3`, …), must stay exactly as written here. `ui/src/text/ja.ts` points to a chapter with `spec:<chapter number>`.

`tools/design/spec_check.py` checks these files.

## Preface

初心者向け DAW「meloet」（案の名前は「計測」）を作る係へわたす書類。画面のモックはデザインのキャンバスにあり、ここには数値、部品、決まり、文面をまとめる。数値はすべて拡大率 100 % のときの px。

第 12 版 ・ 2026-11-22（第 7 回のあと ・ 第 8 回の問い）。第 11 版からの差は 7 章にまとめた。決める人は meloet のオーナーさん（前の版の「ヤチヨさん」は「オーナーさん」と読みかえる）。
