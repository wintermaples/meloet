//! A state put into a plugin while the engine runs its blocks, and a process whose answer
//! breaks: the plugin processes as `Hosts` drives them (the real `meloet-plughost` with its fake
//! plugins, and a stand-in process). Every process is gone when its test ends.

mod plughost;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_core::plugins::hosts::{
    DEFAULT_GEOMETRY, HostError, HostNews, HostOptions, Hosts, LoadSpec, Loaded, wait,
};
use meloet_engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineParts, PluginInstrument,
    monotonic_us,
};
use meloet_plugin::PluginKey;

use plughost::{alive, wait_for};

const RATE: u32 = 48_000;
/// The PC's quantum: 256 frames at 48 kHz.
const BLOCK: usize = 256;

fn engine() -> EngineParts {
    let mut parts = Engine::new(EngineConfig {
        sample_rate: RATE,
        max_frames: BLOCK,
        max_tracks: 1,
    });
    parts.engine.prepare(RATE);
    parts
}

fn hosts(exe: PathBuf) -> (Hosts, Receiver<HostNews>) {
    let (tx, news) = mpsc::channel();
    let options = HostOptions {
        exe,
        env: plughost::fake_env(),
        geometry: DEFAULT_GEOMETRY,
    };
    let hosts = Hosts::start(options, move |news| {
        let _ = tx.send(news);
    });
    (hosts, news)
}

fn load(hosts: &Hosts, parts: &EngineParts, fake: &str) -> Loaded {
    let spec = LoadSpec {
        key: PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap(),
        bundle: "/nowhere/fake.lv2".into(),
        sample_rate: RATE,
        outputs: Default::default(),
    };
    let block = parts.handle.plugin_block().clone();
    wait(|done| hosts.load(spec, block, done)).unwrap()
}

fn level_state(level: f64) -> Vec<u8> {
    level.to_le_bytes().to_vec()
}

/// What core does for a state the AI link puts in (`PluginAsk::PutState`): `set_state`, then
/// `state` at once. While the engine runs the plugin's blocks, the fake `late` takes a state
/// only with its next block, as Surge XT does; the state said after the put is the new one.
#[test]
fn a_state_put_while_the_engine_runs_is_the_state_said_next() {
    let mut parts = engine();
    let (hosts, news) = hosts(plughost::exe());
    let late = load(&hosts, &parts, "late");
    let id = late.id;
    let instrument = Box::new(PluginInstrument(late.slot));
    assert!(parts.handle.swap_instrument(0, instrument).is_ok());
    assert!(
        parts
            .handle
            .send(EngineCommand::Audition {
                track: 0,
                keys: Chord::new(&[69]),
                velocity: 100,
            })
            .is_ok()
    );
    let stop = Arc::new(AtomicBool::new(false));
    let peaks = Arc::new(Mutex::new(Vec::new()));
    let (stopping, peaked) = (stop.clone(), peaks.clone());
    let blocks = std::thread::spawn(move || {
        let mut out = vec![0.0f32; BLOCK * 2];
        let step = Duration::from_secs_f64(BLOCK as f64 / f64::from(RATE));
        let mut next = Instant::now();
        while !stopping.load(SeqCst) {
            let now = monotonic_us();
            let timing = BlockTiming {
                callback_us: now,
                playback_us: now + 10_000,
            };
            parts.engine.process(&mut out, timing);
            parts.handle.drop_retired();
            let peak = (0..BLOCK).fold(0.0f32, |peak, f| out[f * 2].abs().max(peak));
            peaked.lock().unwrap().push(peak);
            next += step;
            std::thread::sleep(next.saturating_duration_since(Instant::now()));
        }
        parts
    });
    std::thread::sleep(Duration::from_millis(50));
    for round in 0..10 {
        let level = 0.1 + 0.05 * f64::from(round);
        wait(|done| hosts.set_state(id, level_state(level), done)).unwrap();
        let said = wait(|done| hosts.state(id, done)).unwrap();
        assert_eq!(said, level_state(level), "round {round}");
    }
    std::thread::sleep(Duration::from_millis(30));
    stop.store(true, SeqCst);
    let parts = blocks.join().unwrap();
    // The plugin sounded at the last level put in.
    let last = *peaks.lock().unwrap().last().unwrap();
    assert!((last - 0.55).abs() < 0.02, "{last}");
    assert!(news.try_recv().is_err(), "nothing stopped");
    drop((hosts, parts));
}

/// A stand-in for `meloet-plughost`: it greets and loads, and then answers `GetState` with a
/// frame no reader takes (a length past the limit), and lives on without a word.
const BROKEN_HOST: &str = r#"#!/usr/bin/env python3
import json, os, struct, time

def take(n):
    got = b""
    while len(got) < n:
        part = os.read(0, n - len(got))
        if not part:
            raise SystemExit(0)
        got += part
    return got

def say(id, reply):
    text = json.dumps({"kind": "reply", "id": id, "reply": reply}).encode()
    os.write(0, struct.pack("<II", len(text), 0) + text)

while True:
    text_len, blob_len = struct.unpack("<II", take(8))
    message = json.loads(take(text_len))
    take(blob_len)
    request = message["request"]
    if request["type"] == "hello":
        say(message["id"], {"type": "hello", "version": request["version"], "offline": False})
    elif request["type"] == "load":
        say(message["id"], {"type": "loaded", "plugin": {"latency": 0, "audioInputs": 0,
            "audioOutputs": 2, "editor": "none", "live": False}})
    elif request["type"] == "getState":
        os.write(0, struct.pack("<II", 0xffffffff, 0))
        while True:
            time.sleep(1)
    else:
        say(message["id"], {"type": "ok"})
"#;

/// A process whose answer cannot be read is killed: it is told as stopped soon after, and
/// its plugins can be loaded again (docs/architecture.md 20-10).
#[test]
fn a_process_whose_answer_cannot_be_read_is_killed_and_told_as_stopped() {
    let dir = plughost::temp_dir("broken-answer");
    let exe = dir.join("broken-host");
    std::fs::write(&exe, BROKEN_HOST).unwrap();
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let parts = engine();
    let (hosts, news) = hosts(exe);
    let loaded = load(&hosts, &parts, "sine");
    let pid = hosts.processes()[0].pid;
    assert_eq!(
        wait(|done| hosts.state(loaded.id, done)),
        Err(HostError::Gone)
    );
    let asked = Instant::now();
    let stopped = loop {
        match news.recv_timeout(Duration::from_secs(3)) {
            Ok(HostNews::Stopped { slots, .. }) => break slots,
            Ok(_) => {}
            Err(_) => panic!("the process was not told as stopped"),
        }
    };
    eprintln!(
        "told as stopped {:?} after the answer broke",
        asked.elapsed()
    );
    assert_eq!(stopped, [loaded.id]);
    assert!(
        wait_for(Duration::from_secs(2), || (!alive(pid)).then_some(())).is_some(),
        "the process is gone"
    );
    drop((loaded, hosts));
    let _ = std::fs::remove_dir_all(dir);
}
