//! Needs a running PipeWire. Run with `cargo test -p meloet-devices -- --ignored`.

use std::{
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use meloet_devices::{ClockSettings, DeviceWatcher};

fn clock_from_pw_metadata() -> ClockSettings {
    let out = Command::new("pw-metadata")
        .args(["-n", "settings", "0"])
        .output()
        .expect("pw-metadata runs");
    let text = String::from_utf8_lossy(&out.stdout);
    let mut clock = ClockSettings::default();
    for line in text.lines() {
        // update: id:0 key:'clock.rate' value:'48000' type:''
        let Some(rest) = line.split("key:'").nth(1) else {
            continue;
        };
        let Some((key, rest)) = rest.split_once("' value:'") else {
            continue;
        };
        let Some((value, _)) = rest.rsplit_once("' type:") else {
            continue;
        };
        clock.apply(Some(key), Some(value));
    }
    assert!(
        clock.rate.is_some(),
        "pw-metadata gave no clock.rate:\n{text}"
    );
    clock
}

#[test]
#[ignore = "needs a running PipeWire"]
fn first_picture_arrives_within_two_seconds_and_stop_ends_the_thread() {
    let expected_clock = clock_from_pw_metadata();

    let (tx, rx) = mpsc::channel();
    let started = Instant::now();
    let watcher = DeviceWatcher::start(move |picture| {
        let _ = tx.send(picture);
    })
    .expect("watcher starts");

    let picture = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("a picture within 2 seconds");
    println!("first picture after {:?}", started.elapsed());
    for (label, list) in [("output", &picture.outputs), ("input", &picture.inputs)] {
        for e in list {
            println!(
                "{label}: id={} name={} friendly={:?} availability={:?} link={:?} active={} node_name={:?}",
                e.id, e.name, e.friendly, e.availability, e.link, e.active, e.node_name
            );
        }
    }
    println!("clock: {:?}", picture.clock);

    assert!(!picture.outputs.is_empty(), "at least one output");
    assert_eq!(picture.clock, expected_clock);

    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        watcher.stop();
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("stop returns");
    // The sink was dropped with the thread, so the channel is closed.
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}

#[test]
#[ignore = "needs a running PipeWire"]
fn starting_and_stopping_again_and_again_does_not_hang() {
    for _ in 0..5 {
        let (tx, rx) = mpsc::channel();
        let watcher = DeviceWatcher::start(move |picture| {
            let _ = tx.send(picture);
        })
        .expect("watcher starts");
        rx.recv_timeout(Duration::from_secs(2))
            .expect("a picture within 2 seconds");
        watcher.stop();
    }
}
