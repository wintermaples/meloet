//! Where recorded keys land, on the PipeWire of the person using the PC, at whatever quantum it
//! runs now (read, never changed). The metronome sounds through `MELOET_TIMING_OUTPUT` (an
//! output id as `state.outputs.list` gives it); without it, the test lists the outputs and
//! fails. Run with `-- --ignored --nocapture`. See `tools/measure/record-timing.sh`.

mod record_timing;

use std::time::Duration;

use meloet_core::{Core, CoreOptions};
use meloet_midi::vkeys::VirtualKeyboard;
use record_timing::Run;

#[test]
#[ignore = "plays the metronome through a real output"]
fn recorded_keys_land_where_they_were_heard_on_the_real_output() {
    let (name, midi) = record_timing::private_keyboard("live");
    let Ok(keys) = VirtualKeyboard::open(&name) else {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
        return;
    };
    let config = std::env::temp_dir().join(format!("meloet-timing-live-{}", std::process::id()));
    let Ok(output) = std::env::var("MELOET_TIMING_OUTPUT") else {
        let core = Core::start(CoreOptions {
            config_dir: None,
            measure_dir: None,
            midi: None,
            devices: true,
            audio: false,
            songs_dir: None,
            state_dir: None,
            sounds_dir: None,
            plugins: None,
            sf2_dir: None,
        });
        std::thread::sleep(Duration::from_secs(2));
        for o in core.snapshot().state.outputs.list {
            eprintln!("{:?} {:?} unusable={:?}", o.id, o.name, o.unusable);
        }
        core.stop();
        panic!("set MELOET_TIMING_OUTPUT to one of the ids above");
    };
    let bpm: f64 = std::env::var("MELOET_TIMING_BPM")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(137.5);
    let beats: u32 = std::env::var("MELOET_TIMING_BEATS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64);
    record_timing::write_settings(&config, &output);

    let core = Core::start(CoreOptions {
        config_dir: Some(config.clone()),
        measure_dir: None,
        midi: Some(midi),
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
    let audio = core.snapshot().state.audio;
    eprintln!(
        "output {:?}: {} frames running, forced quantum {:?}, {} Hz",
        audio.output_device,
        audio.actual_frames.unwrap_or(0),
        audio.forced_quantum,
        audio.sample_rate.unwrap_or(0)
    );
    let o = record_timing::record(
        &core,
        &latest,
        &keys,
        Run {
            label: format!("live {bpm} +0ms"),
            bpm: Some(bpm),
            beats,
            late_ms: 0.0,
        },
    );
    core.stop();
    let _ = std::fs::remove_dir_all(&config);
    record_timing::write(
        "live",
        std::slice::from_ref(&o),
        &format!(
            "The PipeWire of the person using the PC, output {output}, quantum as it runs. \
             Error = recorded tick − (beat + dither). t = tick."
        ),
    );
    record_timing::check(&o);
}
