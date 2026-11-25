---
id: AI-004
title: get_automation gives at most 2000 points of a lane with no paging; later points cannot be seen or named
area: AI
kind: ui-ux
severity: low
status: reported
found: 2026-11-25
---

# AI-004: get_automation gives at most 2000 points of a lane with no paging; later points cannot be seen or named

## Where

crates/meloet-ai/src/tools/automation.rs:217 (`lane.points.iter().enumerate().take(MAX_NOTES)`)
crates/meloet-ai/src/tools/automation.rs:108-111 (`TrackArgs`: no `offset` / `limit`)

## What happens

`get_automation` gives only the first 2000 points of each lane, with `total` beside them, and has no `offset` or `limit` to get the rest. `move_automation` and `delete_automation` take the points by their index from `get_automation`, so a point at index 2000 or later can be neither seen nor named with knowledge of where it is. A lane has no limit on its points (`meloet_model::automation::put_points` has none), and a lane written while playing (「● 書く」) or by `write_automation` calls one after another can pass 2000.

## What should happen

Every point can be read: "Long lists come in pages: `limit`, `offset`, and `total`, `count`, `next_offset`" (docs/development/ai-tools.md, "How every tool answers"; the tool INSTRUCTIONS say the same). The sibling `get_notes` has `first` and `max` for the same reason.

## How to trigger

Read from the code, not run. Write 2001 or more points into one lane (two `write_automation` calls of 2000 points at different ticks), then call `get_automation`: `total` is over 2000 but the last points are missing and there is no way to ask for them.

## Cause

The cap of 2000 (`MAX_NOTES`) was put on the list without paging arguments.

## Suggested fix

Add optional `offset` (or `first`) and `limit` (default and most 2000) to `get_automation`, applied per lane or to one lane chosen by `target`, and give `next_offset` when points are left. Indexes stay the indexes in the lane. No protocol change; docs/development/ai-tools.md gains the two arguments.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
