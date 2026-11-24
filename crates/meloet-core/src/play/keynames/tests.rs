use meloet_model::BuiltinInstrument;

use super::*;
use crate::ai::{AiAsk, AiInstrument};
use crate::settings::Settings;
use crate::state::{Input, Model};

/// The screen's wording of the drum names, `"<prefix><id>": e("<name>"`, in file order.
fn screen_names(prefix: &str) -> Vec<(String, String)> {
    let ja = include_str!("../../../../../ui/src/text/ja.ts");
    let mut names = Vec::new();
    for line in ja.lines() {
        let Some(rest) = line.trim().strip_prefix(&format!("\"{prefix}")) else {
            continue;
        };
        let Some((id, rest)) = rest.split_once("\": e(\"") else {
            continue;
        };
        let Some((name, _)) = rest.split_once('"') else {
            continue;
        };
        names.push((id.to_owned(), name.to_owned()));
    }
    names
}

#[test]
fn the_names_are_the_ones_the_screen_shows() {
    let kit: Vec<(String, String)> = (KIT_NAMES.iter())
        .map(|(id, name)| (id.to_string(), name.to_string()))
        .collect();
    assert_eq!(kit, screen_names("drum."));
    for piece in DrumPiece::ROWS {
        assert_ne!(kit_name(piece), piece.id(), "{piece:?} has a name");
    }
    let gm: Vec<(String, String)> = (GM_NAMES.iter().enumerate())
        .map(|(at, name)| ((usize::from(GM_FIRST) + at).to_string(), name.to_string()))
        .collect();
    assert_eq!(gm, screen_names("gmDrum."));
    assert_eq!(gm_name(34), None);
    assert_eq!(gm_name(36), Some("キック"));
    assert_eq!(gm_name(81), Some("トライアングル"));
    assert_eq!(gm_name(82), None);
}

fn ask(m: &mut Model, track: u32) -> AiAnswer {
    m.handle(Input::Ai(AiAsk::KeyNames { track }));
    m.take_ai_answer().expect("an answer")
}

fn known(answer: AiAnswer) -> AiKeyMap {
    match answer {
        AiAnswer::KeyNames(map) => *map,
        other => panic!("{other:?}"),
    }
}

fn add(m: &mut Model, b: BuiltinInstrument) -> u32 {
    m.handle(Input::Ai(AiAsk::AddTrack {
        instrument: AiInstrument::Builtin(b),
    }));
    match m.take_ai_answer() {
        Some(AiAnswer::Done(done)) => done.track.expect("a track"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_built_in_kit_names_its_twelve_drums() {
    let m = &mut Model::new(Settings::default());
    let electro = add(m, BuiltinInstrument::ElectroKit);
    let studio = add(m, BuiltinInstrument::StudioKit);
    let steps = m.ai_song().undo_count;

    let map = known(ask(m, electro));
    assert!(map.kit && map.certain);
    assert_eq!(map.source, AiKeySource::Builtin);
    let keys: Vec<(u8, &str, &str)> = (map.keys.iter())
        .map(|k| (k.key, k.name.as_deref().unwrap(), k.id.unwrap()))
        .collect();
    assert_eq!(keys.len(), 12);
    assert_eq!(keys[0], (36, "キック", "kick"));
    assert_eq!(keys[2], (38, "スネア", "snare"));
    assert_eq!(keys[4], (42, "クローズハイハット", "closedHiHat"));
    assert_eq!(keys[11], (51, "ライド", "ride"));
    assert!(keys.windows(2).all(|w| w[0].0 < w[1].0));
    // Computed: every drum sounds.
    assert!(map.keys.iter().all(|k| k.sounds == Some(true)));
    assert_eq!(map.range, Some((36, 51)));
    assert!(map.notes.is_empty());

    // Sampled, and no sound pack in this test: the names, and that it plays a sine.
    let map = known(ask(m, studio));
    assert!(map.kit);
    assert_eq!(map.keys.len(), 12);
    assert!(map.keys.iter().all(|k| k.sounds.is_none()));
    assert_eq!(map.notes, [AiKeyNote::NotInstalled]);
    assert_eq!(m.ai_song().undo_count, steps, "reading is no step");
}

#[test]
fn an_instrument_has_no_names() {
    let m = &mut Model::new(Settings::default());
    let lead = add(m, BuiltinInstrument::SynthLead);
    let map = known(ask(m, lead));
    assert!(!map.kit && map.certain);
    assert_eq!(map.source, AiKeySource::None);
    assert!(map.keys.is_empty());
    assert_eq!(map.range, Some((0, 127)));
    assert_eq!(ask(m, 99), AiAnswer::Refused(AiRefusal::NoSuchTrack(99)));
    assert_eq!(ask(m, 0), AiAnswer::Refused(AiRefusal::NoSuchTrack(0)));
}

fn plugin_sound() -> InstrumentRef {
    InstrumentRef::Plugin(Arc::new(meloet_model::PluginRef::new(
        "lv2:urn:x",
        "X",
        "v",
        "/x",
    )))
}

#[test]
fn a_plugin_is_named_by_what_it_says_and_guessed_when_it_is_a_drum_that_says_nothing() {
    let named = vec![
        KeyName {
            key: 38,
            name: " Snare Center ".into(),
        },
        KeyName {
            key: 36,
            name: "Kick Drum".into(),
        },
        KeyName {
            key: 36,
            name: "Kick again".into(),
        },
        KeyName {
            key: 200,
            name: "Too high".into(),
        },
        KeyName {
            key: 40,
            name: "  ".into(),
        },
    ];
    let map = plugin_keys(
        plugin_sound(),
        &super::super::keys::cleaned(Some(named)),
        false,
    );
    assert!(map.kit && map.certain);
    assert_eq!(map.source, AiKeySource::Lv2Midnam);
    let keys: Vec<(u8, Option<&str>)> = (map.keys.iter())
        .map(|k| (k.key, k.name.as_deref()))
        .collect();
    assert_eq!(keys, [(36, Some("Kick Drum")), (38, Some("Snare Center"))]);

    let map = plugin_keys(plugin_sound(), &[], true);
    assert!(map.kit && !map.certain);
    assert_eq!(map.source, AiKeySource::GeneralMidiGuess);
    assert_eq!(map.keys.len(), 47);
    assert_eq!(map.keys[1].name.as_deref(), Some("キック"));

    let map = plugin_keys(plugin_sound(), &[], false);
    assert!(!map.kit && map.certain && map.keys.is_empty());
    assert_eq!(map.source, AiKeySource::None);
}

#[test]
fn a_bank_in_memory_says_which_keys_sound() {
    let mut map = map_of(InstrumentRef::Builtin(BuiltinInstrument::StudioKit), true);
    map.keys = vec![
        AiKey {
            key: 36,
            name: Some("キック".into()),
            id: Some("kick"),
            sounds: None,
            source: AiKeySource::Builtin,
        },
        AiKey {
            key: 39,
            name: Some("クラップ".into()),
            id: Some("clap"),
            sounds: None,
            source: AiKeySource::Builtin,
        },
    ];
    let json = r#"{"format":"meloet-instrument","version":1,"kind":"studioKit","regions":[
        {"sample":"a.wav","keyLo":36,"keyHi":36,"root":36},
        {"sample":"a.wav","keyLo":40,"keyHi":40,"root":40}]}"#;
    let dir = std::env::temp_dir().join(format!("k11keys-bank-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_wav(&dir.join("a.wav"));
    std::fs::write(dir.join("instrument.json"), json).unwrap();
    let bank = meloet_sound::sampler::load_instrument(&dir.join("instrument.json"), 1);
    let _ = std::fs::remove_dir_all(&dir);
    let bank = bank.expect("the little bank reads");
    sounding(&mut map, &bank);
    let keys: Vec<(u8, Option<bool>, bool)> = (map.keys.iter())
        .map(|k| (k.key, k.sounds, k.name.is_some()))
        .collect();
    assert_eq!(
        keys,
        [
            (36, Some(true), true),
            (39, Some(false), true),
            (40, Some(true), false)
        ]
    );
    assert_eq!(map.range, Some((36, 40)));
}

/// A short mono 16-bit WAV.
fn write_wav(path: &std::path::Path) {
    let frames: Vec<i16> = (0..480).map(|i| ((i % 48) * 500) as i16).collect();
    let data: Vec<u8> = frames.iter().flat_map(|s| s.to_le_bytes()).collect();
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + data.len() as u32).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(48_000u32.to_le_bytes());
    wav.extend(96_000u32.to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend((data.len() as u32).to_le_bytes());
    wav.extend(data);
    std::fs::write(path, wav).unwrap();
}
