---
id: UI-PARITY-012
title: Up and down in a built-in effect's parameter list can choose a row out of view; the plugin list scrolls to it
area: UI-PARITY
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# UI-PARITY-012: Up and down in a built-in effect's parameter list can choose a row out of view; the plugin list scrolls to it

## Where

ui/src/components/params/BuiltinList.svelte:109-123 (`onkeydown`: ↑ ↓ call `pick(next)` only)
ui/src/components/params/BuiltinList.svelte:75-78 (`pick` focuses the list with `preventScroll: true` and sends `selectParam`; nothing scrolls)
ui/src/components/params/BuiltinList.svelte:129 (the rows are in a `ScrollArea` with no bound `viewport`)
The sibling that does it right: ui/src/components/params/PluginList.svelte:105-113 (`reveal`) and :167-171 (`pick(next); reveal(next);`)

## What happens

In the params panel of a built-in effect with more rows than fit (the 4-band EQ has 10 rows plus group headings; with the preset list open or a low window, fewer fit), ↑ ↓ move the chosen row (gray.4, accent line) past the bottom or the top of the visible part. The list does not scroll, so the chosen row is out of view and the person does not see which one Enter or ← → will act on. A plugin's parameter list scrolls so the chosen row stays visible. The note list, the CC list and the history band also keep the row in view (NoteListPanel.svelte:126, CcListPanel.svelte:108, history/HistoryList.svelte:92).

## What should happen

The same as PluginList: after ↑ ↓ the chosen row is scrolled into view. Spec 2 "Scroll bar" names parameters among the lists that scroll this way, and spec 4 gives the same ↑ ↓ for "In the parameters" whatever the effect is.

## How to trigger

Read from the code, not run.
1. Make the window low (1024 × 640) and open a lower band so the right rail is short, or open 「プリセット ▾」 above the rows.
2. Put a 4-band EQ on a track and choose it; click the first row.
3. Press ↓ several times: `sel.row` moves to rows below the visible part; the ScrollArea's `scrollTop` never changes.

## Cause

BuiltinList was not given the `reveal` step PluginList has (PluginList binds the ScrollArea's `viewport` and sets `scrollTop`; BuiltinList does not bind it).

## Suggested fix

Bind the ScrollArea's `viewport` in BuiltinList and, after `pick(next)` from a key, scroll the row `[data-param="…"]` into view (for example `el.scrollIntoView({ block: "nearest" })`, as EndpointRow.svelte:32 does, or a `reveal` like PluginList's). No protocol, wording or look change.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
