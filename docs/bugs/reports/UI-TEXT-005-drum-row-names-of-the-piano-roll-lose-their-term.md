---
id: UI-TEXT-005
title: Drum row names of the piano roll lose their term tips in English
area: UI-TEXT
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-TEXT-005: Drum row names of the piano roll lose their term tips in English

## Where

ui/src/components/pianoroll/drums.ts:111-115 (`DRUM_TERMS`, `drumTerm`)
ui/src/components/pianoroll/Roll.svelte:996 (`term: drumTerm(name)`)
ui/src/components/pianoroll/Roll.svelte:1181 (the tip is drawn only when `k.term` is set)

## What happens

On a drum track the row names of the piano roll (キック, スネア, クローズハイハット …) carry the term tips of spec 5 in Japanese. In English the names are "Kick", "Snare", "Closed hi-hat" …, `drumTerm` finds none of its Japanese words in them and returns null, so no drum row has a tip.

## What should happen

spec 7-readings, D4 "Term tips": "in English a name shown in English (drum names, …) finds its term too". `termParts` (format.ts:79) already finds the English word of a term in English text; it is only never reached here because `drumTerm` gives null.

## How to trigger

Read from the code, not run. English page, open the piano roll on a drum track with a built-in kit (or an SF2 kit), rest the mouse on "Kick": no tip. In Japanese, resting on 「キック」 shows 「キック — いちばん低い、足で鳴らす大きな太鼓…」.

## Cause

`drumTerm(name)` compares the shown name with the Japanese words only (`name.includes(word)`), but `drumName` gives the name through `t()`, which is English on an English page.

## Suggested fix

In `drumTerm`, also match the English word of each term when the page is English: for each Japanese word, test the shown name against `findTerm(word)?.term` (case-insensitive, as `termParts` does), and return the Japanese word (the key `use:term` takes). No wording change; no protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
