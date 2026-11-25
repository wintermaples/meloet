---
id: UI-ARRANGE-009
title: The take switch tag's prev and next stay pressable while recording and counting in, but Rust takes no take switch then
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
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

Confirmed by reading (not run). `Arrange.svelte:1990, 1996` always send `stepTake`; `step_take` returns at once while `busy()` (`crates/meloet-core/src/play.rs:1295-1298`). The context menu rows and the menu bar row for the same action are disabled while busy, and the piano roll's tag disables its buttons.

One root with UI-ROLL-008 (which now holds UI-PARITY-005), UI-ARRANGE-013 and UI-PARITY-010: parts that look usable while recording or counting in though Rust refuses the intent. Each is fixed in its own file, but by one rule: read the same `busy` (transport not stopped in the recording sense, as `songBusy` in `ui/src/keys/attach.ts` and the `busy` already in `Arrange.svelte`), show the disabled state, send nothing.

Fix plan: in `ui/src/components/Arrange.svelte` give ◂ and ▸ `disabled={busy}` (or `aria-disabled` plus no send, as the arrange's other disabled buttons do) and the disabled look. Same two buttons as UI-ARRANGE-008; fix together. No protocol or wording change.

## Fix

`ui/src/components/Arrange.svelte`: ◂ and ▸ of the take switch tag get `disabled={busy}` (recording or count-in) and the disabled look (with UI-ARRANGE-008); a disabled button sends nothing.
Test: `ui/tests/arrange.test.ts` "while recording or counting in, ◂ ▸ of the switch tab are disabled and send nothing".
