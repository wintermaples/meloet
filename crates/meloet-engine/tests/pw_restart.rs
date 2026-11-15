//! The output stream when the PipeWire daemon goes away, on a daemon of the test's own (never
//! the one of the person using the PC). Run with `-- --ignored`; needs the `pipewire` program.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

mod common;
mod private_pw;

use std::thread::sleep;
use std::time::{Duration, Instant};

use meloet_engine::{
    AudioOutput, Engine, EngineConfig, EngineMeters, OutputTarget, PlaceholderSynth,
};

fn wait_until(what: &str, limit: Duration, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + limit;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        sleep(Duration::from_millis(10));
    }
}

fn wait_callbacks(meters: &EngineMeters, more_than: u64) {
    wait_until("the stream to run", Duration::from_secs(5), || {
        meters.callbacks() > more_than + 10
    });
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn a_killed_daemon_ends_the_stream_and_gives_the_engine_back() {
    let Some(mut pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let target = OutputTarget {
        node_name: private_pw::SINK.into(),
    };
    let mut parts = Engine::new(EngineConfig::default());
    let _ = parts
        .handle
        .swap_instrument(0, Box::new(PlaceholderSynth::new()));
    let meters = parts.handle.meters().clone();

    let running = AudioOutput::open(&target, 256, parts.engine).expect("output opens");
    wait_callbacks(&meters, 0);
    let lost_before = meters.streams_lost();

    let killed = Instant::now();
    pw.kill();
    wait_until("the stream to be lost", Duration::from_secs(2), || {
        meters.streams_lost() > lost_before
    });
    let noticed = killed.elapsed();
    let callbacks = meters.callbacks();
    sleep(Duration::from_millis(200));
    assert_eq!(
        meters.callbacks(),
        callbacks,
        "no block after the daemon died"
    );

    let closing = Instant::now();
    let engine = running
        .close()
        .expect("the engine comes back after the daemon died");
    let closed = closing.elapsed();

    let opened_without = AudioOutput::open(&target, 256, engine);
    let engine = match opened_without {
        Ok(_) => panic!("opened a stream with no daemon"),
        Err(e) => {
            eprintln!("open without a daemon: {e}");
            *e.engine.expect("the engine comes back from a failed open")
        }
    };

    pw.restart();
    let callbacks = meters.callbacks();
    let running = AudioOutput::open(&target, 256, engine).expect("output opens again");
    wait_callbacks(&meters, callbacks);
    let engine = running.close().expect("engine back after a normal close");
    drop(engine);

    eprintln!(
        "lost noticed after {noticed:?}, close took {closed:?} | xruns {} | lost {} | other {}",
        meters.xruns(),
        meters.streams_lost(),
        meters.other_errors(),
    );
    assert_eq!(meters.streams_lost(), lost_before + 1);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
