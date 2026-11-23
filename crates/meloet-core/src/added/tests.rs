use std::sync::Arc;

use meloet_model::{Kept, SF2_DRUM_BANK, Sf2Ref, StripId, edit, mixer};
use meloet_protocol::{
    PluginFormatView, PluginKindView, PluginRowView, Sf2FileView, Sf2PresetView,
};

use super::*;

const SURGE: &str = "lv2:https://surge-synthesizer.github.io/lv2/surge-xt";
const MVERB: &str = "lv2:http://distrho.sf.net/plugins/MVerb";

fn plugin(key: &str, name: &str, path: &str) -> Arc<PluginRef> {
    Arc::new(PluginRef::new(key, name, "A Vendor", path))
}

fn preset(file: &str, bank: u16, program: u8, name: &str) -> InstrumentRef {
    InstrumentRef::Sf2(Arc::new(Sf2Ref {
        file: file.into(),
        sha256: "".into(),
        bank,
        program,
        name: name.into(),
        kept: Kept::NONE,
    }))
}

/// Tracks: the piano, Surge XT with MVerb in its first slot, two presets of Old.sf2, one of
/// FluidR3_GM.sf2. MVerb also on the master.
fn song() -> Doc {
    let mut doc = Doc::new_default();
    let surge = InstrumentRef::Plugin(plugin(SURGE, "Surge XT", "/home/me/.lv2/Surge XT.lv2"));
    for instrument in [
        surge,
        preset("Old.sf2", 0, 0, "Grand"),
        preset("Old.sf2", SF2_DRUM_BANK, 0, "Kit"),
        preset("FluidR3_GM.sf2", 0, 27, "Clean Guitar"),
    ] {
        doc = edit::add_track(&doc, None, instrument, meloet_model::Language::Ja)
            .unwrap()
            .0
            .doc;
    }
    let surge_track = StripId::Track(doc.tracks[1].id);
    let mverb = || plugin(MVERB, "MVerb", "");
    doc = mixer::add_plugin_effect(&doc, surge_track, 0, mverb())
        .unwrap()
        .doc;
    mixer::add_plugin_effect(&doc, StripId::Master, 2, mverb())
        .unwrap()
        .doc
}

fn library(files: &[(&str, &[(u16, u8)])], keys: &[&str]) -> LibraryView {
    LibraryView {
        sf2: files
            .iter()
            .map(|(file, presets)| Sf2FileView {
                file: (*file).into(),
                presets: presets
                    .iter()
                    .map(|&(bank, program)| Sf2PresetView {
                        bank,
                        program,
                        name: String::new(),
                        family: GmFamily::Piano,
                    })
                    .collect(),
            })
            .collect(),
        plugins: keys
            .iter()
            .map(|key| PluginRowView {
                name: String::new(),
                vendor: String::new(),
                kind: PluginKindView::Effect(meloet_protocol::PluginEffectCategory::Other),
                key: "vst3:00".into(),
                formats: vec![
                    PluginFormatView {
                        format: PluginFormat::Vst3,
                        key: "vst3:00".into(),
                    },
                    PluginFormatView {
                        format: PluginFormat::Lv2,
                        key: (*key).into(),
                    },
                ],
            })
            .collect(),
        opened_sf2: None,
        pack_missing: false,
    }
}

#[test]
fn what_the_song_uses_and_the_library_does_not_have() {
    let doc = song();
    let home = Path::new("/home/me");
    assert_eq!(
        missing(&doc, &LibraryView::default(), Some(home)),
        [
            MissingView::Sf2 {
                file: "Old.sf2".into(),
                tracks: 2
            },
            MissingView::Sf2 {
                file: "FluidR3_GM.sf2".into(),
                tracks: 1
            },
            MissingView::Plugin {
                key: SURGE.into(),
                name: "Surge XT".into(),
                vendor: "A Vendor".into(),
                format: Some(PluginFormat::Lv2),
                role: PluginRole::Instrument,
                shown_path: Some("~/.lv2/Surge XT.lv2".into()),
            },
            MissingView::Plugin {
                key: MVERB.into(),
                name: "MVerb".into(),
                vendor: "A Vendor".into(),
                format: Some(PluginFormat::Lv2),
                role: PluginRole::Effect,
                shown_path: None,
            },
        ],
        "each file and each plugin once, though MVerb sits in two slots"
    );
    // A plugin counts as there in whichever of its row's formats the song names it. A file
    // that is there is found, even when it lacks a preset the song uses.
    let has = library(&[("Old.sf2", &[(0, 0)])], &[MVERB]);
    assert_eq!(
        missing(&doc, &has, None),
        [
            MissingView::Sf2 {
                file: "FluidR3_GM.sf2".into(),
                tracks: 1
            },
            MissingView::Plugin {
                key: SURGE.into(),
                name: "Surge XT".into(),
                vendor: "A Vendor".into(),
                format: Some(PluginFormat::Lv2),
                role: PluginRole::Instrument,
                shown_path: Some("/home/me/.lv2/Surge XT.lv2".into()),
            },
        ]
    );
    let all = library(
        &[
            ("Old.sf2", &[(0, 0), (128, 0)]),
            ("FluidR3_GM.sf2", &[(0, 27)]),
        ],
        &[MVERB, SURGE],
    );
    assert_eq!(missing(&doc, &all, None), []);
    assert_eq!(
        missing(&Doc::new_default(), &LibraryView::default(), None),
        []
    );
}

#[test]
fn why_a_track_sounds_as_the_sine_wave() {
    let doc = song();
    let sounds = [BuiltinInstrument::GrandPiano, BuiltinInstrument::Sine];
    let why = |library: &LibraryView| -> Vec<Option<StandIn>> {
        (doc.tracks.iter())
            .map(|t| stand_in(&t.instrument, library, &sounds))
            .collect()
    };
    assert_eq!(
        why(&LibraryView::default()),
        [
            None,
            Some(StandIn::PluginMissing),
            Some(StandIn::Sf2Missing),
            Some(StandIn::Sf2Missing),
            Some(StandIn::Sf2Missing)
        ]
    );
    // The kit of Old.sf2 is not in the file that is there: its track still stands in.
    let has = library(&[("Old.sf2", &[(0, 0)])], &[SURGE]);
    assert_eq!(
        why(&has),
        [
            None,
            None,
            None,
            Some(StandIn::Sf2Missing),
            Some(StandIn::Sf2Missing)
        ]
    );
    let strings = InstrumentRef::Builtin(BuiltinInstrument::Strings);
    assert_eq!(
        stand_in(&strings, &has, &sounds),
        Some(StandIn::PackMissing),
        "a sound of the pack that this PC does not have"
    );
    assert_eq!(builtin_or_sine(&strings), BuiltinInstrument::Strings);
    for t in &doc.tracks[1..] {
        assert_eq!(builtin_or_sine(&t.instrument), BuiltinInstrument::Sine);
    }
}

#[test]
fn the_song_s_sounds_and_slots_as_the_screen_names_them() {
    let doc = song();
    let views: Vec<SoundView> = (doc.tracks.iter())
        .map(|t| sound_view(&t.instrument, PluginState::Loading))
        .collect();
    assert_eq!(
        views[..2],
        [
            SoundView::Builtin {
                instrument: meloet_protocol::InstrumentKind::GrandPiano
            },
            SoundView::Plugin(PluginView {
                key: SURGE.into(),
                name: "Surge XT".into(),
                format: Some(PluginFormat::Lv2),
                state: PluginState::Loading,
            }),
        ]
    );
    assert_eq!(
        views[3],
        SoundView::Sf2 {
            file: "Old.sf2".into(),
            bank: 128,
            program: 0,
            name: "Kit".into(),
            family: GmFamily::DrumKit,
        }
    );
    assert_eq!(
        sound_ref(&doc.tracks[4].instrument),
        SoundRef::Sf2 {
            file: "FluidR3_GM.sf2".into(),
            bank: 0,
            program: 27
        }
    );
    assert_eq!(
        sound_name(&doc.tracks[4].instrument),
        SoundName::Sf2 {
            name: "Clean Guitar".into(),
            file: "FluidR3_GM.sf2".into()
        }
    );
    assert_eq!(
        sound_name(&doc.tracks[1].instrument),
        SoundName::Plugin {
            name: "Surge XT".into()
        }
    );

    let mut master = (*doc.master).clone();
    master.inserts[0].as_mut().unwrap().bypassed = true;
    let slots = slots_view(&master.inserts, |slot| match slot {
        2 => PluginState::Stopped,
        _ => PluginState::Ready,
    });
    assert_eq!(
        slots,
        [
            Some(InsertView {
                id: master.inserts[0].as_ref().unwrap().id.0,
                effect: InsertEffectView::Builtin {
                    effect: EffectKind::Limiter
                },
                bypassed: true,
            }),
            None,
            Some(InsertView {
                id: master.inserts[2].as_ref().unwrap().id.0,
                effect: InsertEffectView::Plugin(PluginView {
                    key: MVERB.into(),
                    name: "MVerb".into(),
                    format: Some(PluginFormat::Lv2),
                    state: PluginState::Stopped,
                }),
                bypassed: false,
            }),
            None,
        ]
    );
    // A key of a format this meloet does not read is still shown, with no format.
    assert_eq!(format_of("clap:org.surge-synth-team.surge-xt"), None);
    assert_eq!(format_of("vst3:not hex"), None);
    assert_eq!(
        format_of("vst3:565354414263646566676869707172AB"),
        Some(PluginFormat::Vst3)
    );
}

#[test]
fn a_plugin_on_a_track_whose_sf2_file_is_found_is_still_looked_for() {
    let doc = song();
    let on_old = StripId::Track(doc.tracks[2].id);
    let chorus = plugin("lv2:urn:chorus", "Chorus", "");
    let doc = mixer::add_plugin_effect(&doc, on_old, 3, chorus)
        .unwrap()
        .doc;
    let has = library(
        &[
            ("Old.sf2", &[(0, 0), (128, 0)]),
            ("FluidR3_GM.sf2", &[(0, 27)]),
        ],
        &[MVERB, SURGE],
    );
    assert_eq!(
        missing(&doc, &has, None),
        [MissingView::Plugin {
            key: "lv2:urn:chorus".into(),
            name: "Chorus".into(),
            vendor: "A Vendor".into(),
            format: Some(PluginFormat::Lv2),
            role: PluginRole::Effect,
            shown_path: None,
        }]
    );
}
