//! The real `meloet-plughost` binary with real plugins of this PC (Surge XT, LSP, Zam,
//! Dragonfly). Ignored by default: `scripts/check-all.sh --devices` runs them. A test whose
//! plugin is not installed says so and passes. No window is opened.

mod common;

use std::path::Path;

use common::{GEOMETRY, Host, block, note_off, note_on, peak};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::{EngineEnd, FromPlugin, ToPlugin, Transport};
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

/// How many parameters are for people, and how many only take a keyboard's knobs.
fn knobs_apart(plugin: (&str, &str)) -> Option<(usize, usize)> {
    if !installed(plugin) {
        return None;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let _engine = load(&mut host, 0, plugin);
    let list = params(&mut host, 0);
    let knobs = list.iter().filter(|p| p.midi_cc).count();
    if let Some(first) = list.iter().find(|p| p.midi_cc) {
        let last = list.iter().rfind(|p| p.midi_cc).unwrap();
        eprintln!("{}: {} .. {}", plugin.0, first.name, last.name);
    }
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    Some((list.len() - knobs, knobs))
}

#[test]
#[ignore = "needs Surge XT (VST3)"]
fn surge_xt_vst3_has_774_parameters_and_2080_for_a_keyboards_knobs() {
    if let Some(counts) = knobs_apart(SURGE_VST3) {
        assert_eq!(counts, (774, 2080));
    }
}

#[test]
#[ignore = "needs Surge XT (LV2)"]
fn surge_xt_lv2_has_none_for_a_keyboards_knobs() {
    if let Some(counts) = knobs_apart(SURGE_LV2) {
        assert_eq!(counts, (775, 0));
    }
}

#[test]
#[ignore = "needs LSP Plugins and Zam plugins (VST3)"]
fn plugins_that_map_no_knob_only_parameters_keep_all_of_theirs() {
    for plugin in [LSP_COMP_VST3, LSP_LIMITER_VST3, ZAMCOMP_VST3] {
        if let Some((_, knobs)) = knobs_apart(plugin) {
            assert_eq!(knobs, 0, "{}", plugin.0);
        }
    }
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

/// A block of memory as JUCE writes it into a string: the length, a dot, then six bits to a
/// character, the low bits first.
fn juce_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b".ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+";
    let mut text = format!("{}.", bytes.len());
    for at in 0..(bytes.len() * 8).div_ceil(6) {
        let bit = |n: usize| {
            let n = at * 6 + n;
            usize::from(
                bytes
                    .get(n / 8)
                    .is_some_and(|byte| byte >> (n % 8) & 1 == 1),
            )
        };
        let six = (0..6).fold(0, |six, n| six | bit(n) << n);
        text.push(TABLE[six] as char);
    }
    text
}

const SURGE_SEQUENCE: &str = "/usr/share/surge-xt/patches_factory/Sequences/Acid Seq 1.fxp";

/// The song at `frame` of a run at one tempo in 4/4, playing.
fn song_at(frame: u64, bpm: f64) -> Transport {
    let quarters = frame as f64 / 48_000.0 * bpm / 60.0;
    Transport {
        flags: Transport::PLAYING,
        meter_num: 4,
        meter_den: 4,
        tempo_bpm: bpm,
        position_quarters: quarters,
        bar_start_quarters: (quarters / 4.0).floor() * 4.0,
        loop_start_quarters: 0.0,
        loop_end_quarters: 0.0,
        position_frames: frame as i64,
        bar: (quarters / 4.0) as u64,
    }
}

/// Holds one key for `blocks` blocks while the song plays at `bpm`, and gives how bright each
/// block was (how steep the wave is for its level: the sequencer moves the filter).
fn sequence_at(host: &mut Host, slot: u32, state: &[u8], bpm: f64, blocks: usize) -> Vec<f32> {
    sequence_along(host, slot, SURGE_LV2, state, blocks, &|frame| {
        song_at(frame, bpm)
    })
}

/// As `sequence_at`, with the song `song` gives for the first frame of each block.
fn sequence_along(
    host: &mut Host,
    slot: u32,
    plugin: (&str, &str),
    state: &[u8],
    blocks: usize,
    song: &dyn Fn(u64) -> Transport,
) -> Vec<f32> {
    let mut engine = load(host, slot, plugin);
    assert_eq!(
        host.ask_with(Request::SetState { slot }, state).0,
        Reply::Ok
    );
    let mut bright = Vec::new();
    for n in 0..blocks {
        assert!(engine.begin(256));
        engine.write_input(0, &[0.0; 256]);
        engine.write_input(1, &[0.0; 256]);
        engine.set_transport(&song(n as u64 * 256));
        if n == 20 {
            assert!(engine.push_event(note_on(48)));
        }
        engine.submit();
        assert!(engine.wait(meloet_plugin::ipc::futex::monotonic_ns() + common::PATIENT_NS));
        let mut left = [0.0f32; 256];
        engine.read_output(0, &mut left);
        let edges: Vec<f32> = left.windows(2).map(|w| w[1] - w[0]).collect();
        bright.push(rms(&edges) / rms(&left).max(1e-6));
    }
    bright
}

/// A Surge XT patch (what follows the 60 bytes of header in its file) with every step
/// sequencer of scene A running with the tempo. The patch is "sub3", the length of its XML
/// as a little endian u32, 24 more bytes, then the XML.
fn synced(patch: &[u8]) -> Vec<u8> {
    let mut out = patch.to_vec();
    let mut added = 0u32;
    for lfo in 6..12 {
        let rate = format!("<a_lfo{lfo}_rate ");
        let Some(at) = out.windows(rate.len()).position(|w| w == rate.as_bytes()) else {
            continue;
        };
        let end = at + out[at..].iter().position(|&b| b == b'>').unwrap();
        if out[at..end].windows(9).any(|w| w == b"temposync") {
            continue;
        }
        let mark = b"temposync=\"1\" ";
        // Before the "/>" that closes the element.
        out.splice(end - 1..end - 1, mark.iter().copied());
        added += mark.len() as u32;
    }
    let xml = u32::from_le_bytes(out[4..8].try_into().unwrap()) + added;
    out[4..8].copy_from_slice(&xml.to_le_bytes());
    out
}

/// After how many blocks `series` is most like itself again, looking from `least` to `most`.
fn repeats_after(series: &[f32], least: usize, most: usize) -> usize {
    let mean = series.iter().sum::<f32>() / series.len() as f32;
    let alike = |lag: usize| {
        let pairs = series.iter().zip(&series[lag..]);
        pairs.map(|(a, b)| (a - mean) * (b - mean)).sum::<f32>() / (series.len() - lag) as f32
    };
    (least..=most)
        .max_by(|&a, &b| alike(a).total_cmp(&alike(b)))
        .unwrap()
}

/// The `time:Position` meloet sends reaches a JUCE plugin: a factory patch of Surge XT, its
/// step sequencers set to run with the tempo, plays its 16 steps in half a bar of the song,
/// whatever the tempo. Without a position Surge XT runs them at 120 whatever the song does.
#[test]
#[ignore = "needs Surge XT (LV2) and its factory patches"]
fn surge_xt_lv2_runs_its_sequencer_at_the_tempo_of_the_song() {
    if !installed(SURGE_LV2) || !Path::new(SURGE_SEQUENCE).exists() {
        eprintln!("{SURGE_SEQUENCE} is not installed; nothing tested");
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let mut first = load(&mut host, 0, SURGE_LV2);
    run(&mut first, 10, &[]);
    // The state of a Surge XT just made, with the patch put in the place of its own: the
    // patch file is 60 bytes of header and then what Surge XT keeps as its state.
    let state = host.state(0);
    let key = b"StateString> \"";
    let from = state.windows(key.len()).position(|w| w == key).unwrap() + key.len();
    let to = from + state[from..].iter().position(|&b| b == b'"').unwrap();
    let patch = synced(&std::fs::read(SURGE_SEQUENCE).unwrap()[60..]);
    let state = [&state[..from], juce_base64(&patch).as_bytes(), &state[to..]].concat();
    // Tempos at which neither the bar nor its half comes near the 2 s a bar takes at 120.
    for bpm in [75.0, 90.0, 100.0, 150.0] {
        let bar_blocks = 4.0 * 60.0 / bpm * 48_000.0 / 256.0;
        let bright = sequence_at(&mut host, 1, &state, bpm, (bar_blocks * 8.0) as usize);
        assert_eq!(host.ask(Request::Unload { slot: 1 }), Reply::Ok);
        let lag = repeats_after(&bright[40..], 60, (bar_blocks * 0.9) as usize);
        let half_bar = bar_blocks / 2.0;
        println!("{bpm}: repeats after {lag} blocks, half a bar is {half_bar:.1}");
        assert!(
            (lag as f64 - half_bar).abs() < half_bar * 0.02 + 1.0,
            "at {bpm} the sequence repeats after {lag} blocks, half a bar is {half_bar:.1}"
        );
    }
}

/// The song at `frame` of a run in 4/4 whose tempo goes in a straight line over the beats
/// from `from` to `to` in `quarters` quarter notes (docs/architecture.md 21-1), playing. As
/// the engine does it, the tempo is the one at the frame, not a mean over the block.
fn ramp_at(frame: u64, from: f64, to: f64, quarters: f64) -> Transport {
    let slope = (to - from) / quarters;
    let minutes = frame as f64 / 48_000.0 / 60.0;
    let at = from / slope * (slope * minutes).exp_m1();
    Transport {
        tempo_bpm: from + slope * at,
        position_quarters: at,
        bar_start_quarters: (at / 4.0).floor() * 4.0,
        bar: (at / 4.0) as u64,
        ..song_at(frame, from)
    }
}

/// `series` (one value a block) laid out again over the song: 32 values to a quarter note.
fn over_quarters(series: &[f32], quarters_at: &dyn Fn(usize) -> f64) -> Vec<f32> {
    let mut out = Vec::new();
    let mut block = 0;
    while block + 1 < series.len() {
        let want = out.len() as f64 / 32.0;
        let (a, b) = (quarters_at(block), quarters_at(block + 1));
        if want > b {
            block += 1;
            continue;
        }
        let part = ((want - a) / (b - a)).clamp(0.0, 1.0) as f32;
        out.push(series[block] + (series[block + 1] - series[block]) * part);
    }
    out
}

/// The tempo meloet puts into the `ProcessContext` reaches a VST3 plugin, also while it
/// changes: Surge XT's step sequencers, set to run with the tempo, play their 16 steps in
/// half a bar of the song at four tempos, and all along a ramp from 90 to 150.
#[test]
#[ignore = "needs Surge XT (VST3) and its factory patches"]
fn surge_xt_vst3_runs_its_sequencer_at_the_tempo_of_the_song() {
    if !installed(SURGE_VST3) || !Path::new(SURGE_SEQUENCE).exists() {
        eprintln!("{SURGE_SEQUENCE} is not installed; nothing tested");
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let mut first = load(&mut host, 0, SURGE_VST3);
    run(&mut first, 10, &[]);
    // meloet's VST3 state is 4 bytes of its own, the length of the component's part, and that
    // part: for a JUCE plugin, what the plugin keeps as its state (JUCE's own data after it
    // may be left out). The patch file is 60 bytes of header and then that state.
    let own = host.state(0);
    let patch = synced(&std::fs::read(SURGE_SEQUENCE).unwrap()[60..]);
    let state = [&own[..4], &(patch.len() as u64).to_le_bytes(), &patch[..]].concat();

    for bpm in [75.0, 90.0, 100.0, 150.0] {
        let bar_blocks = 4.0 * 60.0 / bpm * 48_000.0 / 256.0;
        let song = |frame| song_at(frame, bpm);
        let blocks = (bar_blocks * 8.0) as usize;
        let bright = sequence_along(&mut host, 1, SURGE_VST3, &state, blocks, &song);
        assert_eq!(host.ask(Request::Unload { slot: 1 }), Reply::Ok);
        let lag = repeats_after(&bright[40..], 60, (bar_blocks * 0.9) as usize);
        let half_bar = bar_blocks / 2.0;
        println!("{bpm}: repeats after {lag} blocks, half a bar is {half_bar:.1}");
        assert!(
            (lag as f64 - half_bar).abs() < half_bar * 0.02 + 1.0,
            "at {bpm} the sequence repeats after {lag} blocks, half a bar is {half_bar:.1}"
        );
    }

    // 16 bars from 90 to 150, and 16 bars back down.
    for (from, to) in [(90.0, 150.0), (150.0, 90.0)] {
        let quarters = 64.0;
        let seconds = 60.0 * quarters / (to - from) * f64::ln(to / from);
        let blocks = (seconds * 48_000.0 / 256.0) as usize;
        let song = |frame| ramp_at(frame, from, to, quarters);
        let bright = sequence_along(&mut host, 1, SURGE_VST3, &state, blocks, &song);
        assert_eq!(host.ask(Request::Unload { slot: 1 }), Reply::Ok);
        let laid = over_quarters(&bright, &|block| {
            ramp_at(block as u64 * 256, from, to, quarters).position_quarters
        });
        // Half a bar is 64 values. Each third of the ramp by itself: a plugin that kept the
        // first tempo would repeat ever later (or earlier) along the song.
        let third = laid.len() / 3;
        for (n, part) in laid[32..].chunks(third).take(3).enumerate() {
            let lag = repeats_after(part, 26, 115);
            println!(
                "{from} to {to}, third {}: repeats after {:.3} quarter notes",
                n + 1,
                lag as f64 / 32.0
            );
            assert!(
                (62..=66).contains(&lag),
                "{from} to {to}, third {}: the sequence repeats after {} quarter notes, not 2",
                n + 1,
                lag as f64 / 32.0
            );
        }
    }
}

/// `song` in 6/8: the meter and the bar's start change, the tempo and the position stay in
/// quarter notes.
fn in_six_eight(song: Transport) -> Transport {
    let bar = (song.position_quarters / 3.0).floor();
    Transport {
        meter_num: 6,
        meter_den: 8,
        bar_start_quarters: bar * 3.0,
        bar: bar as u64,
        ..song
    }
}

/// In 6/8 the tempo still counts quarter notes for both formats: Surge XT's step sequencers
/// play their 16 steps in two quarter notes at one tempo and along a ramp, and the LV2 one
/// follows a ramp in 4/4 as the VST3 one does.
#[test]
#[ignore = "needs Surge XT (VST3 and LV2) and its factory patches"]
fn surge_xt_runs_its_sequencer_by_quarter_notes_in_six_eight() {
    if !installed(SURGE_VST3) || !installed(SURGE_LV2) || !Path::new(SURGE_SEQUENCE).exists() {
        eprintln!("Surge XT or {SURGE_SEQUENCE} is not installed; nothing tested");
        return;
    }
    let patch = synced(&std::fs::read(SURGE_SEQUENCE).unwrap()[60..]);
    for plugin in [SURGE_LV2, SURGE_VST3] {
        let mut host = Host::start_with(GEOMETRY, &[]);
        let mut first = load(&mut host, 0, plugin);
        run(&mut first, 10, &[]);
        let own = host.state(0);
        let state = if plugin == SURGE_LV2 {
            let key = b"StateString> \"";
            let from = own.windows(key.len()).position(|w| w == key).unwrap() + key.len();
            let to = from + own[from..].iter().position(|&b| b == b'"').unwrap();
            [&own[..from], juce_base64(&patch).as_bytes(), &own[to..]].concat()
        } else {
            [&own[..4], &(patch.len() as u64).to_le_bytes(), &patch[..]].concat()
        };
        let name = plugin.0;

        for bpm in [90.0, 120.0, 150.0] {
            let half_blocks = 2.0 * 60.0 / bpm * 48_000.0 / 256.0;
            let song = |frame| in_six_eight(song_at(frame, bpm));
            let blocks = (half_blocks * 16.0) as usize;
            let bright = sequence_along(&mut host, 1, plugin, &state, blocks, &song);
            assert_eq!(host.ask(Request::Unload { slot: 1 }), Reply::Ok);
            // From below half the length (twice the tempo) to short of twice the length,
            // where a sequence at the right tempo is like itself again too.
            let lag = repeats_after(&bright[40..], 30, (half_blocks * 1.6) as usize);
            println!(
                "{name} 6/8 {bpm}: repeats after {lag} blocks, 2 quarter notes are {half_blocks:.1}"
            );
            assert!(
                (lag as f64 - half_blocks).abs() < half_blocks * 0.02 + 1.0,
                "{name} in 6/8 at {bpm} the sequence repeats after {lag} blocks, not {half_blocks:.1}"
            );
        }

        for (six_eight, from, to) in [
            (false, 90.0, 150.0),
            (false, 150.0, 90.0),
            (true, 90.0, 150.0),
            (true, 150.0, 90.0),
        ] {
            let quarters = 64.0;
            let seconds = 60.0 * quarters / (to - from) * f64::ln(to / from);
            let blocks = (seconds * 48_000.0 / 256.0) as usize;
            let song = |frame| {
                let song = ramp_at(frame, from, to, quarters);
                if six_eight { in_six_eight(song) } else { song }
            };
            let bright = sequence_along(&mut host, 1, plugin, &state, blocks, &song);
            assert_eq!(host.ask(Request::Unload { slot: 1 }), Reply::Ok);
            let laid = over_quarters(&bright, &|block| {
                ramp_at(block as u64 * 256, from, to, quarters).position_quarters
            });
            let third = laid.len() / 3;
            let meter = if six_eight { "6/8" } else { "4/4" };
            for (n, part) in laid[32..].chunks(third).take(3).enumerate() {
                let lag = repeats_after(part, 26, 115);
                println!(
                    "{name} {meter} {from} to {to}, third {}: repeats after {:.3} quarter notes",
                    n + 1,
                    lag as f64 / 32.0
                );
                assert!(
                    (62..=66).contains(&lag),
                    "{name} {meter} {from} to {to}, third {}: repeats after {} quarter notes, not 2",
                    n + 1,
                    lag as f64 / 32.0
                );
            }
        }
    }
}

/// The same keys and the same input through a host for an export or a playing one, each a
/// process of its own; left and right in a row.
fn one_run(plugin: (&str, &str), instrument: bool, offline: bool) -> Vec<f32> {
    let mut host = Host::start_as(GEOMETRY, &[], offline);
    let mut engine = load(&mut host, 0, plugin);
    let mut out = Vec::new();
    for n in 0..120 {
        // Loud and quiet in turn, so that a compressor has something to do.
        let level = if instrument { 0.0 } else { 0.6 } * (0.2 + (n % 30) as f32 / 30.0);
        let input: Vec<f32> = (0..256)
            .map(|i| {
                let t = (n * 256 + i) as f32 / 48_000.0;
                level * (t * 220.0 * std::f32::consts::TAU).sin()
            })
            .collect();
        let events = match (instrument, n) {
            (true, 2) => vec![note_on(60)],
            (true, 60) => vec![note_off(60), note_on(67)],
            _ => Vec::new(),
        };
        let (left, right) = block(&mut engine, &input, &events);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        out.extend(left);
        out.extend(right);
    }
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    out
}

/// How far two runs are apart, against how loud they are.
fn apart(a: &[f32], b: &[f32]) -> f32 {
    let diff: Vec<f32> = a.iter().zip(b).map(|(a, b)| a - b).collect();
    rms(&diff) / rms(a).max(1e-9)
}

/// An effect gives the same samples in two exports, and the samples it gives when it plays.
fn export_like_playing(plugin: (&str, &str)) {
    if !installed(plugin) {
        return;
    }
    let [first, second, playing] =
        [true, true, false].map(|offline| one_run(plugin, false, offline));
    assert!(rms(&first) > 0.01, "{}", rms(&first));
    assert!(first == second, "two exports are not the same sound");
    assert!(first == playing, "the export is not the playing sound");
}

/// How loud each tenth of a second of a run is.
fn loudness(run: &[f32]) -> Vec<f32> {
    run.chunks(9600).map(rms).collect()
}

/// Surge XT starts its oscillators where it likes: no two runs of it give the same samples,
/// playing or not. What an export must keep is the notes and how loud they are.
fn surge_export_like_playing(plugin: (&str, &str)) {
    if !installed(plugin) {
        return;
    }
    let [first, second, playing, again] =
        [true, true, false, false].map(|offline| one_run(plugin, true, offline));
    println!(
        "{}: two exports {} apart, export and playing {}, two playing {}",
        plugin.0,
        apart(&first, &second),
        apart(&first, &playing),
        apart(&playing, &again)
    );
    println!("export  {:?}", loudness(&first));
    println!("playing {:?}", loudness(&playing));
    assert!(rms(&first) > 0.01, "{}", rms(&first));
    for other in [&second, &playing, &again] {
        for (at, (a, b)) in loudness(&first).iter().zip(loudness(other)).enumerate() {
            assert!((a - b).abs() <= 0.1 * a.max(b) + 1e-3, "{at}: {a} {b}");
        }
    }
}

#[test]
#[ignore = "needs LSP Plugins (VST3)"]
fn lsp_compressor_vst3_exports_the_same_sound_twice_and_as_it_plays() {
    export_like_playing(LSP_COMP_VST3);
}

#[test]
#[ignore = "needs LSP Plugins (LV2)"]
fn lsp_compressor_lv2_exports_the_same_sound_twice_and_as_it_plays() {
    export_like_playing(LSP_COMP_LV2);
}

#[test]
#[ignore = "needs Zam plugins (VST3 and LV2)"]
fn zamcomp_exports_the_same_sound_twice_and_as_it_plays() {
    export_like_playing(ZAMCOMP_VST3);
    export_like_playing(ZAMCOMP_LV2);
}

#[test]
#[ignore = "needs Dragonfly Reverb (LV2)"]
fn dragonfly_hall_lv2_exports_the_same_sound_twice_and_as_it_plays() {
    export_like_playing(DRAGONFLY_LV2);
}

#[test]
#[ignore = "needs Surge XT (VST3)"]
fn surge_xt_vst3_exports_its_notes_as_loud_as_it_plays_them() {
    surge_export_like_playing(SURGE_VST3);
}

#[test]
#[ignore = "needs Surge XT (LV2)"]
fn surge_xt_lv2_exports_its_notes_as_loud_as_it_plays_them() {
    surge_export_like_playing(SURGE_LV2);
}
