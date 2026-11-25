---
id: ENGINE-001
title: The master inserts' latency is not added to the heard position: play line, recorded keys and metronome are off by it
area: ENGINE
kind: bug
severity: medium
status: needs-lead
found: 2026-11-25
---

# ENGINE-001: The master inserts' latency is not added to the heard position: play line, recorded keys and metronome are off by it

## Where

crates/meloet-engine/src/engine.rs:2729-2731 (`dac_us`: `playback_us + frame / rate`, nothing for the master's inserts)
crates/meloet-engine/src/engine.rs:2739-2758 (`segment_now`: every clock stretch starts at `dac_us`)
crates/meloet-engine/src/engine.rs:1808-1811 (the look-ahead covers the tracks' instrument and inserts only)
crates/meloet-engine/src/engine.rs:1643-1648 (`master_latency` is read only by the export: crates/meloet-core/src/export/job.rs:501, 820)
crates/meloet-engine/src/engine.rs:3084-3110 (the metronome's clicks join after the master's inserts, so they are not delayed with the song)

## What happens

When an insert of the master delays the sound (a plugin that reports latency, such as a limiter with look-ahead or a linear-phase EQ), everything that comes through the master reaches the ear that many frames (L) later than the engine's clock says. Nothing makes up for it while playing:

- The clock stretches say the song position at frame 0 of a stretch reaches the DAC at `playback_us`, but what the person hears there is the song L frames earlier. The play line on the screen (telemetry `heard_now`) runs ahead of the sound by L.
- A key the person plays in time with the song is stamped with the position the clock gives for its time (`stamp`, `stamp_recent`, the stop position in `stop`), which is L later than what they heard: recorded and kept notes come out late by L. The vocal recording uses the same clock (`frame_at`, `time_of_frame`), so a voice sung to the song is placed late by L too.
- The metronome is added after the master (it skips the master's strip), so its clicks sound L before the beats of the song they belong to.

With a 5 ms look-ahead limiter that is 240 frames at 48 kHz; a linear-phase EQ can be tens of ms.

## What should happen

docs/architecture.md 20-8: "Master insert latency is the same for everything: not compensated, but added to the position at the ear (chapter 11)." (The Japanese original, docs/history/architecture.ja.md:841, says the same: マスターのインサートの遅れは … 「耳に届いている位置」に足す.) The export already treats it (it cuts `master_latency()` frames at the head); the live clock does not.

## How to trigger

Read from the code, not run.

1. Put a plugin effect that reports latency on the master (any plugin whose `getLatencySamples` / `lv2:latency` is not 0, such as a look-ahead limiter; which of the plugins on this PC do was not checked).
2. Turn the metronome on and play: the click comes before the song's beat by the plugin's latency.
3. Record keys in time with the song: the notes land late by the same amount; the play line is ahead of what is heard.

## Cause

The heard-position clock is built only from cpal's `playback` time and the frame count (`dac_us`). `Strip::latency()` of the master strip is summed only for `Engine::master_latency()`, which only the export reads. The tracks' latencies are made up for by reading their notes early (`lookahead`), but the master's latency is the same for every track and is not added anywhere on the live path.

## Suggested fix

Needs a decision by the lead on where L goes, because the clock is also the frame line of the latency measurement (`time_of_frame` for the probe click, which is added after the master and is not delayed):

- Add the master's latency (as of the block) to the time each stretch's song position reaches the ear, for example a separate per-segment offset `master_us = master_latency / rate` that `heard`, `stamp` and `segment_at` add, while `frame_at` / `time_of_frame` (the engine-frame line used by the probe and by `vocalrec`) keep the plain DAC time; and
- delay the metronome clicks by the same L (start a click L frames after its beat frame), so the click and the song meet at the ear again.

No protocol or song-file change. A test like `tests/lookahead.rs` with a fake master effect of 100 frames latency can check that a key played at the time the beat is heard is stamped on the beat.

## Verification

Real, checked by reading (no test run; it needs a plugin on the master that reports latency, and I did not start plugins). Architecture 20-8 says the master inserts' latency is "not compensated, but added to the position at the ear (chapter 11)". In the code `Strip::latency()` of the master is read only by `Engine::master_latency()` (`engine.rs:1646`), and that is called only by the export (`export/job.rs`). `dac_us` / `segment_now` build every clock stretch from `playback_us` alone, and the metronome's `click_sum` is added to the output after the master's inserts (`engine.rs:3091-3104`). Only plugin effects report a latency (`Effect::latency` is 0 for every built-in effect, `effect.rs:46`), so this shows only with a plugin on the master that has latency; with meloet's own limiter nothing is off. Chapter 11 itself has no word on it, and `docs/progress.md` does not list it as open.

Corrections: none to the lines. On the clock and the latency measurement: the probe click is added after the master (`self.probe.add`, `engine.rs:3087`) and the measurement reads `TransportClock::time_of_frame` (`probe.rs:199`); the vocal recorder reads `frame_at` / `kept_frame_at` / `time_of_frame` (`vocalrec.rs:912, 1030`, `service/input.rs:404, 534`). So the frame line must stay the plain DAC time, or the measured latency would grow by the master's latency.

Why `needs-lead`: it is a real-time change of the clock (a new field in `Segment`, which is packed into fixed words), of the metronome and of the recorded-position rules, and it needs a measurement with a real latent plugin. Choices:
1. Do what 20-8 says. Place: a per-stretch `ear_us` (`master_strip.latency() / rate`, read once a block) in `Segment` (`clock.rs`), added only where a time is turned into a song position (`Segment::at`, so `heard_at`, `heard_now`, and the engine's own `stamp` / `stamp_recent` / stop position), not in `frame_at` / `time_of_frame`. The core's `vocal_mark` (`service/input.rs:395-412`), which turns a song frame into a time through `time_of_frame`, adds the same amount. The metronome click starts `master_latency` frames after its beat frame. Cost: engine (audio thread, clock word layout), core vocal marks, tests like `tests/lookahead.rs`, and a measurement (`record-timing.sh`) with a latent plugin on the master. This is the one I recommend, as its own job after the sweep (meloet-very-hard), not as a quick fix.
2. Only delay the metronome by the master's latency. Cheap, fixes the click against the song, leaves the play line and recorded keys late by L.
3. Change 20-8 to say it is not added, and list it in `docs/progress.md` as a known limit. No code.

Severity `medium` stands: recorded positions are wrong by L, but only with a latent plugin on the master, and taking it off while recording is the way around.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
