//! The whole core when the PipeWire daemon is missing at start, and when it goes away and comes
//! back, on a daemon of the test's own (never the one of the person using the PC). Run with
//! `-- --ignored`; needs `pipewire`, `wireplumber` and `pw-link`.

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;

use std::time::{Duration, Instant};

use meloet_core::{Core, CoreOptions};
use meloet_protocol::{AppState, StatusMessage};

fn wait(core: &Core, what: &str, limit: Duration, done: impl Fn(&AppState) -> bool) {
    let started = Instant::now();
    loop {
        let state = core.snapshot().state;
        if done(&state) {
            eprintln!(
                "{what}: after {:?} | {:?}",
                started.elapsed(),
                state.status.message
            );
            return;
        }
        assert!(
            started.elapsed() < limit,
            "{what}: {:?} {:?}",
            state.status,
            state.audio
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn running(state: &AppState) -> bool {
    state.audio.running && state.audio.actual_frames.is_some()
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn the_core_waits_for_pipewire_and_comes_back_after_it_stopped() {
    let Some(mut pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let config = pw.dir().join("config-meloet");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{0}", "output_name": "{0}", "scale": 100 }}"#,
            private_pw::SINK
        ),
    )
    .unwrap();

    // No PipeWire at start: nothing is said, and the remembered output opens once it is there.
    pw.kill();
    let core = Core::start(CoreOptions {
        config_dir: Some(config),
        measure_dir: None,
        midi: None,
        devices: true,
        audio: true,
        songs_dir: None,
        state_dir: None,
    });
    std::thread::sleep(Duration::from_millis(1500));
    let state = core.snapshot().state;
    assert!(!state.audio.running);
    assert_ne!(
        state.status.message,
        Some(StatusMessage::AudioSystemStopped)
    );
    pw.restart();
    wait(
        &core,
        "opened after a late start",
        Duration::from_secs(5),
        running,
    );

    for round in 0..2 {
        pw.kill();
        wait(&core, "stopped is said", Duration::from_secs(2), |s| {
            !s.audio.running && s.status.message == Some(StatusMessage::AudioSystemStopped)
        });
        let song = core.snapshot().state.song;
        std::thread::sleep(Duration::from_millis(700 * (round + 1)));
        pw.restart();
        wait(&core, "running again", Duration::from_secs(10), |s| {
            running(s) && s.status.message != Some(StatusMessage::AudioSystemStopped)
        });
        assert_eq!(core.snapshot().state.song, song, "the song stays as it was");
    }
    core.stop();
}
