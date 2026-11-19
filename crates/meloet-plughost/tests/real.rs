//! The real `meloet-plughost` binary with real plugins of this PC (Surge XT, LSP, Zam,
//! Dragonfly). Ignored by default: `scripts/check-all.sh --devices` runs them. A test whose
//! plugin is not installed says so and passes. No window is opened.

mod common;

use std::path::Path;

use common::{GEOMETRY, Host, block, note_off, note_on, peak};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::{EngineEnd, FromPlugin, ToPlugin};
use meloet_plugin::wire::{Notice, ParamInfo, ParamShape, Reply, Request};

const SURGE_VST3: (&str, &str) = (
    "vst3:ABCDEF019182FAEB566D624153675854",
    "/usr/lib/vst3/Surge XT.vst3",
);
const SURGE_LV2: (&str, &str) = (
    "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
    "/usr/lib/lv2/Surge XT.lv2",
);
const LSP_COMP_VST3: (&str, &str) = (
    "vst3:647370206B31732020202020756E7363",
    "/usr/lib/vst3/lsp-plugins.vst3",
);
const LSP_COMP_LV2: (&str, &str) = (
    "lv2:http://lsp-plug.in/plugins/lv2/compressor_stereo",
    "/usr/lib/lv2/lsp-plugins.lv2",
);
const LSP_LIMITER_VST3: (&str, &str) = (
    "vst3:64737020623173202020202072667563",
    "/usr/lib/vst3/lsp-plugins.vst3",
);
const LSP_LIMITER_LV2: (&str, &str) = (
    "lv2:http://lsp-plug.in/plugins/lv2/limiter_stereo",
    "/usr/lib/lv2/lsp-plugins.lv2",
);
const ZAMCOMP_VST3: (&str, &str) = (
    "vst3:2046504473616C63504D435A00000000",
    "/usr/lib/vst3/ZamComp.vst3",
);
const ZAMCOMP_LV2: (&str, &str) = ("lv2:urn:zamaudio:ZamComp", "/usr/lib/lv2/ZamComp.lv2");
const DRAGONFLY_LV2: (&str, &str) = (
    "lv2:https://github.com/michaelwillis/dragonfly-reverb",
    "/usr/lib/lv2/DragonflyHallReverb.lv2",
);

fn installed(plugin: (&str, &str)) -> bool {
    let there = Path::new(plugin.1).exists();
    if !there {
        eprintln!("{} is not installed; nothing tested", plugin.1);
    }
    there
}

fn load(host: &mut Host, slot: u32, plugin: (&str, &str)) -> EngineEnd {
    let key: PluginKey = plugin.0.parse().unwrap();
    host.load_plugin(slot, &key, plugin.1).unwrap().0
}

fn params(host: &mut Host, slot: u32) -> Vec<ParamInfo> {
    match host.ask(Request::Params { slot }) {
        Reply::Params { params } => params,
        other => panic!("{other:?}"),
    }
}

fn set(host: &mut Host, slot: u32, id: u32, value: f64) {
    assert!(matches!(
        host.ask(Request::SetParam { slot, id, value }),
        Reply::Param { .. }
    ));
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// `blocks` blocks of silence in, with `first` in the first one; the left output of all.
fn run(engine: &mut EngineEnd, blocks: usize, first: &[ToPlugin]) -> Vec<f32> {
    let mut out = Vec::new();
    for n in 0..blocks {
        let events = if n == 0 { first } else { &[] };
        let (left, right) = block(engine, &[0.0; 256], events);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        out.extend(left);
    }
    out
}

/// A 220 Hz sine through an effect for `blocks` blocks; left and right output.
fn through(engine: &mut EngineEnd, blocks: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut all_left, mut all_right) = (Vec::new(), Vec::new());
    for n in 0..blocks {
        let input: Vec<f32> = (0..256)
            .map(|i| {
                let t = (n * 256 + i) as f32 / 48_000.0;
                0.3 * (t * 220.0 * std::f32::consts::TAU).sin()
            })
            .collect();
        let (left, right) = block(engine, &input, &[]);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        all_left.extend(left);
        all_right.extend(right);
    }
    (all_left, all_right)
}

/// Sound from keys, parameters that read back as set, and a state that brings both to a
/// plugin just made.
///
/// What "reads back" proves differs: the VST3 controller answers for itself. Surge XT's LV2
/// does not answer `patch:Get`, so there the values read are the ones meloet keeps, and it is
/// the sound (the global volume turned down) that shows they reached the plugin and came back
/// with the state.
fn instrument_round_trip(plugin: (&str, &str), least_params: usize) {
    if !installed(plugin) {
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let mut first = load(&mut host, 0, plugin);
    assert_eq!(peak(&run(&mut first, 10, &[])), 0.0);
    let sound = run(&mut first, 40, &[note_on(60)]);
    assert!(rms(&sound) > 0.01, "{}", rms(&sound));
    run(&mut first, 100, &[note_off(60)]);

    let list = params(&mut host, 0);
    assert!(list.len() >= least_params, "{}", list.len());
    let volume = list
        .iter()
        .find(|p| p.name == "Global Volume")
        .expect("Surge XT has a global volume");
    // 20 continuous parameters to values of their own; the global volume all the way down.
    let moved: Vec<(u32, f64)> = list
        .iter()
        .filter(|p| p.automatable && p.shape == ParamShape::Continuous && p.id != volume.id)
        .filter(|p| p.name.starts_with("A Osc") || p.name.starts_with("A Filter"))
        .take(20)
        .enumerate()
        .map(|(n, p)| (p.id, 0.2 + 0.03 * n as f64))
        .collect();
    assert_eq!(moved.len(), 20);
    for &(id, value) in &moved {
        set(&mut host, 0, id, value);
    }
    set(&mut host, 0, volume.id, 0.0);
    // Through the slot's ring too, as automation would.
    let (ring_id, ring_value) = (moved[0].0, 0.77);
    run(
        &mut first,
        20,
        &[ToPlugin::Param {
            frame: 0,
            id: ring_id,
            value: ring_value,
        }],
    );
    let near = |a: f64, b: f64| (a - b).abs() < 2e-3;
    for &(id, value) in &moved[1..] {
        let (read, _) = host.param(0, id);
        assert!(near(read, value), "{id}: {read} for {value}");
    }
    assert!(near(host.param(0, ring_id).0, ring_value));
    // The volume reached the sound, not only the list.
    let quiet = run(&mut first, 40, &[note_on(60)]);
    assert!(rms(&quiet) < rms(&sound) / 20.0, "{}", rms(&quiet));

    let state = host.state(0);
    assert!(state.len() > 1000, "{}", state.len());
    // Two plugins just made: one gets the state, one does not.
    let mut restored = load(&mut host, 1, plugin);
    let mut fresh = load(&mut host, 2, plugin);
    assert_eq!(
        host.ask_with(Request::SetState { slot: 1 }, &state).0,
        Reply::Ok
    );
    // The first one takes its own state too: a plugin may keep a value a little off from
    // what it was sent (Surge XT snaps some), and then both must be off the same way.
    assert_eq!(
        host.ask_with(Request::SetState { slot: 0 }, &state).0,
        Reply::Ok
    );
    run(&mut first, 20, &[]);
    run(&mut restored, 20, &[]);
    run(&mut fresh, 20, &[]);
    let (mut as_sent, mut changed) = (0, 0);
    for &(id, value) in &moved {
        let value = if id == ring_id { ring_value } else { value };
        let (kept, _) = host.param(0, id);
        let (read, _) = host.param(1, id);
        // A plugin just made may not tell its values at all (Surge XT's LV2).
        let untouched = host.value(2, id);
        assert!(
            near(read, kept),
            "restored {id}: {read}, the first has {kept}"
        );
        as_sent += usize::from(near(read, value));
        changed += usize::from(untouched.is_none_or(|untouched| !near(read, untouched)));
    }
    assert!(
        changed >= 12,
        "{changed} of 20 differ from a plugin just made"
    );
    assert!(as_sent >= 4, "{as_sent} of 20 came back as sent");
    assert!(near(host.param(1, volume.id).0, 0.0));
    let restored_sound = run(&mut restored, 40, &[note_on(60)]);
    let fresh_sound = run(&mut fresh, 40, &[note_on(60)]);
    assert!(rms(&fresh_sound) > 0.01, "{}", rms(&fresh_sound));
    assert!(
        rms(&restored_sound) < rms(&fresh_sound) / 20.0,
        "{}",
        rms(&restored_sound)
    );

    // What is not a state is refused, and the plugin goes on.
    assert!(matches!(
        host.ask_with(Request::SetState { slot: 1 }, b"not a state")
            .0,
        Reply::Error { .. }
    ));
    for slot in 0..3 {
        assert_eq!(host.ask(Request::Unload { slot }), Reply::Ok);
    }
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    let status = host.child.wait_for_exit(std::time::Duration::from_secs(10));
    assert_eq!(status.and_then(|s| s.code()), Some(0));
}

#[test]
#[ignore = "needs Surge XT (VST3)"]
fn surge_xt_vst3_sounds_takes_parameters_and_keeps_its_state() {
    instrument_round_trip(SURGE_VST3, 700);
}

#[test]
#[ignore = "needs Surge XT (LV2)"]
fn surge_xt_lv2_sounds_takes_parameters_and_keeps_its_state() {
    instrument_round_trip(SURGE_LV2, 700);
}

/// Sound through, a parameter that reads back, and two plugins just made that sound the same
/// sample for sample once both have the first one's state.
fn effect_round_trip(plugin: (&str, &str)) {
    if !installed(plugin) {
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let mut first = load(&mut host, 0, plugin);
    let (left, right) = through(&mut first, 40);
    assert!(rms(&left[2560..]) > 0.01, "{}", rms(&left));
    assert!(rms(&right[2560..]) > 0.01, "{}", rms(&right));

    let list = params(&mut host, 0);
    let moved: Vec<(u32, f64)> = list
        .iter()
        .filter(|p| p.automatable && p.shape == ParamShape::Continuous)
        .take(6)
        .enumerate()
        .map(|(n, p)| (p.id, 0.3 + 0.05 * n as f64))
        .collect();
    assert!(!moved.is_empty(), "{list:?}");
    for &(id, value) in &moved {
        set(&mut host, 0, id, value);
    }
    through(&mut first, 10);
    let near = |a: f64, b: f64| (a - b).abs() < 2e-3;
    for &(id, value) in &moved {
        let (read, text) = host.param(0, id);
        assert!(near(read, value), "{id}: {read} ({text}) for {value}");
    }

    let state = host.state(0);
    let mut one = load(&mut host, 1, plugin);
    let mut two = load(&mut host, 2, plugin);
    for slot in [1, 2] {
        assert_eq!(
            host.ask_with(Request::SetState { slot }, &state).0,
            Reply::Ok
        );
    }
    for &(id, value) in &moved {
        let (read, _) = host.param(1, id);
        assert!(near(read, value), "restored {id}: {read} for {value}");
    }
    assert_eq!(through(&mut one, 40), through(&mut two, 40));
}

#[test]
#[ignore = "needs LSP Plugins (VST3)"]
fn lsp_compressor_vst3_round_trip() {
    effect_round_trip(LSP_COMP_VST3);
}

#[test]
#[ignore = "needs LSP Plugins (LV2)"]
fn lsp_compressor_lv2_round_trip() {
    effect_round_trip(LSP_COMP_LV2);
}

#[test]
#[ignore = "needs Zam plugins (VST3)"]
fn zamcomp_vst3_round_trip() {
    effect_round_trip(ZAMCOMP_VST3);
}

#[test]
#[ignore = "needs Zam plugins (LV2)"]
fn zamcomp_lv2_round_trip() {
    effect_round_trip(ZAMCOMP_LV2);
}

#[test]
#[ignore = "needs Dragonfly Reverb (LV2)"]
fn dragonfly_hall_lv2_round_trip() {
    effect_round_trip(DRAGONFLY_LV2);
}

/// The limiter looks ahead, so it delays: meloet is told how much, at load or with a block.
fn says_its_delay(plugin: (&str, &str)) {
    if !installed(plugin) {
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let key: PluginKey = plugin.0.parse().unwrap();
    let (mut engine, loaded) = host.load_plugin(0, &key, plugin.1).unwrap();
    let mut latency = loaded.latency;
    // A plugin may say it only once it runs, and through its main thread.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while latency == 0 && std::time::Instant::now() < until {
        through(&mut engine, 4);
        while let Some(event) = engine.pop_event() {
            if let FromPlugin::Latency { frames } = event {
                latency = frames;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!((16..48_000).contains(&latency), "{latency}");
    if loaded.latency != latency {
        // The socket says so too.
        let told = Notice::LatencyChanged {
            slot: 0,
            frames: latency,
        };
        for _ in 0..200 {
            host.ask(Request::Params { slot: 0 });
            if host.notices.contains(&told) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(host.notices.contains(&told), "{:?}", host.notices);
    }
}

#[test]
#[ignore = "needs LSP Plugins (VST3)"]
fn lsp_limiter_vst3_says_its_delay() {
    says_its_delay(LSP_LIMITER_VST3);
}

#[test]
#[ignore = "needs LSP Plugins (LV2)"]
fn lsp_limiter_lv2_says_its_delay() {
    says_its_delay(LSP_LIMITER_LV2);
}
