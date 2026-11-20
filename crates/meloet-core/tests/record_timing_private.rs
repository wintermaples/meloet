//! Where recorded keys land, on a PipeWire of the test's own with the quantum forced to 64, 128,
//! 256, 512 and 1024 frames in turn (the PipeWire of the person using the PC is never touched).
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber`, `pw-link` and the ALSA
//! sequencer. `MELOET_TIMING_QUANTA=64,256`, `MELOET_TIMING_BEATS=64` and `MELOET_TIMING_MAPS=0`
//! narrow it; `MELOET_TIMING_TEMPOS=120,30` and `MELOET_TIMING_METER=6/8` change the tempos
//! and the meter of the runs at one tempo. After the runs at one tempo, each quantum records twice on a timeline with tempo
//! points, two ramps and two meter changes.
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

/// The beats pressed on the changing timeline: to the end of its last ramp and six more.
const MAP_BEATS: u32 = 64;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;

/// 4/4 at 96; 104 from bar 5, going in a straight line to 120 at bar 9; 3/4 from bar 9; 6/8
/// from bar 13 (the beat is an eighth note from there); 150 from bar 14, going down to 90 at
/// bar 16. Bars as shown, from 1.
fn set_changing_timeline(core: &Core) {
    use meloet_protocol::Command as C;
    core.send(C::GoToStart);
    core.send(C::SetTempo { bpm: 96.0 });
    for command in [
        C::AddTempoPoint {
            tick: 16 * Q,
            bpm: 104.0,
        },
        C::AddTempoPoint {
            tick: 32 * Q,
            bpm: 120.0,
        },
        C::SetTempoRamp {
            tick: 16 * Q,
            on: true,
        },
        C::AddMeterChange {
            bar: 8,
            num: 3,
            den: 4,
        },
        C::AddMeterChange {
            bar: 12,
            num: 6,
            den: 8,
        },
        C::AddTempoPoint {
            tick: 47 * Q,
            bpm: 150.0,
        },
        C::AddTempoPoint {
            tick: 53 * Q,
            bpm: 90.0,
        },
        C::SetTempoRamp {
            tick: 47 * Q,
            on: true,
        },
    ] {
        core.send(command);
    }
    let t = core.snapshot().state.timeline;
    let points: Vec<(u64, f64, bool)> = (t.tempo_points.iter())
        .map(|p| (p.tick, p.tempo_bpm, p.ramp))
        .collect();
    assert_eq!(
        points,
        [
            (16 * Q, 104.0, true),
            (32 * Q, 120.0, false),
            (47 * Q, 150.0, true),
            (53 * Q, 90.0, false)
        ]
    );
    let flags: Vec<(u32, u8, u8)> = (t.meter_changes.iter())
        .map(|c| (c.bar, c.meter_num, c.meter_den))
        .collect();
    assert_eq!(flags, [(8, 3, 4), (12, 6, 8)]);
    assert_eq!((t.tempo_bpm, t.meter_num, t.meter_den), (96.0, 4, 4));
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
    // `MELOET_TIMING_MAPS=0`: only the runs at one tempo.
    let maps = env_list("MELOET_TIMING_MAPS", &[1])[0] != 0;
    let tempos: Vec<f64> = std::env::var("MELOET_TIMING_TEMPOS").map_or_else(
        |_| vec![96.0, 137.5],
        |v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
    );
    let meter: Option<(u8, u8)> = std::env::var("MELOET_TIMING_METER").ok().map(|v| {
        let (num, den) = v.split_once('/').expect("a meter as 6/8");
        (num.parse().expect("a meter"), den.parse().expect("a meter"))
    });
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
            sf2_dir: None,
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

        let mut meter_label = String::new();
        if let Some((num, den)) = meter {
            core.send(meloet_protocol::Command::SetMeter { num, den });
            let t = core.snapshot().state.timeline;
            assert_eq!((t.meter_num, t.meter_den), (num, den));
            meter_label = format!(" {num}/{den}");
        }
        for &bpm in &tempos {
            for late_ms in [0.0, 5.0] {
                let label = format!("private q{quantum}{meter_label} {bpm} +{late_ms}ms");
                let o = record_timing::record(
                    &core,
                    &latest,
                    &keys,
                    Run {
                        label,
                        bpm: Some(bpm),
                        beats,
                        late_ms,
                    },
                );
                eprintln!("{}", o.table_row());
                outcomes.push(o);
            }
        }
        if maps {
            set_changing_timeline(&core);
            for late_ms in [0.0, 5.0] {
                let label = format!("private q{quantum} map +{late_ms}ms");
                let o = record_timing::record(
                    &core,
                    &latest,
                    &keys,
                    Run {
                        label,
                        bpm: None,
                        beats: MAP_BEATS,
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
        assert_eq!((on_time.run.late_ms, late.run.late_ms), (0.0, 5.0));
        let shift =
            stats(&late.offsets_from_beat_ms()).mean - stats(&on_time.offsets_from_beat_ms()).mean;
        assert!(
            (shift - 5.0).abs() < 0.5,
            "{}: the late keys moved {shift} ms",
            late.run.label
        );
    }
}
