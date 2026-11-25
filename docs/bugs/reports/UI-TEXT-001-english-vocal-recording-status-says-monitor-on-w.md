---
id: UI-TEXT-001
title: English vocal recording status says monitor on while monitoring is off
area: UI-TEXT
kind: parity
severity: medium
status: reported
found: 2026-11-25
---

# UI-TEXT-001: English vocal recording status says monitor on while monitoring is off

## Where

ui/src/components/StatusLine.svelte:259
ui/src/components/StatusLine.svelte:264
ui/src/text/en.ts:1154 (`msg.recordingVocal`)
ui/src/text/en.ts:1567 (`msg.recordingVocals`)
ui/src/text/en.ts:1295 (`meter.monitorOn`)

## What happens

With the page in English, recording a vocal track while its monitor is off still shows "● Recording · 6 Vocal (In 1) · monitor on · Space to stop" (and the same for two or more vocal tracks). The status line says the monitor is on when it is off. In Japanese the part 「モニター オン ・ 」 is taken out as meant.

## What should happen

The part "monitor on · " is left out when monitoring is off, as the comment at StatusLine.svelte:244 says (「モニター オン ・ 」 is left out when monitoring is off) and as the Japanese page does (spec 6 "Recording vocals", "Recording 2 or more vocal tracks").

## How to trigger

Read from the code, not run. Language English; add a vocal track, turn its 「モニター」 off, press record. `recordingVocalsParts(keys, vocals, monitor=false)` builds `monitorOn = "Monitor on · "` and calls `text.replace("Monitor on · ", "")` on "… · monitor on · …": no match, nothing is removed.

## Cause

The code finds the piece to remove by the text of another key: `t("meter.monitorOn")` + `STATUS_JOIN`. In Japanese both say 「モニター オン」; in English `meter.monitorOn` is "Monitor on" (capital M, a label) while the sentences have "monitor on" (small m, mid-sentence). `String.replace` is case-sensitive.

## Suggested fix

Smallest: in en.ts make the piece inside `msg.recordingVocal` and `msg.recordingVocals` match what the code removes, or (better, not tied to letter case) give the sentence without the monitor part its own way: for example compare case-insensitively, or have the code remove the text between the hole before it and `STATUS_JOIN` by a separate key used only for this (`meter.monitorOnInline`: 「モニター オン」 / "monitor on"). A new key needs English only (the Japanese is the existing 「モニター オン」); no protocol change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
