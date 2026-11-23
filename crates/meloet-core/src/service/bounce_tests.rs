//! 「音のクリップにする」 (round 12, A7) through the real threads: the export's job renders
//! the track, the thread that reads audio files in takes the file, the rules make the step.

use std::path::Path;

use meloet_protocol::{AppState, StatusMessage};

use super::*;
use crate::settings::temp_dir;

fn wait(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let state = core.snapshot().state;
        if done(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "{what}: {:?}", state.status);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The names of the files in `dir`, sorted.
fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = (std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_jobs_of_a_bounce_are_apart_from_the_exports_and_the_ai_links() {
    const { assert!(crate::state::BOUNCE_JOBS > 1 << 32) };
    const { assert!(crate::state::BOUNCE_JOBS < super::export::AI_JOBS) };
}

#[test]
fn a_track_turned_into_audio_goes_into_the_song_and_a_cancel_leaves_no_file() {
    let root = temp_dir("bounce-core");
    let state_dir = root.join("state");
    let core = Core::start(CoreOptions {
        state_dir: Some(state_dir.clone()),
        cache_dir: Some(root.join("cache")),
        ..CoreOptions::offline()
    });
    let work = crate::vocalrec::work_audio_dir(&state_dir);
    core.send(Command::CreateClip { track: 1, tick: 0 });
    core.send(Command::BounceTrack { track: 1 });
    let state = wait(&core, "the audio is in", |s| s.song.tracks.len() == 2);
    let new = &state.song.tracks[1];
    assert_eq!(new.name, format!("{}（音）", state.song.tracks[0].name));
    assert!(state.song.tracks[0].muted);
    assert!(state.audio_work.bounce.is_none());
    assert!(matches!(
        state.status.message,
        Some(StatusMessage::Bounced { .. })
    ));
    let file = new.clips[0].audio.as_ref().unwrap().file.clone();
    assert_eq!(
        files(&work),
        std::slice::from_ref(&file),
        "only the file of the song"
    );

    // Undo leaves the file in the work place, as a recording's.
    core.send(Command::Undo);
    wait(&core, "undone", |s| s.song.tracks.len() == 1);
    assert_eq!(files(&work), std::slice::from_ref(&file));

    // Cancelled at once: no step, and nothing more in the work place once the job ended.
    core.send(Command::BounceTrack { track: 1 });
    core.send(Command::CancelBounce);
    let state = wait(&core, "cancelled", |s| s.audio_work.bounce.is_none());
    assert_eq!(state.status.message, Some(StatusMessage::BounceCancelled));
    let export = state_dir.join("export");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !files(&export).is_empty() {
        assert!(Instant::now() < deadline, "the job ends");
        std::thread::sleep(Duration::from_millis(20));
    }
    // A last look a moment later: nothing came in after the job ended.
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(files(&work), [file]);
    assert_eq!(core.snapshot().state.song.tracks.len(), 1);
    core.stop();
    let _ = std::fs::remove_dir_all(root);
}

/// How often the left channel crosses zero upwards, a second at a time, over the first
/// `seconds`, and how loud it is.
fn pitch_and_level(samples: &[f32], rate: u32, seconds: usize) -> (f32, f32) {
    let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
    let part = &left[..(seconds * rate as usize).min(left.len())];
    let ups = (part.windows(2))
        .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
        .count();
    let rms = (part.iter().map(|v| v * v).sum::<f32>() / part.len().max(1) as f32).sqrt();
    (ups as f32 * rate as f32 / part.len().max(1) as f32, rms)
}

/// The real Surge XT (VST3): the audio is made through the export's own plugin process, with
/// what was changed in the plugin and is not in the song yet.
#[test]
#[ignore = "needs Surge XT (VST3)"]
fn surge_xt_a_track_turned_into_audio_sounds_as_it_plays_now() {
    use super::plug_tests::{SURGE_BUNDLE, plughost, plugin_of, state_when};
    use crate::plugins::hosts::wait as answer;
    use meloet_protocol::{PluginState, ShelfMode, SoundRef};

    assert!(Path::new(SURGE_BUNDLE).is_dir(), "no {SURGE_BUNDLE}");
    let root = plughost::temp_dir("bounce-surge");
    std::fs::create_dir_all(root.join("vst3")).unwrap();
    std::os::unix::fs::symlink(SURGE_BUNDLE, root.join("vst3/Surge XT.vst3")).unwrap();
    let state_dir = root.join("state");
    let core = Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        state_dir: Some(state_dir.clone()),
        cache_dir: Some(root.join("caches")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(root.join("cache")),
            usual_places: vec![root.join("vst3")],
            scan_timeout: Duration::from_secs(30),
            child_env: Vec::new(),
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    });
    let state = state_when(&core, "the scan", |s| {
        s.library.plugins.iter().any(|row| row.name == "Surge XT")
    });
    let row = (state.library.plugins.iter())
        .find(|row| row.name == "Surge XT")
        .unwrap();
    let surge = SoundRef::Plugin {
        key: row.key.clone(),
    };
    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    core.send(Command::ShelfSelectSound {
        sound: Some(surge.clone()),
    });
    state_when(&core, "Surge XT is read", |s| {
        s.shelf.selected_sound == Some(surge.clone()) && s.shelf.loading_sound.is_none()
    });
    core.send(Command::ShelfConfirm);
    state_when(&core, "on its track", |s| {
        plugin_of(s, 1) == Some(PluginState::Ready)
    });
    let track = core.snapshot().state.song.tracks[1].id;
    core.send(Command::AddNote {
        take: None,
        start: 0,
        length: 4 * 1_920,
        key: 60,
        velocity: 100,
    });
    let work = crate::vocalrec::work_audio_dir(&state_dir);
    let bounce = |count: usize| {
        core.send(Command::BounceTrack { track });
        let state = wait(&core, "the audio is in", |s| s.song.tracks.len() == count);
        assert!(
            matches!(state.status.message, Some(StatusMessage::Bounced { .. })),
            "{:?}",
            state.status
        );
        let new = &state.song.tracks[2];
        let audio = new.clips[0].audio.as_ref().unwrap();
        let samples = crate::export::fixture::read_wav(&work.join(&audio.file));
        pitch_and_level(&samples, audio.sample_rate, 1)
    };
    let (first_hz, first_rms) = bounce(3);
    eprintln!("as it starts: {first_hz} Hz, {first_rms}");
    assert!(first_rms > 0.01, "Surge XT sounds");

    // An octave up, which the song does not have yet.
    let id = core.track_plugin(1);
    core.peek(move |w| {
        let hosts = &w.plugins.as_ref().unwrap().hosts;
        let params = answer(|done| hosts.params(id, done)).unwrap();
        let octave = params.iter().find(|p| p.name == "A Octave").unwrap().id;
        answer(|done| hosts.set_param(id, octave, 1.0, done)).unwrap();
    });
    let (hz, rms) = bounce(4);
    eprintln!("changed: {hz} Hz, {rms}");
    assert!(rms > 0.01);
    assert!(hz > first_hz * 1.5, "{first_hz} {hz}");
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}
