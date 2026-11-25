---
id: UI-ARRANGE-010
title: Time signature flag box: an unreadable bottom number is reported as the top number
area: UI-ARRANGE
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-010: Time signature flag box: an unreadable bottom number is reported as the top number

## Where

ui/src/components/timeline/TempoBand.svelte:302 (`reject={() => send({ type: "rejectValue", field: "meterNum", kept: String(editFlag.num), range: null })}`)

## What happens

In the open 「テンポ ・ 拍子の変更」 band, a time signature flag's box (double press on a flag, or 「拍子を打ちこむ…」) sends `rejectValue` with `field: "meterNum"` and the top number for every text it cannot read. When the top number is fine and the bottom is not (「4/x」, 「4/」), the status line says 「拍子の上は数字で打ちます。もとの 4 のままです。」, blaming the top number. `ValueInput` passes the typed text to `reject`, but the band ignores it.

## What should happen

The transport's time signature box tells which half is wrong: `rejectMeter(draft)` in `ui/src/components/Transport.svelte:303-306` uses `meterFieldOf(draft)` (bottom when the top is 1 to 12) and keeps that half's number. The flag box should say the same for the same text (spec 6 "Not readable as a number": 「{field}は数字で打ちます。もとの {kept} のままです。」).

## How to trigger

Read from the code, not run. Open the band, double press the 4/4 flag, type 「4/x」, press Enter: the status line names 「拍子の上」. The same text in the transport's time signature box names 「拍子の下」.

## Cause

The `reject` callback is written with a fixed `field: "meterNum"` instead of choosing the field from the typed text.

## Suggested fix

Use the typed text as the transport does: `reject={(draft) => { const field = meterFieldOf(draft); send({ type: "rejectValue", field, kept: String(field === "meterNum" ? editFlag.num : editFlag.den), range: null }); }}` (`meterFieldOf` is in `components/time.ts`). No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
