use meloet_plugin::scan::{BundleRecord, BundleStamp};
use meloet_plugin::{PluginKey, PluginRole};

use super::*;

pub(crate) const SURGE_VST3: &str = "vst3:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
pub(crate) const SURGE_LV2: &str = "lv2:https://surge-synthesizer.github.io/lv2/surge-xt";
pub(crate) const COMP_LV2: &str = "lv2:urn:zam:ZamComp";
pub(crate) const COMP_VST3: &str = "vst3:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
pub(crate) const DRUMS_LV2: &str = "lv2:urn:x:drums";

pub(crate) fn info(
    key: &str,
    name: &str,
    vendor: &str,
    kind: PluginKind,
    window: bool,
) -> PluginInfo {
    let instrument = kind.role() == PluginRole::Instrument;
    PluginInfo {
        key: key.parse::<PluginKey>().unwrap(),
        name: name.into(),
        vendor: vendor.into(),
        version: "1".into(),
        kind,
        editor: if window {
            EditorKind::X11
        } else {
            EditorKind::Unsupported
        },
        audio_inputs: if instrument { 0 } else { 2 },
        audio_outputs: 2,
    }
}

pub(crate) fn bundle(path: &str, plugins: Vec<PluginInfo>) -> BundleRecord {
    BundleRecord {
        path: path.into(),
        format: plugins[0].key.format(),
        stamp: BundleStamp {
            modified_ns: 1,
            size: 1,
        },
        plugins,
        stopped: Vec::new(),
    }
}

/// Surge XT as VST3 (a synth) and as LV2 (LV2 says no more than "an instrument"), ZamComp as
/// LV2 (with a window meloet opens) and as VST3 (without), and a drum machine as LV2.
pub(crate) fn scanned() -> ScanCache {
    let synth = PluginKind::Instrument(InstrumentCategory::Synth);
    let other = PluginKind::Instrument(InstrumentCategory::Other);
    let drum = PluginKind::Instrument(InstrumentCategory::Drum);
    let dynamics = PluginKind::Effect(EffectCategory::Dynamics);
    let mut cache = ScanCache::default();
    for record in [
        bundle(
            "/p/lv2/Surge XT.lv2",
            vec![info(SURGE_LV2, "Surge XT", "Surge Synth Team", other, true)],
        ),
        bundle(
            "/p/vst3/Surge XT.vst3",
            vec![info(
                SURGE_VST3,
                "Surge XT",
                "Surge Synth Team",
                synth,
                true,
            )],
        ),
        bundle(
            "/p/lv2/ZamComp.lv2",
            vec![
                info(COMP_LV2, "ZamComp", "Damien Zammit", dynamics, true),
                info(DRUMS_LV2, "beat box", "x", drum, false),
            ],
        ),
        bundle(
            "/p/vst3/ZamComp.vst3",
            vec![info(COMP_VST3, "ZamComp", "Damien Zammit", dynamics, false)],
        ),
        bundle(
            "/p/vst3/Surge XT Effects.vst3",
            vec![info(
                "vst3:CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                "Surge XT Effects",
                "Surge Synth Team",
                PluginKind::Effect(EffectCategory::Other),
                true,
            )],
        ),
    ] {
        cache.put(record);
    }
    cache
}

fn catalog() -> Catalog {
    let mut catalog = Catalog::new(ShelfSettings::default());
    catalog.set_plugins(&scanned());
    catalog
}

fn row<'a>(view: &'a LibraryView, name: &str) -> &'a PluginRowView {
    (view.plugins.iter().find(|r| r.name == name)).unwrap_or_else(|| panic!("no row {name}"))
}

#[test]
fn the_formats_of_a_plugin_are_one_row_in_the_order_of_their_names() {
    let view = catalog().view(&[], false);
    let names: Vec<&str> = view.plugins.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["beat box", "Surge XT", "Surge XT Effects", "ZamComp"],
        "without regard to case; a name that only starts the same is another row"
    );
    let surge = row(&view, "Surge XT");
    assert_eq!(surge.vendor, "Surge Synth Team");
    assert_eq!(
        surge.kind,
        PluginKindView::Instrument(PluginInstrumentCategory::Synth),
        "the first kind that says something"
    );
    assert_eq!(
        surge.formats,
        [
            PluginFormatView {
                format: PluginFormat::Vst3,
                key: SURGE_VST3.into()
            },
            PluginFormatView {
                format: PluginFormat::Lv2,
                key: SURGE_LV2.into()
            },
        ]
    );
    assert_eq!(surge.key, SURGE_VST3, "both open a window: VST3 before LV2");
    let comp = row(&view, "ZamComp");
    assert_eq!(comp.key, COMP_LV2, "the one whose window meloet opens");
    assert_eq!(
        comp.kind,
        PluginKindView::Effect(PluginEffectCategory::Dynamics)
    );
    assert_eq!(row(&view, "beat box").formats.len(), 1);
}

#[test]
fn the_format_a_row_uses_is_the_chosen_one_then_the_one_of_the_song() {
    let mut catalog = catalog();
    let key = |c: &Catalog, song: &[&str], name: &str| row(&c.view(song, false), name).key.clone();
    assert_eq!(key(&catalog, &[SURGE_LV2], "Surge XT"), SURGE_LV2);
    assert_eq!(key(&catalog, &[COMP_VST3], "ZamComp"), COMP_VST3);
    assert_eq!(key(&catalog, &[COMP_VST3], "Surge XT"), SURGE_VST3);

    assert!(!catalog.use_format("lv2:urn:not:there"));
    assert!(catalog.use_format(COMP_VST3));
    assert!(!catalog.use_format(COMP_VST3), "chosen already");
    assert_eq!(key(&catalog, &[], "ZamComp"), COMP_VST3);
    assert_eq!(
        key(&catalog, &[COMP_LV2], "ZamComp"),
        COMP_VST3,
        "what the person chose comes before what the song uses"
    );
    assert!(catalog.use_format(COMP_LV2));
    assert_eq!(catalog.settings().plugin_formats, [COMP_LV2]);
    assert_eq!(key(&catalog, &[], "ZamComp"), COMP_LV2);
}

#[test]
fn a_plugin_is_starred_as_a_row_and_effects_start_with_two_stars() {
    let mut catalog = catalog();
    let effect = |effect| EffectRef::Builtin { effect };
    let plugin = |key: &str| SoundRef::Plugin { key: key.into() };
    use meloet_protocol::EffectKind::{Chorus, Delay, Reverb};
    assert_eq!(
        catalog.favorite_effects(&[]),
        [effect(Reverb), effect(Delay)]
    );
    assert!(catalog.toggle_effect(effect(Reverb)));
    assert!(catalog.toggle_effect(effect(Chorus)));
    assert!(catalog.toggle_effect(EffectRef::Plugin {
        key: COMP_VST3.into()
    }));
    assert_eq!(
        catalog.favorite_effects(&[]),
        [
            effect(Delay),
            effect(Chorus),
            EffectRef::Plugin {
                key: COMP_LV2.into()
            }
        ],
        "named by the key the row uses"
    );
    // An instrument is no effect, and what the shelf does not have gets no star.
    assert!(!catalog.toggle_effect(EffectRef::Plugin {
        key: SURGE_VST3.into()
    }));
    assert!(!catalog.toggle_sound(plugin(COMP_LV2)));
    assert!(!catalog.toggle_sound(plugin("lv2:urn:not:there")));

    assert!(catalog.toggle_sound(plugin(SURGE_LV2)));
    assert_eq!(catalog.favorite_sounds(&[]), [plugin(SURGE_VST3)]);
    assert!(catalog.use_format(SURGE_LV2));
    assert_eq!(catalog.favorite_sounds(&[]), [plugin(SURGE_LV2)]);
    assert!(
        catalog.toggle_sound(plugin(SURGE_VST3)),
        "the star of the row"
    );
    assert_eq!(catalog.favorite_sounds(&[]), []);
}

#[test]
fn a_plugin_is_taken_into_a_song_with_its_bundle_and_its_phrase() {
    let catalog = catalog();
    let surge = catalog.plugin(SURGE_LV2).unwrap();
    assert!(surge.instrument);
    assert_eq!(&*surge.plugin.key, SURGE_LV2);
    assert_eq!(&*surge.plugin.name, "Surge XT");
    assert_eq!(&*surge.plugin.path, "/p/lv2/Surge XT.lv2");
    assert!(surge.plugin.state.is_empty());
    assert_eq!(surge.phrase, InstrumentGroup::Synth);
    assert_eq!(
        catalog.plugin(DRUMS_LV2).unwrap().phrase,
        InstrumentGroup::Drums
    );
    assert!(!catalog.plugin(COMP_LV2).unwrap().instrument);
    assert!(catalog.plugin("lv2:urn:not:there").is_none());
    // An effect is no sound for a track.
    assert!(
        catalog
            .instrument(&SoundRef::Plugin {
                key: COMP_LV2.into()
            })
            .is_none()
    );
    assert_eq!(catalog.bundles().len(), 6);
}
