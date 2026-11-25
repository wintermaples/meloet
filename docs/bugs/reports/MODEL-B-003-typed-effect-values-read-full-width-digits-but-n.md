---
id: MODEL-B-003
title: Typed effect values read full-width digits but not full-width unit letters, so 500ms in full width becomes 500 s
area: MODEL-B
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-B-003: Typed effect values read full-width digits but not full-width unit letters, so 500ms in full width becomes 500 s

## Where

crates/meloet-model/src/effects.rs:122-130 (`ParamSpec::read`: only `０`-`９`, `−`, `－`, `＋`, `．` are turned into ASCII)
crates/meloet-model/src/effects.rs:146-152 (the unit is matched against ASCII `ms`, `s`, `k` only)
ui/src/components/params/ParamRow.svelte:174 (the screen sends the typed text as it is)

## What happens

Typing into a built-in effect's value box with the Japanese input method in full-width mode gives full-width letters for the unit. 「５００ｍｓ」 in the reverb's 「響きの長さ」 (seconds) reads as 500 (the unit 「ｍｓ」 is not seen), is clamped to 10 s, and the status line says the value was put at the end of its range. 「１.５ｓ」 in the compressor's attack (ms) becomes 1.5 ms instead of 1500 ms, and 「２ｋＨｚ」 in an EQ frequency becomes 2 Hz (clamped to the lowest). The same full-width text without the unit, or with an ASCII unit, reads right.

## What should happen

The unit is followed whatever width it is typed in: docs/architecture.md 20-13 ("Typing for built-in effects ... converting 「500 ms」, 「1.5 s」, 「2 kHz」"), and `read` already takes full-width digits, signs and the full-width point (its own comment: "１２０"). The screen's other typed fields normalize the whole text with NFKC first (ui/src/components/valueClamp.ts:23, ui/src/components/time.ts:64, ui/src/components/params/clipFields.ts:23), so full-width letters work there.

## How to trigger

Read from the code, not run.
1. Put a reverb on a track, open the params panel, double-click 「響きの長さ」.
2. With the input method on (full-width alphanumerics), type 500ms and press Enter: the box holds 「５００ｍｓ」, sent as `TypeParam { text }`.
3. `Play::type_param` (crates/meloet-core/src/play/mixer.rs:498) calls `spec.read`: the number is 500, `unit` is 「ｍｓ」 (lower case of full-width is still full-width), `starts_with("ms")` is false, scale 1. The value becomes 10.0 s (clamped) with `ParamClamped`.

## Cause

The character map in `read` covers digits and a few signs but not the full-width Latin letters U+FF21..U+FF3A / U+FF41..U+FF5A, so `unit.starts_with("ms")`, `starts_with('s')` and `starts_with('k')` never match a full-width unit.

## Suggested fix

In the character map of `ParamSpec::read`, also turn full-width Latin letters (`'Ａ'..='Ｚ'`, `'ａ'..='ｚ'`) into their ASCII letters (subtract 0xFEE0), or have the screen send `typed.normalize("NFKC")` as its sibling fields do (that would also cover plugin values typed in the same box). No protocol or song-file change; a case like 「５００ｍｓ」 in `a_typed_value_reads_with_or_without_its_unit` would watch it.

## Verification

Confirmed by reading (no test run). `ParamSpec::read` maps only full-width digits, `−`, `－`, `＋` and `．` to ASCII; the unit is compared with ASCII `ms`, `s`, `k` after `to_lowercase()`, which leaves full-width letters full-width. `ParamRow.svelte:174` sends the typed text as it is. So 「５００ｍｓ」 for a parameter in seconds reads as 500. Severity `low` stands (needs the input method in full-width alphanumerics and a typed unit of another scale).

Corrections: none.

Fix plan: `crates/meloet-model/src/effects.rs` `ParamSpec::read`: add `'Ａ'..='Ｚ' | 'ａ'..='ｚ'` to the character map (to the ASCII letter, code − 0xFEE0), and the full-width space `'\u{3000}'` to a plain space while there. Rust side only, so the AI link's typed values get it too. Cases 「５００ｍｓ」, 「１．５ｓ」, 「２ｋＨｚ」 in the test `a_typed_value_reads_with_or_without_its_unit`.

## Fix

`crates/meloet-model/src/effects.rs` `ParamSpec::read`: full-width Latin letters (`Ａ`-`Ｚ`, `ａ`-`ｚ`) become their ASCII letters before the unit is read (Rust side, so the AI link's typed values get it too). The full-width space of the plan needed no line: `trim` and `trim_start` already take it as a space (the case 「５００　ＭＳ」 shows it). Test: cases 「５００ｍｓ」, 「５００　ＭＳ」, 「１．５ｓ」, 「２ｋＨｚ」 added to `effects::tests::a_typed_value_reads_with_or_without_its_unit`.
