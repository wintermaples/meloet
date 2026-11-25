---
id: UI-ARRANGE-013
title: While recording, a drag on the arrange ruler still shows a new loop range that Rust then refuses
area: UI-ARRANGE
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-013: While recording, a drag on the arrange ruler still shows a new loop range that Rust then refuses

## Where

ui/src/components/Arrange.svelte:1263-1286 (`rulerDown` / `rulerMove` / `rulerUp`: no check of `busy`)
crates/meloet-core/src/play.rs:1012-1015 (`set_loop`: `if self.busy() { return; }`)
crates/meloet-core/src/play.rs:928-931 (`locate`: the same)

## What happens

While recording or counting in, a press and drag on the arrange's bar ruler starts a loop range drag as usual: the accent loop mark and the loop lines move to the new range and the heading shows 「ループ a–b」 for it. On release `setLoop` is sent, core ignores it (busy), no rev comes, and the preview is dropped (`rev === before`), so the mark jumps back. A plain press sends `locate`, which core ignores as well. Nothing tells the person why.

## What should happen

The other drags of the arrange do not start while recording or counting in: clips only get chosen (`Arrange.svelte:1381-1385`), markers do not move (`MarkerRow.svelte:65`, `moved = !busy && …`), tempo points and flags only get chosen (`TempoBand.svelte:99`, `:167`), automation points do not move (`AutoLane.svelte:131`). The ruler should do the same: no loop preview while `busy` (architecture 10: actions not taken during the count-in and recording).

## How to trigger

Read from the code, not run. Start recording (R). Drag on the ruler from bar 3 to bar 6: the loop mark shows 3–6 while dragging; release: it goes back to the old range.

## Cause

`rulerDown` starts `drag` without looking at `busy`; `rulerMove` sets `preview` from it.

## Suggested fix

Return early in `rulerDown` while `busy` (or do not set `preview` in `rulerMove` while `busy`). The piano roll's ruler has the same handlers (`pianoroll/Roll.svelte:1028-1039`, outside this area). No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
