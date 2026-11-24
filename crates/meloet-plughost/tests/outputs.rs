//! The output sets of a plugin through the real `meloet-plughost` binary, with the fake
//! instrument `outs` (`formats::fake::OUTS`): its sets as `Loaded` tells them, settings from
//! the first block and later, and the capture of an export's host.

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
