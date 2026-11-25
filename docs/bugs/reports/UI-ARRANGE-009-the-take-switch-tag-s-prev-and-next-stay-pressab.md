---
id: UI-ARRANGE-009
title: The take switch tag's prev and next stay pressable while recording and counting in, but Rust takes no take switch then
area: UI-ARRANGE
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# UI-ARRANGE-009: The take switch tag's prev and next stay pressable while recording and counting in, but Rust takes no take switch then

## Where

ui/src/components/Arrange.svelte:762-775 (`switcher`: shown whatever the transport state)
ui/src/components/Arrange.svelte:1988-1999 (◂ / ▸ always send `stepTake`, never disabled)
crates/meloet-core/src/play.rs:1295-1298 (`step_take`: `if self.busy() … return None`)

## What happens

While recording or counting in, the take switch tag above the chosen clip still shows ◂ and ▸ as usable buttons. Pressing them sends `stepTake`; core refuses it while busy and says nothing, so nothing happens.

## What should happen

Architecture 10 ("Actions during recording and count-in"): "Not taken: … switch take …". Parts that cannot be used now are shown disabled (CLAUDE.md "Parts with no content yet … show them in the 'disabled' state"; spec 2 "Parts and states"). The siblings already do it: in the same arrange, the clip's context menu disables 「次のテイク」 / 「前のテイク」 while busy (`Arrange.svelte:1593-1594`, `change = !busy`), the menu bar does too (`menu/menus.ts:279`, `canStepTake` needs `!busy`), and the piano roll's tag disables its ◂ ▸ while its clip is recorded (`pianoroll/Toolbar.svelte:225, 234`).

## How to trigger

Read from the code, not run. Choose a clip with two takes, start recording on another track (R), press ▸ on the tag: nothing changes and nothing is said.

## Cause

The switcher buttons have no `busy` check and no disabled look.

## Suggested fix

Give the two buttons the disabled state while `busy` (as the other disabled buttons of the arrange do: `aria-disabled` plus the disabled look, and do not send). No protocol or wording change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
