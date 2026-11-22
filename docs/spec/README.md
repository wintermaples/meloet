# meloet design spec

This is the design spec of meloet. It is the source of truth: the code and the tests follow it.

The mocks in [`docs/design/mock/`](../design/mock/) are sample pictures of the screen. When a mock and this spec do not agree, this spec is right.

## Chapters

1. [1 List of values](1-tokens.md)
2. [2 Parts and states](2-parts.md)
3. [3 Rules of the bands](3-bands.md)
4. [4 List of keys](4-keys.md)
5. [5 Term tips](5-terms.md)
6. [6 Wording of errors and hints](6-messages.md)
7. [7 この版で変えた所](7-changes.md)

Other files:

- [`samples.css`](samples.css): the style of the original spec page. The font lists (`--sans`, `--mono`) and the sizes of the sample parts in chapter 2 are written here.
- [`glossary.md`](glossary.md): Japanese words of the spec and the screen, and the English names used for them.

Text that is shown on the screen, and the names of the values (`color.gray.4`, `space.3`, …), must stay exactly as written here. `ui/src/text/ja.ts` points to a chapter with `spec:<chapter number>`.

`tools/design/spec_check.py` checks these files.

## Preface

This document is for the developers who build 「meloet」, a DAW for beginners (its working name was 「計測」). The screen mocks are on the design canvas; this document collects the values, the parts, the rules and the wording. Every value is in px at scale 100 %.

Version 12 of the design spec (2026-11-22; after round 7, with the questions for round 8). The changes from version 11 are in chapter 7. The person who decides is the owner of meloet (in older versions, read 「ヤチヨさん」 as the owner). From 2026-11-23 the spec lives in this repository and changes here.
