//! Plugins through the whole core, with the real `meloet-plughost` and its fake plugins: on
//! a track, in an insert slot, saved with the song and back when it is opened, stopped,
//! loaded again and taken out.

#[path = "../../tests/plughost/mod.rs"]
pub(super) mod plughost;

use crate::seen::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

use meloet_protocol::{
    AppState, EffectRef, InsertEffectView, InsertRef, InstrumentKind, PluginSlotRef, PluginState,
    ShelfMode, SongFile, SoundRef, SoundView, StatusMessage, StripRef,
};

use super::*;
use crate::plugins::hosts::{ProcessInfo, wait};
use plughost::wait_for;

pub(super) const PATIENCE: Duration = Duration::from_secs(60);
const SINE: &str = "lv2:urn:meloet:fake:sine";
const PASS: &str = "lv2:urn:meloet:fake:pass";
/// The fake sine's one parameter.
const LEVEL: u32 = 0;

pub(super) fn options(root: &Path) -> CoreOptions {
    let bundle = root.join("lv2/found.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(root.join("cache")),
            usual_places: vec![root.join("lv2")],
            scan_timeout: Duration::from_secs(20),
            child_env: plughost::fake_env(),
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    }
}

pub(super) fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s)))
        .unwrap_or_else(|| panic!("{what}: {:#?}", core.snapshot().state.shelf))
}

pub(super) fn plugin_of(state: &AppState, track: usize) -> Option<PluginState> {
    match &state.song.tracks.get(track)?.sound {
        SoundView::Plugin(p) => Some(p.state),
        _ => None,
    }
}

fn master_plugin(state: &AppState) -> Option<PluginState> {
    match &state.song.master.slots[3].as_ref()?.effect {
        InsertEffectView::Plugin(p) => Some(p.state),
        InsertEffectView::Builtin { .. } => None,
    }
}

impl Core {
    /// Runs `look` on the core thread and gives what it gives.
    pub(super) fn peek<T: Send + 'static>(
        &self,
        look: impl FnOnce(&mut Worker) -> T + Send + 'static,
    ) -> T {
        let (tx, rx) = mpsc::channel();
        let look = Box::new(move |worker: &mut Worker| {
            let _ = tx.send(look(worker));
        });
        self.tx.send(Msg::Peek(look)).expect("the core runs");
        rx.recv().expect("the core answers")
    }

    pub(super) fn processes(&self) -> Vec<ProcessInfo> {
        self.peek(|w| w.plugins.as_ref().unwrap().hosts.processes())
    }

    /// The slot of the plugin that is the instrument of the song's track at `index`.
    pub(super) fn track_plugin(&self, index: usize) -> SlotId {
        self.peek(move |w| {
            let track = w.model.view().song.tracks[index].id;
            let place = PluginPlace::Instrument(meloet_model::TrackId(track));
            let tickets = w.model.plugin_tickets();
            let (_, ticket) = tickets.iter().find(|(p, _)| *p == place).expect("loaded");
            w.plugins.as_ref().unwrap().ids[ticket].0
        })
    }

    fn level(&self, id: SlotId) -> f64 {
        self.peek(move |w| {
            let hosts = &w.plugins.as_ref().unwrap().hosts;
            wait(|done| hosts.param(id, LEVEL, done)).unwrap().0
        })
    }
}

pub(super) fn kill(pid: u32) {
    let killed = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status();
    assert!(killed.is_ok_and(|s| s.success()), "kill {pid}");
}

#[test]
fn plugins_on_a_track_and_in_a_slot_are_saved_come_back_stop_and_are_loaded_again() {
    let root = plughost::temp_dir("plug-song");
    let core = Core::start(options(&root));
    let pick: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let (shell_core, shell_pick) = (core.tx.clone(), pick.clone());
    core.set_shell(Box::new(move |request| {
        if let ShellRequest::PickSongFile { .. } = request {
            let path = lock(&shell_pick).clone();
            let _ = shell_core.send(Msg::Input(Input::SongFilePicked(path), None));
        }
    }));
    let state = state_when(&core, "the scan", |s| s.library.plugins.len() == 2);
    let names: Vec<&str> = (state.library.plugins.iter())
        .map(|row| row.name.as_str())
        .collect();
    assert_eq!(names, ["Fake Pass", "Fake Sine"]);

    // A new track with the fake sine: read first, then tried, then the track's own.
    let sine = SoundRef::Plugin { key: SINE.into() };
    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    core.send(Command::ShelfSelectSound {
        sound: Some(sine.clone()),
    });
    state_when(&core, "the sine is read", |s| {
        s.shelf.selected_sound == Some(sine.clone()) && s.shelf.loading_sound.is_none()
    });
    core.send(Command::ShelfConfirm);
    let state = core.snapshot().state;
    assert_eq!(state.song.tracks[1].name, "Fake Sine");
    assert_eq!(plugin_of(&state, 1), Some(PluginState::Ready));

    // The effect on the master.
    let strip = StripRef::Master;
    core.send(Command::OpenShelf {
        mode: ShelfMode::InsertEffect { strip, slot: 3 },
    });
    core.send(Command::ShelfChooseEffect {
        effect: Some(EffectRef::Plugin { key: PASS.into() }),
    });
    core.send(Command::ShelfConfirm);
    state_when(&core, "the effect is read", |s| {
        master_plugin(s) == Some(PluginState::Ready)
    });
    let processes = core.processes();
    assert_eq!(
        processes.len(),
        2,
        "one process for each plugin: {processes:?}"
    );

    // The person turns the sine down (here: over the socket, as the plugin's window would).
    let id = core.track_plugin(1);
    assert_eq!(core.level(id), 0.5);
    core.peek(move |w| {
        let hosts = &w.plugins.as_ref().unwrap().hosts;
        wait(|done| hosts.set_param(id, LEVEL, 0.25, done)).unwrap();
    });

    // Saved: the states the plugins have now are in the file, and the core did not wait.
    core.send(Command::SaveSong);
    let state = state_when(&core, "saved", |s| s.song.file == SongFile::Saved);
    let path = root.join("songs").join(&state.song.name);
    let path = path.join(format!("{}.{}", state.song.name, songfile::EXTENSION));
    let saved = songfile::read(&path).expect("the song reads");
    let level = |song: &Song| match &song.doc.tracks[1].instrument {
        InstrumentRef::Plugin(p) => p.state.to_vec(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        level(&saved),
        0.25f64.to_le_bytes(),
        "the state at the save"
    );
    assert!(
        saved.doc.master.inserts[3]
            .as_ref()
            .unwrap()
            .plugin_ref()
            .is_some()
    );

    // A new song: the engine hands the plugins back and their processes end.
    core.send(Command::NewSong);
    wait_for(PATIENCE, || core.processes().is_empty().then_some(()))
        .unwrap_or_else(|| panic!("the processes stay: {:?}", core.processes()));
    for process in &processes {
        wait_for(PATIENCE, || (!plughost::alive(process.pid)).then_some(()))
            .unwrap_or_else(|| panic!("{} is still there", process.pid));
    }

    // Opened: both are loaded again, the sine with the level it was saved with.
    *lock(&pick) = Some(path);
    core.send(Command::OpenSong);
    let state = state_when(&core, "opened", |s| {
        plugin_of(s, 1) == Some(PluginState::Ready) && master_plugin(s) == Some(PluginState::Ready)
    });
    assert_eq!(state.song.tracks[1].stand_in, None);
    assert!(state.shelf.missing.is_empty());
    let id = core.track_plugin(1);
    assert_eq!(core.level(id), 0.25);

    // The sine's process is killed: said, and the track's plugin is stopped.
    let track = state.song.tracks[1].id;
    let slot = PluginSlotRef::Instrument { track };
    let pid_of = |core: &Core| {
        let processes = core.processes();
        processes
            .iter()
            .find(|p| p.key.to_string() == SINE)
            .unwrap()
            .pid
    };
    kill(pid_of(&core));
    let state = state_when(&core, "stopped", |s| {
        plugin_of(s, 1) == Some(PluginState::Stopped)
    });
    let stopped = StatusMessage::PluginStopped {
        name: "Fake Sine".into(),
        slot,
    };
    assert_eq!(state.status.message, Some(stopped));
    assert_eq!(
        master_plugin(&state),
        Some(PluginState::Ready),
        "another process"
    );

    // "もう一度読みこむ": a new process, with what was remembered of it.
    core.send(Command::ReloadPlugin { slot });
    state_when(&core, "loaded again", |s| {
        plugin_of(s, 1) == Some(PluginState::Ready)
    });
    assert_eq!(core.level(core.track_plugin(1)), 0.25);
    assert!(!matches!(
        core.snapshot().state.status.message,
        Some(StatusMessage::PluginStopped { .. })
    ));

    // Stopped again and "外す": the track sounds as the sine wave, as one step; undo loads
    // the plugin again as the song has it.
    kill(pid_of(&core));
    state_when(&core, "stopped again", |s| {
        plugin_of(s, 1) == Some(PluginState::Stopped)
    });
    core.send(Command::RemovePlugin { slot });
    let state = core.snapshot().state;
    assert_eq!(state.song.tracks[1].instrument(), InstrumentKind::Sine);
    assert_eq!(state.song.tracks[1].name, "Fake Sine", "the name stays");
    core.send(Command::Undo);
    state_when(&core, "back", |s| {
        plugin_of(s, 1) == Some(PluginState::Ready)
    });
    assert_eq!(core.level(core.track_plugin(1)), 0.25);

    // The effect is taken out: its process ends, the sine's stays.
    let insert = InsertRef { strip, slot: 3 };
    core.send(Command::RemoveEffect { insert });
    wait_for(PATIENCE, || (core.processes().len() == 1).then_some(()))
        .unwrap_or_else(|| panic!("{:?}", core.processes()));
    let left: Vec<u32> = core.processes().iter().map(|p| p.pid).collect();
    drop(core);
    assert!(
        left.iter().all(|pid| !plughost::alive(*pid)),
        "none is left"
    );
    let _ = std::fs::remove_dir_all(root);
}

pub(super) const SURGE_BUNDLE: &str = "/usr/lib/vst3/Surge XT.vst3";

/// What a rendered note is told by: how loud it is and how often it crosses zero upwards.
#[derive(Debug, Clone, Copy)]
struct Heard {
    rms: f32,
    hz: f32,
}

impl Core {
    /// Plays middle C on the song's track at `index` through the engine (no stream runs: the
    /// blocks are run here, each within its real time) and tells what came out.
    fn hear(&self, index: usize) -> Heard {
        self.peek(move |w| {
            let seat = w.model.track_seats()[index].expect("a seat");
            let on = EngineCommand::Audition {
                track: seat,
                keys: Chord::new(&[60]),
                velocity: 100,
            };
            assert!(w.audio.handle.send(on).is_ok());
            let rate = w.audio.handle.meters().sample_rate() as f32;
            let mut out = [0.0f32; 512];
            let mut left = Vec::new();
            let mut block = |w: &mut Worker, keep: bool| {
                let now = monotonic_us();
                let timing = meloet_engine::BlockTiming {
                    callback_us: now,
                    playback_us: now + 10_000,
                };
                let engine = w.audio.engine.as_mut().expect("no stream has it");
                engine.process(&mut out, timing);
                if keep {
                    left.extend(out.iter().step_by(2).copied());
                }
                std::thread::sleep(Duration::from_micros(5_333));
            };
            // The attack is left out; then about half a second.
            for n in 0..120 {
                block(w, n >= 30);
            }
            assert!(w.audio.handle.send(EngineCommand::AuditionOff).is_ok());
            for _ in 0..200 {
                block(w, false);
            }
            let rms = (left.iter().map(|v| v * v).sum::<f32>() / left.len() as f32).sqrt();
            let ups = (left.windows(2))
                .filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0)
                .count();
            Heard {
                rms,
                hz: ups as f32 * rate / left.len() as f32,
            }
        })
    }
}

/// The real Surge XT (VST3) on a track: what the person changed in it is in the saved song,
/// and the song opened again sounds the same.
#[test]
#[ignore = "needs Surge XT (VST3)"]
fn surge_xt_sounds_the_same_after_the_song_is_saved_and_opened() {
    assert!(Path::new(SURGE_BUNDLE).is_dir(), "no {SURGE_BUNDLE}");
    let root = plughost::temp_dir("plug-surge");
    // A place with nothing but Surge XT: the scan of every plugin of this PC takes a minute.
    std::fs::create_dir_all(root.join("vst3")).unwrap();
    std::os::unix::fs::symlink(SURGE_BUNDLE, root.join("vst3/Surge XT.vst3")).unwrap();
    let core = Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
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
    let pick: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let (shell_core, shell_pick) = (core.tx.clone(), pick.clone());
    core.set_shell(Box::new(move |request| {
        if let ShellRequest::PickSongFile { .. } = request {
            let path = lock(&shell_pick).clone();
            let _ = shell_core.send(Msg::Input(Input::SongFilePicked(path), None));
        }
    }));
    let state = state_when(&core, "the scan", |s| {
        s.library.plugins.iter().any(|row| row.name == "Surge XT")
    });
    let row = (state.library.plugins.iter())
        .find(|row| row.name == "Surge XT")
        .unwrap();
    assert!(row.key.starts_with("vst3:"), "{row:?}");
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
    let state = core.snapshot().state;
    assert_eq!(state.song.tracks[1].name, "Surge XT");
    assert_eq!(plugin_of(&state, 1), Some(PluginState::Ready));
    let first = core.hear(1);
    eprintln!("as it starts: {first:?}");
    assert!(first.rms > 0.01, "Surge XT sounds: {first:?}");

    // The person makes another sound of it: an octave up and quieter.
    let id = core.track_plugin(1);
    let count = core.peek(move |w| {
        let hosts = &w.plugins.as_ref().unwrap().hosts;
        let params = wait(|done| hosts.params(id, done)).unwrap();
        let find = |name: &str| {
            let found = params.iter().find(|p| p.name == name);
            found.map(|p| p.id).unwrap_or_else(|| {
                let names: Vec<&str> = params.iter().map(|p| p.name.as_str()).take(80).collect();
                panic!("no parameter {name} in {names:?}")
            })
        };
        let (octave, volume) = (find("A Octave"), find("Global Volume"));
        wait(|done| hosts.set_param(id, octave, 1.0, done)).unwrap();
        wait(|done| hosts.set_param(id, volume, 0.5, done)).unwrap();
        params.len()
    });
    eprintln!("{count} parameters");
    let changed = core.hear(1);
    eprintln!("changed: {changed:?}");
    assert!(changed.hz > first.hz * 1.5, "higher: {first:?} {changed:?}");
    assert!(
        changed.rms < first.rms * 0.8,
        "quieter: {first:?} {changed:?}"
    );

    core.send(Command::SaveSong);
    let state = state_when(&core, "saved", |s| s.song.file == SongFile::Saved);
    let path = root.join("songs").join(&state.song.name);
    let path = path.join(format!("{}.{}", state.song.name, songfile::EXTENSION));
    let saved = songfile::read(&path).expect("the song reads");
    let InstrumentRef::Plugin(in_file) = &saved.doc.tracks[1].instrument else {
        panic!("no plugin in the file");
    };
    eprintln!("the state in the file: {} bytes", in_file.state.len());
    assert!(in_file.state.len() > 1000);

    core.send(Command::NewSong);
    wait_for(PATIENCE, || core.processes().is_empty().then_some(()))
        .unwrap_or_else(|| panic!("the process stays: {:?}", core.processes()));
    *lock(&pick) = Some(path);
    core.send(Command::OpenSong);
    state_when(&core, "opened", |s| {
        plugin_of(s, 1) == Some(PluginState::Ready)
    });
    let again = core.hear(1);
    eprintln!("opened again: {again:?}");
    let near = |a: f32, b: f32, part: f32| (a - b).abs() <= part * b;
    assert!(near(again.hz, changed.hz, 0.05), "{changed:?} {again:?}");
    assert!(near(again.rms, changed.rms, 0.15), "{changed:?} {again:?}");
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_scan_waits_for_the_screen() {
    let root = plughost::temp_dir("plug-screen");
    let mut waiting = options(&root);
    if let Some(plugins) = waiting.plugins.as_mut() {
        plugins.wait_for_screen = true;
    }
    let core = Core::start(waiting);
    std::thread::sleep(Duration::from_millis(500));
    let held = core.peek(|w| w.plugins.as_ref().unwrap().held.as_ref().map(Vec::len));
    assert_eq!(held, Some(1), "the first scan is held");
    assert!(core.snapshot().state.library.plugins.is_empty());
    core.subscribe(Box::new(|_| true), Box::new(|_| true));
    state_when(&core, "the scan after the screen", |s| {
        s.library.plugins.len() == 2
    });
    let held = core.peek(|w| w.plugins.as_ref().unwrap().held.is_none());
    assert!(held, "nothing is held any more");
}
