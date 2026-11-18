//! Tests that need the real PipeWire output. Run with `-- --ignored`.
//!
//! The sink is `MELOET_TEST_OUTPUT` (a node.name) or else the first `Audio/Sink` that
//! `pw-cli ls Node` lists. They make a short, quiet sound.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

mod common;

use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use meloet_engine::{
    AudioOutput, Engine, EngineCommand, EngineConfig, InstrumentEvent, MidiIn, MixStrip,
    OutputTarget, PlaceholderSynth, monotonic_us,
};

fn target() -> OutputTarget {
    if let Ok(node_name) = std::env::var("MELOET_TEST_OUTPUT") {
        return OutputTarget { node_name };
    }
    let listing = Command::new("pw-cli")
        .args(["ls", "Node"])
        .output()
        .expect("pw-cli runs");
    let listing = String::from_utf8_lossy(&listing.stdout).into_owned();
    let value = |line: &str| line.split('"').nth(1).map(str::to_owned);
    let mut name = None;
    for line in listing.lines() {
        if line.trim_start().starts_with("id ") {
            name = None;
        } else if line.contains("node.name = ") {
            name = value(line);
        } else if line.contains("media.class = \"Audio/Sink\"") {
            return OutputTarget {
                node_name: name.expect("node.name comes before media.class"),
            };
        }
    }
    panic!("no Audio/Sink node found; set MELOET_TEST_OUTPUT");
}

fn quiet_note(midi_in: &mut rtrb::Producer<MidiIn>, note: u8, on: bool) {
    let event = if on {
        InstrumentEvent::NoteOn { note, velocity: 50 }
    } else {
        InstrumentEvent::NoteOff { note }
    };
    midi_in
        .push(MidiIn {
            event,
            t_us: monotonic_us(),
        })
        .expect("midi ring has room");
}

#[test]
#[ignore = "needs a PipeWire output"]
fn real_output_runs_two_seconds_without_touching_the_allocator() {
    let target = target();
    let mut parts = Engine::new(EngineConfig::default());
    let _ = parts
        .handle
        .swap_instrument(0, Box::new(PlaceholderSynth::new()));
    let _ = parts.handle.send(EngineCommand::SetStrip {
        strip: MixStrip::Track(0),
        gain: 0.3,
        pan: 0.0,
    });
    let meters = parts.handle.meters().clone();

    let running = AudioOutput::open(&target, 256, parts.engine).expect("output opens");
    sleep(Duration::from_millis(500));
    let _ = parts.handle.send(EngineCommand::TestTone);
    for (i, note) in [60u8, 64, 67].into_iter().enumerate() {
        quiet_note(&mut parts.midi_in, note, true);
        sleep(Duration::from_millis(250));
        quiet_note(&mut parts.midi_in, note, false);
        assert!(
            meters.take_track_peak(0)[0] > 0.0,
            "note {i} reached the track meter"
        );
    }
    sleep(Duration::from_millis(750));

    let frames = meters.last_frames();
    let callbacks = meters.callbacks();
    let mut delays = Vec::new();
    while let Some(record) = parts.handle.pop_latency() {
        assert!(
            record.callback_us >= record.key_us,
            "callback clock is the MIDI clock: {record:?}"
        );
        assert!(record.playback_us >= record.callback_us, "{record:?}");
        delays.push((record.playback_us - record.key_us) as f64 / 1000.0);
    }
    eprintln!(
        "target {} | rate {} Hz | asked 256, running {frames} frames | callbacks {callbacks} | late {} | \
         xruns {} | lost {} | other errors {} | load {} permille | key-to-DAC ms {delays:.1?}",
        target.node_name,
        running.sample_rate(),
        meters.late_callbacks(),
        meters.xruns(),
        meters.streams_lost(),
        meters.other_errors(),
        meters.load_permille(),
    );
    assert!(frames > 0);
    let expected = 2.0 * f64::from(running.sample_rate()) / f64::from(frames);
    assert!(
        callbacks as f64 > expected * 0.7,
        "{callbacks} callbacks, about {expected:.0} expected"
    );
    assert_eq!(delays.len(), 3);
    assert!(delays.iter().all(|ms| *ms < 200.0), "{delays:?}");

    assert!(running.close().is_some(), "the engine comes back");
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

#[test]
#[ignore = "needs a PipeWire output"]
fn engine_survives_two_output_switches() {
    let target = target();
    let mut parts = Engine::new(EngineConfig::default());
    let _ = parts
        .handle
        .swap_instrument(0, Box::new(PlaceholderSynth::new()));
    let _ = parts.handle.send(EngineCommand::SetStrip {
        strip: MixStrip::Track(0),
        gain: 0.3,
        pan: 0.0,
    });
    let meters = parts.handle.meters().clone();

    let mut engine = Some(parts.engine);
    let mut callbacks_before = 0;
    // Open, then switch twice. This PC has one sink, so the "new" output is the same node again.
    for round in 0..3u8 {
        let running =
            AudioOutput::open(&target, 256, engine.take().unwrap()).expect("output opens");
        sleep(Duration::from_millis(300));
        quiet_note(&mut parts.midi_in, 60 + round * 4, true);
        sleep(Duration::from_millis(250));
        quiet_note(&mut parts.midi_in, 60 + round * 4, false);
        sleep(Duration::from_millis(350));

        assert!(
            meters.callbacks() > callbacks_before,
            "round {round}: callbacks run"
        );
        callbacks_before = meters.callbacks();
        assert!(
            parts.handle.pop_latency().is_some(),
            "round {round}: the same MIDI ring still feeds the engine"
        );
        assert!(parts.handle.pop_latency().is_none());
        assert!(
            meters.take_track_peak(0)[0] > 0.0,
            "round {round}: the same instrument still sounds"
        );
        eprintln!(
            "round {round}: running {} frames at {} Hz",
            meters.last_frames(),
            running.sample_rate()
        );

        engine = running.close();
        assert!(engine.is_some(), "round {round}: the engine comes back");
        let stopped_at = meters.callbacks();
        sleep(Duration::from_millis(50));
        assert_eq!(
            meters.callbacks(),
            stopped_at,
            "round {round}: no callback after close"
        );
    }
    assert!(!parts.midi_in.is_abandoned());
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

/// Runs with or without PipeWire: either way the open fails and must hand the engine back.
#[test]
fn unknown_output_gives_the_engine_back() {
    let parts = Engine::new(EngineConfig::default());
    let target = OutputTarget {
        node_name: "meloet-test.no-such-node".to_owned(),
    };
    let Err(error) = AudioOutput::open(&target, 256, parts.engine) else {
        panic!("a node that does not exist cannot be opened");
    };
    assert!(error.engine.is_some(), "{error}");
}
