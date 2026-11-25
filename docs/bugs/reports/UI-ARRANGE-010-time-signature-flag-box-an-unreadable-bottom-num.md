---
id: UI-ARRANGE-010
title: Time signature flag box: an unreadable bottom number is reported as the top number
area: UI-ARRANGE
kind: parity
severity: low
status: fixed
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

Confirmed by reading (not run). `TempoBand.svelte:302` sends a fixed `field: "meterNum"` and ignores the draft; `ValueInput.svelte:39` does pass the typed text (`reject(draft)`); the transport's box uses `meterFieldOf(draft)` (`Transport.svelte:303-306`, `time.ts:88-92`: bottom when the top reads as 1 to 12). Both boxes read with the same `readMeter`.

Also reported as UI-PARITY-004 (the same line, the same fix; that report was merged into this one and deleted). From it: the header of `timeline/ValueInput.svelte` says "The look and rules are the same as the transport's number box (NumberField.svelte)", and spec 6 "Not readable as a number" names the field that could not be read and its old value.

Fix plan (as suggested): in `ui/src/components/timeline/TempoBand.svelte:302` use the draft, `reject={(draft) => { const field = meterFieldOf(draft); send({ type: "rejectValue", field, kept: String(field === "meterNum" ? editFlag.num : editFlag.den), range: null }); }}`; add a case to `ui/tests/round6.timeline.test.ts` (「4/x」 on a flag → `field: "meterDen"`). No protocol or wording change.

## Fix

Not committed yet (the lead commits).

- `ui/src/components/timeline/TempoBand.svelte`: the flag box's `reject` uses the typed text: `meterFieldOf(draft)` picks the half, and that half's old number is kept (as the transport's box). Also covers the old UI-PARITY-004.

Test: `ui/tests/round6.timeline.test.ts` — "an unreadable lower number typed on a flag is told as the lower number, as the transport's box does (spec 6)".
