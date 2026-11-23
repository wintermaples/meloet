//! A state taken right after a value was set holds that value, with blocks running as
//! meloet's engine runs them: ZamComp's VST3 (of DPF) learns the value only in its next block,
//! and its state said the old one when it was taken at once (50 of 50 sets). Ignored by
//! default: it needs the plugins of this PC; one that is not installed is passed over.
//!
//! ```text
//! cargo test -p meloet-plughost --test real_params -- --ignored --nocapture --test-threads=1
//! ```

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

use common::{GEOMETRY, Host, block};
use meloet_plugin::PluginKey;
use meloet_plugin::wire::{ParamShape, Reply, Request};

type Plugin = (&'static str, &'static str, &'static str);

const PLUGINS: [Plugin; 4] = [
    (
        "ZamComp (VST3)",
        "vst3:2046504473616C63504D435A00000000",
        "/usr/lib/vst3/ZamComp.vst3",
    ),
    (
        "ZamComp (LV2)",
        "lv2:urn:zamaudio:ZamComp",
        "/usr/lib/lv2/ZamComp.lv2",
    ),
    (
        "Surge XT (VST3)",
        "vst3:ABCDEF019182FAEB566D624153675854",
        "/usr/lib/vst3/Surge XT.vst3",
    ),
    (
        "Surge XT (LV2)",
        "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
        "/usr/lib/lv2/Surge XT.lv2",
    ),
];

/// A block of 256 frames at 48 kHz.
const BLOCK: Duration = Duration::from_micros(5333);

fn fifty_sets(plugin: Plugin) {
    if !Path::new(plugin.2).exists() {
        eprintln!("{} is not installed; nothing tested", plugin.2);
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let key: PluginKey = plugin.1.parse().unwrap();
    let (mut engine, _) = host.load_plugin(0, &key, plugin.2).unwrap();
    let Reply::Params { params } = host.ask(Request::Params { slot: 0 }) else {
        panic!("a list")
    };
    let param = (params.iter())
        .find(|p| p.shape == ParamShape::Continuous && p.automatable && !p.midi_cc)
        .expect("a smooth parameter");
    let stop = Arc::new(AtomicBool::new(false));
    let running = stop.clone();
    let blocks = std::thread::spawn(move || {
        while !running.load(Relaxed) {
            block(&mut engine, &[0.0; 256], &[]);
            std::thread::sleep(BLOCK);
        }
        engine
    });
    let mut last = host.state(0);
    let mut kept = 0;
    let mut slowest = Duration::ZERO;
    for i in 0..50 {
        let value = 0.1 + 0.015 * f64::from(i);
        host.ask(Request::SetParam {
            slot: 0,
            id: param.id,
            value,
        });
        let asked = Instant::now();
        let state = host.state(0);
        slowest = slowest.max(asked.elapsed());
        if state == last {
            kept += 1;
        }
        last = state;
    }
    stop.store(true, Relaxed);
    let _engine = blocks.join().unwrap();
    eprintln!(
        "{} {:?}: {kept} of 50 states kept the old value; the slowest took {:.1} ms",
        plugin.0,
        param.name,
        slowest.as_secs_f64() * 1000.0
    );
    assert_eq!(kept, 0);

    // No blocks: the state is taken as it is after a while, and only once after a value.
    host.ask(Request::SetParam {
        slot: 0,
        id: param.id,
        value: 0.9,
    });
    let asked = Instant::now();
    host.state(0);
    let first = asked.elapsed();
    let asked = Instant::now();
    host.state(0);
    let second = asked.elapsed();
    eprintln!(
        "  with no blocks: {:.1} ms, then {:.1} ms",
        first.as_secs_f64() * 1000.0,
        second.as_secs_f64() * 1000.0
    );
    assert!(first < Duration::from_millis(500), "{first:?}");
    assert!(second < Duration::from_millis(20), "{second:?}");
}

#[test]
#[ignore = "needs ZamComp and Surge XT"]
fn a_state_taken_after_a_set_holds_the_value() {
    for plugin in PLUGINS {
        fifty_sets(plugin);
    }
}
