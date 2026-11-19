//! The real `meloet-plughost` binary with the fake plugins: blocks through the shared memory,
//! requests over the socket, and what meloet's side sees when a plugin crashes or hangs.
//! Every child is killed when its test ends.

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{EngineEnd, Geometry, Mapping, ToPlugin};
use meloet_plugin::wire::{
    FromHost, LoadedPlugin, Reply, Request, ScanOutput, ToHost, WIRE_VERSION, WireError,
    read_frame, write_frame,
};
use meloet_plugin::{PluginFormat, PluginKey, launch};

const EXE: &str = env!("CARGO_BIN_EXE_meloet-plughost");
const FAKE: &str = "MELOET_PLUGHOST_FAKE";
const GEOMETRY: Geometry = Geometry {
    max_frames: 256,
    channels: 2,
    slots: 4,
    to_plugin_events: 64,
    from_plugin_events: 64,
};
/// One block at 48 kHz, and how much of it the engine would wait.
const BLOCK: Duration = Duration::from_micros(5333);
const BUDGET_NS: u64 = 4_000_000;
/// For blocks that must be answered: long, so a busy PC does not fail the test.
const PATIENT_NS: u64 = 5_000_000_000;

struct Killed(Child);

impl Killed {
    fn wait_for_exit(&mut self, within: Duration) -> Option<ExitStatus> {
        let until = Instant::now() + within;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return Some(status);
            }
            if Instant::now() > until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Host {
    child: Killed,
    control: UnixStream,
    map: Arc<Mapping>,
    next_id: u64,
}

impl Host {
    fn start() -> Host {
        let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
        let (ours, theirs) = UnixStream::pair().unwrap();
        let child = launch::host_command(Path::new(EXE), theirs.as_fd(), map.fd(), None)
            .env(FAKE, "1")
            .spawn()
            .unwrap();
        drop(theirs);
        ours.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut host = Host {
            child: Killed(child),
            control: ours,
            map,
            next_id: 1,
        };
        assert_eq!(
            host.ask(Request::Hello {
                version: WIRE_VERSION
            }),
            Reply::Hello {
                version: WIRE_VERSION
            }
        );
        host
    }

    fn ask_with(&mut self, request: Request, blob: &[u8]) -> (Reply, Vec<u8>) {
        let id = self.next_id;
        self.next_id += 1;
        write_frame(&mut self.control, &ToHost { id, request }, blob).unwrap();
        match read_frame::<FromHost>(&mut self.control).unwrap() {
            (FromHost::Reply { id: got, reply }, blob) => {
                assert_eq!(got, id);
                (reply, blob)
            }
            (other, _) => panic!("{other:?}"),
        }
    }

    fn ask(&mut self, request: Request) -> Reply {
        self.ask_with(request, &[]).0
    }

    /// Makes the engine's end of the slot, then loads the fake plugin into it.
    fn load(&mut self, slot: u32, fake: &str) -> (EngineEnd, LoadedPlugin) {
        let engine = EngineEnd::new(self.map.clone(), slot).unwrap();
        let reply = self.ask(Request::Load {
            slot,
            key: PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap(),
            bundle: "/nowhere/fake.lv2".into(),
            sample_rate: 48_000,
            max_frames: 256,
        });
        match reply {
            Reply::Loaded { plugin } => (engine, plugin),
            other => panic!("{other:?}"),
        }
    }
}

/// One answered block: `input` on both channels, `events` at their frames; the left and right
/// output.
fn block(engine: &mut EngineEnd, input: &[f32], events: &[ToPlugin]) -> (Vec<f32>, Vec<f32>) {
    assert!(engine.begin(input.len()));
    engine.write_input(0, input);
    engine.write_input(1, input);
    for event in events {
        assert!(engine.push_event(*event));
    }
    engine.submit();
    assert!(engine.wait(monotonic_ns() + PATIENT_NS));
    let (mut left, mut right) = (vec![9.0; input.len()], vec![9.0; input.len()]);
    engine.read_output(0, &mut left);
    engine.read_output(1, &mut right);
    (left, right)
}

fn note_on(key: u8) -> ToPlugin {
    ToPlugin::Midi {
        frame: 0,
        data: [0x90, key, 100],
    }
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, x| m.max(x.abs()))
}

#[test]
fn a_fake_instrument_sounds_and_takes_parameters_and_state() {
    let mut host = Host::start();
    let (mut engine, plugin) = host.load(0, "sine");
    assert_eq!(
        plugin,
        LoadedPlugin {
            latency: 0,
            audio_inputs: 0,
            audio_outputs: 2,
            editor: false
        }
    );
    let silence = [0.0f32; 256];

    let (left, _) = block(&mut engine, &silence, &[]);
    assert_eq!(peak(&left), 0.0);

    // A4 at the default level of 0.5: 440 Hz, so more than two periods in 256 frames.
    let (left, right) = block(&mut engine, &silence, &[note_on(69)]);
    assert_eq!(left, right);
    assert!((peak(&left) - 0.5).abs() < 0.01, "{}", peak(&left));
    let crossings = left
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    assert!((4..=5).contains(&crossings), "{crossings}");

    // A note that starts in the middle of the block starts there.
    block(
        &mut engine,
        &silence,
        &[ToPlugin::Midi {
            frame: 0,
            data: [0x80, 69, 0],
        }],
    );
    let (left, _) = block(
        &mut engine,
        &silence,
        &[ToPlugin::Midi {
            frame: 100,
            data: [0x90, 69, 100],
        }],
    );
    assert_eq!(peak(&left[..100]), 0.0);
    assert!(peak(&left[100..]) > 0.4);

    // The level through the slot's ring, from the block it is sent with.
    let (left, _) = block(
        &mut engine,
        &silence,
        &[ToPlugin::Param {
            frame: 0,
            id: 0,
            value: 1.0,
        }],
    );
    assert!((peak(&left) - 1.0).abs() < 0.01);

    // The level over the socket, and read back there.
    assert_eq!(
        host.ask(Request::SetParam {
            slot: 0,
            id: 0,
            value: 0.25
        }),
        Reply::Ok
    );
    let (left, _) = block(&mut engine, &silence, &[]);
    assert!((peak(&left) - 0.25).abs() < 0.01);
    assert_eq!(
        host.ask(Request::GetParam { slot: 0, id: 0 }),
        Reply::Param {
            value: 0.25,
            text: "25 %".into()
        }
    );
    match host.ask(Request::Params { slot: 0 }) {
        Reply::Params { params } => {
            assert_eq!(params.len(), 1);
            assert_eq!(params[0].name, "Level");
        }
        other => panic!("{other:?}"),
    }

    // The state out, changed, and back in: the blob of the frame carries it.
    let (reply, state) = host.ask_with(Request::GetState { slot: 0 }, &[]);
    assert_eq!((reply, state.len()), (Reply::State, 8));
    assert_eq!(
        host.ask(Request::SetParam {
            slot: 0,
            id: 0,
            value: 0.75
        }),
        Reply::Ok
    );
    assert_eq!(
        host.ask_with(Request::SetState { slot: 0 }, &state).0,
        Reply::Ok
    );
    let (left, _) = block(&mut engine, &silence, &[]);
    assert!((peak(&left) - 0.25).abs() < 0.01);
    assert!(matches!(
        host.ask_with(Request::SetState { slot: 0 }, &[1, 2, 3]).0,
        Reply::Error { .. }
    ));

    // A reset silences it.
    assert!(engine.begin(256));
    engine.set_flags(meloet_plugin::ipc::block_flags::RESET);
    engine.submit();
    assert!(engine.wait(monotonic_ns() + PATIENT_NS));
    let mut out = [1.0f32; 256];
    engine.read_output(0, &mut out);
    assert_eq!(peak(&out), 0.0);

    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert!(matches!(
        host.ask(Request::GetParam { slot: 0, id: 0 }),
        Reply::Error { .. }
    ));
    assert_eq!(host.ask(Request::Quit), Reply::Ok);
    assert_eq!(
        host.child
            .wait_for_exit(Duration::from_secs(5))
            .unwrap()
            .code(),
        Some(0)
    );
}

#[test]
fn two_slots_run_side_by_side_and_a_slot_is_used_again() {
    let mut host = Host::start();
    let (mut sine, _) = host.load(0, "sine");
    let (mut pass, plugin) = host.load(3, "pass");
    assert_eq!(plugin.audio_inputs, 2);
    block(&mut sine, &[0.0; 64], &[note_on(60)]);
    for n in 0..300u32 {
        let frames = 1 + (n as usize * 41) % 256;
        let input: Vec<f32> = (0..frames)
            .map(|i| (n as usize + i) as f32 * 0.001)
            .collect();
        let (left, right) = block(&mut pass, &input, &[]);
        assert_eq!(left, input, "block {n}");
        assert_eq!(right, input, "block {n}");
        let (left, _) = block(&mut sine, &input, &[]);
        assert!(peak(&left) <= 0.5001);
    }
    assert!(!sine.stopped() && !pass.stopped());

    // The slot is taken while its plugin is there, and free again after.
    let key = PluginKey::lv2("urn:meloet:fake:pass").unwrap();
    let again = Request::Load {
        slot: 3,
        key,
        bundle: "/x".into(),
        sample_rate: 48_000,
        max_frames: 256,
    };
    assert!(matches!(host.ask(again), Reply::Error { .. }));
    assert_eq!(host.ask(Request::Unload { slot: 3 }), Reply::Ok);
    drop(pass);
    let (mut pass, _) = host.load(3, "pass");
    let (left, _) = block(&mut pass, &[0.5; 16], &[]);
    assert_eq!(left, [0.5; 16]);
}

#[test]
fn requests_that_cannot_be_done_are_answered_with_an_error() {
    let mut host = Host::start();
    let load = |slot, key: &str, max_frames| Request::Load {
        slot,
        key: key.parse().unwrap(),
        bundle: "/x".into(),
        sample_rate: 48_000,
        max_frames,
    };
    for request in [
        load(0, "lv2:urn:meloet:fake:nothing", 256),
        load(99, "lv2:urn:meloet:fake:pass", 256),
        load(0, "lv2:urn:meloet:fake:pass", 257),
        load(0, "lv2:urn:example:not-built-yet", 256),
        load(0, "vst3:647370206B31732020202020756E7363", 256),
        Request::Unload { slot: 1 },
        Request::Params { slot: 1 },
        Request::OpenEditor { slot: 0 },
    ] {
        assert!(
            matches!(host.ask(request.clone()), Reply::Error { .. }),
            "{request:?}"
        );
    }
    // Still there and well.
    let (mut pass, _) = host.load(0, "pass");
    assert_eq!(block(&mut pass, &[1.0; 8], &[]).0, [1.0; 8]);
}

#[test]
fn nothing_is_done_before_hello_and_a_closed_socket_ends_the_process() {
    let map = Mapping::create(GEOMETRY).unwrap();
    let (mut ours, theirs) = UnixStream::pair().unwrap();
    let child = launch::host_command(Path::new(EXE), theirs.as_fd(), map.fd(), None)
        .env(FAKE, "1")
        .spawn()
        .unwrap();
    let mut child = Killed(child);
    drop(theirs);
    ours.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write_frame(
        &mut ours,
        &ToHost {
            id: 5,
            request: Request::Unload { slot: 0 },
        },
        &[],
    )
    .unwrap();
    let (answer, _) = read_frame::<FromHost>(&mut ours).unwrap();
    assert!(matches!(
        answer,
        FromHost::Reply {
            id: 5,
            reply: Reply::Error { .. }
        }
    ));
    drop(ours);
    assert_eq!(
        child.wait_for_exit(Duration::from_secs(5)).unwrap().code(),
        Some(0)
    );
}

/// Runs blocks on the engine's clock until the slot is stopped; how many blocks after the
/// one the plugin failed in, and how long that took.
fn blocks_until_stopped(engine: &mut EngineEnd) -> (u32, Duration) {
    let failed_at = Instant::now();
    let mut next = failed_at;
    let mut silent = 0;
    loop {
        if !engine.begin(256) {
            assert!(engine.stopped());
            return (silent, failed_at.elapsed());
        }
        engine.write_input(0, &[0.5; 256]);
        if silent == 0 {
            assert!(engine.push_event(note_on(60)));
        }
        engine.submit();
        assert!(
            !engine.wait(monotonic_ns() + BUDGET_NS),
            "a failed plugin answered"
        );
        silent += 1;
        assert!(silent < 5);
        next += BLOCK;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
}

#[test]
fn a_plugin_that_crashes_is_seen_within_two_blocks() {
    let mut host = Host::start();
    let (mut engine, _) = host.load(1, "segv");
    for _ in 0..50 {
        assert_eq!(block(&mut engine, &[0.5; 256], &[]).0, [0.5; 256]);
    }
    let (silent, took) = blocks_until_stopped(&mut engine);
    // The block it crashed in went without output; at the next one the slot is stopped.
    assert_eq!(silent, 1);
    assert!(took < BLOCK * 2, "{took:?}");
    let status = host
        .child
        .wait_for_exit(Duration::from_secs(10))
        .expect("the process is gone");
    assert_eq!(status.signal(), Some(libc::SIGSEGV));
    // The socket says so too, later than the audio thread knew.
    assert!(matches!(
        read_frame::<FromHost>(&mut host.control),
        Err(WireError::Closed)
    ));
}

#[test]
fn a_plugin_that_hangs_is_seen_within_two_blocks() {
    let mut host = Host::start();
    let (mut engine, _) = host.load(1, "hang");
    let (mut other, _) = host.load(2, "pass");
    for _ in 0..50 {
        assert_eq!(block(&mut engine, &[0.5; 256], &[]).0, [0.5; 256]);
    }
    let (silent, took) = blocks_until_stopped(&mut engine);
    assert_eq!(silent, 1);
    assert!(took < BLOCK * 2, "{took:?}");
    // The process is still there, and its audio thread is stuck: the other plugins of the
    // same process stop with it (docs/architecture.md 20-1).
    assert!(
        host.child
            .wait_for_exit(Duration::from_millis(50))
            .is_none()
    );
    assert!(other.begin(256));
    other.submit();
    assert!(!other.wait(monotonic_ns() + BUDGET_NS));
    assert!(!other.begin(256));
    assert!(other.stopped());
}

struct Scan {
    child: Killed,
    out: UnixStream,
}

fn scan(format: PluginFormat, bundle: &str, skip: &[&str], fake: bool) -> Scan {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let skip: Vec<String> = skip.iter().map(|s| s.to_string()).collect();
    let mut command = launch::scan_command(
        Path::new(EXE),
        format,
        Path::new(bundle),
        &skip,
        theirs.as_fd(),
        None,
    );
    if fake {
        command.env(FAKE, "1");
    } else {
        command.env_remove(FAKE);
    }
    let child = Killed(command.spawn().unwrap());
    drop(theirs);
    ours.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    Scan { child, out: ours }
}

impl Scan {
    /// Every frame up to `Done` or `Broken`, or up to the error that ended the reading.
    fn frames(&mut self) -> (Vec<ScanOutput>, Option<WireError>) {
        let mut frames = Vec::new();
        loop {
            match read_frame::<ScanOutput>(&mut self.out) {
                Ok((frame, _)) => {
                    let last = matches!(frame, ScanOutput::Done | ScanOutput::Broken { .. });
                    frames.push(frame);
                    if last {
                        return (frames, None);
                    }
                }
                Err(e) => return (frames, Some(e)),
            }
        }
    }

    fn exit_code(&mut self) -> Option<i32> {
        self.child
            .wait_for_exit(Duration::from_secs(5))
            .unwrap()
            .code()
    }
}

/// A frame in short: `probing <id>`, `found <name>`, `pluginBroken <id>`, `broken`, `done`.
fn short(frame: &ScanOutput) -> String {
    match frame {
        ScanOutput::Probing { id, .. } => format!("probing {id}"),
        ScanOutput::Found { plugin } => format!("found {}", plugin.name),
        ScanOutput::PluginBroken { id, .. } => format!("pluginBroken {id}"),
        ScanOutput::Broken { .. } => "broken".into(),
        ScanOutput::Done => "done".into(),
    }
}

fn shorts(frames: &[ScanOutput]) -> Vec<String> {
    frames.iter().map(short).collect()
}

#[test]
fn scan_mode_writes_one_plugin_after_another_and_ends() {
    let mut found = scan(PluginFormat::Lv2, "/nowhere/found.lv2", &[], true);
    let (frames, error) = found.frames();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(
        shorts(&frames),
        [
            "probing urn:meloet:fake:sine",
            "found Fake Sine",
            "probing urn:meloet:fake:pass",
            "found Fake Pass",
            "done"
        ]
    );
    assert_eq!(
        frames[0],
        ScanOutput::Probing {
            id: "urn:meloet:fake:sine".into(),
            name: Some("Fake Sine".into())
        }
    );
    match &frames[1] {
        ScanOutput::Found { plugin } => {
            assert_eq!(plugin.key, PluginKey::lv2("urn:meloet:fake:sine").unwrap());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(found.exit_code(), Some(0));

    let mut empty = scan(PluginFormat::Vst3, "/nowhere/empty.vst3", &[], true);
    let (frames, error) = empty.frames();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(frames, [ScanOutput::Done]);
    assert_eq!(empty.exit_code(), Some(0));

    let mut broken = scan(PluginFormat::Vst3, "/nowhere/broken.vst3", &[], true);
    let (frames, error) = broken.frames();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(shorts(&frames), ["broken"]);
    assert_eq!(broken.exit_code(), Some(0));
}

#[test]
fn a_plugin_that_crashes_stops_only_itself() {
    let mut mixed = scan(PluginFormat::Lv2, "/nowhere/mixed.lv2", &[], true);
    let (frames, error) = mixed.frames();
    assert!(matches!(error, Some(WireError::Closed)), "{error:?}");
    assert_eq!(
        shorts(&frames),
        [
            "probing urn:meloet:fake:a",
            "found Fake A",
            "probing urn:meloet:fake:b"
        ]
    );
    let status = mixed.child.wait_for_exit(Duration::from_secs(10)).unwrap();
    assert_eq!(status.signal(), Some(libc::SIGSEGV));

    // Scanned again without the plugin that stopped it, the bundle reads to its end.
    let mut again = scan(
        PluginFormat::Lv2,
        "/nowhere/mixed.lv2",
        &["urn:meloet:fake:b"],
        true,
    );
    let (frames, error) = again.frames();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(
        shorts(&frames),
        [
            "probing urn:meloet:fake:a",
            "found Fake A",
            "probing urn:meloet:fake:c",
            "found Fake C",
            "done"
        ]
    );
    assert_eq!(again.exit_code(), Some(0));
}

#[test]
fn a_scan_that_crashes_or_hangs_stops_after_probing() {
    let mut crashed = scan(PluginFormat::Lv2, "/nowhere/segv.lv2", &[], true);
    let (frames, error) = crashed.frames();
    assert!(matches!(error, Some(WireError::Closed)), "{error:?}");
    assert_eq!(shorts(&frames), ["probing urn:meloet:fake:segv"]);
    let status = crashed
        .child
        .wait_for_exit(Duration::from_secs(10))
        .unwrap();
    assert_eq!(status.signal(), Some(libc::SIGSEGV));

    let mut hung = scan(PluginFormat::Lv2, "/nowhere/hang.lv2", &[], true);
    assert_eq!(
        short(&read_frame::<ScanOutput>(&mut hung.out).unwrap().0),
        "probing urn:meloet:fake:hang"
    );
    hung.out
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    let (frames, error) = hung.frames();
    assert!(frames.is_empty(), "{frames:?}");
    assert!(matches!(error, Some(WireError::Io(_))), "{error:?}");
    assert!(
        hung.child
            .wait_for_exit(Duration::from_millis(10))
            .is_none()
    );
}

#[test]
fn the_fake_plugins_are_not_there_without_the_variable() {
    // The formats are not built yet: a real bundle reads as broken, and nothing crashes.
    for bundle in ["/nowhere/segv.lv2", "/nowhere/mixed.lv2"] {
        let mut real = scan(PluginFormat::Lv2, bundle, &[], false);
        let (frames, error) = real.frames();
        assert!(error.is_none(), "{error:?}");
        assert_eq!(shorts(&frames), ["broken"]);
        assert_eq!(real.exit_code(), Some(0));
    }
}

#[test]
fn a_bad_command_line_ends_with_an_error_code() {
    for args in [
        &[][..],
        &["host"],
        &["host", "--control-fd", "999", "--memory-fd", "998"],
        &["scan", "clap", "/x"],
        &["dance"],
    ] {
        let status = std::process::Command::new(EXE).args(args).status().unwrap();
        assert_eq!(status.code(), Some(2), "{args:?}");
    }
}
