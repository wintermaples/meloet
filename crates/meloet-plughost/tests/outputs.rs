//! The output sets of a plugin through the real `meloet-plughost` binary, with the fake
//! instrument `outs` (`formats::fake::OUTS`): its sets as `Loaded` tells them, settings from
//! the first block and later, and the capture of an export's host. Also the output buses of
//! Surge XT (VST3), when it is installed (ignored by default).

mod common;

use common::{FAKE, GEOMETRY, Host, block, note_on};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::EngineEnd;
use meloet_plugin::wire::{
    CapturedSet, MAX_BLOB_LEN, OutputSet, OutputSetting, OutputSettings, Reply, Request,
};

/// What each output of `outs` gives while a key is held.
const A: (f32, f32) = (0.0625, 0.125);
const B: f32 = 0.03125;
const C: f32 = 0.25;
const D: f32 = 0.5;

fn setting(id: &str, gain_db: f32, pan: Option<f32>, on: Option<bool>) -> OutputSetting {
    OutputSetting {
        id: id.into(),
        gain_db,
        pan,
        on,
    }
}

fn settings(sets: Vec<OutputSetting>) -> OutputSettings {
    OutputSettings { rule: 1, sets }
}

fn load(host: &mut Host, slot: u32, outputs: OutputSettings) -> Reply {
    let geometry = host.map.geometry();
    host.ask(Request::Load {
        slot,
        key: PluginKey::lv2("urn:meloet:fake:outs").unwrap(),
        bundle: "/nowhere/fake.lv2".into(),
        sample_rate: 48_000,
        max_frames: geometry.max_frames,
        outputs,
    })
}

fn is_error(reply: Reply) -> bool {
    matches!(reply, Reply::Error { .. })
}

#[test]
fn settings_hold_from_the_first_block_and_move_over_20_ms() {
    let mut host = Host::start();
    let mut engine = EngineEnd::new(host.map.clone(), 0).unwrap();
    let off = settings(vec![setting("c", 0.0, None, Some(false))]);
    let Reply::Loaded { plugin } = load(&mut host, 0, off) else {
        panic!("not loaded");
    };
    let set = |id: &str, name: &str, channels, on, pan| OutputSet {
        id: id.into(),
        name: name.into(),
        channels,
        on,
        pan,
    };
    assert_eq!(
        plugin.outputs,
        [
            set("a", "Fake Pair", 2, true, 0.0),
            set("b", "Fake Middle", 1, true, 0.0),
            set("c", "Fake Left", 1, true, -1.0),
            set("d", "Fake Spare", 1, false, 0.0),
        ]
    );
    let silence = [0.0f32; 256];
    // `c` is off from the first sound on.
    let (left, right) = block(&mut engine, &silence, &[note_on(60)]);
    assert!(left.iter().all(|&x| x == A.0 + B), "{left:?}");
    assert!(right.iter().all(|&x| x == A.1 + B), "{right:?}");

    // The whole table again: `d` on, so `c` is back as the rule says.
    let on = settings(vec![setting("d", 0.0, None, Some(true))]);
    assert_eq!(
        host.ask(Request::SetOutputs {
            slot: 0,
            outputs: on
        }),
        Reply::Ok
    );
    let mut lefts = Vec::new();
    for _ in 0..5 {
        lefts.extend(block(&mut engine, &silence, &[]).0);
    }
    let (from, to) = (A.0 + B, A.0 + B + C + D);
    let step = (C + D) / 960.0;
    assert!((lefts[0] - from - step).abs() < 1e-6, "{}", lefts[0]);
    for pair in lefts.windows(2) {
        assert!((pair[1] - pair[0]).abs() <= step * 1.001, "{pair:?}");
    }
    assert_ne!(lefts[958], to);
    assert!(lefts[959..].iter().all(|&x| x == to));
    let (_, right) = block(&mut engine, &silence, &[]);
    assert!(right.iter().all(|&x| x == A.1 + B + D));

    // Settings of a slot with no plugin, and of a plugin with no sets, are refused or let be.
    assert!(is_error(host.ask(Request::SetOutputs {
        slot: 3,
        outputs: OutputSettings::default(),
    })));
    host.load(1, "sine");
    assert_eq!(
        host.ask(Request::SetOutputs {
            slot: 1,
            outputs: OutputSettings::default(),
        }),
        Reply::Ok
    );
    // Only a host for an export keeps sound.
    assert!(is_error(host.ask(Request::Capture {
        slot: 0,
        frames: 100
    })));
    assert!(is_error(host.ask(Request::Captured { slot: 0 })));
}

#[test]
fn a_host_for_an_export_keeps_the_sound_of_each_set() {
    let mut host = Host::start_as(GEOMETRY, &[(FAKE, "1")], true);
    let mut engine = EngineEnd::new(host.map.clone(), 0).unwrap();
    let changed = settings(vec![
        setting("a", -12.0, Some(1.0), Some(false)),
        setting("c", 6.0, Some(0.5), None),
    ]);
    assert!(matches!(load(&mut host, 0, changed), Reply::Loaded { .. }));
    assert!(is_error(host.ask(Request::Captured { slot: 0 })));
    // Too long for one message: four sets of eight bytes a frame.
    let most = MAX_BLOB_LEN as u64 / 32;
    match host.ask(Request::Capture {
        slot: 0,
        frames: most + 1,
    }) {
        Reply::Error { message } => assert!(message.contains(&most.to_string()), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        host.ask(Request::Capture {
            slot: 0,
            frames: 300
        }),
        Reply::Ok
    );
    let silence = [0.0f32; 256];
    block(&mut engine, &silence, &[note_on(60)]);
    block(&mut engine, &silence, &[]);
    let (reply, blob) = host.ask_with(Request::Captured { slot: 0 }, &[]);
    let frames = 300;
    let captured = |id: &str| CapturedSet {
        id: id.into(),
        frames,
    };
    assert_eq!(
        reply,
        Reply::Captured {
            sets: vec![captured("a"), captured("b"), captured("c"), captured("d")]
        }
    );
    assert_eq!(blob.len(), 4 * 300 * 8);
    let floats: Vec<f32> = (blob.as_chunks::<4>().0.iter())
        .map(|&b| f32::from_le_bytes(b))
        .collect();
    // Each set alone at 0 dB where the rule puts it, `a` too though it is off.
    for (set, (left, right)) in [A, (B, B), (C, 0.0), (D, D)].into_iter().enumerate() {
        let sound = &floats[set * 600..][..600];
        assert!(
            sound.chunks(2).all(|pair| pair == [left, right]),
            "set {set}: {:?}",
            &sound[..4]
        );
    }
    // Taken: nothing is kept any more.
    assert!(is_error(host.ask(Request::Captured { slot: 0 })));
}

/// Keeps the sound of `blocks` blocks with `events` in the first; the sets the host answers
/// with, and how many bytes of sound.
fn capture(
    host: &mut Host,
    engine: &mut EngineEnd,
    blocks: usize,
    events: &[meloet_plugin::ipc::ToPlugin],
) -> (Vec<CapturedSet>, usize) {
    let frames = 256 * blocks as u64;
    let request = Request::Capture { slot: 0, frames };
    assert_eq!(host.ask(request), Reply::Ok);
    let silence = [0.0f32; 256];
    for at in 0..blocks {
        let (left, right) = block(engine, &silence, if at == 0 { events } else { &[] });
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    }
    match host.ask_with(Request::Captured { slot: 0 }, &[]) {
        (Reply::Captured { sets }, blob) => (sets, blob.len()),
        other => panic!("{other:?}"),
    }
}

#[test]
#[ignore = "needs Surge XT (VST3)"]
fn the_output_buses_of_a_vst3_instrument_through_the_wire() {
    let bundle = "/usr/lib/vst3/Surge XT.vst3";
    if !std::path::Path::new(bundle).exists() {
        eprintln!("{bundle} is not installed; nothing tested");
        return;
    }
    let mut host = Host::start_as(GEOMETRY, &[], true);
    let mut engine = EngineEnd::new(host.map.clone(), 0).unwrap();
    let scene_a = settings(vec![setting("bus1", 0.0, None, Some(true))]);
    let reply = host.ask(Request::Load {
        slot: 0,
        key: "vst3:ABCDEF019182FAEB566D624153675854".parse().unwrap(),
        bundle: bundle.into(),
        sample_rate: 48_000,
        max_frames: GEOMETRY.max_frames,
        outputs: scene_a.clone(),
    });
    let Reply::Loaded { plugin } = reply else {
        panic!("{reply:?}");
    };
    let ids: Vec<&str> = plugin.outputs.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["bus0", "bus1", "bus2"]);
    let measured =
        |sets: &[CapturedSet]| -> Vec<String> { sets.iter().map(|set| set.id.clone()).collect() };
    // Scene A heard from the load: measured with the main bus; scene B is not on.
    let (sets, bytes) = capture(&mut host, &mut engine, 10, &[note_on(60)]);
    assert_eq!(measured(&sets), ["bus0", "bus1"]);
    assert!(sets.iter().all(|set| set.frames == 2560));
    assert_eq!(bytes, 2 * 2560 * 8);

    // Off: down over 20 ms, then switched off by the host by itself.
    let off = OutputSettings::default();
    let request = Request::SetOutputs {
        slot: 0,
        outputs: off,
    };
    assert_eq!(host.ask(request), Reply::Ok);
    let silence = [0.0f32; 256];
    for _ in 0..8 {
        block(&mut engine, &silence, &[]);
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    let (sets, _) = capture(&mut host, &mut engine, 4, &[]);
    assert_eq!(measured(&sets), ["bus0"]);

    // On again, through the wire.
    let request = Request::SetOutputs {
        slot: 0,
        outputs: scene_a,
    };
    assert_eq!(host.ask(request), Reply::Ok);
    let (sets, _) = capture(&mut host, &mut engine, 4, &[note_on(64)]);
    assert_eq!(measured(&sets), ["bus0", "bus1"]);
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
}
