---
id: UI-SHELL-006
title: The mixer band tab always says 0.0 dB, not the master fader value
area: UI-SHELL
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-SHELL-006: The mixer band tab always says 0.0 dB, not the master fader value

## Where

ui/src/components/BandRow.svelte:35-43 (`count: t("band.masterDb", { db: t("common.db", { value: dbNumber(0) }) })`)

## What happens

The band tab 「ミキサー」 in the band tab row always shows 「マスター 0.0 dB」, whatever the master fader is set to (for example −6.0 dB, or −∞). The comment says "It stays at 0.0 dB for now", which was true before the master strip had a fader; `SongView.master.gainDb` (ui/src/bridge/generated/MasterView.ts) now holds the value.

## What should happen

docs/spec/3-bands.md:11 (table of lower bands): "Mixer | Band tab: the value of the master fader (not the loudness. A new song is 0.0 dB)". Spec 2 "Band tab" shows 「▸ ミキサー −6.2 dB M」. docs/progress.md ("The mixer band's tab shows the master's hand value…") also takes for granted that it shows the hand value.

## How to trigger

Read from the code, not run. Open the mixer (M), move the master fader to −6 dB, fold the mixer: the tab still says 「マスター 0.0 dB」.

## Cause

The value is a constant `dbNumber(0)`; `app.song.master.gainDb` is never read in BandRow.svelte.

## Suggested fix

Fill the hole from `app.song.master.gainDb` with the same number form the master strip uses (`faderText` in ui/src/components/mixer/scale.ts, or `dbNumber` with the −∞ text for null). No protocol change; the wording 「マスター {db}」 stays.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
