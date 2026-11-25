---
id: UI-BRIDGE-002
title: IBM Plex Mono 600 is not loaded, so number fields at weight 600 get a made-up bold
area: UI-BRIDGE
kind: ui-ux
severity: low
status: confirmed
found: 2026-11-25
---

# UI-BRIDGE-002: IBM Plex Mono 600 is not loaded, so number fields at weight 600 get a made-up bold

## Where

ui/src/design/fonts.ts:6-7 (only `@fontsource/ibm-plex-mono/400.css` and `500.css` are loaded)
The parts that ask for Mono at 600:
ui/src/components/arrange/FadeBox.svelte:102-116 (`.fade-field`, the box of 「曲の終わりにフェードアウトをつける…」)
ui/src/components/Arrange.svelte:2344-2358 (`.repeat-field`, the repeat count box of a clip)
ui/src/components/timeline/MarkerRow.svelte:379-393 (`.space-field`, the box of 「この前に空きを入れる…」)

## What happens

The page declares `@font-face` rules for 'IBM Plex Mono' at 400 and 500 only. The three typed-number boxes above set `font-family: var(--font-mono)` with `font-weight: var(--text-s-weight-600)`. With no 600 face in the family, the browser takes the 500 face and, because 600 counts as bold, draws a synthesized ("faux") bold of it (CSS Fonts `font-synthesis-weight: auto`, the default). The digits in these boxes are thicker and smeared compared with the real Plex Mono glyphs used for the numbers at 400 and 500. Any other Mono text that inherits weight 600 from its box gets the same.

## What should happen

Spec 1 "Text": numbers are IBM Plex Mono, and 600 is an allowed weight of `text.s`. CLAUDE.md: the font is IBM Plex from the @fontsource woff2 files as they are. A weight the screen uses should have its real face loaded, as fonts.ts already does for IBM Plex Sans JP 400, 500 and 600.

## How to trigger

Read from the code, not run. Open the box of 「曲の終わりにフェードアウトをつける…」, the repeat count box of a clip, or the bars box of 「この前に空きを入れる…」 on a marker, and look at the digits typed in it next to other Mono numbers.

## Cause

fonts.ts loads `@fontsource/ibm-plex-mono` 400 and 500, while parts ask for Mono at 600 (`--text-s-weight-600`). `ui/node_modules/@fontsource/ibm-plex-mono/600.css` is there but not imported.

## Suggested fix

Add `import "@fontsource/ibm-plex-mono/600.css";` to ui/src/design/fonts.ts (same package and license, OFL-1.1; `node licenses/check.mjs` should stay green). The other choice, if the lead prefers fewer font files, is to draw those three boxes at 500; that is a look decision.

## Verification

Confirmed by reading (the look itself was not seen in the app). Facts checked:
- `ui/src/design/fonts.ts` imports `@fontsource/ibm-plex-mono/400.css` and `500.css` only; the package in `ui/node_modules/@fontsource/ibm-plex-mono` ships 100 to 700 (`600.css` is there).
- Three typed-number boxes ask for Mono at 600 (`FadeBox.svelte:112-114`, `Arrange.svelte:2356`, `MarkerRow.svelte:391`); they follow the track name box of spec 2 ("Track name input box | … (background gray.0, border gray.10, weight 600)") with digits in it.
- The weight is in the tokens: spec 1 line 142 "`text.s` | 12 / 15 | 16 / 20 | 400 / 500 / 600 | Body text, labels, buttons, numbers in tables", line 144 "`text.l` | … | 600 | Document headings, the count-in number", and line 138 "numbers, positions and values are IBM Plex Mono"; `ui/src/design/tokens.ts:59-61` has the same weights. So Mono at 600 is asked for by the spec itself (a number in `text.l`, and any `m-num` digits inside a 600 heading inherit 600 too); the three boxes do not use a weight the tokens lack. The fix is the report's first choice, not the other way round.
- That the browser then draws a synthesized bold of the 500 face is from the CSS Fonts rules (weight matching falls back to 500; `font-synthesis-weight: auto`) and from memory of WebKit's rule (a request of 600 or more with a face below it gets synthetic bold); not looked up in the installed WebKitGTK and not seen on the screen. Even without synthesis the face shown would be 500, not the 600 the tokens say.

Fix plan: add `import "@fontsource/ibm-plex-mono/600.css";` to `ui/src/design/fonts.ts` (files used as they are, OFL-1.1); run `node licenses/check.mjs` and check `ui/src/components/about/` lists nothing per weight; look at the three boxes and one 600 heading with digits in `tools/design/shoot_ui.py`.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
