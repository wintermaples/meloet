//! Presets through the host's socket: the fake `choices`, and real LV2 plugins of this PC
//! (ignored by default; one that is not installed is said and passed over): ZamComp's three
//! presets in its own bundle, ZynAddSubFX's 1149 in a bundle of presets, and a preset written
//! and read back.

mod common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use common::Host;
use meloet_plugin::PluginKey;
use meloet_plugin::wire::{PresetInfo, Reply, Request};

const ZAMCOMP: (&str, &str) = ("lv2:urn:zamaudio:ZamComp", "/usr/lib/lv2/ZamComp.lv2");
const ZYN: (&str, &str) = (
    "lv2:http://zynaddsubfx.sourceforge.net",
    "/usr/lib/lv2/ZynAddSubFX.lv2",
);
const ZYN_PRESETS: &str = "/usr/lib/lv2/ZynAddSubFX.lv2presets";

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k11presets-host-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn installed(plugin: (&str, &str)) -> bool {
    let there = Path::new(plugin.1).exists();
    if !there {
        eprintln!("{} is not installed; nothing tested", plugin.1);
    }
    there
}

fn presets(host: &mut Host, slot: u32, bundles: &[PathBuf]) -> Vec<PresetInfo> {
    let bundles = bundles.to_vec();
    match host.ask(Request::Presets { slot, bundles }) {
        Reply::Presets { presets } => presets,
        other => panic!("{other:?}"),
    }
}

fn preset_state(host: &mut Host, slot: u32, uri: &str, bundles: &[PathBuf]) -> Vec<u8> {
    let request = Request::PresetState {
        slot,
        uri: uri.into(),
        bundles: bundles.to_vec(),
    };
    match host.ask_with(request, &[]) {
        (Reply::State, state) => state,
        other => panic!("{other:?}"),
    }
}

fn put(host: &mut Host, slot: u32, state: &[u8]) {
    let (reply, _) = host.ask_with(Request::SetState { slot }, state);
    assert_eq!(reply, Reply::Ok);
}

fn save(host: &mut Host, slot: u32, dir: &Path, file: &str, label: &str) -> Reply {
    host.ask(Request::SavePreset {
        slot,
        dir: dir.into(),
        file: file.into(),
        label: label.into(),
    })
}

#[test]
fn the_fake_lists_reads_and_saves_its_presets() {
    let mut host = Host::start();
    host.load(0, "choices");
    host.load(1, "sine");
    let listed = presets(&mut host, 0, &[]);
    let labels: Vec<&str> = listed.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels, ["Quiet", "Loud"]);
    assert_eq!(listed[1].bank.as_deref(), Some("Fake Bank"));
    let loud = preset_state(&mut host, 0, &listed[1].uri, &[]);
    put(&mut host, 0, &loud);
    assert_eq!(host.param(0, 0).0, 0.875);

    let bundle = temp("fake").join("choices_Mine.preset.lv2");
    let saved = save(&mut host, 0, &bundle, "Mine.ttl", "Mine");
    let Reply::Presets { presets: saved } = saved else {
        panic!("{saved:?}");
    };
    assert_eq!(saved[0].label, "Mine");
    let listed = presets(&mut host, 0, std::slice::from_ref(&bundle));
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[2], saved[0]);
    assert_eq!(
        preset_state(&mut host, 0, &saved[0].uri, std::slice::from_ref(&bundle)),
        loud
    );

    // A plugin with no presets, and one that is not there.
    assert!(matches!(
        host.ask(Request::Presets {
            slot: 1,
            bundles: Vec::new()
        }),
        Reply::Error { .. }
    ));
    assert!(matches!(
        host.ask(Request::Presets {
            slot: 5,
            bundles: Vec::new()
        }),
        Reply::Error { .. }
    ));
    std::fs::remove_dir_all(bundle.parent().unwrap()).unwrap();
}

#[test]
#[ignore = "needs ZamComp (LV2)"]
fn zamcomp_lists_loads_and_saves_presets() {
    if !installed(ZAMCOMP) {
        return;
    }
    let mut host = Host::start_with(common::GEOMETRY, &[]);
    let key: PluginKey = ZAMCOMP.0.parse().unwrap();
    host.load_plugin(0, &key, ZAMCOMP.1).unwrap();
    let listed = presets(&mut host, 0, &[]);
    for p in &listed {
        eprintln!("{p:?}");
    }
    let labels: Vec<&str> = listed.iter().map(|p| p.label.as_str()).collect();
    assert_eq!(labels.len(), 3, "{labels:?}");
    assert!(labels.contains(&"PoppySnare"), "{labels:?}");
    let before = host.state(0);
    let values = |host: &mut Host| match host.ask(Request::Params { slot: 0 }) {
        Reply::Params { params } => (params.iter())
            .map(|p| (p.name.clone(), p.value))
            .collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    let start = values(&mut host);

    let poppy = listed.iter().find(|p| p.label == "PoppySnare").unwrap();
    let state = preset_state(&mut host, 0, &poppy.uri, &[]);
    assert_eq!(
        host.state(0),
        before,
        "making a preset's state leaves the plugin as it was"
    );
    eprintln!("{}", String::from_utf8_lossy(&state[8..]));
    put(&mut host, 0, &state);
    let poppy_values = values(&mut host);
    eprintln!("values {start:?} -> {poppy_values:?}");
    assert_ne!(poppy_values, start);

    // Written as the person's preset, listed with its label, and read back the same.
    let home = temp("zamcomp");
    let bundle = home.join(".lv2/ZamComp_k11_Test.preset.lv2");
    let saved = save(&mut host, 0, &bundle, "k11_Test.ttl", "k11 Test");
    let Reply::Presets { presets: saved } = saved else {
        panic!("{saved:?}");
    };
    eprintln!("saved {saved:?}");
    for file in std::fs::read_dir(&bundle).unwrap().flatten() {
        eprintln!(
            "--- {}\n{}",
            file.path().display(),
            std::fs::read_to_string(file.path()).unwrap()
        );
    }
    assert_eq!(saved[0].label, "k11 Test");
    put(&mut host, 0, &before);
    assert_eq!(values(&mut host), start);
    let listed = presets(&mut host, 0, std::slice::from_ref(&bundle));
    assert_eq!(listed.len(), 4);
    let mine = preset_state(&mut host, 0, &saved[0].uri, &[]);
    put(&mut host, 0, &mine);
    assert_eq!(values(&mut host), poppy_values);

    // Not another plugin's preset, and not a place outside a bundle folder.
    let request = Request::PresetState {
        slot: 0,
        uri: "urn:zamaudio:ZamGate#preset001".into(),
        bundles: Vec::new(),
    };
    assert!(matches!(host.ask(request), Reply::Error { .. }));
    let bad = save(&mut host, 0, Path::new("relative"), "x.ttl", "x");
    assert!(matches!(bad, Reply::Error { .. }), "{bad:?}");
    let bad = save(&mut host, 0, &bundle, "../x.ttl", "x");
    assert!(matches!(bad, Reply::Error { .. }), "{bad:?}");
    std::fs::remove_dir_all(&home).unwrap();
}

#[test]
#[ignore = "needs ZynAddSubFX (LV2) and its presets"]
fn zynaddsubfx_lists_its_bundle_of_presets() {
    if !installed(ZYN) || !installed((ZYN.0, ZYN_PRESETS)) {
        return;
    }
    let mut host = Host::start_with(common::GEOMETRY, &[]);
    let key: PluginKey = ZYN.0.parse().unwrap();
    host.load_plugin(0, &key, ZYN.1).unwrap();
    let own = presets(&mut host, 0, &[]);
    let bundles = [PathBuf::from(ZYN_PRESETS)];
    let started = Instant::now();
    let listed = presets(&mut host, 0, &bundles);
    let first = started.elapsed();
    let started = Instant::now();
    let again = presets(&mut host, 0, &bundles);
    eprintln!(
        "ZynAddSubFX: {} in its own bundle, {} with the bundle of presets (first {first:?}, \
         again {:?})",
        own.len(),
        listed.len(),
        started.elapsed()
    );
    assert_eq!(listed, again);
    assert!(listed.len() >= 1000);
    let banks: std::collections::BTreeSet<_> =
        listed.iter().filter_map(|p| p.bank.clone()).collect();
    eprintln!(
        "{} banks, e.g. {:?}; e.g. {:?}",
        banks.len(),
        banks.iter().take(5).collect::<Vec<_>>(),
        listed[0]
    );
    let started = Instant::now();
    let state = preset_state(&mut host, 0, &listed[0].uri, &bundles);
    eprintln!(
        "its state: {} bytes in {:?}",
        state.len(),
        started.elapsed()
    );
    let started = Instant::now();
    put(&mut host, 0, &state);
    eprintln!("put in {:?}", started.elapsed());
}
