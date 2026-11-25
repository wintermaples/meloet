---
id: AI-001
title: Toggle tools (set_mute, set_solo, bypass_effect, set_loop_on) read, then send a toggle, so two calls at once flip back
area: AI
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# AI-001: Toggle tools (set_mute, set_solo, bypass_effect, set_loop_on) read, then send a toggle, so two calls at once flip back

## Where

crates/meloet-ai/src/tools/mixer.rs:450-472 (`set_mute`: reads `muted`, then sends `Command::ToggleMute` / `ToggleReturnMute`)
crates/meloet-ai/src/tools/mixer.rs:489-494 (`set_solo`: reads `soloed`, then sends `ToggleSolo`)
crates/meloet-ai/src/tools/mixer.rs:597-609 (`bypass_effect`: reads `bypassed`, then sends `ToggleBypass`)
crates/meloet-ai/src/tools/timing.rs:293-300 (`set_loop_on`: reads `loop_on`, then sends `ToggleLoop`)

## What happens

These four tools take the wanted state (`on: true/false`) but change the song with a toggle intent. The tool reads the song, compares, and when it differs sends the toggle. Between the read and the moment the core runs the toggle, anything else that flips the same switch makes the toggle go the wrong way. Two `set_mute {track: 2, on: true}` calls that run at the same time both read "not muted" and both send `ToggleMute`: the track ends unmuted, and both answers say `changed: true` ("Done"). The same happens when the person presses M (or S, the bypass button, L) in that window. The tools are marked `idempotent_hint = true`, and an AI app may send the same call again or several calls at once.

## What should happen

`set_mute {on: true}` leaves the track muted, whatever ran at the same time; a second call is "no change" (docs/development/ai-tools.md, "No change": "Tools that turn a value into a toggle (`set_mute`, `set_solo`, `bypass_effect`, `set_loop_on` …) answer the same when it is so already"), and the tool's `idempotent_hint` says so.

## How to trigger

Read from the code, not run. One connection: send two `tools/call` requests `set_mute {"track": 1, "on": true}` without waiting for the first answer (rmcp 3.11 `service.rs` runs every request of a connection as its own task, `spawn_service_task`, so both read the song before either toggle reaches the core). Or two AI apps connected at once (up to 8 connections). Or the person presses M on that track while the call is between its read and its ask.

## Cause

The state is read on the AI thread (`self.read()`) and the change is a toggle that the core applies later on its own thread; there is no check that the state is still the one read. Outside a batch nothing holds the person's intents or other connections' asks between the read and the toggle.

## Suggested fix

Let the core decide with the wanted value: an AI ask such as `AiAsk::SetToggle { what, on }` (or the existing screen command path with a guard) that, on the core thread, does nothing (`Unchanged`) when the switch is already `on` and otherwise sends the toggle. No protocol change for the screen (`AiAsk` is core-internal); no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
