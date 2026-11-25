---
id: PROTO-001
title: Core sends every keyboard CC and pitch bend as an Event message, and the screen drops them all
area: PROTO
kind: parity
severity: low
status: reported
found: 2026-11-25
---

# PROTO-001: Core sends every keyboard CC and pitch bend as an Event message, and the screen drops them all

## Where

crates/meloet-protocol/src/lib.rs:4168 (`Event::Control`), crates/meloet-protocol/src/lib.rs:4173 (`Event::Bend`)
crates/meloet-core/src/service.rs:1410 (`hub.event(protocol_event(event, t_us))` for every MIDI event)
crates/meloet-core/src/service.rs:1452-1457 (`protocol_event`: every `MidiEvent::Control` and `MidiEvent::PitchBend` becomes an `Event`)
ui/src/gl/engine.ts:341-342 (`if (e.type !== "note") return;`)
ui/src/components/InputMeter.svelte:197-198, ui/src/components/StatusLine.svelte:904-905, ui/src/components/arrange/fileDrag.svelte.ts:55 (the other `onEvent` listeners: notes and file drags only)

## What happens

Every CC of every controller (mod wheel, expression, sustain, any knob or fader of the keyboard) and every pitch-bend value that arrives from a keyboard is turned into an `Event::Control` / `Event::Bend`, serialized to JSON and sent on the messages Channel, under the same lock that publishes patches (`Hub::event`). On the screen no listener reads them: the four `onEvent` listeners all return unless the event is `note` (or a file drag). The sustain, modulation and bend the screen shows come from the telemetry (`Telemetry.sustain`, `modulation`, `pitchBend`) instead. So these two news kinds are sent and never used; a pitch-bend wheel or an expression pedal moved for a while makes hundreds of such messages a second, each one parsed by the web page for nothing.

## What should happen

docs/architecture.md 3-1, item 6 says pedals, bend and modulation are told at once on the messages Channel, so one of the two sides is not what the contract says: either the screen uses them (the pedal and wheel readings of the input meter move the moment they arrive, as keys do through `Event::Note`), or core does not send what the screen never reads and the contract says the telemetry carries them. The brief's parity rule (b): a news kind core sends must be one the screen reads.

## How to trigger

Read from the code, not run. Move the pitch-bend wheel or the mod wheel of a plugged-in keyboard: `service.rs` sends one `Event::Bend` / `Event::Control` per MIDI message; in the page, `engine.onEvent`, `InputMeter`, `StatusLine` and `fileDrag` each get it and return at their first line.

## Cause

`protocol_event` maps all four `MidiEvent` kinds to `Event`, but every screen listener was written for `Event::Note` (and `FileDrag`/`FileDrop`) only; the screen took the pedal, modulation and bend readings from the telemetry instead, and the Control/Bend events were left with no reader.

## Suggested fix

Smallest: in `crates/meloet-core/src/service.rs`, send only `Event::Note` from the MIDI thread (return early for `Control` and `PitchBend` before `hub.event`), and change docs/architecture.md 3-1 item 6 to say pedals, bend and modulation come in the telemetry. Leaving the `Event::Control`/`Event::Bend` variants in the protocol unused, or removing them, is a protocol change (version bump) the lead decides. The other way (the input meter reading the events) is also possible but adds screen code. Needs a decision by the lead on which side follows the other.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
