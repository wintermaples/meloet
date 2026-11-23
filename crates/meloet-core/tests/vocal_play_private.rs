//! Playing a vocal clip through the whole core, on a PipeWire of the test's own
//! (docs/architecture.md 22-8, 22-18 A1). The sink's monitor is the input, so a vocal track
//! records what meloet plays.
//!
//! First the metronome is recorded onto a vocal track. Then the metronome is turned off and
//! that clip is played while a second vocal track records: what it records is the clip as
//! the engine sounds it. Each click must lie in the second file where its beat is, as it did
//! in the first: the clip sounds on the frames it was recorded on, at every buffer size.
//!
//! The read-ahead thread opens the first file through a store of the test's own.
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber` and `pw-link`.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_core::pcm::{PcmFile, PcmStore};
use meloet_core::{Core, CoreOptions};
use meloet_protocol::{AppState, Command, TrackKindView, TransportState};
use record_timing::wait_for;

/// 120: a beat is half a second, 24000 frames.
const BEAT_FRAMES: f64 = 24_000.0;

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

/// A wav file as `vocalrec` writes it: (channels, interleaved samples).
fn read_wav(path: &Path) -> (u16, Vec<f32>) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
    let samples = (bytes[44..].chunks_exact(usize::from(bits / 8)))
        .map(|f| match bits {
            16 => f32::from(i16::from_le_bytes([f[0], f[1]])) / 32_768.0,
            24 => (i32::from_le_bytes([0, f[0], f[1], f[2]]) >> 8) as f32 / 8_388_608.0,
            _ => f32::from_le_bytes([f[0], f[1], f[2], f[3]]),
        })
        .collect();
    (channels, samples)
}

/// The store of the test: whatever audio is asked for, it is this one wav file, read whole.
struct OneWav(PathBuf);

struct Held {
    channels: u16,
    samples: Vec<f32>,
}

impl PcmFile for Held {
    fn channels(&self) -> u8 {
        self.channels as u8
    }
    fn frames(&self) -> u64 {
        (self.samples.len() / usize::from(self.channels)) as u64
    }
    fn read(&mut self, frame: u64, out: &mut [f32]) -> io::Result<usize> {
        let channels = usize::from(self.channels);
        let from = (frame as usize * channels).min(self.samples.len());
        let count = (self.samples.len() - from).min(out.len()) / channels;
        out[..count * channels].copy_from_slice(&self.samples[from..from + count * channels]);
        Ok(count)
    }
}

impl PcmStore for OneWav {
    fn open(&self, _: &[u8; 16], sample_rate: u32) -> Option<Box<dyn PcmFile>> {
        assert_eq!(sample_rate, 48_000);
        let (channels, samples) = read_wav(&self.0);
        Some(Box::new(Held { channels, samples }))
    }
}

fn vocal_tracks(state: &AppState) -> Vec<(u32, bool, usize)> {
    (state.song.tracks.iter())
        .filter_map(|t| match t.kind {
            TrackKindView::Vocal { armed, .. } => Some((t.id, armed, t.clips.len())),
            _ => None,
        })
        .collect()
}

/// The song inside the recovery file, once it holds `takes` vocal takes.
fn kept_song(state: &Path, takes: usize) -> serde_json::Value {
    let file = state.join("recovery.json");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let song = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .map(|whole| whole["song"].clone())
            .filter(|song| {
                song["audioTakes"]
                    .as_array()
                    .is_some_and(|t| t.len() == takes)
            });
        if let Some(song) = song {
            return song;
        }
        assert!(
            Instant::now() < until,
            "no recovery file with {takes} takes"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Records for a little over three seconds, from the head, after a bar of count-in.
fn record(core: &Core, clips: usize) {
    core.send(Command::GoToStart);
    std::thread::sleep(Duration::from_millis(300));
    core.send(Command::Record);
    wait_for(core, "it records", Duration::from_secs(10), |s| {
        s.transport.state == TransportState::Recording && !s.transport.recording_vocals.is_empty()
    });
    std::thread::sleep(Duration::from_millis(3_300));
    core.send(Command::PlayStop);
    wait_for(
        core,
        "the voice is in the song",
        Duration::from_secs(10),
        |s| {
            vocal_tracks(s).iter().map(|t| t.2).sum::<usize>() == clips
                && s.transport.recording_vocals.is_empty()
        },
    );
}

/// Where each click lies in the file of the take with this number, less where its beat
/// is, in frames; and how loud the loudest sample of the take is.
fn clicks(state: &Path, takes: usize, index: usize) -> (PathBuf, Vec<f64>, f32) {
    let song = kept_song(state, takes);
    let take = &song["audioTakes"][index];
    let (first_frame, frames) = (
        take["firstFrame"].as_u64().unwrap(),
        take["frames"].as_u64().unwrap(),
    );
    let audio = take["audio"].as_u64().unwrap();
    let source = (song["song"]["audio"].as_array().unwrap().iter())
        .find(|a| a["id"].as_u64() == Some(audio))
        .unwrap_or_else(|| panic!("no audio {audio}"));
    let path = state.join("work/音").join(source["file"].as_str().unwrap());
    let (channels, heard) = read_wav(&path);
    assert_eq!(channels, 1);
    assert!(first_frame + frames <= heard.len() as u64);
    let beats = (frames as f64 / BEAT_FRAMES).floor() as usize;
    assert!(beats >= 5, "{beats} beats in {frames} frames");
    let errors = (0..beats)
        .map(|beat| {
            let expected = first_frame as f64 + beat as f64 * BEAT_FRAMES;
            let from = (expected as usize).saturating_sub(4_000);
            let onset = (heard[from..].iter())
                .position(|s| s.abs() > 0.02)
                .map(|i| from + i);
            onset.map_or(f64::NAN, |at| at as f64 - expected)
        })
        .collect();
    let loudest = (heard[first_frame as usize..].iter()).fold(0.0f32, |m, s| m.max(s.abs()));
    (path, errors, loudest)
}

/// `None`: the input skipped while one of the two was recorded (the PC was busy), so the
/// run says nothing.
fn play_what_was_recorded(
    root: &Path,
    buffer: u32,
    bits: u8,
    attempt: u32,
) -> Option<(Vec<f64>, Vec<f64>)> {
    let tag = format!("b{buffer}-{attempt}");
    let (config, state) = (
        root.join(format!("config-{tag}")),
        root.join(format!("state-{tag}")),
    );
    settings(&config, buffer, bits);
    let core = Core::start(CoreOptions {
        config_dir: Some(config),
        measure_dir: None,
        midi: None,
        devices: true,
        audio: true,
        songs_dir: None,
        state_dir: Some(state.clone()),
        cache_dir: Some(root.join("cache")),
        pc_language: meloet_protocol::Language::Ja,
        sounds_dir: None,
        pack: None,
        updates: None,
        plugins: None,
        sf2_dir: None,
    });
    wait_for(&core, "the output runs", Duration::from_secs(10), |s| {
        s.audio.running && s.audio.actual_frames == Some(buffer)
    });
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
    // The streams run before WirePlumber has linked them.
    std::thread::sleep(Duration::from_millis(1_000));

    // The metronome, onto the first vocal track.
    record(&core, 1);
    let mut faults = core.snapshot().state.transport.recording_faults;
    // The wave: once the recording is in the song its peaks come from the peaks file,
    // and no longer say that more is coming.
    let audio = (core.snapshot().state.song.tracks.iter())
        .flat_map(|t| &t.clips)
        .find_map(|c| c.audio.as_ref().map(|a| a.audio))
        .expect("a vocal clip");
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        let peaks = core.peaks(audio, 0, 0, 16).expect("peaks of the recording");
        assert_eq!((peaks[0], peaks[2]), (4, 0));
        if peaks[4] & 1 == 0 {
            assert!(u32::from_le_bytes(peaks[20..24].try_into().unwrap()) > 500);
            break;
        }
        assert!(Instant::now() < until, "the peaks still say they grow");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (first_file, recorded, level) = clicks(&state, 1, 0);
    assert!(level > 0.1, "the metronome was recorded: {level}");

    // That clip, without the metronome, onto a second vocal track.
    // At one buffer size the core's own store opens the file (from the work place); at the
    // others the store of the test does.
    if buffer != 256 {
        core.set_pcm_store(Arc::new(OneWav(first_file)));
    }
    let first = vocal_tracks(&core.snapshot().state)[0].0;
    core.send(Command::ToggleMetronome);
    core.send(Command::AddVocalTrack);
    core.send(Command::ToggleVocalArm { track: first });
    wait_for(
        &core,
        "only the second track records",
        Duration::from_secs(5),
        |s| {
            let tracks = vocal_tracks(s);
            !s.transport.metronome && tracks.len() == 2 && !tracks[0].1 && tracks[1].1
        },
    );
    std::thread::sleep(Duration::from_millis(500));
    record(&core, 2);
    faults = faults.or(core.snapshot().state.transport.recording_faults);
    let (_, played, level) = clicks(&state, 2, 1);
    assert!(level > 0.1, "the clip was heard: {level}");
    drop(core);
    if let Some(faults) = faults {
        eprintln!("buffer {buffer}: the input skipped ({faults:?}); once more");
        return None;
    }
    Some((recorded, played))
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn a_vocal_clip_sounds_on_the_frames_it_was_recorded_on() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    // SAFETY: no core runs yet, so no other thread reads the environment.
    unsafe { std::env::set_var("MELOET_INPUT_SINK_MONITOR", private_pw::SINK) };
    let root = pw.dir().join("vocal-play");
    for (buffer, bits) in [(64, 32), (256, 24), (1024, 16)] {
        let (recorded, played) = (0..5)
            .find_map(|attempt| play_what_was_recorded(&root, buffer, bits, attempt))
            .expect("a run in which the input did not skip");
        eprintln!(
            "buffer {buffer}, {bits} bits: clicks less their beats in frames, \
             recorded {recorded:?}, played {played:?}"
        );
        for (beat, error) in recorded.iter().enumerate() {
            assert!(
                error.abs() <= 1.0,
                "buffer {buffer}, recorded beat {beat}: {error} frames"
            );
        }
        // The second recording is a little shorter or longer than the first; every beat
        // both have must be on its frame.
        assert!(played.len() >= 5);
        for (beat, error) in played.iter().enumerate().take(recorded.len()) {
            assert!(
                error.abs() <= 1.0,
                "buffer {buffer}, played beat {beat}: {error} frames"
            );
        }
        // The peaks of both recordings became peaks files.
        let peaks = root.join("cache/peaks");
        let files = std::fs::read_dir(&peaks).map_or(0, |d| d.flatten().count());
        assert!(files >= 2, "{files} peaks files in {}", peaks.display());
    }
}
