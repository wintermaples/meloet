//! The input and the output on one clock (docs/architecture.md 22-6), on a PipeWire daemon
//! of the test's own: the sink's monitor is the input, so what the engine plays comes back
//! with no device between. A click that leaves at one frame must come back at the frame the
//! two streams' times say, whatever the quantum. Run with `-- --ignored`; needs the
//! `pipewire` and `wireplumber` programs.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

mod common;
mod private_pw;

use std::thread::sleep;
use std::time::{Duration, Instant};

use meloet_engine::{
    AudioInput, AudioOutput, Engine, EngineCommand, EngineConfig, InputBlock, InputFeed,
    InputTarget, OutputTarget, ProbeHeard, ProbeVerdict, Voice, judge_probes, probe_wave,
};

const RATE: u32 = 48_000;

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        sleep(Duration::from_millis(5));
    }
}

/// The way round at `quantum`, in frames: what five clicks say, each one's answer.
fn way_round(quantum: u32) -> (ProbeVerdict, Vec<Option<f64>>, u32) {
    let mut parts = Engine::new(EngineConfig::default());
    let meters = parts.handle.meters().clone();
    let output = AudioOutput::open(
        &OutputTarget {
            node_name: private_pw::SINK.into(),
        },
        quantum,
        parts.engine,
    )
    .expect("the output opens");
    let mut input = AudioInput::open(
        &InputTarget {
            node_name: private_pw::SINK.into(),
            sink_monitor: true,
        },
        output.sample_rate(),
        quantum,
    )
    .expect("the sink's monitor opens as an input");
    assert_eq!(input.sample_rate(), RATE);
    assert_eq!(input.channels(), 2);
    let in_meters = input.meters().clone();
    let mut record = input.take_record().expect("the record ring");
    wait_until("both streams to run", Duration::from_secs(5), || {
        meters.callbacks() > 20 && in_meters.frames() > 20 * u64::from(quantum)
    });
    assert_eq!(meters.last_frames(), quantum, "the output's quantum");
    assert_eq!(in_meters.last_frames(), quantum, "the input's quantum");
    eprintln!(
        "quantum {quantum}: the PC says out {} us, in {} us",
        meters.out_delay_us(),
        in_meters.delay_us()
    );

    // The streams run before WirePlumber has linked them: wait until a click comes round.
    let _ = in_meters.take_peak(0);
    wait_until("the loop to be linked", Duration::from_secs(10), || {
        assert!(parts.handle.send(EngineCommand::Probe).is_ok());
        sleep(Duration::from_millis(100));
        in_meters.take_peak(0).0 > 0.01
    });
    sleep(Duration::from_millis(200));
    while parts.handle.pop_probe().is_some() {}

    in_meters.set_recording(true);
    let mut blocks: Vec<InputBlock> = Vec::new();
    let mut heard: Vec<f32> = Vec::new();
    let mut first_frame = None;
    let mut into = vec![0.0f32; 2 * 8_192];
    let mut sent = Vec::new();
    let mut take = |blocks: &mut Vec<InputBlock>, heard: &mut Vec<f32>| {
        while let Some(block) = record.pop(&mut into) {
            assert!(!block.dropped, "the ring ran full");
            first_frame.get_or_insert(block.first_frame);
            heard.extend(into[..2 * block.frames as usize].chunks(2).map(|f| f[0]));
            blocks.push(block);
        }
    };
    for _ in 0..5 {
        assert!(parts.handle.send(EngineCommand::Probe).is_ok());
        let until = Instant::now() + Duration::from_millis(250);
        while Instant::now() < until {
            take(&mut blocks, &mut heard);
            sent.extend(std::iter::from_fn(|| parts.handle.pop_probe()));
            sleep(Duration::from_millis(2));
        }
    }
    take(&mut blocks, &mut heard);
    assert_eq!(sent.len(), 5);
    assert_eq!(in_meters.record_dropped(), 0);
    let first_frame = first_frame.expect("input came");

    let wave = probe_wave(RATE);
    let clock = parts.handle.clock().clone();
    let all = ProbeHeard {
        blocks: &blocks,
        first_frame,
        heard: &heard,
        sample_rate: RATE,
    };
    let answers: Vec<Option<f64>> = sent
        .iter()
        .map(|probe| {
            let left_us = clock.time_of_frame(probe.frame as f64).expect("the clock");
            all.way_round(&wave, left_us).map(|(us, _)| us)
        })
        .collect();

    input.close();
    drop(output);
    (judge_probes(&answers, RATE), answers, quantum)
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn a_click_comes_back_on_the_frame_the_two_clocks_say() {
    let Some(_pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let frame_us = 1e6 / f64::from(RATE);
    let mut worst = 0.0f64;
    for quantum in [64, 128, 256, 512, 1024] {
        let (verdict, answers, quantum) = way_round(quantum);
        let in_frames: Vec<Option<f64>> = (answers.iter())
            .map(|a| a.map(|us| (us / frame_us * 100.0).round() / 100.0))
            .collect();
        eprintln!("quantum {quantum}: X_sw {verdict:?} us, each in frames {in_frames:?}");
        let ProbeVerdict::Measured(x_us) = verdict else {
            panic!("quantum {quantum}: {verdict:?} {answers:?}");
        };
        worst = worst.max((x_us / frame_us).abs());
    }
    // With nothing between the sink and its monitor, the way round is nothing.
    assert!(worst <= 0.5, "X_sw is {worst} frames at its worst");
}

/// The voice of a vocal track on real streams: the monitor of the sink is sounded again by
/// the engine, and neither callback takes memory.
#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn the_input_opens_feeds_a_voice_and_closes_without_touching_the_allocator() {
    let Some(_pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let mut parts = Engine::new(EngineConfig::default());
    let meters = parts.handle.meters().clone();
    let sink = private_pw::SINK;
    let output = AudioOutput::open(
        &OutputTarget {
            node_name: sink.into(),
        },
        256,
        parts.engine,
    )
    .expect("the output opens");

    let no_such = AudioInput::open(
        &InputTarget {
            node_name: sink.into(),
            sink_monitor: false,
        },
        RATE,
        256,
    );
    assert!(no_such.is_err(), "a sink is not in the list of inputs");

    for round in 0..3 {
        let mut input = AudioInput::open(
            &InputTarget {
                node_name: sink.into(),
                sink_monitor: true,
            },
            output.sample_rate(),
            256,
        )
        .expect("the input opens");
        let in_meters = input.meters().clone();
        let feed = InputFeed::new(input.take_monitor().expect("once"), in_meters.clone());
        assert!(input.take_monitor().is_none());
        assert!(parts.handle.set_input(Some(feed)).is_ok());
        let voice = Voice {
            left: 0,
            right: 1,
            on: true,
        };
        let command = EngineCommand::SetVoice {
            track: 0,
            voice: Some(voice),
        };
        assert!(parts.handle.send(command).is_ok());
        let before = in_meters.frames();
        wait_until("the input to run", Duration::from_secs(5), || {
            in_meters.frames() > before + 48_000 / 2 && in_meters.monitoring()
        });
        assert_eq!(in_meters.last_frames(), 256, "round {round}");
        assert_eq!(in_meters.monitor_dropped(), 0, "round {round}");
        assert!(!in_meters.stream_lost());

        assert!(parts.handle.set_input(None).is_ok());
        wait_until("the feed to come back", Duration::from_secs(2), || {
            parts.handle.drop_retired() > 0
        });
        let frames = in_meters.frames();
        input.close();
        sleep(Duration::from_millis(100));
        assert!(
            in_meters.frames() <= frames + 1_024,
            "closed: no more blocks"
        );
        assert!(!in_meters.stream_lost(), "closing is not losing");
    }
    assert!(meters.callbacks() > 100);
    drop(output);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
