---
id: UI-PARITY-004
title: An unreadable lower number typed on a time signature flag is reported as the upper number, unlike the transport box
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-004: An unreadable lower number typed on a time signature flag is reported as the upper number, unlike the transport box

## Where

ui/src/components/timeline/TempoBand.svelte:302 (`reject={() => send({ type: "rejectValue", field: "meterNum", kept: String(editFlag.num), range: null })}`)
The sibling that does it right: ui/src/components/Transport.svelte:303-306 (`rejectMeter` uses `meterFieldOf(draft)` from ui/src/components/time.ts:88-92 and keeps the number of that field)

## What happens

Both boxes read the time signature with the same `readMeter`. When the text cannot be read and Enter is pressed:
- In the transport's 「拍子」 box, `meterFieldOf` tells which half is wrong: 「3/x」 is reported for 「拍子の下」 with the old lower number kept (spec 6 form 「…は数字で打ちます。もとの 4 のままです。」).
- In the box of a time signature flag (double press on a flag in the 「テンポ ・ 拍子の変更」 row, or 「拍子を打ちこむ…」), the report is always for 「拍子の上」 with the upper number, even when the upper number was fine and only the lower one is wrong.

## What should happen

The header of ui/src/components/timeline/ValueInput.svelte says "The look and rules are the same as the transport's number box (NumberField.svelte)". The flag box should name the same field and keep the same number as the transport box does (spec 6 "Not readable as a number": the field that could not be read and its old value).

## How to trigger

Read from the code, not run. Open the tempo / time signature row, double press the 4/4 flag, type 「4/x」 and press Enter: the status line speaks of 「拍子の上」 and 「4」. Type the same in the transport's 拍子 box: it speaks of 「拍子の下」.

## Cause

TempoBand.svelte hard-codes `field: "meterNum"` and `kept: String(editFlag.num)` instead of reading the draft, and the `reject` callback of ValueInput (which gets the draft) ignores its argument.

## Suggested fix

Use the draft: `reject={(draft) => { const field = meterFieldOf(draft); send({ type: "rejectValue", field, kept: String(field === "meterNum" ? editFlag.num : editFlag.den), range: null }); }}`, as Transport.svelte:303-306. No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
