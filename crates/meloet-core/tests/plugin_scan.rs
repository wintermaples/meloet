//! The scan for plugins through the whole core, with the real `meloet-plughost` and its fake
//! bundles (`found`, `empty`, `broken`, `segv`, `hang`, `mixed`) in temporary places.

mod plughost;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions, ShellRequest, settings};
use meloet_plugin::scan::{BundleRecord, BundleStamp, StopReason, StoppedEntry};
use meloet_plugin::{PluginFormat, ScanCache};
use meloet_protocol::{
    AppState, Command, PluginsView, ScanStopReason, ScanView, StatusMessage, StoppedPluginView,
};

use plughost::{hosts_running, temp_dir, wait_for};

/// Long enough for a child to start on a busy PC, short enough for the test.
const TIMEOUT: Duration = Duration::from_millis(1500);
const PATIENCE: Duration = Duration::from_secs(60);

fn options(root: &Path) -> CoreOptions {
    CoreOptions {
        config_dir: Some(root.join("config")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(root.join("cache")),
            usual_places: vec![root.join("lv2"), root.join("vst3")],
            scan_timeout: TIMEOUT,
            child_env: plughost::fake_env(),
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    }
}

fn bundle(root: &Path, path: &str) -> PathBuf {
    let bundle = root.join(path);
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    bundle
}

fn state_when(core: &Core, what: &str, done: impl Fn(&AppState) -> bool) -> AppState {
    wait_for(PATIENCE, || Some(core.snapshot().state).filter(|s| done(s)))
        .unwrap_or_else(|| panic!("{what}: {:#?}", core.snapshot().state.plugins))
}

fn stopped<'a>(view: &'a PluginsView, file: &str, id: Option<&str>) -> &'a StoppedPluginView {
    view.stopped
        .iter()
        .find(|s| s.path.ends_with(file) && s.id.as_deref() == id)
        .unwrap_or_else(|| panic!("{file} {id:?} is not in {:#?}", view.stopped))
}

/// The nice value of a process; `None` when it is gone.
fn nice_of(pid: u32) -> Option<i32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // After the name in brackets: the state is field 3, nice is field 19.
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(16)?.parse().ok()
}

fn names(cache: &ScanCache) -> Vec<String> {
    let mut names: Vec<String> = cache
        .plugins()
        .map(|(b, p)| format!("{}: {}", b.file_name(), p.name))
        .collect();
    names.sort();
    names
}

#[test]
fn the_first_scan_then_only_what_changed_then_everything_again() {
    let root = temp_dir("scan");
    for path in [
        "lv2/found.lv2",
        "lv2/mixed.lv2",
        "lv2/segv.lv2",
        "lv2/hang.lv2",
        "vst3/empty.vst3",
        "vst3/broken.vst3",
    ] {
        bundle(&root, path);
    }
    std::fs::create_dir_all(root.join("lv2/not-a-bundle")).unwrap();

    // The first start: everything, with its count.
    let core = Core::start(options(&root));
    let running = state_when(&core, "the scan counts", |s| {
        matches!(s.plugins.scan, ScanView::Running { .. })
    });
    assert!(
        matches!(running.plugins.scan, ScanView::Running { total: 6, .. }),
        "{:?}",
        running.plugins.scan
    );
    // The child that hangs is there until its time is up: it runs at the lowest priority.
    let nice = wait_for(PATIENCE, || {
        let hang = root.join("lv2/hang.lv2");
        hosts_running(&hang).first().and_then(|&pid| nice_of(pid))
    });
    assert_eq!(nice, Some(19));
    let done = state_when(&core, "the scan ends", |s| {
        matches!(s.plugins.scan, ScanView::Finished { .. })
    });
    let finished = ScanView::Finished {
        added: 4,
        stopped: 4,
    };
    assert_eq!(done.plugins.scan, finished);
    let said = StatusMessage::ScanFinished {
        added: 4,
        stopped: 4,
    };
    assert_eq!(done.status.message, Some(said));
    let view = &done.plugins;
    // Only B of the mixed bundle stopped; it is named as the child said before it crashed.
    let b = stopped(view, "mixed.lv2", Some("urn:meloet:fake:b"));
    assert_eq!(
        (b.name.as_str(), b.reason),
        ("Fake B", ScanStopReason::Crashed)
    );
    let segv = stopped(view, "segv.lv2", Some("urn:meloet:fake:segv"));
    assert_eq!(
        (segv.name.as_str(), segv.reason),
        ("Fake Segv", ScanStopReason::Crashed)
    );
    let hang = stopped(view, "hang.lv2", Some("urn:meloet:fake:hang"));
    assert_eq!(
        (hang.name.as_str(), hang.reason),
        ("Fake Hang", ScanStopReason::TimedOut)
    );
    let broken = stopped(view, "broken.vst3", None);
    assert_eq!(
        (broken.name.as_str(), broken.reason),
        ("broken", ScanStopReason::Broken)
    );
    // In the order of the places: LV2 before VST3.
    assert!(view.stopped[3].path.ends_with("broken.vst3"));
    core.stop();
    assert_eq!(
        hosts_running(&root),
        Vec::<u32>::new(),
        "no scanning child is left"
    );

    let cache = ScanCache::load(&root.join("cache"));
    assert_eq!(cache.bundles.len(), 6);
    assert_eq!(
        names(&cache),
        [
            "found: Fake Pass",
            "found: Fake Sine",
            "mixed: Fake A",
            "mixed: Fake C"
        ]
    );
    assert_eq!(
        cache
            .get(&root.join("vst3/empty.vst3"))
            .map(|b| b.plugins.len()),
        Some(0)
    );

    // The second start: one bundle is new, one is gone. No count, and what stopped is not
    // read again (the bundle that hangs would take its time).
    bundle(&root, "vst3/maker/found.vst3");
    std::fs::remove_dir_all(root.join("vst3/broken.vst3")).unwrap();
    let core = Core::start(options(&root));
    let found = state_when(&core, "new plugins are told", |s| {
        s.status.message == Some(StatusMessage::PluginsFound { count: 2 })
    });
    assert_eq!(found.plugins.scan, ScanView::Idle);
    assert_eq!(found.plugins.stopped.len(), 3);
    core.stop();
    let cache = ScanCache::load(&root.join("cache"));
    assert_eq!(cache.bundles.len(), 6);
    assert_eq!(cache.plugins().count(), 6);
    assert!(cache.get(&root.join("vst3/broken.vst3")).is_none());

    // A third start at which nothing changed says nothing; then "もう一度スキャン".
    let core = Core::start(options(&root));
    std::thread::sleep(Duration::from_millis(300));
    let quiet = core.snapshot().state;
    assert_eq!(quiet.plugins.scan, ScanView::Idle);
    assert!(!matches!(
        quiet.status.message,
        Some(StatusMessage::PluginsFound { .. })
    ));
    core.send(Command::RescanPlugins);
    let done = state_when(&core, "the scan ends again", |s| {
        matches!(s.plugins.scan, ScanView::Finished { .. })
    });
    let finished = ScanView::Finished {
        added: 6,
        stopped: 3,
    };
    assert_eq!(done.plugins.scan, finished);
    let b = stopped(&done.plugins, "mixed.lv2", Some("urn:meloet:fake:b"));
    assert_eq!(b.name, "Fake B");
    core.stop();
    assert_eq!(hosts_running(&root), Vec::<u32>::new());
    assert_eq!(names(&ScanCache::load(&root.join("cache"))).len(), 6);
    std::fs::remove_dir_all(root).unwrap();
}

/// A scan file that says `stopped` stopped in the bundle, as it is on the disk now.
fn remembered(path: &Path, stopped: &[(Option<&str>, &str)]) -> BundleRecord {
    BundleRecord {
        path: path.into(),
        format: PluginFormat::Lv2,
        stamp: BundleStamp::read(path).unwrap(),
        plugins: Vec::new(),
        stopped: stopped
            .iter()
            .map(|&(id, name)| StoppedEntry {
                id: id.map(str::to_owned),
                name: name.into(),
                reason: StopReason::Crashed,
            })
            .collect(),
    }
}

#[test]
fn trying_a_stopped_plugin_again() {
    let root = temp_dir("retry");
    let found = bundle(&root, "lv2/found.lv2");
    let whole = bundle(&root, "lv2/sub/found.lv2");
    let mixed = bundle(&root, "lv2/mixed.lv2");
    // As if Sine of the first bundle, the whole second bundle and B and C of the mixed one
    // had stopped a scan before.
    let cache = ScanCache {
        bundles: vec![
            remembered(&found, &[(Some("urn:meloet:fake:sine"), "Old Sine")]),
            remembered(&whole, &[(None, "found")]),
            remembered(
                &mixed,
                &[
                    (Some("urn:meloet:fake:b"), "Fake B"),
                    (Some("urn:meloet:fake:c"), "Fake C"),
                ],
            ),
        ],
    };
    cache.save(&root.join("cache")).unwrap();

    let core = Core::start(options(&root));
    std::thread::sleep(Duration::from_millis(300));
    let before = core.snapshot().state.plugins;
    assert_eq!(
        before.stopped.len(),
        4,
        "unchanged bundles are not read at start"
    );
    core.send(Command::OpenScanStopped);

    let retry = |path: &Path, id: Option<&str>| Command::RetryStoppedPlugin {
        path: path.display().to_string(),
        id: id.map(str::to_owned),
    };
    let said = |core: &Core, message: StatusMessage| {
        state_when(core, &format!("{message:?}"), |s| {
            s.status.message.as_ref() == Some(&message)
        })
    };

    // One plugin. Pass, which no scan had come to, is read with it.
    core.send(retry(&found, Some("urn:meloet:fake:sine")));
    let state = said(
        &core,
        StatusMessage::ScanRetryAdded {
            name: "Old Sine".into(),
        },
    );
    assert_eq!(state.plugins.stopped.len(), 3);

    // B stops again; A, which was never read, comes with it since nothing said to pass it.
    core.send(retry(&mixed, Some("urn:meloet:fake:b")));
    let state = said(
        &core,
        StatusMessage::ScanRetryStopped {
            name: "Fake B".into(),
        },
    );
    assert_eq!(state.plugins.stopped.len(), 3);
    assert!(!stopped(&state.plugins, "mixed.lv2", Some("urn:meloet:fake:b")).retrying);

    // C reads now.
    core.send(retry(&mixed, Some("urn:meloet:fake:c")));
    let state = said(
        &core,
        StatusMessage::ScanRetryAdded {
            name: "Fake C".into(),
        },
    );
    assert_eq!(state.plugins.stopped.len(), 2);

    // The whole bundle.
    core.send(retry(&whole, None));
    let state = said(
        &core,
        StatusMessage::ScanRetryAdded {
            name: "found".into(),
        },
    );
    assert_eq!(state.plugins.stopped.len(), 1);
    core.stop();

    let cache = ScanCache::load(&root.join("cache"));
    assert_eq!(
        names(&cache),
        [
            "found: Fake Pass",
            "found: Fake Pass",
            "found: Fake Sine",
            "found: Fake Sine",
            "mixed: Fake A",
            "mixed: Fake C"
        ]
    );
    assert_eq!(cache.stopped().count(), 1);
    assert_eq!(hosts_running(&root), Vec::<u32>::new());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_place_is_added_and_removed() {
    let root = temp_dir("places");
    bundle(&root, "lv2/found.lv2");
    let extra = root.join("extra");
    bundle(&root, "extra/mixed.lv2");

    let core = Core::start(options(&root));
    state_when(&core, "the first scan ends", |s| {
        matches!(s.plugins.scan, ScanView::Finished { added: 2, .. })
    });
    let (asked, requests) = mpsc::channel();
    core.set_shell(Box::new(move |request| {
        let _ = asked.send(request);
    }));

    // The window is put away: nothing changes.
    core.send(Command::AddPluginPlace);
    let request = requests.recv_timeout(PATIENCE).unwrap();
    assert!(
        matches!(request, ShellRequest::PickPluginPlace { .. }),
        "{request:?}"
    );
    core.plugin_place_picked(None);
    assert_eq!(core.snapshot().state.plugins.places.len(), 2);

    core.send(Command::AddPluginPlace);
    requests.recv_timeout(PATIENCE).unwrap();
    core.plugin_place_picked(Some(extra.clone()));
    let state = state_when(&core, "the place's plugins are told", |s| {
        s.status.message == Some(StatusMessage::PluginsFound { count: 2 })
    });
    let places = &state.plugins.places;
    assert_eq!(places.len(), 3);
    assert_eq!(places[2].path, extra.display().to_string());
    assert!(places[2].added && !places[0].added);
    assert_eq!(state.plugins.stopped.len(), 1);
    assert_eq!(
        settings::load(&root.join("config")).plugin_places,
        std::slice::from_ref(&extra)
    );
    core.stop();
    assert_eq!(ScanCache::load(&root.join("cache")).plugins().count(), 4);

    // The place is remembered; removing it takes its plugins off the record.
    let core = Core::start(options(&root));
    let state = core.snapshot().state;
    assert_eq!(state.plugins.places.len(), 3);
    core.send(Command::RemovePluginPlace {
        path: extra.display().to_string(),
    });
    let state = core.snapshot().state;
    assert_eq!(state.plugins.places.len(), 2);
    assert!(state.plugins.stopped.is_empty());
    assert!(
        settings::load(&root.join("config"))
            .plugin_places
            .is_empty()
    );
    core.stop();
    let cache = ScanCache::load(&root.join("cache"));
    assert_eq!(names(&cache), ["found: Fake Pass", "found: Fake Sine"]);
    assert_eq!(hosts_running(&root), Vec::<u32>::new());
    std::fs::remove_dir_all(root).unwrap();
}
