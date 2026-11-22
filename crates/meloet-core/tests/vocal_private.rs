//! Recording the input through the whole core, on a PipeWire of the test's own
//! (docs/architecture.md 22-6, 22-7). The sink's monitor is the input, so what a vocal track
//! records is what meloet plays: the metronome. Each click must then lie in the recorded
//! file where the take says its beat is, at every buffer size.
//!
//! The same run looks at what a recording leaves on the disk while it goes on (as a SIGKILL
//! would leave it): a second core, started on a copy of the state folder taken then, brings
//! the voice back as a clip.
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber` and `pw-link`.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use meloet_core::{Core, CoreOptions};
use meloet_protocol::{AppState, Command, RecoveryView, Telemetry, TrackKindView, TransportState};
use record_timing::wait_for;

const RATE: f64 = 48_000.0;
/// 120: a beat is half a second, 24000 frames.
const BEAT_FRAMES: f64 = 24_000.0;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;

fn settings(dir: &Path, buffer: u32, bits: u8) {
    std::fs::create_dir_all(dir).unwrap();
    let sink = private_pw::SINK;
    std::fs::write(
        dir.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{sink}", "output_name": "{sink}", "scale": 100,
                 "metronome": true, "count_in_bars": 1, "buffer": {buffer}, "record_bits": {bits} }}"#
        ),
    )
    .unwrap();
}

fn start(config: &Path, state: &Path, audio: bool) -> Core {
    Core::start(CoreOptions {
        config_dir: Some(config.to_owned()),
        measure_dir: None,
        midi: None,
        devices: audio,
        audio,
        songs_dir: None,
        state_dir: Some(state.to_owned()),
        sounds_dir: None,
        plugins: None,
        sf2_dir: None,
        cache_dir: None,
    })
}

fn vocal_clips(state: &AppState) -> usize {
    (state.song.tracks.iter())
        .filter(|t| matches!(t.kind, TrackKindView::Vocal { .. }))
        .map(|t| t.clips.len())
        .sum()
}

/// A wav file as `vocalrec` writes it: (channels, the first channel's samples).
fn read_wav(path: &Path) -> (u16, Vec<f32>) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
    let frame = usize::from(bits / 8) * usize::from(channels);
    let first = (bytes[44..].chunks_exact(frame))
        .map(|f| match bits {
            16 => f32::from(i16::from_le_bytes([f[0], f[1]])) / 32_768.0,
            24 => (i32::from_le_bytes([0, f[0], f[1], f[2]]) >> 8) as f32 / 8_388_608.0,
            _ => f32::from_le_bytes([f[0], f[1], f[2], f[3]]),
        })
        .collect();
    (channels, first)
}

/// The song inside the recovery file, once it holds a vocal take.
fn kept_song(state: &Path) -> serde_json::Value {
    let file = state.join("recovery.json");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let song = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .map(|whole| whole["song"].clone())
            .filter(|song| song["audioTakes"].as_array().is_some_and(|t| !t.is_empty()));
        if let Some(song) = song {
            return song;
        }
        assert!(Instant::now() < until, "no recovery file with a vocal take");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Run {
    /// Where each click lies, less where its beat is, in frames.
    errors: Vec<f64>,
    first_frame: u64,
    /// The state folder as it was two seconds into the recording.
    killed: PathBuf,
}

fn record_the_metronome(root: &Path, buffer: u32, bits: u8) -> Run {
    let tag = format!("b{buffer}");
    let (config, state) = (
        root.join(format!("config-{tag}")),
        root.join(format!("state-{tag}")),
    );
    settings(&config, buffer, bits);
    let core = start(&config, &state, true);
    let mics = std::sync::Arc::new(std::sync::Mutex::new(0usize));
    let seen = mics.clone();
    core.subscribe(
        Box::new(|_| true),
        Box::new(move |bytes| {
            if let Ok(t) = Telemetry::decode(bytes) {
                let mut most = seen.lock().unwrap();
                *most = (*most).max(t.mics.len());
            }
            true
        }),
    );
    wait_for(&core, "the output runs", Duration::from_secs(10), |s| {
        s.audio.running && s.audio.actual_frames.is_some()
    });
    let audio = core.snapshot().state.audio;
    assert_eq!(audio.requested_frames, buffer);
    assert_eq!(
        audio.actual_frames,
        Some(buffer),
        "the private daemon gives what is asked"
    );
    core.send(Command::SetTempo { bpm: 120.0 });
    core.send(Command::AddVocalTrack);
    wait_for(&core, "the input opens", Duration::from_secs(10), |s| {
        s.inputs.open
            && (s.song.tracks.iter()).any(|t| {
                matches!(
                    t.kind,
                    TrackKindView::Vocal {
                        input_ready: true,
                        armed: true,
                        ..
                    }
                )
            })
    });
    wait_for(
        &core,
        "the PC's delays are read",
        Duration::from_secs(5),
        |s| s.audio.latency.reported_ms.is_some(),
    );
    let reported = core.snapshot().state.audio.latency.reported_ms.unwrap();
    // The null sink and its monitor have no delay of their own: the round trip is the block
    // PipeWire says the output's stream takes, and the block the input's frames are behind.
    let block_ms = buffer as f32 / 48.0;
    assert!(
        (reported - 2.0 * block_ms).abs() < 0.2,
        "{reported} ms for a block of {block_ms} ms"
    );
    // The streams run before WirePlumber has linked them.
    std::thread::sleep(Duration::from_millis(1_000));

    core.send(Command::Record);
    wait_for(&core, "it records", Duration::from_secs(10), |s| {
        s.transport.state == TransportState::Recording && !s.transport.recording_vocals.is_empty()
    });
    std::thread::sleep(Duration::from_millis(2_000));
    // What a SIGKILL now would leave: the file with its head and what was written, and the
    // note about the recording.
    let killed = root.join(format!("killed-{tag}"));
    copy_dir(&state, &killed);
    std::thread::sleep(Duration::from_millis(1_300));
    core.send(Command::PlayStop);
    wait_for(
        &core,
        "the voice is in the song",
        Duration::from_secs(10),
        |s| vocal_clips(s) == 1 && s.transport.recording_vocals.is_empty(),
    );
    assert!(*mics.lock().unwrap() == 1, "the mic meter ran");
    assert!(!state.join("work/rec.json").exists(), "the note is gone");

    let song = kept_song(&state);
    let take = &song["audioTakes"][0];
    let (first_frame, frames) = (
        take["firstFrame"].as_u64().unwrap(),
        take["frames"].as_u64().unwrap(),
    );
    let source = &song["song"]["audio"][0];
    let name = source["file"].as_str().unwrap();
    assert_eq!(source["rate"].as_u64(), Some(48_000));
    let (channels, heard) = read_wav(&state.join("work/音").join(name));
    assert_eq!(channels, 1);
    assert_eq!(source["frames"].as_u64(), Some(heard.len() as u64));
    assert!(first_frame + frames <= heard.len() as u64);
    // The recording started at the head (tick 0) after a bar of count-in: the take's tick 0
    // is its first beat, and a click is there and at every beat after it.
    let beats = (frames as f64 / BEAT_FRAMES).floor() as usize;
    assert!(beats >= 5, "{beats} beats in {frames} frames");
    let errors: Vec<f64> = (0..beats)
        .map(|beat| {
            let expected = first_frame as f64 + beat as f64 * BEAT_FRAMES;
            let from = (expected as usize).saturating_sub(4_000);
            let onset = (heard[from..].iter())
                .position(|s| s.abs() > 0.02)
                .map(|i| from + i);
            onset.map_or(f64::NAN, |at| at as f64 - expected)
        })
        .collect();
    drop(core);
    Run {
        errors,
        first_frame,
        killed,
    }
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn the_voice_lies_where_it_was_heard_and_survives_the_end_of_meloet() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    // SAFETY: no core runs yet, so no other thread reads the environment.
    unsafe { std::env::set_var("MELOET_INPUT_SINK_MONITOR", private_pw::SINK) };
    let root = pw.dir().join("vocal");
    let mut killed = None;
    for (buffer, bits) in [(64, 16), (256, 24), (1024, 32)] {
        let run = record_the_metronome(&root, buffer, bits);
        eprintln!(
            "buffer {buffer}, {bits} bits: first frame {}, clicks less their beats in frames {:?}",
            run.first_frame, run.errors
        );
        // The file begins half a beat before the song rolls, to the block.
        let early = run.first_frame as f64 - BEAT_FRAMES / 2.0;
        assert!(
            early.abs() <= f64::from(buffer) + 1.0,
            "{early} frames from half a beat"
        );
        // On a busy PC the output may miss cycles and the recording leaves out what was
        // sung then (docs/architecture.md 22-26): a click whose head fell there is found
        // late, alone, and the beats on each side are where they belong.
        let on = |beat: Option<&f64>| beat.is_none_or(|e| e.abs() <= 1.0);
        for (beat, error) in run.errors.iter().enumerate() {
            let alone = error.abs() < 1_000.0
                && on(beat.checked_sub(1).and_then(|b| run.errors.get(b)))
                && on(run.errors.get(beat + 1));
            assert!(
                error.abs() <= 1.0 || alone,
                "buffer {buffer}, beat {beat}: {error} frames"
            );
        }
        killed.get_or_insert(run.killed);
    }

    // The next start after meloet ended in the middle of a recording.
    let state = killed.unwrap();
    let note = std::fs::read(state.join("work/rec.json")).expect("the note was there");
    let note: serde_json::Value = serde_json::from_slice(&note).unwrap();
    let name = note["files"][0]["file"].as_str().unwrap().to_owned();
    assert!(
        note["marks"]["roll"].is_array(),
        "the head was placed: {note}"
    );
    let wav = state.join("work/音").join(&name);
    let unfinished = std::fs::read(&wav).unwrap();
    assert_eq!(
        unfinished[40..44],
        [0xFF; 4],
        "the length was not known yet"
    );
    let written = (unfinished.len() - 44) / 2;
    assert!(written > 48_000, "{written} frames were on the disk");

    let core = start(&root.join("config-b64"), &state, false);
    wait_for(
        &core,
        "the start looks at the state folder",
        Duration::from_secs(5),
        |s| s.workspace.recovery.is_some(),
    );
    let view = core.snapshot().state;
    let Some(RecoveryView::Recovered {
        vocals: Some(kept), ..
    }) = view.workspace.recovery
    else {
        panic!("{:?}", view.workspace.recovery);
    };
    assert_eq!(kept.count, 1);
    assert_eq!(vocal_clips(&view), 1);
    let clip = (view.song.tracks.iter())
        .flat_map(|t| &t.clips)
        .find(|c| c.audio.is_some())
        .unwrap();
    assert_eq!((clip.start, clip.end), (0, kept.until));
    // Two seconds were recorded when the copy was taken: about four beats.
    assert!((3 * Q..=5 * Q).contains(&kept.until), "{}", kept.until);
    // The note goes once the recovery file holds the clip, which is written at once.
    let until = Instant::now() + Duration::from_secs(5);
    while state.join("work/rec.json").exists() {
        assert!(Instant::now() < until, "the note is still there");
        std::thread::sleep(Duration::from_millis(20));
    }
    let kept_file = std::fs::read(state.join("recovery.json")).unwrap();
    let kept_file: serde_json::Value = serde_json::from_slice(&kept_file).unwrap();
    assert_eq!(
        kept_file["song"]["audioTakes"].as_array().map(Vec::len),
        Some(1)
    );
    let mended = std::fs::read(&wav).unwrap();
    assert_eq!(
        mended[40..44],
        ((written * 2) as u32).to_le_bytes(),
        "the head is put right"
    );
    drop(core);
    let _ = RATE;
}
