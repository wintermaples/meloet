---
id: UI-SHELL-007
title: About's license search shows only the match count in headings, not matches / all
area: UI-SHELL
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-SHELL-007: About's license search shows only the match count in headings, not matches / all

## Where

ui/src/components/about/AboutDialog.svelte:137 (「音の素材」 heading count `sounds.length`)
ui/src/components/about/AboutDialog.svelte:162 (「書体」 heading count `fonts.length`)
ui/src/components/about/AboutDialog.svelte:175 (license headings `g.parts.length`)
ui/src/components/about/licenses.ts:97-107 (`partGroups` keeps only the matching parts, so the total of a heading is lost)

## What happens

On the second page of 「meloet について」, while something is typed in the search box, each heading shows only how many rows match (「▾ MIT 1」), the same as when nothing is typed.

## What should happen

docs/spec/3-bands.md:131: "While searching, only the headings with matches are shown open, and the count of a heading becomes 「合う数 / 全部」." The sibling search in the shelf does this already (ui/src/components/shelfList.ts:183, `shelf.matchCount` 「{shown} / {total}」 while searching), and spec 3:141 says the shelf works "the same as the second page of 「meloet について」".

## How to trigger

Read from the code, not run. Help ▸ 「meloet について」 ▸ 「オープンソースのライセンス ▸」, type 「tok」: the MIT heading reads 「MIT 1」 instead of 「MIT 1 / <all MIT parts>」. (ui/tests/about.test.ts:258 asserts the current form, "▾ MIT 1", so the test needs the same change.)

## Cause

The heading count is always the length of the filtered list; the unfiltered count is never passed to `sectionHead`.

## Suggested fix

While `searching`, pass `t("shelf.matchCount", { shown, total })` as the count, with the total from `list.sounds.length`, `list.fonts.length` and the per-license count before filtering (for example `partGroups(parts, "")`). The wording 「{shown} / {total}」 exists (mock:F5-ShelfSearch); if a key of its own under `about.` is wanted, that is new wording for the lead. No protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
