---
id: AI-004
title: get_automation gives at most 2000 points of a lane with no paging; later points cannot be seen or named
area: AI
kind: ui-ux
severity: low
status: confirmed
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

Confirmed by reading. `get_automation` takes only `track` (`TrackArgs`) and gives `lane.points…take(MAX_NOTES)` (2000) with `total`; there is no way to read the points after the first 2000 of a lane, while `move_automation` and `delete_automation` name points by the index this tool gives. I kept it as a fault and not a wish because the sibling has the missing half: `get_notes` takes `first` and `max` for the same cap (`tools.rs:705-710`), and the tool instructions and `docs/development/ai-tools.md` say "Long lists come in pages". A lane has no limit on its points. Severity `low` stands (a lane of more than 2000 points is rare).

Corrections: none.

Fix plan: `crates/meloet-ai/src/tools/automation.rs`: `get_automation` gets its own arguments (`track`, optional `first` and `max` as `get_notes` names them, default and most 2000), applied to every lane's points; each lane keeps `total` and gains `next_first` (null when none are left); indexes stay the indexes in the lane. `docs/development/ai-tools.md` names the two arguments. Test in `crates/meloet-ai/tests/`: a lane of 2001 points, the second page holds index 2000.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
