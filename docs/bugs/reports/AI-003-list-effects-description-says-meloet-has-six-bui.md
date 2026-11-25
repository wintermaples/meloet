---
id: AI-003
title: list_effects description says meloet has six built-in effects; there are nine
area: AI
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# AI-003: list_effects description says meloet has six built-in effects; there are nine

## Where

crates/meloet-ai/src/tools/params.rs:582-585 (the doc comment of `list_effects`, which rmcp sends as the tool's description in `tools/list`)

## What happens

The description an AI app reads says "meloet's own six with their parameters (on the first page)". The answer's `builtin` lists nine: eq4, compressor, reverb, delay, chorus, limiter, gate, deEsser, saturation (`EffectKind::ALL` is `[EffectKind; 9]`, crates/meloet-model/src/effects.rs:417). `insert_effect`'s own argument description (mixer.rs:222-223) names all nine.

## What should happen

The description matches what the tool gives: nine built-in effects (or no number). The task brief and docs/development/ai-tools.md say each tool's description should match what it really does; the sibling `EffectArgs.effect` description lists the nine.

## How to trigger

Read from the code, not run. Ask an MCP client for `tools/list` and read `list_effects`' description; call `list_effects` and count `builtin`.

## Cause

Gate, de-esser and saturation were added in round 12 (CLAUDE.md: the effect kinds `gate`, `deEsser`, `saturation`); the description was not updated.

## Suggested fix

Write "meloet's own nine" (or "meloet's own effects") in the doc comment. The description stays under 60 words (`every_tool_says_what_it_is`). No protocol change.

## Verification

Confirmed by reading. The doc comment of `list_effects` (`crates/meloet-ai/src/tools/params.rs:582-585`), which is the tool's description, says "meloet's own six"; `EffectKind::ALL` has nine (gate, deEsser and saturation since round 12), and the answer's `builtin` lists them all. Severity `low`.

Fix plan: `crates/meloet-ai/src/tools/params.rs`: "meloet's own nine" (the word count stays the same, so `every_tool_says_what_it_is` still passes). Check `docs/development/ai-tools.md` for the same number and change it there too if it says six.

## Fix

`crates/meloet-ai/src/tools/params.rs`: the description of `list_effects` says "meloet's own nine" (the description is a doc comment the rmcp macro reads, so it cannot be counted from `EffectKind::ALL`). `docs/development/ai-tools.md` gave no number. Watched by `every_tool_says_what_it_is` (word count) only.
