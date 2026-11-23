//! What each key of a track's sound is (`AiAsk::KeyNames`) through the whole core: plugins in
//! the real `meloet-plughost` with its fake `drums` bundle (one names its keys, one does not),
//! and SF2 presets of a little file in meloet's SF2 folder, read off the core thread.

#[allow(dead_code)]
#[path = "../../meloet-sound/tests/support/sf2.rs"]
mod font;
mod plughost;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use meloet_core::ai::{
    AiAnswer, AiAsk, AiInstrument, AiKeyMap, AiKeyNames, AiKeyNote, AiKeySource, AiRefusal,
};
use meloet_core::play::keynames::read_sf2;
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use meloet_protocol::{AppState, PluginState, SoundView};

use font::{Font, Inst, KEY_RANGE, Preset, SAMPLE_ID, Smp, flat, range};
use plughost::{temp_dir, wait_for};

const PATIENCE: Duration = Duration::from_secs(60);
const DRUMS: &str = "lv2:urn:meloet:fake:drums";
const KIT: &str = "lv2:urn:meloet:fake:kit";
const SF2: &str = "Kit.sf2";

fn options(root: &Path) -> CoreOptions {
    let bundle = root.join("lv2/drums.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
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

/// Asks until the plugin has answered.
fn names(core: &Core, track: u32) -> AiKeyMap {
    wait_for(PATIENCE, || match core.ai(AiAsk::KeyNames { track }) {
        AiAnswer::KeyNames(AiKeyNames::Known(map)) => Some(*map),
        AiAnswer::KeyNames(AiKeyNames::Waiting) => None,
        other => panic!("{other:?}"),
    })
    .expect("the plugin names its keys")
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
    state_when(&core, "the scan", |s| s.library.plugins.len() == 2);

    // A plugin that names its keys (LV2 midnam in a real one).
    let drums = add(&core, AiInstrument::Plugin(DRUMS.into()));
    let kit = add(&core, AiInstrument::Plugin(KIT.into()));
    let ready = |s: &AppState, track: u32| {
        let view = &s.song.tracks[track as usize - 1].sound;
        matches!(view, SoundView::Plugin(p) if p.state == PluginState::Ready)
    };
    state_when(&core, "both plugins sound", |s| {
        ready(s, drums) && ready(s, kit)
    });
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
