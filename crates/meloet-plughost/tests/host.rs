//! The real `meloet-plughost` binary with the fake plugins: blocks through the shared memory,
//! requests over the socket, and what meloet's side sees when a plugin crashes or hangs.
//! Every child is killed when its test ends.

mod common;

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{FAKE, GEOMETRY, Host, Killed, PATIENT_NS, block, exe, note_on, peak};
use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{EngineEnd, HANG_MS, Mapping, ToPlugin};
use meloet_plugin::wire::{
    FromHost, LoadedPlugin, OutputSettings, ParamNow, Reply, Request, ScanOutput, ToHost,
    WireError, read_frame, write_frame,
};
use meloet_plugin::{EditorKind, PluginFormat, PluginKey, launch};

/// One block at 48 kHz, and how much of it the engine would wait.
const BLOCK: Duration = Duration::from_micros(5333);
const BUDGET_NS: u64 = 4_000_000;

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
            editor: EditorKind::None,
            live: false,
            outputs: Vec::new(),
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
        Reply::Param {
            now: ParamNow {
                id: 0,
                value: Some(0.25),
                text: Some("25 %".into())
            }
        }
    );
    let (left, _) = block(&mut engine, &silence, &[]);
    assert!((peak(&left) - 0.25).abs() < 0.01);
    assert_eq!(
        host.ask(Request::GetParam { slot: 0, id: 0 }),
        Reply::Param {
            now: ParamNow {
                id: 0,
                value: Some(0.25),
                text: Some("25 %".into())
            }
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
        Reply::Param {
            now: ParamNow {
                id: 0,
                value: Some(0.75),
                text: Some("75 %".into())
            }
        }
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
        outputs: OutputSettings::default(),
    };
    assert!(matches!(host.ask(again), Reply::Error { .. }));
    assert_eq!(host.ask(Request::Unload { slot: 3 }), Reply::Ok);
    drop(pass);
    let (mut pass, _) = host.load(3, "pass");
    let (left, _) = block(&mut pass, &[0.5; 16], &[]);
    assert_eq!(left, [0.5; 16]);
}

#[test]
fn a_plugin_says_what_window_it_has_and_lists_its_parameters_with_values_and_texts() {
    use meloet_plugin::wire::{ParamShape, TextValue};

    let mut host = Host::start();
    let editor = |host: &mut Host, slot, fake| host.load(slot, fake).1.editor;
    assert_eq!(editor(&mut host, 0, "window"), EditorKind::X11);
    assert_eq!(editor(&mut host, 1, "notyet"), EditorKind::Unsupported);
    assert_eq!(editor(&mut host, 2, "sine"), EditorKind::None);

    let Reply::Params { params } = host.ask(Request::Params { slot: 0 }) else {
        panic!("no list");
    };
    let shown: Vec<_> = (params.iter())
        .map(|p| (p.id, p.name.as_str(), p.group.as_deref(), p.shape))
        .collect();
    assert_eq!(
        shown,
        [
            (0, "Level", None, ParamShape::Continuous),
            (1, "Mode", Some("Shape"), ParamShape::Stepped { steps: 3 }),
            (2, "Loud", Some("Shape"), ParamShape::Toggle),
        ]
    );
    assert_eq!(params[1].labels, ["Soft", "Hard", "Odd"]);
    assert!(params[0].labels.is_empty() && params[2].labels.is_empty());
    let now: Vec<_> = (params.iter())
        .map(|p| (p.value, p.text.as_deref()))
        .collect();
    assert_eq!(
        now,
        [
            (Some(0.5), Some("50 %")),
            (Some(0.0), Some("Soft")),
            (Some(0.0), Some("Off"))
        ]
    );

    // A stepped parameter lands on a step, and the answer says where.
    let set = host.ask(Request::SetParam {
        slot: 0,
        id: 1,
        value: 0.6,
    });
    let landed = ParamNow {
        id: 1,
        value: Some(0.5),
        text: Some("Hard".into()),
    };
    assert_eq!(set, Reply::Param { now: landed });

    let texts = host.ask(Request::Texts {
        slot: 0,
        id: 0,
        values: vec![0.0, 1.0],
    });
    let ends = vec![Some("0 %".to_string()), Some("100 %".to_string())];
    assert_eq!(texts, Reply::Texts { texts: ends });

    let read = |host: &mut Host, id, text: &str| match host.ask(Request::ReadText {
        slot: 0,
        id,
        text: text.into(),
    }) {
        Reply::Read { read } => read,
        other => panic!("{other:?}"),
    };
    let mut read = |id, text: &str| read(&mut host, id, text);
    let value = |value, clamped, text: &str| {
        Some(TextValue {
            value,
            clamped,
            text: Some(text.into()),
            low: Some("0 %".into()),
            high: Some("100 %".into()),
        })
    };
    assert_eq!(read(0, "25 %"), value(0.25, false, "25 %"));
    assert_eq!(read(0, " 100"), value(1.0, false, "100 %"));
    assert_eq!(read(0, "250 %"), value(1.0, true, "100 %"));
    assert_eq!(read(0, "-3"), value(0.0, true, "0 %"));
    // The wide digits and signs of a Japanese keyboard.
    assert_eq!(read(0, "３０"), value(0.3, false, "30 %"));
    assert_eq!(read(0, "１２．０ %"), value(0.12, false, "12 %"));
    assert_eq!(read(0, "−３"), value(0.0, true, "0 %"));
    assert_eq!(read(0, "－３"), value(0.0, true, "0 %"));
    assert_eq!(read(0, "loud"), None);
    assert_eq!(
        read(1, "odd").map(|read| (read.value, read.text)),
        Some((1.0, Some("Odd".into())))
    );
    assert_eq!(read(2, "On").map(|read| read.value), Some(1.0));
    // A parameter the plugin does not have.
    assert_eq!(read(7, "1"), None);
    // Nothing was set by reading.
    assert_eq!(host.param(0, 0), (0.5, "50 %".into()));
    assert!(matches!(
        host.ask(Request::GetParam { slot: 0, id: 7 }),
        Reply::Error { .. }
    ));
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
        outputs: OutputSettings::default(),
    };
    for request in [
        load(0, "lv2:urn:meloet:fake:nothing", 256),
        load(99, "lv2:urn:meloet:fake:pass", 256),
        load(0, "lv2:urn:meloet:fake:pass", 257),
        load(0, "lv2:urn:example:not-built-yet", 256),
        load(0, "vst3:647370206B31732020202020756E7363", 256),
        Request::Unload { slot: 1 },
        Request::Params { slot: 1 },
        Request::OpenEditor {
            slot: 0,
            title: "x".into(),
        },
        Request::CloseEditor { slot: 0 },
        Request::Texts {
            slot: 0,
            id: 0,
            values: vec![0.0],
        },
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
    let child = launch::host_command(Path::new(exe()), theirs.as_fd(), map.fd(), None, false)
        .unwrap()
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

/// The block that brings the plugin its first note, which it fails in: no answer.
fn failing_block(engine: &mut EngineEnd) {
    assert!(engine.begin(256));
    engine.write_input(0, &[0.5; 256]);
    assert!(engine.push_event(note_on(60)));
    engine.submit();
    assert!(
        !engine.wait(monotonic_ns() + BUDGET_NS),
        "a failed plugin answered"
    );
}

#[test]
fn a_plugin_that_crashes_takes_its_process_with_it() {
    let mut host = Host::start();
    let (mut engine, _) = host.load(1, "segv");
    for _ in 0..50 {
        assert_eq!(block(&mut engine, &[0.5; 256], &[]).0, [0.5; 256]);
    }
    failing_block(&mut engine);
    // The process is gone at once; that is how meloet knows (the pidfd), not by the time
    // the request has been held.
    let status = host
        .child
        .wait_for_exit(Duration::from_secs(10))
        .expect("the process is gone");
    assert_eq!(status.signal(), Some(libc::SIGSEGV));
    assert!(!engine.begin(256) && engine.overdue());
    host.map.mark_stopped();
    assert!(engine.stopped() && !engine.overdue());
    // The socket says so too.
    assert!(matches!(
        read_frame::<FromHost>(&mut host.control),
        Err(WireError::Closed)
    ));
}

#[test]
fn a_plugin_that_hangs_is_stopped_after_seconds_and_not_slept_for() {
    let mut host = Host::start();
    let (mut engine, _) = host.load(1, "hang");
    let (mut other, _) = host.load(2, "pass");
    for _ in 0..50 {
        assert_eq!(block(&mut engine, &[0.5; 256], &[]).0, [0.5; 256]);
    }
    failing_block(&mut engine);
    let failed_at = Instant::now();
    let hang = Duration::from_millis(u64::from(HANG_MS));
    // A plugin that takes long for one block (a patch being read) looks the same for a
    // while: the blocks go on without it, and without sleeping for it.
    let mut longest = Duration::ZERO;
    loop {
        std::thread::sleep(BLOCK);
        assert!(!engine.begin(256));
        let asked = Instant::now();
        assert!(!engine.await_overdue(monotonic_ns() + BUDGET_NS));
        if failed_at.elapsed() > BLOCK * 4 {
            longest = longest.max(asked.elapsed());
        }
        if engine.stopped() {
            break;
        }
        assert_eq!(block(&mut other, &[0.25; 256], &[]).0, [0.25; 256]);
        assert!(failed_at.elapsed() < hang * 2, "never stopped");
    }
    // The time is counted from when the plugin took the block, one block before.
    let took = failed_at.elapsed() + BLOCK * 2;
    assert!(took >= hang, "{took:?}");
    assert!(
        longest < BLOCK / 2,
        "slept {longest:?} for a plugin at work"
    );
    // The process is still there, with an audio thread stuck in the plugin: the other plugins
    // of the same process are stopped with it, without a wait (docs/architecture.md 20-1).
    assert!(
        host.child
            .wait_for_exit(Duration::from_millis(50))
            .is_none()
    );
    assert!(!other.begin(256));
    assert!(other.stopped() && !other.overdue());
}

struct Scan {
    child: Killed,
    out: UnixStream,
}

fn scan(format: PluginFormat, bundle: &str, skip: &[&str], fake: bool) -> Scan {
    let (ours, theirs) = UnixStream::pair().unwrap();
    let skip: Vec<String> = skip.iter().map(|s| s.to_string()).collect();
    let mut command = launch::scan_command(
        Path::new(exe()),
        format,
        Path::new(bundle),
        &skip,
        theirs.as_fd(),
        None,
    )
    .unwrap();
    if fake {
        command.env(FAKE, "1");
    } else {
        command.env_remove(FAKE);
    }
    let child = Killed(command.spawn().unwrap());
    drop((command, theirs));
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
    // Without the variable these are bundles like any other, and there is nothing at their
    // paths: they read as broken, and nothing crashes.
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
    for args in [&[][..], &["host"], &["scan", "clap", "/x"], &["dance"]] {
        let status = std::process::Command::new(exe())
            .args(args)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(2), "{args:?}");
    }
}

#[test]
fn the_log_says_which_priority_the_audio_thread_got() {
    // The default (whatever this PC allows), rtkit asked on purpose, and nothing asked for:
    // each ends in one line of the log, and the host answers either way.
    for choice in ["", "rtkit", "off"] {
        let log = std::env::temp_dir().join(format!(
            "meloet-plughost-test-{}-{choice}.log",
            std::process::id()
        ));
        let file = std::fs::File::create(&log).unwrap();
        let map = Mapping::create(GEOMETRY).unwrap();
        let (mut ours, theirs) = UnixStream::pair().unwrap();
        let child = launch::host_command(
            Path::new(exe()),
            theirs.as_fd(),
            map.fd(),
            Some(file.as_fd()),
            false,
        )
        .unwrap()
        .env(FAKE, "1")
        .env("MELOET_PLUGHOST_RT", choice)
        .spawn()
        .unwrap();
        let mut child = Killed(child);
        drop(theirs);
        ours.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let quit = ToHost {
            id: 1,
            request: Request::Quit,
        };
        write_frame(&mut ours, &quit, &[]).unwrap();
        read_frame::<FromHost>(&mut ours).unwrap();
        assert_eq!(
            child.wait_for_exit(Duration::from_secs(5)).unwrap().code(),
            Some(0)
        );
        let text = std::fs::read_to_string(&log).unwrap();
        let _ = std::fs::remove_file(&log);
        println!("{choice:?}: {}", text.trim());
        let got = text.contains("audio thread at real-time priority");
        let not = text.contains("no real-time priority for the audio thread");
        assert!(got != not, "{choice:?}: {text}");
        if choice == "off" {
            assert!(not, "{text}");
        }
    }
}

#[test]
fn a_host_for_an_export_says_so_and_makes_its_plugins_ready_as_offline() {
    // `Host::start_as` reads the `offline` of the host's hello.
    let mut playing = Host::start();
    let mut export = Host::start_as(GEOMETRY, &[(FAKE, "1")], true);
    for (host, offline) in [(&mut playing, false), (&mut export, true)] {
        let (mut mode, plugin) = host.load(0, "mode");
        assert!(!plugin.live);
        let (left, right) = block(&mut mode, &[0.5; 64], &[]);
        let all = if offline { 1.0 } else { 0.0 };
        assert!(left.iter().chain(&right).all(|x| *x == all), "{offline}");
        // A plugin that cannot run faster than it plays says so either way.
        assert!(host.load(1, "live").1.live);
        // A block many times longer than it lasts is waited for.
        let (mut slow, _) = host.load(2, "slow");
        assert!(slow.begin(64));
        slow.write_input(0, &[0.25; 64]);
        slow.submit();
        assert!(slow.wait_through());
        let mut out = [0.0f32; 64];
        slow.read_output(0, &mut out);
        assert_eq!(out, [0.25; 64]);
    }
}

#[test]
fn a_host_for_an_export_asks_for_no_priority_and_gives_way() {
    let log = std::env::temp_dir().join(format!(
        "meloet-plughost-test-{}-offline.log",
        std::process::id()
    ));
    let file = std::fs::File::create(&log).unwrap();
    let map = Mapping::create(GEOMETRY).unwrap();
    let (mut ours, theirs) = UnixStream::pair().unwrap();
    let child = launch::host_command(
        Path::new(exe()),
        theirs.as_fd(),
        map.fd(),
        Some(file.as_fd()),
        true,
    )
    .unwrap()
    .env(FAKE, "1")
    .spawn()
    .unwrap();
    let mut child = Killed(child);
    drop(theirs);
    ours.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // Long enough for the audio thread to have asked, if it did.
    std::thread::sleep(Duration::from_millis(300));
    let threads: Vec<(String, String)> = std::fs::read_dir(format!("/proc/{}/task", child.0.id()))
        .unwrap()
        .flatten()
        .filter_map(|task| std::fs::read_to_string(task.path().join("stat")).ok())
        .filter_map(|stat| {
            // After the name's bracket: the nice value is the 19th field of the line,
            // the 17th from here; the scheduling policy the 41st, the 39th from here.
            let rest: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
            Some((rest.get(38)?.to_string(), rest.get(16)?.to_string()))
        })
        .collect();
    // Every thread is an ordinary one at nice 19.
    assert!(
        threads.len() >= 2
            && threads
                .iter()
                .all(|(policy, nice)| policy == "0" && nice == "19"),
        "{threads:?}"
    );
    let quit = ToHost {
        id: 1,
        request: Request::Quit,
    };
    write_frame(&mut ours, &quit, &[]).unwrap();
    read_frame::<FromHost>(&mut ours).unwrap();
    assert_eq!(
        child.wait_for_exit(Duration::from_secs(5)).unwrap().code(),
        Some(0)
    );
    let text = std::fs::read_to_string(&log).unwrap();
    let _ = std::fs::remove_file(&log);
    assert!(!text.contains("priority"), "{text}");
}
