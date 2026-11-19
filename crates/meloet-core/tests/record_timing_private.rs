//! Where recorded keys land, on a PipeWire of the test's own with the quantum forced to 64, 128,
//! 256, 512 and 1024 frames in turn (the PipeWire of the person using the PC is never touched).
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber`, `pw-link` and the ALSA
//! sequencer. `MELOET_TIMING_QUANTA=64,256` and `MELOET_TIMING_BEATS=64` narrow it.
//! See `tools/measure/record-timing.sh`.

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::process::Command;
use std::time::Duration;

use meloet_core::{Core, CoreOptions};
use meloet_midi::vkeys::VirtualKeyboard;
use record_timing::{Outcome, Run, stats};

fn env_list(name: &str, default: &[u32]) -> Vec<u32> {
    std::env::var(name).map_or_else(
        |_| default.to_vec(),
        |v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
    )
}

fn force_quantum(pw: &private_pw::PrivatePipeWire, quantum: u32) {
    let remote = pw.dir().join("pipewire-0");
    let pw_metadata = |args: &[&str]| {
        let out = Command::new("pw-metadata")
            .env_remove("PIPEWIRE_REMOTE")
            .env("PIPEWIRE_RUNTIME_DIR", pw.dir())
            .arg("-r")
            .arg(&remote)
            .args(["-n", "settings", "0"])
            .args(args)
            .output()
            .expect("pw-metadata runs");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    pw_metadata(&["clock.force-quantum", &quantum.to_string()]);
    let now = pw_metadata(&[]);
    assert!(
        now.contains(&format!("key:'clock.force-quantum' value:'{quantum}'")),
        "the private daemon took no quantum: {now}"
    );
}

#[test]
#[ignore = "starts a PipeWire daemon of its own and plays for minutes"]
fn recorded_keys_land_where_they_were_heard_at_every_quantum() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let (name, midi) = record_timing::private_keyboard("private");
    let Ok(keys) = VirtualKeyboard::open(&name) else {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
        return;
    };
    let quanta = env_list("MELOET_TIMING_QUANTA", &[64, 128, 256, 512, 1024]);
    let beats = env_list("MELOET_TIMING_BEATS", &[64])[0];
    let config = pw.dir().join("config-meloet");
    record_timing::write_settings(&config, private_pw::SINK);

    let mut outcomes: Vec<Outcome> = Vec::new();
    for quantum in quanta {
        // As the PC of the person using it does it (`clock.force-quantum` in the `settings`
        // metadata), but on the private daemon only: the remote is its socket, by full path.
        force_quantum(&pw, quantum);

        let core = Core::start(CoreOptions {
            config_dir: Some(config.clone()),
            measure_dir: None,
            midi: Some(midi.clone()),
            devices: true,
            audio: true,
            songs_dir: None,
            state_dir: None,
            sounds_dir: None,
            plugins: None,
        });
        let latest = record_timing::listen(&core);
        record_timing::wait_for(&core, "the output runs", Duration::from_secs(10), |s| {
            s.audio.running && s.audio.actual_frames.is_some()
        });
        record_timing::wait_for(&core, "the keyboard is seen", Duration::from_secs(3), |s| {
            s.keyboards.list.iter().any(|k| k.name == name)
        });
        let frames = core.snapshot().state.audio.actual_frames;
        assert_eq!(
            frames,
            Some(quantum),
            "the stream runs at the fixed quantum"
        );

        for bpm in [96.0, 137.5] {
            for late_ms in [0.0, 5.0] {
                let label = format!("private q{quantum} {bpm} +{late_ms}ms");
                let o = record_timing::record(
                    &core,
                    &latest,
                    &keys,
                    Run {
                        label,
                        bpm,
                        beats,
                        late_ms,
                    },
                );
                eprintln!("{}", o.table_row());
                outcomes.push(o);
            }
        }
        core.stop();
    }

    record_timing::write(
        "private",
        &outcomes,
        "Private PipeWire (null sink, quantum set by clock.force-quantum), virtual keyboard \
         through the ALSA sequencer. Error = recorded tick − (beat + dither + late). \
         t = tick.",
    );
    for o in &outcomes {
        record_timing::check(o);
    }
    // The late runs sit 5 ms behind the runs on time with the same quantum and tempo.
    for pair in outcomes.chunks(2) {
        let [on_time, late] = pair else { continue };
        let shift =
            stats(&late.offsets_from_beat_ms()).mean - stats(&on_time.offsets_from_beat_ms()).mean;
        assert!(
            (shift - 5.0).abs() < 0.5,
            "{}: the late keys moved {shift} ms",
            late.run.label
        );
    }
}
