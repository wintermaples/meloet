//! SF2 files through the whole core: added from the PC's window and by dropping, copied into
//! meloet's folder, tried, put on tracks, saved, and missing on a PC that does not have them.
//! With little files made here, and with FluidR3 when this PC has it.

#[allow(dead_code)]
#[path = "../../meloet-sound/tests/support/sf2.rs"]
mod font;
mod plughost;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_core::{Core, CoreOptions, ShellRequest};
use meloet_protocol::{
    AppState, Band, Command, FileDropView, GmFamily, MissingRef, MissingView, ShelfMode, SongFile,
    SoundRef, SoundView, StandIn, StatusMessage,
};

use plughost::{temp_dir, wait_for};

const PATIENCE: Duration = Duration::from_secs(120);
const RICH: &str = "Rich.sf2";

/// What the PC's windows answer next.
type Pick = Arc<Mutex<Option<PathBuf>>>;

fn start(root: &Path, sf2: &str) -> (Arc<Core>, Pick) {
    let core = Arc::new(Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        sf2_dir: Some(root.join(sf2)),
        ..CoreOptions::offline()
    }));
    let pick = Pick::default();
    let (weak, answer) = (Arc::downgrade(&core), pick.clone());
    core.set_shell(Box::new(move |request| {
        let Some(core) = weak.upgrade() else {
            return;
        };
        let path = answer.lock().unwrap().clone();
        match request {
            ShellRequest::PickSf2 { .. } => core.sf2_picked(path),
            ShellRequest::PickSongFile { .. } => core.song_file_picked(path),
            _ => {}
        }
    }));
    (core, pick)
}

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s))).unwrap_or_else(|| {
        let state = core.snapshot().state;
        panic!("{what}: {:#?} {:#?}", state.status, state.shelf)
    })
}

fn write(root: &Path, folder: &str, name: &str, bytes: &[u8]) -> PathBuf {
    let dir = root.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn add(core: &Core, pick: &Pick, path: &Path) {
    *pick.lock().unwrap() = Some(path.to_path_buf());
    core.send(Command::AddSf2);
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    (sha2::Sha256::digest(bytes).iter())
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The hashes the song file at `path` has for its SF2 tracks, in the order of the tracks.
fn sf2_hashes(path: &Path) -> Vec<String> {
    let song: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    (song["song"]["tracks"].as_array().unwrap().iter())
        .filter(|t| t["instrument"]["kind"] == "sf2")
        .map(|t| t["instrument"]["sha256"].as_str().unwrap().to_string())
        .collect()
}

fn preset(file: &str, bank: u16, program: u8) -> SoundRef {
    SoundRef::Sf2 {
        file: file.into(),
        bank,
        program,
    }
}

/// Tries the preset and waits until its samples are read.
fn select(core: &Core, sound: &SoundRef) {
    core.send(Command::ShelfSelectSound {
        sound: Some(sound.clone()),
    });
    state_when(core, "the preset is read", |s| {
        s.shelf.selected_sound.as_ref() == Some(sound) && s.shelf.loading_sound.is_none()
    });
}

#[test]
fn a_file_is_copied_in_replaced_by_one_of_its_name_and_a_broken_one_is_not_added() {
    let root = temp_dir("sf2-add");
    let (core, pick) = start(&root, "sf2");
    assert!(core.snapshot().state.library.sf2.is_empty());

    let rich = font::rich().bytes();
    let source = write(&root, "in", RICH, &rich);
    add(&core, &pick, &source);
    let state = state_when(&core, "added", |s| s.library.sf2.len() == 1);
    assert_eq!(
        state.status.message,
        Some(StatusMessage::Sf2Added {
            file: RICH.into(),
            presets: 1,
            kits: 1,
            skipped: 0
        })
    );
    assert_eq!(state.library.opened_sf2.as_deref(), Some(RICH));
    assert!(state.workspace.layout.right_open.contains(&Band::Shelf));
    let presets = &state.library.sf2[0].presets;
    let listed: Vec<(u16, u8, &str, GmFamily)> = (presets.iter())
        .map(|p| (p.bank, p.program, p.name.as_str(), p.family))
        .collect();
    assert_eq!(
        listed,
        [
            (0, 0, "Rich", GmFamily::Piano),
            (128, 0, "Kit", GmFamily::DrumKit)
        ]
    );
    // meloet has its own copy: the file the person picked may go.
    let copy = root.join("sf2").join(RICH);
    assert_eq!(std::fs::read(&copy).unwrap(), rich);
    std::fs::remove_file(&source).unwrap();

    // Dropped on the window: a file of the same name with other presets takes its place.
    let tone = font::sine(440.0, 48_000, 4800, 0.4);
    let sample = font::Smp::new("tone", tone, 48_000, 69);
    let simple = font::Font::simple(vec![sample], vec![vec![(font::SAMPLE_ID, 0)]]).bytes();
    let other = write(&root, "in2", RICH, &simple);
    let song = root.join("in2/曲.meloet");
    core.files_over(vec![other.clone(), song.clone()]);
    let state = state_when(&core, "held over the window", |s| {
        s.workspace.file_drop.is_some()
    });
    let held = FileDropView {
        name: RICH.into(),
        addable: true,
        files: 2,
        sf2: 1,
        audio: 0,
    };
    assert_eq!(state.workspace.file_drop, Some(held));
    assert_eq!(state.status.message, Some(StatusMessage::Sf2Dragging));
    core.files_dropped(vec![other, song]);
    let state = state_when(&core, "replaced", |s| {
        s.library.sf2[0].presets.len() == 1 && s.workspace.file_drop.is_none()
    });
    assert_eq!(state.library.sf2.len(), 1);
    assert_eq!(std::fs::read(&copy).unwrap(), simple);

    // A file that is no SoundFont, and a SoundFont 3 file: not added, and said.
    let mut sf3 = font::rich();
    sf3.version = (3, 1);
    for (name, bytes) in [
        ("Bad.sf2", b"RIFF, but nothing more".to_vec()),
        ("Three.sf2", sf3.bytes()),
    ] {
        let path = write(&root, "in", name, &bytes);
        add(&core, &pick, &path);
        let said = StatusMessage::Sf2Unreadable { file: name.into() };
        state_when(&core, name, |s| s.status.message == Some(said.clone()));
        assert!(!root.join("sf2").join(name).exists());
    }
    assert_eq!(core.snapshot().state.library.sf2.len(), 1);
    let left: Vec<String> = (std::fs::read_dir(root.join("sf2")).unwrap().flatten())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn presets_go_on_tracks_are_saved_and_are_missing_on_a_pc_without_the_file() {
    let root = temp_dir("sf2-song");
    let rich = write(&root, "in", RICH, &font::rich().bytes());
    let (core, pick) = start(&root, "sf2");
    add(&core, &pick, &rich);
    state_when(&core, "added", |s| s.library.sf2.len() == 1);

    // A new track with the piano and one with the kit, each named after its family.
    for sound in [preset(RICH, 0, 0), preset(RICH, 128, 0)] {
        core.send(Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        });
        select(&core, &sound);
        core.send(Command::ShelfConfirm);
    }
    let state = core.snapshot().state;
    let names: Vec<&str> = state.song.tracks.iter().map(|t| t.name.as_str()).collect();
    // No sound pack in this test: the first track is the sine wave.
    assert_eq!(names, ["シンセ", "ピアノ", "ドラム"]);
    assert!(matches!(
        &state.song.tracks[1].sound,
        SoundView::Sf2 { file, bank: 0, program: 0, name, family: GmFamily::Piano }
            if file == RICH && name == "Rich"
    ));
    assert_eq!(state.song.tracks[1].drum_keys, None);
    let rows = state.song.tracks[2].drum_keys.clone().unwrap();
    assert_eq!(rows.len(), 47, "the kit has a sound on the keys 35 to 81");
    assert_eq!(rows[..3], [49, 57, 55], "the cymbals first");
    assert!(
        state
            .song
            .tracks
            .iter()
            .skip(1)
            .all(|t| t.stand_in.is_none())
    );
    // A track gets the kit in place of its sound.
    let track = state.song.tracks[1].id;
    core.send(Command::OpenShelf {
        mode: ShelfMode::Replace { track },
    });
    select(&core, &preset(RICH, 128, 0));
    core.send(Command::ShelfConfirm);
    let said = core.snapshot().state.status.message;
    assert!(
        matches!(said, Some(StatusMessage::InstrumentReplacedSound { .. })),
        "{said:?}"
    );
    core.send(Command::Undo);

    core.send(Command::SaveSong);
    let state = state_when(&core, "saved", |s| s.song.file == SongFile::Saved);
    let name = state.song.name.clone();
    let path = root
        .join("songs")
        .join(&name)
        .join(format!("{name}.meloet"));
    assert!(path.is_file());
    // The song says which file it was, by the hash of its bytes.
    let hash = sha256(&font::rich().bytes());
    assert_eq!(sf2_hashes(&path), [hash.clone(), hash.clone()]);
    drop(core);
    // The hash was noted beside the copy, for the starts after.
    let listed = meloet_core::sf2store::list(&root.join("sf2"));
    assert_eq!(listed[0].sha256, hash);

    // The next start has the file, and the song opens with its presets.
    let (core, pick) = start(&root, "sf2");
    let state = core.snapshot().state;
    assert_eq!(state.library.sf2.len(), 1, "read from meloet's folder");
    assert_eq!(state.library.opened_sf2, None);
    *pick.lock().unwrap() = Some(path.clone());
    core.send(Command::OpenSong);
    let state = state_when(&core, "opened", |s| s.song.tracks.len() == 3);
    assert!(
        state
            .song
            .tracks
            .iter()
            .skip(1)
            .all(|t| t.stand_in.is_none())
    );
    assert!(state.shelf.missing.is_empty());
    assert_eq!(
        state.song.tracks[2].drum_keys.as_ref().map(Vec::len),
        Some(47)
    );
    drop(core);

    // A PC without the file: the tracks sound as the sine wave, and it is said.
    let (core, pick) = start(&root, "sf2-of-another-pc");
    *pick.lock().unwrap() = Some(path);
    core.send(Command::OpenSong);
    let state = state_when(&core, "opened without the file", |s| {
        s.song.tracks.len() == 3
    });
    let stand_ins: Vec<Option<StandIn>> = state.song.tracks.iter().map(|t| t.stand_in).collect();
    assert_eq!(stand_ins[1..], [Some(StandIn::Sf2Missing); 2]);
    assert_eq!(state.song.tracks[2].drum_keys, None);
    // It is said, and the shelf has the notice of the file.
    let said = StatusMessage::Sf2Missing {
        file: RICH.into(),
        tracks: 2,
    };
    assert_eq!(state.status.message, Some(said));
    let notice = MissingView::Sf2 {
        file: RICH.into(),
        tracks: 2,
    };
    assert_eq!(state.shelf.missing, [notice]);
    // 「SF2 を足す…」 with a file of that name: the tracks are tied to it, though it is not
    // the same file byte for byte (the hash in the song is a record only).
    let mut other = font::rich();
    other.version = (2, 4);
    assert_ne!(sha256(&other.bytes()), hash);
    let other = write(&root, "in-other", RICH, &other.bytes());
    add(&core, &pick, &other);
    let state = state_when(&core, "tied", |s| {
        s.song.tracks.iter().skip(1).all(|t| t.stand_in.is_none())
    });
    assert!(state.shelf.missing.is_empty());
    assert!(state.song.tracks[2].drum_keys.is_some());
    assert_eq!(state.song.undo_count, 0, "the song did not change");
    // 「曲から外す」 does nothing for a file that is there.
    let missing = MissingRef::Sf2 { file: RICH.into() };
    core.send(Command::RemoveMissing { missing });
    assert_eq!(core.snapshot().state.song.undo_count, 0);
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

/// With the real FluidR3_GM.sf2 (141 MB) when this PC has it where the sound pack is built
/// from.
#[test]
fn fluidr3_is_added_listed_and_played_when_this_pc_has_it() {
    let Some(source) =
        dirs_home().map(|h| h.join(".cache/meloet/sounds-src/fluidr3/FluidR3_GM.sf2"))
    else {
        return;
    };
    if !source.is_file() {
        eprintln!("no {}: skipped", source.display());
        return;
    }
    let root = temp_dir("sf2-fluid");
    let (core, pick) = start(&root, "sf2");
    let began = Instant::now();
    add(&core, &pick, &source);
    let state = state_when(&core, "added", |s| s.library.sf2.len() == 1);
    eprintln!("FluidR3 added in {:.2} s", began.elapsed().as_secs_f64());
    let Some(StatusMessage::Sf2Added {
        file,
        presets,
        kits,
        ..
    }) = state.status.message.clone()
    else {
        panic!("{:?}", state.status.message);
    };
    eprintln!("{file}: 音色 {presets} ・ ドラムのキット {kits}");
    assert_eq!(file, "FluidR3_GM.sf2");
    // FluidR3 has 189 presets, 31 of them in the drum bank (Standard 1 to 7, Room 1 to 7, …).
    assert_eq!((presets, kits), (158, 31));
    let listed = &state.library.sf2[0].presets;
    assert_eq!(listed.len() as u32, presets + kits);
    assert!(presets >= 128);
    let families: Vec<GmFamily> = listed.iter().map(|p| p.family).collect();
    assert!(families.is_sorted_by_key(|f| GmFamily::ALL.iter().position(|g| g == f)));
    assert_eq!((listed[0].bank, listed[0].program), (0, 0));

    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    let began = Instant::now();
    select(&core, &preset(&file, 0, 0));
    eprintln!("the piano read in {:.2} s", began.elapsed().as_secs_f64());
    core.send(Command::ShelfConfirm);
    select_kit(&core, &file);
    let state = core.snapshot().state;
    assert_eq!(state.song.tracks[1].name, "ピアノ");
    assert!(
        state.song.tracks[2]
            .drum_keys
            .as_ref()
            .is_some_and(|k| k.len() > 40)
    );
    drop(core);

    let began = Instant::now();
    let (core, _) = start(&root, "sf2");
    eprintln!("the next start took {:.3} s", began.elapsed().as_secs_f64());
    assert_eq!(
        core.snapshot().state.library.sf2[0].presets.len(),
        listed.len()
    );
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

fn select_kit(core: &Core, file: &str) {
    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    select(core, &preset(file, 128, 0));
    core.send(Command::ShelfConfirm);
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
