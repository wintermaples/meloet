use meloet_plugin::scan::{
    BundleRecord, BundleStamp, EditorKind, PluginInfo, StopReason, StoppedEntry,
};
use meloet_plugin::{EffectCategory, PluginFormat as Format, PluginKey, PluginKind};
use meloet_protocol::{PluginFormat, PluginPlaceView, ScanStopReason, ScanView, StoppedPluginView};

use super::*;
use crate::play::tests::{cmd, ready, status};
use crate::plugins::ScanWhat;

fn usual() -> Vec<PathBuf> {
    vec!["/p/lv2".into(), "/p/vst3".into()]
}

fn plugin(id: &str) -> PluginInfo {
    PluginInfo {
        key: PluginKey::lv2(id).unwrap(),
        name: format!("Plugin {id}"),
        vendor: "Maker".into(),
        version: "1".into(),
        kind: PluginKind::Effect(EffectCategory::Other),
        editor: EditorKind::None,
        audio_inputs: 2,
        audio_outputs: 2,
    }
}

fn record(
    path: &str,
    plugins: &[&str],
    stopped: &[(Option<&str>, &str, StopReason)],
) -> BundleRecord {
    BundleRecord {
        path: path.into(),
        format: if path.ends_with(".vst3") {
            Format::Vst3
        } else {
            Format::Lv2
        },
        stamp: BundleStamp {
            modified_ns: 1,
            size: 1,
        },
        plugins: plugins.iter().map(|id| plugin(id)).collect(),
        stopped: stopped
            .iter()
            .map(|&(id, name, reason)| StoppedEntry {
                id: id.map(str::to_owned),
                name: name.into(),
                reason,
            })
            .collect(),
    }
}

/// `ready()` with the test's places in place of the ones of this PC.
fn model() -> Model {
    ready().with_plugin_places(usual())
}

fn start(m: &mut Model, cache: ScanCache, first: bool) -> Vec<Effect> {
    m.handle(Input::PluginsStart {
        cache: Box::new(cache),
        first,
    })
}

fn jobs(effects: &[Effect]) -> Vec<ScanJob> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::ScanPlugins(job) => Some((**job).clone()),
            _ => None,
        })
        .collect()
}

fn saved(effects: &[Effect]) -> Option<ScanCache> {
    effects.iter().find_map(|e| match e {
        Effect::SavePluginScan(cache) => Some((**cache).clone()),
        _ => None,
    })
}

fn news(m: &mut Model, news: ScanNews) -> Vec<Effect> {
    m.handle(Input::PluginScan(news))
}

fn bundle(m: &mut Model, job: u64, record: BundleRecord) -> Vec<Effect> {
    news(
        m,
        ScanNews::Bundle {
            job,
            record: Box::new(record),
        },
    )
}

fn counted(m: &mut Model, job: u64, total: u32) {
    let gone = Vec::new();
    news(m, ScanNews::Counted { job, total, gone });
}

fn scan_view(m: &Model) -> ScanView {
    m.view().plugins.scan
}

/// A cache with a good bundle, a bundle that stopped as a whole, and one with a plugin that
/// stopped.
fn three() -> [BundleRecord; 3] {
    [
        record(
            "/p/vst3/Old.vst3",
            &[],
            &[(None, "Old", StopReason::TimedOut)],
        ),
        record("/p/lv2/good.lv2", &["urn:a", "urn:b"], &[]),
        record(
            "/p/lv2/mixed.lv2",
            &["urn:c", "urn:e"],
            &[(Some("urn:d"), "Fog", StopReason::Crashed)],
        ),
    ]
}

fn scanned() -> (Model, ScanCache) {
    let cache = ScanCache {
        bundles: three().to_vec(),
    };
    let mut m = model();
    let job = jobs(&start(&mut m, cache.clone(), false))[0].id;
    counted(&mut m, job, 0);
    news(&mut m, ScanNews::Done { job });
    (m, cache)
}

#[test]
fn the_first_scan_counts_and_says_what_it_put_on_the_shelf() {
    let mut m = model();
    assert!(
        jobs(&cmd(&mut m, Command::RescanPlugins)).is_empty(),
        "no scanner yet"
    );
    let asked = jobs(&start(&mut m, ScanCache::default(), true));
    assert_eq!(asked.len(), 1);
    let places = usual();
    assert_eq!(asked[0].what, ScanWhat::Places { places, all: false });
    assert_eq!(asked[0].known, ScanCache::default());
    let job = asked[0].id;
    assert_eq!(scan_view(&m), ScanView::Idle, "not before it has counted");

    counted(&mut m, job, 3);
    assert_eq!(scan_view(&m), ScanView::Running { done: 0, total: 3 });
    let [old, good, mixed] = three();
    bundle(&mut m, job, old);
    bundle(&mut m, job + 7, record("/p/lv2/other.lv2", &["urn:z"], &[]));
    assert_eq!(scan_view(&m), ScanView::Running { done: 1, total: 3 });
    // What is done is there before the scan is over.
    assert_eq!(m.view().plugins.stopped.len(), 1);
    bundle(&mut m, job, good);
    bundle(&mut m, job, mixed);
    assert_eq!(scan_view(&m), ScanView::Running { done: 3, total: 3 });

    let effects = news(&mut m, ScanNews::Done { job });
    assert_eq!(saved(&effects).map(|c| c.bundles.len()), Some(3));
    let said = StatusMessage::ScanFinished {
        added: 4,
        stopped: 2,
    };
    assert_eq!(status(&m), Some(said));
    let finished = ScanView::Finished {
        added: 4,
        stopped: 2,
    };
    assert_eq!(scan_view(&m), finished);
    // In the order of the places, whatever the order the bundles were done in.
    assert_eq!(
        m.view().plugins.stopped,
        [
            StoppedPluginView {
                name: "Fog".into(),
                format: PluginFormat::Lv2,
                reason: ScanStopReason::Crashed,
                path: "/p/lv2/mixed.lv2".into(),
                id: Some("urn:d".into()),
                shown_path: "/p/lv2/mixed.lv2".into(),
                retrying: false,
            },
            StoppedPluginView {
                name: "Old".into(),
                format: PluginFormat::Vst3,
                reason: ScanStopReason::TimedOut,
                path: "/p/vst3/Old.vst3".into(),
                id: None,
                shown_path: "/p/vst3/Old.vst3".into(),
                retrying: false,
            },
        ]
    );
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_ne!(
        status(&m),
        Some(StatusMessage::ScanFinished {
            added: 4,
            stopped: 2
        })
    );
    assert_eq!(scan_view(&m), finished, "the strip stays");
}

#[test]
fn a_first_scan_that_finds_no_bundle_says_nothing() {
    let mut m = model();
    let before = status(&m);
    let job = jobs(&start(&mut m, ScanCache::default(), true))[0].id;
    counted(&mut m, job, 0);
    let effects = news(&mut m, ScanNews::Done { job });
    assert_eq!(
        saved(&effects),
        Some(ScanCache::default()),
        "so the next start is not the first"
    );
    assert_eq!(status(&m), before);
    assert_eq!(scan_view(&m), ScanView::Idle);
}

#[test]
fn from_the_second_start_only_new_plugins_are_told() {
    let cache = ScanCache {
        bundles: three().to_vec(),
    };
    let mut m = model();
    let before = status(&m);
    let asked = jobs(&start(&mut m, cache.clone(), false));
    assert_eq!(asked[0].known, cache);
    let job = asked[0].id;
    let gone = vec![PathBuf::from("/p/vst3/Old.vst3")];
    news(
        &mut m,
        ScanNews::Counted {
            job,
            total: 2,
            gone,
        },
    );
    assert_eq!(scan_view(&m), ScanView::Idle, "nothing is shown");
    assert_eq!(
        m.view().plugins.stopped.len(),
        1,
        "the bundle that is gone is forgotten"
    );
    // One bundle is new, and one that was known has one plugin more.
    bundle(&mut m, job, record("/p/lv2/new.lv2", &["urn:n"], &[]));
    bundle(
        &mut m,
        job,
        record("/p/lv2/good.lv2", &["urn:a", "urn:b", "urn:b2"], &[]),
    );
    let effects = news(&mut m, ScanNews::Done { job });
    assert_eq!(saved(&effects).map(|c| c.bundles.len()), Some(3));
    assert_eq!(status(&m), Some(StatusMessage::PluginsFound { count: 2 }));
    assert_eq!(scan_view(&m), ScanView::Idle);

    // A start at which nothing changed says nothing.
    let (m, _) = scanned();
    assert_eq!(status(&m), before);
}

#[test]
fn scan_again_looks_at_everything_and_one_scan_runs_at_a_time() {
    let (mut m, cache) = scanned();
    let asked = jobs(&cmd(&mut m, Command::RescanPlugins));
    let places = usual();
    assert_eq!(asked[0].what, ScanWhat::Places { places, all: true });
    assert_eq!(asked[0].known, cache);
    let again = asked[0].id;
    assert!(jobs(&cmd(&mut m, Command::RescanPlugins)).is_empty());
    // A place added meanwhile waits for its turn.
    cmd(&mut m, Command::AddPluginPlace);
    let effects = m.handle(Input::PluginPlacePicked(Some("/home/me/Plugins".into())));
    assert!(jobs(&effects).is_empty());
    counted(&mut m, again, 2);
    assert_eq!(scan_view(&m), ScanView::Running { done: 0, total: 2 });
    bundle(&mut m, again, record("/p/lv2/good.lv2", &["urn:a"], &[]));
    let effects = news(&mut m, ScanNews::Done { job: again });
    let said = StatusMessage::ScanFinished {
        added: 3,
        stopped: 2,
    };
    assert_eq!(status(&m), Some(said));
    let next = jobs(&effects);
    let places = vec![PathBuf::from("/home/me/Plugins")];
    assert_eq!(next[0].what, ScanWhat::Places { places, all: false });
    assert_eq!(
        next[0].known.plugins().count(),
        3,
        "what the scan before it found"
    );
    counted(&mut m, next[0].id, 1);
    assert_eq!(
        scan_view(&m),
        ScanView::Finished {
            added: 3,
            stopped: 2
        },
        "a place added is scanned without a count"
    );
    bundle(
        &mut m,
        next[0].id,
        record("/home/me/Plugins/x.vst3", &["urn:x"], &[]),
    );
    news(&mut m, ScanNews::Done { job: next[0].id });
    assert_eq!(status(&m), Some(StatusMessage::PluginsFound { count: 1 }));
}

#[test]
fn a_stopped_plugin_is_tried_again_alone() {
    let (mut m, _) = scanned();
    let row = |m: &Model, at: usize| m.view().plugins.stopped[at].clone();
    let fog = Command::RetryStoppedPlugin {
        path: "/p/lv2/mixed.lv2".into(),
        id: Some("urn:d".into()),
    };
    let unknown = Command::RetryStoppedPlugin {
        path: "/p/lv2/mixed.lv2".into(),
        id: Some("urn:c".into()),
    };
    assert!(jobs(&cmd(&mut m, unknown)).is_empty());
    cmd(&mut m, Command::OpenScanStopped);
    let asked = jobs(&cmd(&mut m, fog.clone()));
    let what = ScanWhat::Retry {
        path: "/p/lv2/mixed.lv2".into(),
        id: Some("urn:d".into()),
    };
    assert_eq!(asked[0].what, what);
    assert!(row(&m, 0).retrying && !row(&m, 1).retrying);
    assert!(jobs(&cmd(&mut m, fog.clone())).is_empty(), "once at a time");
    assert_eq!(status(&m), Some(StatusMessage::ScanStoppedOpen));

    // It stopped again.
    let again = record(
        "/p/lv2/mixed.lv2",
        &["urn:c", "urn:e"],
        &[(Some("urn:d"), "Fog", StopReason::TimedOut)],
    );
    bundle(&mut m, asked[0].id, again);
    news(&mut m, ScanNews::Done { job: asked[0].id });
    let said = StatusMessage::ScanRetryStopped { name: "Fog".into() };
    assert_eq!(status(&m), Some(said), "said over the open dialog");
    assert_eq!(row(&m, 0).reason, ScanStopReason::TimedOut);
    assert!(!row(&m, 0).retrying);

    // This time it reads.
    let asked = jobs(&cmd(&mut m, fog));
    assert_eq!(status(&m), Some(StatusMessage::ScanStoppedOpen));
    bundle(
        &mut m,
        asked[0].id,
        record("/p/lv2/mixed.lv2", &["urn:c", "urn:e", "urn:d"], &[]),
    );
    let effects = news(&mut m, ScanNews::Done { job: asked[0].id });
    assert_eq!(saved(&effects).map(|c| c.plugins().count()), Some(5));
    let said = StatusMessage::ScanRetryAdded { name: "Fog".into() };
    assert_eq!(status(&m), Some(said));
    assert_eq!(m.view().plugins.stopped.len(), 1);

    // A whole bundle that still does not read.
    let old = Command::RetryStoppedPlugin {
        path: "/p/vst3/Old.vst3".into(),
        id: None,
    };
    let asked = jobs(&cmd(&mut m, old));
    bundle(
        &mut m,
        asked[0].id,
        record(
            "/p/vst3/Old.vst3",
            &[],
            &[(None, "Old", StopReason::Crashed)],
        ),
    );
    news(&mut m, ScanNews::Done { job: asked[0].id });
    let said = StatusMessage::ScanRetryStopped { name: "Old".into() };
    assert_eq!(status(&m), Some(said));
    cmd(&mut m, Command::CloseScanStopped);
    assert_ne!(status(&m), Some(StatusMessage::ScanStoppedOpen));
}

#[test]
fn a_place_is_added_scanned_alone_and_removed_with_its_plugins() {
    let (mut m, _) = scanned();
    assert_eq!(
        cmd(&mut m, Command::AddPluginPlace),
        [Effect::PickPluginPlace]
    );
    assert!(m.handle(Input::PluginPlacePicked(None)).is_empty());
    let usual_place = Some(PathBuf::from("/p/lv2"));
    assert!(m.handle(Input::PluginPlacePicked(usual_place)).is_empty());

    let effects = m.handle(Input::PluginPlacePicked(Some("/home/me/Plugins".into())));
    let places = vec![PathBuf::from("/home/me/Plugins")];
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SaveSettings(s) if s.plugin_places == places))
    );
    let asked = jobs(&effects);
    assert_eq!(asked[0].what, ScanWhat::Places { places, all: false });
    let shown = m.view().plugins.places;
    assert_eq!(shown.len(), 3);
    assert!(!shown[0].added && !shown[1].added);
    assert_eq!(shown[2].path, "/home/me/Plugins");
    assert!(shown[2].added);
    let same = Some(PathBuf::from("/home/me/Plugins"));
    assert!(m.handle(Input::PluginPlacePicked(same)).is_empty());

    counted(&mut m, asked[0].id, 1);
    bundle(
        &mut m,
        asked[0].id,
        record("/home/me/Plugins/x.vst3", &["urn:x"], &[]),
    );
    news(&mut m, ScanNews::Done { job: asked[0].id });
    assert_eq!(m.plugins.cache().plugins().count(), 5);

    let usual_place = Command::RemovePluginPlace {
        path: "/p/lv2".into(),
    };
    assert!(cmd(&mut m, usual_place).is_empty(), "a usual place stays");
    let remove = Command::RemovePluginPlace {
        path: "/home/me/Plugins".into(),
    };
    let effects = cmd(&mut m, remove);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SaveSettings(s) if s.plugin_places.is_empty()))
    );
    assert_eq!(saved(&effects).map(|c| c.plugins().count()), Some(4));
    assert_eq!(m.view().plugins.places.len(), 2);

    // The places come back with the settings.
    let settings = Settings {
        plugin_places: vec!["/home/me/Plugins".into()],
        ..Settings::default()
    };
    let m = Model::new(settings);
    let last = m.view().plugins.places.last().cloned();
    assert_eq!(
        last,
        Some(PluginPlaceView {
            path: "/home/me/Plugins".into(),
            shown_path: m.view().plugins.places.last().unwrap().shown_path.clone(),
            added: true,
        })
    );
}

#[test]
fn a_bundle_of_a_place_removed_during_its_scan_is_left_out() {
    let (mut m, _) = scanned();
    let effects = m.handle(Input::PluginPlacePicked(Some("/x".into())));
    let job = jobs(&effects)[0].id;
    counted(&mut m, job, 1);
    cmd(&mut m, Command::RemovePluginPlace { path: "/x".into() });
    bundle(&mut m, job, record("/x/y.lv2", &["urn:y"], &[]));
    news(&mut m, ScanNews::Done { job });
    assert_eq!(m.plugins.cache().plugins().count(), 4);
    assert_ne!(status(&m), Some(StatusMessage::PluginsFound { count: 1 }));
}

#[test]
fn the_dialog_and_the_places_heading() {
    let (mut m, _) = scanned();
    let view = m.view().plugins;
    assert!(!view.stopped_open && view.places_open);
    cmd(&mut m, Command::OpenScanStopped);
    assert!(m.view().plugins.stopped_open);
    assert_eq!(status(&m), Some(StatusMessage::ScanStoppedOpen));
    cmd(&mut m, Command::Back);
    assert!(!m.view().plugins.stopped_open, "Esc closes it");

    cmd(&mut m, Command::TogglePluginPlaces);
    assert!(!m.view().plugins.places_open);
    cmd(&mut m, Command::OpenScanStopped);
    cmd(&mut m, Command::OpenPluginPlaces);
    let view = m.view();
    assert!(!view.plugins.stopped_open && view.plugins.places_open);
    assert!(view.workspace.settings.is_some_and(|s| s.advanced_open));
    assert_eq!(view.status.message, Some(StatusMessage::SettingsOpen));
}
