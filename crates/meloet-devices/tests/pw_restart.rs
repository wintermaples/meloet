//! The watcher when the PipeWire daemon goes away, on a daemon of the test's own (never the one
//! of the person using the PC). Run with `-- --ignored`; needs `pipewire` and `wireplumber`.

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;

use std::sync::mpsc;
use std::time::{Duration, Instant};

use meloet_devices::{DeviceWatcher, DevicesEvent};

fn has_sink(event: &DevicesEvent) -> bool {
    matches!(event, DevicesEvent::Picture(p)
        if p.outputs.iter().any(|e| e.node_name.as_deref() == Some(private_pw::SINK)))
}

fn wait_for(rx: &mpsc::Receiver<DevicesEvent>, mut want: impl FnMut(&DevicesEvent) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let event = rx.recv_timeout(left).expect("the event in time");
        if want(&event) {
            return;
        }
    }
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn a_killed_daemon_is_reported_once_and_a_new_watcher_connects_after_restart() {
    let Some(mut pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };

    let (tx, rx) = mpsc::channel();
    let watcher = DeviceWatcher::start(move |event| {
        let _ = tx.send(event);
    })
    .expect("watcher starts");
    wait_for(&rx, has_sink);

    let killed = Instant::now();
    pw.kill();
    wait_for(&rx, |e| *e == DevicesEvent::Lost);
    eprintln!("lost noticed after {:?}", killed.elapsed());
    // The thread has ended and dropped the sink: nothing follows.
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    drop(watcher);

    let refused = DeviceWatcher::start(|_| {});
    assert!(refused.is_err(), "no daemon, no watcher");
    eprintln!("start without a daemon: {}", refused.err().unwrap());

    pw.restart();
    let (tx, rx) = mpsc::channel();
    let watcher = DeviceWatcher::start(move |event| {
        let _ = tx.send(event);
    })
    .expect("watcher starts again");
    wait_for(&rx, has_sink);

    // Stopping on purpose is not a loss.
    watcher.stop();
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
}
