//! The names an SF2 preset's samples give its keys (`Sf2File::key_names`): on a little file
//! made here, and on FluidR3_GM when this PC has it.

mod support;

use std::path::PathBuf;

use meloet_sound::sf2::{Sf2Error, Sf2File};
use support::sf2::*;

fn half(name: &'static str, kind: u16) -> Smp {
    let mut sample = Smp::new(name, flat(1000, 480), 48_000, 60);
    sample.kind = kind;
    sample
}

/// A kit in the drum bank and a piano: a kick in two stereo halves and two velocity layers,
/// a snare on two keys, a clap and a rim of different names on one key, a stereo sample whose
/// name has no mark, and a wide zone.
fn kit() -> Font {
    let samples = vec![
        half("Std Kick(L)", 4),
        half("Std Kick(R)", 2),
        half("Kick Soft", 1),
        half("Snare 1", 1),
        half("Clap", 1),
        half("Rim", 1),
        half("Ride L", 1),
        half("Pad", 1),
    ];
    let zone = |lo, hi, sample| vec![(KEY_RANGE, range(lo, hi)), (SAMPLE_ID, sample)];
    let layer = |velocities: (u8, u8), sample| {
        vec![
            (KEY_RANGE, range(36, 36)),
            (VEL_RANGE, range(velocities.0, velocities.1)),
            (SAMPLE_ID, sample),
        ]
    };
    let drums = vec![
        layer((64, 127), 0),
        layer((64, 127), 1),
        layer((1, 63), 2),
        zone(38, 39, 3),
        zone(40, 40, 4),
        zone(40, 40, 5),
        zone(51, 51, 6),
        zone(60, 72, 7),
    ];
    let insts = vec![
        Inst {
            name: "drums",
            zones: drums,
        },
        Inst {
            name: "keys",
            zones: vec![zone(21, 108, 7)],
        },
    ];
    let presets = vec![
        Preset {
            name: "Kit",
            bank: 128,
            program: 0,
            zones: vec![vec![(INSTRUMENT, 0)]],
        },
        Preset {
            name: "Piano",
            bank: 0,
            program: 0,
            zones: vec![vec![(INSTRUMENT, 1)]],
        },
    ];
    Font::new(samples, insts, presets)
}

#[test]
fn a_kits_keys_are_named_by_the_samples_on_them() {
    let file = TempFile::new("keynames-kit", &kit().bytes());
    let names = file.open().key_names(128, 0).unwrap();
    let named: Vec<(u8, Option<&str>)> = (names.iter())
        .map(|(key, name)| (*key, name.as_deref()))
        .filter(|(key, _)| *key < 60)
        .collect();
    assert_eq!(
        named,
        [
            (36, Some("Std Kick / Kick Soft")),
            (38, Some("Snare 1")),
            (39, Some("Snare 1")),
            (40, Some("Clap / Rim")),
            (51, Some("Ride L")),
        ]
    );
    // A zone of 13 keys plays one sound at different pitches: no names.
    let wide: Vec<u8> = (names.iter())
        .filter(|(_, name)| name.is_none())
        .map(|(key, _)| *key)
        .collect();
    assert_eq!(wide, (60..=72).collect::<Vec<u8>>());
}

#[test]
fn an_instrument_has_keys_and_no_names() {
    let file = TempFile::new("keynames-piano", &kit().bytes());
    let names = file.open().key_names(0, 0).unwrap();
    assert_eq!(names.first(), Some(&(21, None)));
    assert_eq!(names.last(), Some(&(108, None)));
    assert_eq!(names.len(), 88);
    assert!(matches!(
        file.open().key_names(0, 5),
        Err(Sf2Error::NoPreset {
            bank: 0,
            program: 5
        })
    ));
}

fn fluid_r3() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        format!("{home}/.cache/meloet/sounds-src/fluidr3/FluidR3_GM.sf2"),
        "/usr/share/sounds/sf2/FluidR3_GM.sf2".to_string(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
    .or_else(|| {
        eprintln!("FluidR3_GM.sf2 is not on this PC: nothing checked");
        None
    })
}

#[test]
fn fluid_r3s_standard_kit_names_its_drums() {
    let Some(path) = fluid_r3() else { return };
    let file = Sf2File::open(&path).unwrap();
    let names = file.key_names(128, 0).unwrap();
    let name = |key: u8| {
        let found = names.iter().find(|(k, _)| *k == key);
        found.and_then(|(_, name)| name.as_deref())
    };
    for (key, name) in &names {
        eprintln!("{key} {}", name.as_deref().unwrap_or("-"));
    }
    assert_eq!(name(35), Some("Std Kick"));
    assert_eq!(name(36), Some("Std Kick"));
    assert_eq!(name(38), Some("Std Snr 1"));
    assert_eq!(name(42), Some("Hi-Hat Closed"));
    assert_eq!(name(46), Some("Hi-Hat Half-Open"));
    assert_eq!(name(49), Some("Crsh 1"));
    assert!(names.iter().all(|(_, name)| name.is_some()), "{names:?}");
    let piano = file.key_names(0, 0).unwrap();
    assert!(piano.iter().all(|(_, name)| name.is_none()));
}
