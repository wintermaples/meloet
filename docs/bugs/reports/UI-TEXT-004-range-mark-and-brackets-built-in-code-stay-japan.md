---
id: UI-TEXT-004
title: Range mark and brackets built in code stay Japanese in English status lines
area: UI-TEXT
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-TEXT-004: Range mark and brackets built in code stay Japanese in English status lines

## Where

ui/src/components/format.ts:153-155 (`rangeText`: `${low}〜${high}`; used by pianoroll/noteFields.ts:96,237,258,285 and pianoroll/ccFields.ts:144,168)
ui/src/time/bars.ts:150 (`tickRangeText`: `0〜…`; used by valueClamp.ts:115, noteFields.ts:97,108)
ui/src/components/valueClamp.ts:79 (`1〜${max}`)
ui/src/components/valueClamp.ts:113 (`1〜${num}`)
ui/src/components/StatusLine.svelte:162 (`${min}〜${max}` for `params.valueClamped`)
ui/src/components/StatusLine.svelte:250 and :256 (`${track}（${short}）`, full-width brackets)
crates/meloet-core/src/play/params.rs:576 (`format!("{}〜{}", …)`: the range of a plugin parameter, made in Rust and shown as is by StatusLine.svelte:158)

## What happens

With the page in English the status line mixes Japanese marks into English sentences: "Beat is 1〜4, so it is now 4.", "Bars is 1〜64 …", "Tick is 0〜1919 …", "Ratio is 1〜20, so it is now 20.0 : 1.", "Mix is 0 %〜100 % …" for a plugin parameter, note-list and CC fields the same; and while recording two or more vocal tracks "● Recording · 6 Vocal（In 1）, 7 Vocal 2（In 1 · 2） · …". The ranges that come from en.ts are written "1–127", "30.00–300.00" (`value.velocityRange`, `value.tempoRange`), so the same sentence form shows two different range marks.

## What should happen

All screen wording, and the marks that join it, follow the language of the page (CLAUDE.md: wording lives in ui/src/text/; architecture 25-36: joints are live bindings of index.ts that change with the language; spec 7-readings D4 "Joints"). English ranges use "–" as en.ts does; brackets are "(…)" as in en.ts (`msg.recordingVocal` "{track} ({input})").

## How to trigger

Read from the code, not run. English page: type 9 into the beat part of the position box in 4/4 (or a tick over 1919, or 99 bars into a 「くり返す…」 box, or a too-large value into a built-in effect's row); or record two vocal tracks at once.

## Cause

These strings are put together in components with literal 「〜」 and 「（」「）」 instead of a text key or an index.ts joint, so they never change with `useLanguage`.

## Suggested fix

Add a range joint and a bracket form to the text tables (for example `common.range` 「{low}〜{high}」 / "{low}–{high}" and `common.inBrackets` 「{text}（{inner}）」 / "{text} ({inner})", or joints in index.ts next to `LIST_COMMA`) and use them at the listed places. For the plugin range, Rust should send the two ends (it has them as text already) and let the screen join them, which is a protocol change of `ParamEnd`; or the screen replaces the joint, which is fragile. Japanese output stays exactly the same. Needs two new English texts (the forms already used in en.ts); no protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
