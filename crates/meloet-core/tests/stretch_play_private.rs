//! A clip that follows the song's tempo, through the whole core, on a PipeWire of the
//! test's own (docs/architecture.md 22-8, 22-19 pass 2). The sink's monitor is the input, so a
//! vocal track records what meloet plays.
//!
//! The metronome is recorded onto a vocal track at 120: nothing is made for it. Then the
//! song goes to 96: the audio of the clip is made, and while a second vocal track records,
//! the clip plays what was made. The clicks must lie in the second file on the beats of the
//! song at 96. Back at 120 the clip plays its file again and nothing more is made.
//!
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
use meloet_protocol::{
    AppState, AudioJobKind, Command, StretchChange, TrackKindView, TransportState,
};
use record_timing::wait_for;

fn settings(dir: &Path, buffer: u32) {
    std::fs::create_dir_all(dir).unwrap();
    let sink = private_pw::SINK;
    std::fs::write(
        dir.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{sink}", "output_name": "{sink}", "scale": 100,
                 "metronome": true, "count_in_bars": 1, "buffer": {buffer}, "record_bits": 32 }}"#
        ),
    )
    .unwrap();
}

/// A mono wav file as `vocalrec` writes it with 32 bits.
fn read_wav(path: &Path) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(u16::from_le_bytes([bytes[22], bytes[23]]), 1);
    assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 32);
    (bytes[44..].as_chunks::<4>().0.iter())
        .map(|f| f32::from_le_bytes(*f))
        .collect()
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

/// Records from the head for `ms`, after a bar of count-in.
fn record(core: &Core, clips: usize, ms: u64) {
    core.send(Command::GoToStart);
    std::thread::sleep(Duration::from_millis(300));
    core.send(Command::Record);
    wait_for(core, "it records", Duration::from_secs(10), |s| {
        s.transport.state == TransportState::Recording && !s.transport.recording_vocals.is_empty()
    });
    std::thread::sleep(Duration::from_millis(ms));
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

/// Where the first `beats` clicks lie in the file of the take with this number, less
/// where their beats are (`beat_frames` apart), in frames: where the click gets loud, and
/// the middle of its energy.
fn clicks(
    state: &Path,
    takes: usize,
    index: usize,
    beat_frames: f64,
    beats: usize,
) -> (Vec<f64>, Vec<f64>) {
    let song = kept_song(state, takes);
    let take = &song["audioTakes"][index];
    let first_frame = take["firstFrame"].as_u64().unwrap();
    let audio = take["audio"].as_u64().unwrap();
    let source = (song["song"]["audio"].as_array().unwrap().iter())
        .find(|a| a["id"].as_u64() == Some(audio))
        .unwrap_or_else(|| panic!("no audio {audio}"));
    let heard = read_wav(&state.join("work/音").join(source["file"].as_str().unwrap()));
    let found = (0..beats).map(|beat| {
        let expected = first_frame as f64 + beat as f64 * beat_frames;
        let from = (expected as usize).saturating_sub(4_000);
        let window = &heard[from..(from + 12_000).min(heard.len())];
        let top = window.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(top > 0.05, "beat {beat}: no click near its beat ({top})");
        let onset = window.iter().position(|s| s.abs() > top * 0.25).unwrap();
        let (mut weight, mut moment) = (0.0f64, 0.0f64);
        for (i, s) in window.iter().enumerate() {
            let e = f64::from(*s) * f64::from(*s);
            weight += e;
            moment += e * i as f64;
        }
        let at = |i: f64| from as f64 + i - expected;
        (at(onset as f64), at(moment / weight))
    });
    found.unzip()
}

fn making(state: &AppState) -> usize {
    (state.audio_work.jobs.iter())
        .filter(|job| job.kind == AudioJobKind::Rendering)
        .count()
}

fn made_files(cache: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(cache.join("stretch"))
        .map(|dir| dir.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn ratio(state: &AppState) -> Option<f64> {
    (state.song.tracks.iter())
        .flat_map(|t| &t.clips)
        .find_map(|c| c.audio.as_ref())
        .and_then(|a| a.stretch.ratio)
}

/// `None`: the input skipped while one of the two was recorded (the PC was busy), so the
/// run says nothing.
fn follow_the_tempo(root: &Path, buffer: u32, attempt: u32) -> Option<(Vec<f64>, Vec<f64>)> {
    let tag = format!("b{buffer}-{attempt}");
    let (config, state, cache) = (
        root.join(format!("config-{tag}")),
        root.join(format!("state-{tag}")),
        root.join(format!("cache-{tag}")),
    );
    settings(&config, buffer);
    let core = Core::start(CoreOptions {
        config_dir: Some(config),
        measure_dir: None,
        midi: None,
        devices: true,
        audio: true,
        songs_dir: None,
        state_dir: Some(state.clone()),
        cache_dir: Some(cache.clone()),
        sounds_dir: None,
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

    // The metronome at 120, onto the first vocal track: 6 beats and a little.
    record(&core, 1, 3_300);
    let mut faults = core.snapshot().state.transport.recording_faults;
    let (onsets_before, middles_before) = clicks(&state, 1, 0, 24_000.0, 6);
    // Recorded audio in the tempo it was recorded in: nothing is made.
    std::thread::sleep(Duration::from_millis(600));
    let now = core.snapshot().state;
    assert_eq!((making(&now), ratio(&now)), (0, Some(1.0)));
    assert!(made_files(&cache).is_empty());

    // The song slows down to 96. The clip follows: its audio is made.
    core.send(Command::ToggleMetronome);
    core.send(Command::SetTempo { bpm: 96.0 });
    wait_for(
        &core,
        "the audio is being made",
        Duration::from_secs(5),
        |s| making(s) == 1,
    );
    assert_eq!(ratio(&core.snapshot().state), Some(0.8));
    let asked = Instant::now();
    wait_for(&core, "the audio is made", Duration::from_secs(30), |s| {
        making(s) == 0
    });
    eprintln!(
        "buffer {buffer}: made in {:?} (the wait before it starts in it)",
        asked.elapsed()
    );
    let made = made_files(&cache);
    assert_eq!(made.len(), 1, "{made:?}");
    assert!(made[0].extension().is_some_and(|e| e == "wav"));

    // The clip, onto a second vocal track, at 96: its six beats take 3.75 s now.
    let first = vocal_tracks(&core.snapshot().state)[0].0;
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
    record(&core, 2, 4_200);
    faults = faults.or(core.snapshot().state.transport.recording_faults);
    let (onsets, middles) = clicks(&state, 2, 1, 30_000.0, 6);

    // Back at 120 the first clip is where it was recorded and plays its file. The second,
    // recorded at 96, is the one that is stretched now: one more file.
    core.send(Command::SetTempo { bpm: 120.0 });
    wait_for(
        &core,
        "the second clip is being made",
        Duration::from_secs(5),
        |s| making(s) == 1,
    );
    wait_for(
        &core,
        "the second clip is made",
        Duration::from_secs(30),
        |s| making(s) == 0,
    );
    assert_eq!(ratio(&core.snapshot().state), Some(1.0));
    let mut both = made_files(&cache);
    both.sort();
    assert_eq!(both.len(), 2);
    assert!(both.contains(&made[0]));
    // And at 96 again what was made for the first is still there: nothing is made.
    core.send(Command::Undo);
    std::thread::sleep(Duration::from_millis(800));
    let now = core.snapshot().state;
    assert_eq!((making(&now), ratio(&now)), (0, Some(0.8)));
    let mut after = made_files(&cache);
    after.sort();
    assert_eq!(after, both);
    let written = std::fs::metadata(&made[0]).unwrap().len();
    assert!(written > 44 + 4 * 150_000, "{written} bytes");

    // While the song plays: another pitch for the first clip. What is made for it comes in
    // over what sounds, and with the undo what was there before does, without the track's
    // ring being filled anew.
    let clip = (now.song.tracks.iter())
        .flat_map(|t| &t.clips)
        .find(|c| c.audio.is_some())
        .map(|c| c.id)
        .unwrap();
    let counts = core.made_counts();
    assert_eq!((counts.waiting, counts.playing), (0, 1));
    core.send(Command::GoToStart);
    core.send(Command::PlayStop);
    wait_for(&core, "it plays", Duration::from_secs(5), |s| {
        s.transport.state == TransportState::Playing
    });
    let laid_over = |more: u64, what: &str| {
        let until = Instant::now() + Duration::from_secs(8);
        loop {
            let now = core.made_counts();
            if now.splices >= counts.splices + more && now.waiting == 0 && now.playing == 1 {
                break;
            }
            assert!(Instant::now() < until, "{what}: {now:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let change = StretchChange::Semitones { semitones: 2.0 };
    core.send(Command::SetClipStretch { clip, change });
    laid_over(1, "the pitched audio comes in");
    assert_eq!(made_files(&cache).len(), 3);
    core.send(Command::Undo);
    laid_over(2, "the audio from before comes back");
    assert_eq!(made_files(&cache).len(), 3);
    assert_eq!(
        core.snapshot().state.transport.state,
        TransportState::Playing
    );
    core.send(Command::PlayStop);
    drop(core);
    if let Some(faults) = faults {
        eprintln!("buffer {buffer}: the input skipped ({faults:?}); once more");
        return None;
    }
    eprintln!(
        "buffer {buffer}: clicks less their beats in frames. Recorded at 120: where they get \
         loud {onsets_before:.0?}, their middles {middles_before:.0?}. Played at 96: {onsets:.0?}, \
         {middles:.0?}"
    );
    let moved = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a - b).collect::<Vec<f64>>();
    Some((
        moved(&onsets, &onsets_before),
        moved(&middles, &middles_before),
    ))
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn a_clip_that_follows_a_slower_tempo_has_its_clicks_on_the_new_beats() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    // SAFETY: no core runs yet, so no other thread reads the environment.
    unsafe { std::env::set_var("MELOET_INPUT_SINK_MONITOR", private_pw::SINK) };
    let root = pw.dir().join("stretch-play");
    for buffer in [256, 1024] {
        let (onsets, middles) = (0..5)
            .find_map(|attempt| follow_the_tempo(&root, buffer, attempt))
            .expect("a run in which the input did not skip");
        eprintln!(
            "buffer {buffer}: the clicks moved from their beats by {onsets:.0?} frames where \
             they get loud, {middles:.0?} in their middles"
        );
        // 1 ms.
        for (beat, moved) in middles.iter().enumerate() {
            assert!(
                moved.abs() <= 48.0,
                "buffer {buffer}, beat {beat}: {moved} frames"
            );
        }
    }
}
