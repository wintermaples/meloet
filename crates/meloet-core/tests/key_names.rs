//! What each key of a track's sound is (`AiAsk::KeyNames`, and the rows of the drum piano roll
//! of `TrackView`) through the whole core: plugins in the real `meloet-plughost` with its fake
//! `drums` bundle (one names its keys, one does not) and `slowkit` (it names them later), and
//! SF2 presets of a little file in meloet's SF2 folder, read off the core thread.

#[allow(dead_code)]
#[path = "../../meloet-sound/tests/support/sf2.rs"]
mod font;
mod plughost;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use meloet_core::ai::{
    AiAnswer, AiAsk, AiInstrument, AiKeyMap, AiKeyNote, AiKeySource, AiNote, AiPlace, AiRefusal,
};
use meloet_core::midi;
use meloet_core::play::keynames::read_sf2;
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use meloet_protocol::{
    AppState, Command, ExportChange, ExportFormat, ExportRunView, InstrumentKind, PluginState,
    ScaleMode, ShelfMode, SoundView, StatusMessage,
};

use font::{Font, Inst, KEY_RANGE, Preset, SAMPLE_ID, Smp, flat, range};
use plughost::{temp_dir, wait_for};

const PATIENCE: Duration = Duration::from_secs(60);
const DRUMS: &str = "lv2:urn:meloet:fake:drums";
const KIT: &str = "lv2:urn:meloet:fake:kit";
const SLOW_KIT: &str = "lv2:urn:meloet:fake:slowkit";
const SF2: &str = "Kit.sf2";

fn options(root: &Path) -> CoreOptions {
    for bundle in ["drums", "slowkit"] {
        let bundle = root.join(format!("lv2/{bundle}.lv2"));
        std::fs::create_dir_all(&bundle).unwrap();
        std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    }
    CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        sf2_dir: Some(root.join("sf2")),
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

/// A kit whose kick is a stereo sample on 36, with a snare on 38, and a piano on 21 to 108.
fn kit_file() -> Vec<u8> {
    let sample = |name: &'static str, kind: u16| {
        let mut sample = Smp::new(name, flat(1000, 480), 48_000, 60);
        sample.kind = kind;
        sample
    };
    let zone = |lo, hi, sample| vec![(KEY_RANGE, range(lo, hi)), (SAMPLE_ID, sample)];
    let samples = vec![
        sample("Kick(L)", 4),
        sample("Kick(R)", 2),
        sample("Snare", 1),
    ];
    let insts = vec![
        Inst {
            name: "drums",
            zones: vec![zone(36, 36, 0), zone(36, 36, 1), zone(38, 38, 2)],
        },
        Inst {
            name: "keys",
            zones: vec![zone(21, 108, 2)],
        },
    ];
    let preset = |name, bank, instrument| Preset {
        name,
        bank,
        program: 0,
        zones: vec![vec![(font::INSTRUMENT, instrument)]],
    };
    Font::new(
        samples,
        insts,
        vec![preset("Kit", 128, 0), preset("Piano", 0, 1)],
    )
    .bytes()
}

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s)))
        .unwrap_or_else(|| panic!("{what}: {:#?}", core.snapshot().state.song.tracks))
}

fn add(core: &Core, instrument: AiInstrument) -> u32 {
    match core.ai(AiAsk::AddTrack { instrument }) {
        AiAnswer::Done(done) => done.track.expect("a track"),
        other => panic!("{other:?}"),
    }
}

fn names(core: &Core, track: u32) -> AiKeyMap {
    match core.ai(AiAsk::KeyNames { track }) {
        AiAnswer::KeyNames(map) => *map,
        other => panic!("{other:?}"),
    }
}

/// The keys the screen is told the track's plugin names.
fn key_names(state: &AppState, track: u32) -> Vec<(u8, &str)> {
    let names = state.song.tracks[track as usize - 1].key_names.as_ref();
    (names.into_iter().flatten())
        .map(|n| (n.key, n.name.as_str()))
        .collect()
}

fn named(map: &AiKeyMap) -> Vec<(u8, Option<&str>, AiKeySource)> {
    (map.keys.iter())
        .map(|k| (k.key, k.name.as_deref(), k.source))
        .collect()
}

#[test]
fn plugins_and_sf2_presets_name_their_keys() {
    let root = temp_dir("key-names");
    std::fs::create_dir_all(root.join("sf2")).unwrap();
    std::fs::write(root.join("sf2").join(SF2), kit_file()).unwrap();
    let core = Arc::new(Core::start(options(&root)));
    state_when(&core, "the scan", |s| s.library.plugins.len() == 3);

    // A plugin that names its keys (LV2 midnam in a real one).
    let drums = add(&core, AiInstrument::Plugin(DRUMS.into()));
    let kit = add(&core, AiInstrument::Plugin(KIT.into()));
    let ready = |s: &AppState, track: u32| {
        let view = &s.song.tracks[track as usize - 1].sound;
        matches!(view, SoundView::Plugin(p) if p.state == PluginState::Ready)
    };
    let state = state_when(&core, "both plugins sound", |s| {
        ready(s, drums) && ready(s, kit)
    });
    // The screen has them as the rows of the drum piano roll, top row first.
    let view = &state.song.tracks[drums as usize - 1];
    assert_eq!(view.drum_keys, Some(vec![38, 36]));
    assert_eq!(
        key_names(&state, drums),
        [(36, "Fake Kick"), (38, "Fake Snare")]
    );
    let view = &state.song.tracks[kit as usize - 1];
    assert_eq!((&view.drum_keys, &view.key_names), (&None, &None));
    let map = names(&core, drums);
    assert!(map.kit && map.certain);
    assert_eq!(map.source, AiKeySource::Lv2Midnam);
    assert_eq!(
        named(&map),
        [
            (36, Some("Fake Kick"), AiKeySource::Lv2Midnam),
            (38, Some("Fake Snare"), AiKeySource::Lv2Midnam)
        ]
    );

    // A drum plugin that names none: the usual layout, as a guess.
    let map = names(&core, kit);
    assert!(map.kit && !map.certain);
    assert_eq!(map.source, AiKeySource::GeneralMidiGuess);
    assert_eq!(map.keys.len(), 47);
    assert_eq!(map.keys[1].key, 36);
    assert_eq!(map.keys[1].name.as_deref(), Some("キック"));

    // An SF2 kit: the rules guess, the core thread finds the file, and its samples name
    // the keys once it is read.
    let sf2 = |bank| AiInstrument::Sf2 {
        file: SF2.into(),
        bank,
        program: 0,
    };
    let sf2_kit = add(&core, sf2(128));
    let mut map = names(&core, sf2_kit);
    assert!(map.kit && !map.certain);
    assert_eq!(map.source, AiKeySource::GeneralMidiGuess);
    assert_eq!(map.range, Some((36, 38)));
    assert_eq!(map.sf2_path, Some(root.join("sf2").join(SF2)));
    read_sf2(&mut map);
    assert!(map.certain && map.sf2_path.is_none());
    assert_eq!(map.source, AiKeySource::Sf2SampleName);
    assert_eq!(
        named(&map),
        [
            (36, Some("Kick"), AiKeySource::Sf2SampleName),
            (38, Some("Snare"), AiKeySource::Sf2SampleName)
        ]
    );
    assert_eq!(map.notes, [AiKeyNote::SampleNames]);

    // A melodic preset: no names, the keys it plays.
    let piano = add(&core, sf2(0));
    let mut map = names(&core, piano);
    assert!(!map.kit && map.keys.is_empty());
    read_sf2(&mut map);
    assert_eq!(map.range, Some((21, 108)));
    assert!(map.keys.is_empty());

    // The file went away after it was found: the guess stays, and says why.
    let mut map = names(&core, sf2_kit);
    std::fs::remove_file(root.join("sf2").join(SF2)).unwrap();
    read_sf2(&mut map);
    assert_eq!(map.source, AiKeySource::GeneralMidiGuess);
    assert_eq!(map.notes, [AiKeyNote::Sf2Unreadable]);

    // No such track; reading is no step.
    let steps = core.ai_song().unwrap().undo_count;
    assert_eq!(
        core.ai(AiAsk::KeyNames { track: 99 }),
        AiAnswer::Refused(AiRefusal::NoSuchTrack(99))
    );
    assert_eq!(core.ai_song().unwrap().undo_count, steps);
    drop(core);
    let _ = std::fs::remove_dir_all(&root);
}

/// A plugin that reads its kit after it was loaded (as DrumGizmo does) names its keys later,
/// by itself: the rows of the screen and the AI's answer follow.
#[test]
fn names_a_plugin_gives_later_reach_the_screen_and_the_ai() {
    let root = temp_dir("key-names-later");
    let core = Arc::new(Core::start(options(&root)));
    state_when(&core, "the scan", |s| s.library.plugins.len() == 3);
    let slow = add(&core, AiInstrument::Plugin(SLOW_KIT.into()));
    let at = slow as usize - 1;
    let state = state_when(&core, "the names", |s| {
        s.song.tracks[at].key_names.is_some()
    });
    assert_eq!(state.song.tracks[at].drum_keys, Some(vec![38, 36]));
    assert_eq!(
        key_names(&state, slow),
        [(36, "Fake Kick"), (38, "Fake Snare")]
    );
    let map = wait_for(PATIENCE, || {
        match core.ai(AiAsk::KeyNames { track: slow }) {
            AiAnswer::KeyNames(map) => Some(*map),
            AiAnswer::Refused(AiRefusal::PluginNotLoaded) => None,
            other => panic!("{other:?}"),
        }
    })
    .expect("the plugin sounds");
    assert!(map.kit && map.certain);
    assert_eq!(
        named(&map),
        [
            (36, Some("Fake Kick"), AiKeySource::Lv2Midnam),
            (38, Some("Fake Snare"), AiKeySource::Lv2Midnam)
        ]
    );
    drop(core);
    let _ = std::fs::remove_dir_all(&root);
}

/// With the scale locked, a key outside it is refused on a melodic track but written on a
/// plugin that names its keys: the screen shows that track as the drum piano roll.
#[test]
fn a_plugin_that_names_its_keys_writes_keys_outside_the_locked_scale() {
    let root = temp_dir("key-names-scale");
    let core = Arc::new(Core::start(options(&root)));
    state_when(&core, "the scan", |s| s.library.plugins.len() == 3);
    let drums = add(&core, AiInstrument::Plugin(DRUMS.into()));
    let kit = add(&core, AiInstrument::Plugin(KIT.into()));
    let state = state_when(&core, "the names", |s| {
        s.song.tracks[drums as usize - 1].key_names.is_some()
            && matches!(&s.song.tracks[kit as usize - 1].sound,
                SoundView::Plugin(p) if p.state == PluginState::Ready)
    });
    let id = |track: u32| state.song.tracks[track as usize - 1].id;
    // C# major: 38 (the snare) is outside it.
    core.send(Command::SetMusicalScale {
        root: 1,
        mode: ScaleMode::Major,
        lock: true,
        highlight: true,
    });
    let write = |track: u32| {
        core.send(Command::SelectTrack {
            track: id(track),
            from_arm: false,
        });
        core.send(Command::AddNote {
            take: None,
            start: 0,
            length: 480,
            key: 38,
            velocity: 100,
        });
        let state = core.snapshot().state;
        let clips = state.song.tracks[track as usize - 1].clips.len();
        (state.status.message, clips)
    };

    let refused = StatusMessage::OutOfScale {
        key: 38,
        root: 1,
        mode: ScaleMode::Major,
    };
    assert_eq!(write(kit), (Some(refused), 0));
    // With no notice, the status line goes back to what it said before.
    let (said, clips) = write(drums);
    assert!(
        !matches!(said, Some(StatusMessage::OutOfScale { .. })),
        "{said:?}"
    );
    assert_eq!(clips, 1);
    drop(core);
    let _ = std::fs::remove_dir_all(&root);
}

/// A MIDI export writes a plugin that names its keys on channel 10, as a kit, and one that
/// names none on channel 1: the AI link's way (`AiSong::plugin_kits`) and the dialog's.
#[test]
fn a_plugin_that_names_its_keys_is_exported_on_the_drum_channel() {
    let root = temp_dir("key-names-midi");
    let core = Arc::new(Core::start(options(&root)));
    state_when(&core, "the scan", |s| s.library.plugins.len() == 3);
    let drums = add(&core, AiInstrument::Plugin(DRUMS.into()));
    let kit = add(&core, AiInstrument::Plugin(KIT.into()));
    let state = state_when(&core, "the names", |s| {
        s.song.tracks[drums as usize - 1].key_names.is_some()
            && matches!(&s.song.tracks[kit as usize - 1].sound,
                SoundView::Plugin(p) if p.state == PluginState::Ready)
    });
    assert!(state.song.tracks[kit as usize - 1].key_names.is_none());
    for track in [drums, kit] {
        let put = core.ai(AiAsk::PutNotes {
            at: AiPlace::Track(track),
            notes: vec![AiNote {
                start: 0,
                length: 480,
                key: 36,
                velocity: 100,
            }],
        });
        assert!(
            matches!(&put, AiAnswer::Done(d) if d.notes.is_some()),
            "{put:?}"
        );
    }
    let places = [drums as usize - 1, kit as usize - 1];
    let channel = |file: &midi::smf::Smf, place: usize| {
        let notes = &file.tracks[place + 1].notes;
        assert_eq!(notes.len(), 1, "{place}");
        notes[0].channel
    };

    // The AI link: the song as read, with the plugin kits.
    let read = core.ai_song().unwrap();
    let kits: Vec<_> = (read.song.doc.tracks.iter())
        .map(|t| read.plugin_kits.contains(&t.id))
        .collect();
    assert_eq!((kits[places[0]], kits[places[1]]), (true, false));
    let all: Vec<usize> = (0..read.song.doc.tracks.len()).collect();
    let made = midi::midi_file(
        &read.song.doc,
        &all,
        &read.plugin_kits,
        None,
        0,
        1920 * 4,
        1920,
    )
    .unwrap();
    let file = midi::smf::read(&midi::smf::write(&made.file)).unwrap();
    assert_eq!(
        (channel(&file, places[0]), channel(&file, places[1])),
        (midi::DRUM_CHANNEL, 0)
    );

    // The dialog: saved first, so the file goes beside the song.
    let saved = core.ai(AiAsk::SaveAs {
        folder: root.join("songs"),
        name: "kits".into(),
    });
    assert!(!matches!(saved, AiAnswer::Refused(_)), "{saved:?}");
    let song = wait_for(PATIENCE, || core.ai_song()?.file).expect("the song saved");
    core.send(Command::OpenExport);
    core.send(Command::SetExport {
        change: ExportChange::Format {
            format: ExportFormat::Midi,
        },
    });
    core.send(Command::StartExport);
    let state = state_when(&core, "the export", |s| {
        matches!(s.export.run, Some(ExportRunView::Done { .. }))
    });
    let Some(ExportRunView::Done { file, .. }) = state.export.run else {
        unreachable!()
    };
    let path = song.parent().unwrap().join(file);
    let file = midi::smf::read(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        (channel(&file, places[0]), channel(&file, places[1])),
        (midi::DRUM_CHANNEL, 0)
    );
    drop(core);
    let _ = std::fs::remove_dir_all(&root);
}

/// 「ここに置く」 of a drum pattern puts it on a selected plugin that names its keys, as on a
/// drum track; on one that names none it makes a studio kit track, as before.
#[test]
fn a_pattern_goes_on_a_plugin_that_names_its_keys() {
    let root = temp_dir("key-names-pattern");
    let core = Arc::new(Core::start(options(&root)));
    state_when(&core, "the scan", |s| s.library.plugins.len() == 3);
    let drums = add(&core, AiInstrument::Plugin(DRUMS.into()));
    let kit = add(&core, AiInstrument::Plugin(KIT.into()));
    let state = state_when(&core, "the names", |s| {
        s.song.tracks[drums as usize - 1].key_names.is_some()
            && matches!(&s.song.tracks[kit as usize - 1].sound,
                SoundView::Plugin(p) if p.state == PluginState::Ready)
    });
    let id = |track: u32| state.song.tracks[track as usize - 1].id;
    let tracks = state.song.tracks.len();
    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    let place = |track: u32| {
        core.send(Command::SelectTrack {
            track: id(track),
            from_arm: false,
        });
        core.send(Command::PlacePattern {
            id: "eightBeat".into(),
        });
        core.snapshot().state
    };

    let state = place(drums);
    assert_eq!(state.song.tracks.len(), tracks, "no new track");
    let on = &state.song.tracks[drums as usize - 1];
    assert_eq!(on.id, id(drums));
    assert_eq!(on.clips.len(), 1);
    assert_eq!(on.clips[0].start, 0);

    let state = place(kit);
    assert_eq!(state.song.tracks.len(), tracks + 1, "a studio kit track");
    assert!(state.song.tracks[kit as usize - 1].clips.is_empty());
    let made = &state.song.tracks[kit as usize];
    assert_eq!(made.clips.len(), 1);
    assert_eq!(
        made.sound,
        SoundView::Builtin {
            instrument: InstrumentKind::StudioKit
        }
    );
    drop(core);
    let _ = std::fs::remove_dir_all(&root);
}
