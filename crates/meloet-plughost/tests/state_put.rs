//! A state put into a plugin while meloet's engine runs its blocks: some plugins take it in
//! only with their next block (Surge XT; the fake `late`), so what is asked after the put waits
//! for the blocks. The real plugins' measurement is ignored by default:
//!
//! ```text
//! cargo test -p meloet-plughost --features dev-hooks --test state_put -- --ignored \
//!     --nocapture --test-threads=1
//! ```

mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use common::{GEOMETRY, Host, block};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::EngineEnd;
use meloet_plugin::wire::{ParamShape, Reply, Request};

/// A block of 256 frames at 48 kHz: the PC's quantum.
const BLOCK: Duration = Duration::from_micros(5333);

/// Blocks of one slot on the clock, on a thread of their own, counted.
struct Blocks {
    stop: Arc<AtomicBool>,
    count: Arc<AtomicU64>,
    thread: Option<JoinHandle<EngineEnd>>,
}

impl Blocks {
    fn run(mut engine: EngineEnd) -> Blocks {
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU64::new(0));
        let (stopping, counting) = (stop.clone(), count.clone());
        let thread = std::thread::spawn(move || {
            let mut next = Instant::now();
            while !stopping.load(SeqCst) {
                block(&mut engine, &[0.0; 256], &[]);
                counting.fetch_add(1, SeqCst);
                next += BLOCK;
                std::thread::sleep(next.saturating_duration_since(Instant::now()));
            }
            engine
        });
        Blocks {
            stop,
            count,
            thread: Some(thread),
        }
    }

    fn count(&self) -> u64 {
        self.count.load(SeqCst)
    }

    fn stop(mut self) -> EngineEnd {
        self.stop.store(true, SeqCst);
        self.thread.take().unwrap().join().unwrap()
    }
}

/// What a state says of the plugin: an LV2 state without meloet's own head (the values of
/// `patch` parameters it knew when the state was taken).
fn body(state: &[u8]) -> &[u8] {
    match state.strip_prefix(b"ML2S") {
        Some(rest) => {
            let n = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            &rest[4 + 12 * n..]
        }
        None => state,
    }
}

fn put(host: &mut Host, slot: u32, state: &[u8]) {
    let (reply, _) = host.ask_with(Request::SetState { slot }, state);
    assert_eq!(reply, Reply::Ok);
}

type Plugin = (&'static str, &'static str, &'static str);

const REAL: [Plugin; 4] = [
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
];

/// States put in turn while blocks run; each followed by states taken every 2 ms for 60 ms
/// (an LV2 state is taken with the plugin's blocks held off: room is left for them). Prints,
/// for each put, how long the first state took and how many blocks had ended when the state
/// first said what it says for good; asserts that the first state said it.
fn measure(plugin: Plugin) {
    if !Path::new(plugin.2).exists() {
        eprintln!("{} is not installed; nothing measured", plugin.2);
        return;
    }
    let mut host = Host::start_with(GEOMETRY, &[]);
    let key: PluginKey = plugin.1.parse().unwrap();
    let (engine, _) = host.load_plugin(0, &key, plugin.2).unwrap();
    let Reply::Params { params } = host.ask(Request::Params { slot: 0 }) else {
        panic!("a list")
    };
    let param = (params.iter())
        .find(|p| p.shape == ParamShape::Continuous && p.automatable && !p.midi_cc)
        .expect("a smooth parameter")
        .clone();
    let blocks = Blocks::run(engine);
    std::thread::sleep(Duration::from_millis(100));
    let a = host.state(0);
    let value = if param.value.unwrap_or(0.0) > 0.5 {
        0.1
    } else {
        0.9
    };
    host.ask(Request::SetParam {
        slot: 0,
        id: param.id,
        value,
    });
    std::thread::sleep(Duration::from_millis(100));
    let b = host.state(0);
    assert_ne!(
        body(&a),
        body(&b),
        "{}: the parameter is in the state",
        plugin.0
    );

    const PUTS: usize = 20;
    let mut right_at_once = 0;
    let (mut most_blocks, mut slowest, mut quickest) = (0, Duration::ZERO, Duration::MAX);
    for n in 0..PUTS {
        put(&mut host, 0, if n % 2 == 0 { &a } else { &b });
        let (put_at, counted) = (Instant::now(), blocks.count());
        let mut polls: Vec<(u64, Duration, Vec<u8>)> = Vec::new();
        while put_at.elapsed() < Duration::from_millis(60) {
            let state = host.state(0);
            polls.push((blocks.count() - counted, put_at.elapsed(), state));
            std::thread::sleep(Duration::from_millis(2));
        }
        let last = body(&polls.last().unwrap().2).to_vec();
        let from = (polls.iter().rposition(|(_, _, s)| body(s) != last)).map_or(0, |at| at + 1);
        let (ran, after, _) = &polls[from];
        right_at_once += usize::from(from == 0);
        most_blocks = most_blocks.max(*ran);
        slowest = slowest.max(polls[0].1);
        quickest = quickest.min(polls[0].1);
        eprintln!(
            "  put {n}: the first state took {:.1} ms; the state said it for good from the {} \
             of {} taken ({ran} blocks ended, {:.1} ms)",
            polls[0].1.as_secs_f64() * 1000.0,
            from + 1,
            polls.len(),
            after.as_secs_f64() * 1000.0,
        );
    }
    let engine = blocks.stop();
    eprintln!(
        "{}: {right_at_once} of {PUTS} states taken right after the put said it for good; the \
         latest after {most_blocks} blocks; the first state took {:.1} to {:.1} ms",
        plugin.0,
        quickest.as_secs_f64() * 1000.0,
        slowest.as_secs_f64() * 1000.0,
    );

    // Blocks that stopped 300 ms before, and a plugin that never ran one: no wait.
    std::thread::sleep(Duration::from_millis(300));
    put(&mut host, 0, &a);
    let asked = Instant::now();
    let now = host.state(0);
    eprintln!(
        "  blocks stopped 300 ms before: the state right after the put took {:.1} ms and says \
         the new one: {}",
        asked.elapsed().as_secs_f64() * 1000.0,
        body(&now) != body(&b)
    );
    let (_fresh, _) = host.load_plugin(1, &key, plugin.2).unwrap();
    put(&mut host, 1, &b);
    let asked = Instant::now();
    let now = host.state(1);
    eprintln!(
        "  a plugin that ran no block yet: the state right after the put took {:.1} ms and \
         says the new one: {}",
        asked.elapsed().as_secs_f64() * 1000.0,
        body(&now) != body(&a)
    );
    drop(engine);
    assert_eq!(right_at_once, PUTS, "{}", plugin.0);
}

#[test]
#[ignore = "needs Surge XT and ZamComp"]
fn real_plugins_say_a_state_put_while_blocks_run() {
    for plugin in REAL {
        measure(plugin);
    }
}

/// LV2 states in meloet's form whose Turtle names no plugin, another plugin, or nothing at all
/// are refused, and the process goes on.
#[test]
#[ignore = "needs ZamComp and Surge XT (LV2)"]
fn real_lv2_states_for_no_plugin_are_refused() {
    let plugins = [
        ("lv2:urn:zamaudio:ZamComp", "/usr/lib/lv2/ZamComp.lv2"),
        (
            "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
            "/usr/lib/lv2/Surge XT.lv2",
        ),
    ];
    for (key, bundle) in plugins {
        if !Path::new(bundle).exists() {
            eprintln!("{bundle} is not installed; nothing tested");
            continue;
        }
        let mut host = Host::start_with(GEOMETRY, &[]);
        let key: PluginKey = key.parse().unwrap();
        let uri = key.id().to_owned();
        host.load_plugin(0, &key, bundle).unwrap();
        let state = host.state(0);
        let text = std::str::from_utf8(body(&state)).unwrap().to_owned();
        let applies = format!("\tlv2:appliesTo <{uri}> ;\n");
        assert!(text.contains(&applies), "{text}");
        let lv2 = |turtle: &str| {
            let mut bytes = b"ML2S".to_vec();
            bytes.extend(0u32.to_le_bytes());
            bytes.extend(turtle.as_bytes());
            bytes
        };
        for (what, turtle) in [
            ("no appliesTo", text.replace(&applies, "")),
            (
                "another plugin",
                text.replace(&uri, "http://example.org/not-this-plugin"),
            ),
            ("an empty text", String::new()),
            (
                "only prefixes",
                "@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n".into(),
            ),
        ] {
            let (reply, _) = host.ask_with(Request::SetState { slot: 0 }, &lv2(&turtle));
            eprintln!("{key}: {what}: {reply:?}");
            assert!(matches!(reply, Reply::Error { .. }), "{key}: {what}");
            assert_eq!(
                host.state(0),
                state,
                "{key}: {what}: the plugin is as it was"
            );
        }
        put(&mut host, 0, &state);
    }
}

fn level_state(level: f64) -> Vec<u8> {
    level.to_le_bytes().to_vec()
}

/// `late` takes a state in only with its next block while its blocks run: what is asked of it
/// after the put waits for that (its state, and a parameter set, which must land after the
/// state).
#[test]
fn what_comes_after_a_state_put_while_blocks_run_finds_the_state_in() {
    let mut host = Host::start();
    let (engine, _) = host.load(0, "late");
    let blocks = Blocks::run(engine);
    std::thread::sleep(Duration::from_millis(30));
    let mut waited = Vec::new();
    for round in 0..20 {
        let level = 0.1 + 0.02 * f64::from(round);
        put(&mut host, 0, &level_state(level));
        let asked = Instant::now();
        assert_eq!(host.state(0), level_state(level), "round {round}");
        waited.push(asked.elapsed());
    }
    let longest = waited.iter().max().unwrap();
    eprintln!("a state taken right after a put waited {waited:?}");
    assert!(*longest < Duration::from_millis(250), "{longest:?}");
    // Asked once more: nothing to wait for.
    let asked = Instant::now();
    host.state(0);
    assert!(asked.elapsed() < Duration::from_millis(5));

    // A parameter set right after a put comes after the state, not under it.
    put(&mut host, 0, &level_state(0.25));
    host.ask(Request::SetParam {
        slot: 0,
        id: 0,
        value: 0.75,
    });
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(host.state(0), level_state(0.75));

    // Blocks that have stopped are not waited for: the plugin takes the state with its next
    // block, whenever that comes.
    let mut engine = blocks.stop();
    std::thread::sleep(Duration::from_millis(100));
    put(&mut host, 0, &level_state(0.5));
    let asked = Instant::now();
    assert_eq!(host.state(0), level_state(0.75));
    assert!(asked.elapsed() < Duration::from_millis(20));
    block(&mut engine, &[0.0; 256], &[]);
    assert_eq!(host.state(0), level_state(0.5));

    // A plugin that never ran a block takes it at once.
    let (_fresh, _) = host.load(1, "late");
    put(&mut host, 1, &level_state(0.3));
    let asked = Instant::now();
    assert_eq!(host.state(1), level_state(0.3));
    assert!(asked.elapsed() < Duration::from_millis(20));
}
