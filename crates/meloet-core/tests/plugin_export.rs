//! The plugins of an export (docs/architecture.md 22-11) with the real `meloet-plughost` and
//! its fake plugins: processes of their own that run offline, seats that wait for every
//! answer, and what happens when no answer can come. Every process is gone when its test
//! ends.

mod plughost;

use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_core::plugins::export::{
    ExportPlugin, ExportPluginError, ExportSeats, ExportWant, export_seats,
};
use meloet_core::plugins::hosts::{DEFAULT_GEOMETRY, HostOptions, Hosts, LoadSpec, wait};
use meloet_engine::{
    Effect, EffectContext, Instrument, InstrumentEvent, PluginBlock, PluginEffect,
    PluginInstrument, monotonic_us,
};
use meloet_model::{PluginPlace, StripId, TrackId};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::HANG_MS;

use plughost::{alive, wait_for};

const RATE: u32 = 48_000;
const FRAMES: usize = 256;
const CTX: EffectContext = EffectContext { tempo_bpm: 120.0 };
const TRACK: PluginPlace = PluginPlace::Instrument(TrackId(1));

fn insert(slot: usize) -> PluginPlace {
    PluginPlace::Insert(StripId::Track(TrackId(1)), slot)
}

fn options() -> HostOptions {
    HostOptions {
        exe: plughost::exe(),
        env: plughost::fake_env(),
        geometry: DEFAULT_GEOMETRY,
    }
}

fn key(fake: &str) -> PluginKey {
    PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap()
}

fn plugin(place: PluginPlace, fake: &str, state: Vec<u8>) -> ExportPlugin {
    ExportPlugin {
        place,
        name: fake.into(),
        key: key(fake),
        bundle: "/nowhere/fake.lv2".into(),
        state,
    }
}

fn seats(plugins: Vec<ExportPlugin>) -> Result<ExportSeats, ExportPluginError> {
    let want = ExportWant {
        plugins,
        sample_rate: RATE,
        max_frames: FRAMES as u32,
    };
    // A block whose deadline is long past, as an export's engine never sets one.
    export_seats(&options(), want, Arc::new(PluginBlock::default()))
}

fn gone(pids: &[u32]) -> bool {
    wait_for(Duration::from_secs(5), || {
        pids.iter().all(|&pid| !alive(pid)).then_some(())
    })
    .is_some()
}

/// The scheduling policy of every thread of a process (0 is the ordinary one).
fn policies(pid: u32) -> Vec<u32> {
    let tasks = std::fs::read_dir(format!("/proc/{pid}/task")).unwrap();
    tasks
        .flatten()
        .filter_map(|task| {
            let stat = std::fs::read_to_string(task.path().join("stat")).ok()?;
            // After the name, which may hold spaces: the 41st field of the line is the
            // 39th after the closing bracket.
            let rest = stat.rsplit_once(')')?.1;
            rest.split_whitespace().nth(38)?.parse().ok()
        })
        .collect()
}

fn offline_in_command_line(pid: u32) -> bool {
    let line = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap();
    line.split(|b| *b == 0).any(|arg| arg == b"--offline")
}

fn take<T>(list: &mut Vec<(PluginPlace, T)>, place: PluginPlace) -> T {
    let at = list.iter().position(|(p, _)| *p == place).unwrap();
    list.remove(at).1
}

/// `blocks` blocks of a key through the instrument and then through the effects, in order.
fn render(
    instrument: &mut dyn Instrument,
    effects: &mut [Box<dyn Effect>],
    blocks: usize,
) -> Vec<f32> {
    let mut out = Vec::new();
    let (mut left, mut right) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);
    instrument.event(InstrumentEvent::NoteOn {
        note: 69,
        velocity: 100,
    });
    for _ in 0..blocks {
        instrument.ask(FRAMES);
        instrument.render(&mut left, &mut right);
        for effect in effects.iter_mut() {
            effect.process(&mut left, &mut right, &CTX);
        }
        out.extend_from_slice(&left);
    }
    out
}

#[test]
fn an_export_has_offline_processes_of_its_own_and_they_end_with_the_seats() {
    // The playing side: a sine at a level of its own, and what it says its state is.
    let playing = Hosts::start(options(), |_| {});
    let block = Arc::new(PluginBlock::default());
    let load = |fake: &str| {
        let spec = LoadSpec {
            key: key(fake),
            bundle: "/nowhere/fake.lv2".into(),
            sample_rate: RATE,
        };
        wait(|done| playing.load(spec, block.clone(), done)).unwrap()
    };
    let sine = load("sine");
    wait(|done| playing.set_param(sine.id, 0, 0.25, done)).unwrap();
    let state = wait(|done| playing.state(sine.id, done)).unwrap();
    let mode = load("mode");
    let playing_pids: Vec<u32> = playing.processes().iter().map(|p| p.pid).collect();
    assert_eq!(playing_pids.len(), 2);

    let mut seats = seats(vec![
        plugin(TRACK, "sine", state),
        plugin(insert(0), "gain", Vec::new()),
        plugin(insert(1), "mode", Vec::new()),
        plugin(insert(2), "gain", Vec::new()),
    ])
    .unwrap();
    assert!(!seats.realtime);
    assert_eq!(seats.stopped(), None);
    assert_eq!(seats.watches.len(), 4);
    // One process for each kind of plugin, none of them a playing one.
    let pids = seats.pids();
    assert_eq!(pids.len(), 3, "{pids:?}");
    for &pid in &pids {
        assert!(!playing_pids.contains(&pid));
        assert!(offline_in_command_line(pid));
        assert!(policies(pid).iter().all(|p| *p == 0), "{:?}", policies(pid));
    }
    for &pid in &playing_pids {
        assert!(!offline_in_command_line(pid));
    }

    // The sine has the playing one's state, and goes through the gain (a half).
    let mut instrument = take(&mut seats.instruments, TRACK);
    let mut gain = vec![take(&mut seats.effects, insert(0))];
    let out = render(&mut *instrument, &mut gain, 8);
    let peak = out.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
    assert!((peak - 0.125).abs() < 0.005, "{peak}");

    // The export's plugins were made ready as offline, the playing ones were not.
    let mut offline = take(&mut seats.effects, insert(1));
    let (mut left, mut right) = ([0.5f32; FRAMES], [0.5f32; FRAMES]);
    offline.process(&mut left, &mut right, &CTX);
    assert!(left.iter().chain(&right).all(|x| *x == 1.0));
    let mut online = PluginEffect(mode.slot);
    block.set_deadline((monotonic_us() + 5_000_000) * 1000);
    online.process(&mut left, &mut right, &CTX);
    assert!(left.iter().chain(&right).all(|x| *x == 0.0));

    // The playing sine sounds as before, through all of this.
    let mut playing_sine = PluginInstrument(sine.slot);
    playing_sine.event(InstrumentEvent::NoteOn {
        note: 69,
        velocity: 100,
    });
    for _ in 0..8 {
        block.set_deadline((monotonic_us() + 5_000_000) * 1000);
        playing_sine.render(&mut left, &mut right);
    }
    let peak = left.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
    assert!((peak - 0.25).abs() < 0.01, "{peak}");
    assert_eq!(sine.watch.unanswered(), 0);

    // What was taken out goes first, then the seats: the processes end, the playing ones
    // stay.
    drop((instrument, gain, offline));
    drop(seats);
    assert!(gone(&pids), "an export's process is still there");
    assert!(playing_pids.iter().all(|&pid| alive(pid)));
    drop((playing_sine, online));
    drop(playing);
    assert!(gone(&playing_pids));
}

#[test]
fn a_slow_plugin_is_waited_for_and_two_runs_give_the_same_sound() {
    let run = || {
        let mut seats = seats(vec![
            plugin(TRACK, "sine", Vec::new()),
            plugin(insert(0), "slow", Vec::new()),
        ])
        .unwrap();
        let pids = seats.pids();
        let mut instrument = take(&mut seats.instruments, TRACK);
        let mut effects = vec![take(&mut seats.effects, insert(0))];
        let began = Instant::now();
        let out = render(&mut *instrument, &mut effects, 12);
        // `slow` takes 30 ms for a block that lasts 5.3 ms.
        assert!(began.elapsed() >= Duration::from_millis(12 * 30));
        assert_eq!(seats.stopped(), None);
        for (_, watch) in &seats.watches {
            assert_eq!(watch.unanswered(), 0);
        }
        drop((instrument, effects, seats));
        assert!(gone(&pids));
        out
    };
    let first = run();
    // Every block has the sine in it: none was passed over.
    for (at, block) in first.chunks(FRAMES).enumerate() {
        let peak = block.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
        assert!((peak - 0.5).abs() < 0.01, "block {at}: {peak}");
    }
    assert!(first == run(), "the second run is another sound");
}

#[test]
fn a_plugin_that_hangs_or_is_gone_stops_the_export() {
    let (mut left, mut right) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);

    // `hang` never comes back from the block that brings its first note.
    let mut seats_ = seats(vec![
        plugin(TRACK, "sine", Vec::new()),
        plugin(insert(0), "hang", Vec::new()),
    ])
    .unwrap();
    let pids = seats_.pids();
    let mut sine = take(&mut seats_.instruments, TRACK);
    sine.render(&mut left, &mut right);
    assert_eq!(seats_.stopped(), None);
    drop(sine);
    drop(seats_);
    assert!(gone(&pids));

    let mut seats_ = seats(vec![plugin(TRACK, "hang", Vec::new())]).unwrap();
    let pids = seats_.pids();
    let mut hang = take(&mut seats_.instruments, TRACK);
    hang.render(&mut left, &mut right);
    assert_eq!(seats_.stopped(), None);
    hang.event(InstrumentEvent::NoteOn {
        note: 60,
        velocity: 100,
    });
    let began = Instant::now();
    hang.render(&mut left, &mut right);
    let took = began.elapsed();
    let limit = Duration::from_millis(u64::from(HANG_MS));
    assert!(
        took >= limit && took < limit + Duration::from_secs(1),
        "{took:?}"
    );
    assert_eq!(seats_.stopped(), Some(TRACK));
    assert!(left.iter().all(|x| *x == 0.0));
    // The process that hung does not stay.
    assert!(gone(&pids));
    drop(hang);
    drop(seats_);

    // A process that ends under the export: told at once, not after `HANG_MS`.
    let mut seats_ = seats(vec![
        plugin(TRACK, "sine", Vec::new()),
        plugin(insert(0), "gain", Vec::new()),
    ])
    .unwrap();
    let pids = seats_.pids();
    let mut sine = take(&mut seats_.instruments, TRACK);
    let mut gain = take(&mut seats_.effects, insert(0));
    sine.render(&mut left, &mut right);
    gain.process(&mut left, &mut right, &CTX);
    let gain_pid = *pids.last().unwrap();
    let killed = std::process::Command::new("kill")
        .args(["-KILL", &gain_pid.to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let began = Instant::now();
    let told = wait_for(Duration::from_secs(3), || {
        sine.render(&mut left, &mut right);
        gain.process(&mut left, &mut right, &CTX);
        seats_.stopped()
    });
    assert_eq!(told, Some(insert(0)));
    assert!(began.elapsed() < Duration::from_secs(1));
    drop((sine, gain));
    drop(seats_);
    assert!(gone(&pids));
}

#[test]
fn a_plugin_that_says_it_is_live_asks_for_an_export_by_the_clock() {
    let seats_ = seats(vec![
        plugin(insert(0), "pass", Vec::new()),
        plugin(insert(1), "live", Vec::new()),
    ])
    .unwrap();
    assert!(seats_.realtime);
    let pids = seats_.pids();
    drop(seats_);
    assert!(gone(&pids));
}

#[test]
fn a_plugin_that_does_not_load_fails_the_export_and_leaves_no_process() {
    let marker = plughost::temp_dir("export-noload");
    let mut options = options();
    // Named in no plugin's way; only so that this test finds its own processes.
    options
        .env
        .push(("MELOET_TEST_MARK".into(), marker.clone().into()));
    let want = ExportWant {
        plugins: vec![
            plugin(TRACK, "sine", Vec::new()),
            plugin(insert(0), "nosuch", Vec::new()),
        ],
        sample_rate: RATE,
        max_frames: FRAMES as u32,
    };
    let error = export_seats(&options, want, Arc::new(PluginBlock::default())).unwrap_err();
    assert_eq!(
        error,
        ExportPluginError::Load {
            place: insert(0),
            name: "nosuch".into()
        }
    );
    let left = wait_for(Duration::from_secs(5), || {
        marked(&marker).is_empty().then_some(())
    });
    assert!(left.is_some(), "{:?}", marked(&marker));
    let _ = std::fs::remove_dir_all(&marker);
}

/// The pids of the processes whose environment names `marker`.
fn marked(marker: &std::path::Path) -> Vec<u32> {
    let marker = marker.to_string_lossy().into_owned();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = (entry.file_name().to_str()).and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(environ) = std::fs::read(entry.path().join("environ")) else {
            continue;
        };
        if String::from_utf8_lossy(&environ).contains(&marker) {
            found.push(pid);
        }
    }
    found
}
