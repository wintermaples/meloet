//! The export through the whole core (docs/architecture.md 22-11, 22-14): the dialog opens
//! and the check counts, the PC's window gives the place, 「書き出す」 writes the file,
//! 「やめる」 leaves nothing. No device: the song is rendered at the rate asked for.
//!
//! And the watch on the export's engine: no memory is taken or given back inside its
//! blocks. This file has the counting allocator the engine's own tests have; the only
//! engine that runs blocks here is the export's.

mod plughost;

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meloet_core::audiofiles::wav::WavPcm;
use meloet_core::pcm::PcmFile;
use meloet_core::{Core, CoreOptions, ShellRequest};
use meloet_protocol::{
    AppState, Command, ExportChange, ExportCheck, ExportRunView, ExportTail, RecordFormat,
};

use plughost::{temp_dir, wait_for};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

struct Counting;

// SAFETY: every call is passed on to `System` unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if meloet_engine::rt::in_audio_block() {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if meloet_engine::rt::in_audio_block() {
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if meloet_engine::rt::in_audio_block() {
            ALLOCS.fetch_add(1, Relaxed);
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn no_memory_was_taken() {
    assert_eq!((ALLOCS.load(Relaxed), FREES.load(Relaxed)), (0, 0));
}

const PATIENCE: Duration = Duration::from_secs(120);

/// What the PC's windows answer next, and what the file manager was asked to show.
#[derive(Default)]
struct Windows {
    audio: Vec<PathBuf>,
    place: Option<PathBuf>,
    asked: Vec<(Option<PathBuf>, String, bool)>,
    shown: Vec<PathBuf>,
}

type Pick = Arc<Mutex<Windows>>;

fn start(root: &Path) -> (Arc<Core>, Pick) {
    let core = Arc::new(Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        state_dir: Some(root.join("state")),
        cache_dir: Some(root.join("cache")),
        ..CoreOptions::offline()
    }));
    let pick = Pick::default();
    let (weak, answer) = (Arc::downgrade(&core), pick.clone());
    core.set_shell(Box::new(move |request| {
        let Some(core) = weak.upgrade() else {
            return;
        };
        let mut windows = answer.lock().unwrap();
        match request {
            ShellRequest::PickAudio { .. } => core.audio_picked(std::mem::take(&mut windows.audio)),
            ShellRequest::PickExportPlace {
                folder,
                name,
                folder_only,
            } => {
                windows.asked.push((folder, name, folder_only));
                core.export_place_picked(windows.place.take());
            }
            ShellRequest::ShowFile { path } => windows.shown.push(path),
            _ => {}
        }
    }));
    (core, pick)
}

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s)))
        .unwrap_or_else(|| panic!("{what}: {:#?}", core.snapshot().state.export))
}

/// Two seconds at 48000 Hz, 16 bits, mono: a 300 Hz sine at half scale.
fn write_wav(path: &Path) {
    let frames = 96_000u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + 2 * frames).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&48_000u32.to_le_bytes());
    out.extend_from_slice(&96_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(2 * frames).to_le_bytes());
    for i in 0..frames {
        let s = (std::f32::consts::TAU * 300.0 * i as f32 / 48_000.0).sin() * 0.5;
        out.extend_from_slice(&((s * 32_767.0) as i16).to_le_bytes());
    }
    std::fs::write(path, out).unwrap();
}

fn check_done(state: &AppState) -> Option<Option<f32>> {
    match state.export.dialog.as_ref()?.check {
        ExportCheck::Done { peak_db, .. } => Some(peak_db),
        _ => None,
    }
}

fn set(core: &Core, change: ExportChange) {
    core.send(Command::SetExport { change });
}

fn folders(dir: &Path) -> usize {
    std::fs::read_dir(dir).map_or(0, |d| d.count())
}

#[test]
fn a_song_is_checked_written_out_and_shown_and_a_run_is_given_up() {
    {
        let _block = meloet_engine::rt::enter();
        assert!(
            meloet_engine::rt::in_audio_block(),
            "the engine is built with its watch"
        );
    }
    let root = temp_dir("export-whole");
    let from = root.join("from");
    let out = root.join("out");
    std::fs::create_dir_all(&from).unwrap();
    std::fs::create_dir_all(&out).unwrap();
    let wav = from.join("声.wav");
    write_wav(&wav);
    let (core, pick) = start(&root);

    // An empty song has nothing to check.
    core.send(Command::OpenExport);
    let state = state_when(&core, "the dialog is open", |s| s.export.dialog.is_some());
    assert_eq!(
        state.export.dialog.as_ref().unwrap().check,
        ExportCheck::Waiting
    );
    core.send(Command::CloseExport);
    state_when(&core, "the dialog is closed", |s| s.export.dialog.is_none());

    pick.lock().unwrap().audio = vec![wav];
    core.send(Command::ImportAudio { track: None });
    state_when(&core, "the file is in the song", |s| {
        s.song.tracks.iter().any(|t| !t.clips.is_empty())
    });

    // The dialog opens: the song is rendered into the work place, and the check says how
    // loud it is. A sine at half scale, and the master's limiter leaves it alone.
    core.send(Command::OpenExport);
    let state = state_when(&core, "the check is done", |s| check_done(s).is_some());
    let peak = check_done(&state).unwrap().expect("the song sounds");
    assert!((peak - -6.0).abs() < 0.5, "{peak} dB");
    let work = root.join("state/export");
    assert_eq!(folders(&work), 1);

    // Other bits and another format are the last step: the same job goes on.
    set(
        &core,
        ExportChange::Bits {
            bits: RecordFormat::Bits16,
        },
    );
    let state = state_when(&core, "16 bits", |s| {
        s.export
            .dialog
            .as_ref()
            .is_some_and(|d| d.bits == RecordFormat::Bits16)
    });
    assert!(check_done(&state).is_some());
    // No tail is another rendering.
    set(
        &core,
        ExportChange::Tail {
            tail: ExportTail::Off,
        },
    );
    state_when(&core, "the check is done again", |s| {
        s.export
            .dialog
            .as_ref()
            .is_some_and(|d| d.tail == ExportTail::Off)
            && check_done(s).is_some()
    });
    wait_for(PATIENCE, || (folders(&work) == 1).then_some(())).expect("the old job is gone");

    // The PC's window gives the place, and 「書き出す」 writes the file there.
    pick.lock().unwrap().place = Some(out.join("できた.wav"));
    core.send(Command::ChooseExportPlace);
    state_when(&core, "the place is chosen", |s| {
        (s.export.dialog.as_ref()).is_some_and(|d| d.place.ends_with("out/できた.wav"))
    });
    {
        let windows = pick.lock().unwrap();
        assert_eq!(windows.asked.len(), 1);
        assert!(!windows.asked[0].2, "a file, not a folder");
        assert!(windows.asked[0].1.ends_with(".wav"));
    }
    core.send(Command::StartExport);
    let state = state_when(&core, "the export is done", |s| {
        matches!(s.export.run, Some(ExportRunView::Done { .. }))
    });
    assert!(state.export.dialog.is_none());
    let Some(ExportRunView::Done {
        file,
        files,
        sample_rate,
        bits,
        ..
    }) = &state.export.run
    else {
        unreachable!()
    };
    assert_eq!((file.as_str(), *files), ("できた.wav", 1));
    assert_eq!((*sample_rate, *bits), (48_000, RecordFormat::Bits16));
    let written = out.join("できた.wav");
    let mut wav = WavPcm::open(&written).unwrap();
    assert_eq!(
        (wav.sample_rate(), wav.channels(), wav.bits()),
        (48_000, 2, 16)
    );
    // Two seconds of the file, and the clip's place on the song's ticks rounds to frames.
    assert!(
        (95_990..=96_010).contains(&wav.frames()),
        "{}",
        wav.frames()
    );
    let mut audio = vec![0.0f32; 2 * wav.frames() as usize];
    wav.read(0, &mut audio).unwrap();
    let largest = audio.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!((largest - 0.5).abs() < 0.03, "{largest}");
    wait_for(PATIENCE, || (folders(&work) == 0).then_some(())).expect("the work place is empty");

    // 「フォルダをひらく」 shows the file in the PC's file manager.
    core.send(Command::ShowExported);
    wait_for(PATIENCE, || {
        (pick.lock().unwrap().shown == [written.clone()]).then_some(())
    })
    .expect("the file is shown");

    // At the speed it plays the run takes two seconds: 「やめる」 in the middle.
    core.send(Command::OpenExport);
    set(&core, ExportChange::Realtime { on: true });
    pick.lock().unwrap().place = Some(out.join("やめた.wav"));
    core.send(Command::ChooseExportPlace);
    state_when(&core, "the check counts", |s| {
        s.export.dialog.as_ref().is_some_and(|d| d.can_export)
    });
    core.send(Command::StartExport);
    state_when(&core, "the export runs", |s| {
        matches!(
            s.export.run,
            Some(ExportRunView::Running { realtime: true, .. })
        )
    });
    core.send(Command::CancelExport);
    state_when(&core, "the export is given up", |s| {
        s.export.run == Some(ExportRunView::Cancelled)
    });
    wait_for(PATIENCE, || (folders(&work) == 0).then_some(())).expect("the work place is empty");
    assert!(!out.join("やめた.wav").exists());
    assert_eq!(folders(&out), 1);

    core.stop();
    no_memory_was_taken();
    let _ = std::fs::remove_dir_all(root);
}
