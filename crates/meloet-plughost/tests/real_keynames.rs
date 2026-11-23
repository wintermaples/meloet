//! The names real plugins of this PC give their keys: avldrums' kits through LV2 midnam, and
//! Surge XT, which names none. Ignored by default; a test whose plugin is not installed says
//! so and passes. No window is opened.

mod common;

use std::path::Path;

use common::Host;
use meloet_plugin::PluginKey;
use meloet_plugin::wire::{KeyName, Reply, Request};

const BLACK_PEARL: (&str, &str) = (
    "lv2:http://gareus.org/oss/lv2/avldrums#BlackPearl",
    "/usr/lib/lv2/avldrums.lv2",
);
const BUSKMAN: (&str, &str) = (
    "lv2:http://gareus.org/oss/lv2/avldrums#BuskmansHoliday",
    "/usr/lib/lv2/avldrums.lv2",
);
const SURGE_VST3: (&str, &str) = (
    "vst3:ABCDEF019182FAEB566D624153675854",
    "/usr/lib/vst3/Surge XT.vst3",
);
const SURGE_LV2: (&str, &str) = (
    "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
    "/usr/lib/lv2/Surge XT.lv2",
);

/// Black Pearl's names as avldrums 0.7.3 writes them (`mn_drums` with kit 0;
/// docs/research/2026-11-23-plugin-presets-state.md, Q10).
const BLACK_PEARL_NAMES: [(u8, &str); 26] = [
    (36, "Kick Drum"),
    (37, "Snare Sidestick"),
    (38, "Snare Center"),
    (39, "Hand Clap"),
    (40, "Snare Edge"),
    (41, "Floor Tom Center"),
    (42, "Closed Hat"),
    (43, "Floor Tom Edge/Alt"),
    (44, "Pedal Hat"),
    (45, "Tom Center"),
    (46, "Semi Hat"),
    (47, "Tom Edge/Alt"),
    (48, "Swish Hat"),
    (49, "Crash 1"),
    (50, "Crash1 Choked"),
    (51, "Ride Tip"),
    (52, "Ride Choked"),
    (53, "Ride Bell"),
    (54, "Tambourine"),
    (55, "Splash"),
    (56, "Cowbell"),
    (57, "Crash 2"),
    (58, "Crash 2 Choked"),
    (59, "Ride Shank"),
    (60, "Crash3"),
    (61, "Maracas"),
];

fn installed(plugin: (&str, &str)) -> bool {
    let there = Path::new(plugin.1).exists();
    if !there {
        eprintln!("{} is not installed; nothing tested", plugin.1);
    }
    there
}

fn load(host: &mut Host, slot: u32, plugin: (&str, &str)) {
    let key: PluginKey = plugin.0.parse().unwrap();
    host.load_plugin(slot, &key, plugin.1).unwrap();
}

fn key_names(host: &mut Host, slot: u32) -> Option<Vec<KeyName>> {
    match host.ask(Request::KeyNames { slot }) {
        Reply::KeyNames { names } => names,
        other => panic!("{other:?}"),
    }
}

fn pairs(names: &[KeyName]) -> Vec<(u8, &str)> {
    names.iter().map(|n| (n.key, n.name.as_str())).collect()
}

#[test]
#[ignore = "needs avldrums.lv2"]
fn avldrums_kits_name_their_drums_and_keep_the_names_with_their_state() {
    if !installed(BLACK_PEARL) {
        return;
    }
    let mut host = Host::start();
    load(&mut host, 0, BLACK_PEARL);
    let names = key_names(&mut host, 0).expect("Black Pearl has midnam");
    for name in &names {
        eprintln!("Black Pearl {} {}", name.key, name.name);
    }
    assert_eq!(pairs(&names), BLACK_PEARL_NAMES);

    // A state put back reads the names again: the same kit, the same names.
    let state = host.state(0);
    let (reply, _) = host.ask_with(Request::SetState { slot: 0 }, &state);
    assert_eq!(reply, Reply::Ok);
    assert_eq!(key_names(&mut host, 0), Some(names));

    load(&mut host, 1, BUSKMAN);
    let names = key_names(&mut host, 1).expect("Buskman's Holiday has midnam");
    let names = pairs(&names);
    assert_eq!(names.len(), 21);
    assert_eq!(names[0], (35, "Stick Click"));
    assert_eq!(names[1], (36, "Cajon Thump"));
    assert_eq!(names[20], (55, "Bell Tree Up"));
}

#[test]
#[ignore = "needs Surge XT"]
fn surge_xt_names_no_keys() {
    let mut host = Host::start();
    for (slot, plugin) in [(0, SURGE_LV2), (1, SURGE_VST3)] {
        if installed(plugin) {
            load(&mut host, slot, plugin);
            assert_eq!(key_names(&mut host, slot), None, "{}", plugin.0);
        }
    }
}
