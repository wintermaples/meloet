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
use meloet_core::plugins::hosts::{
    DEFAULT_GEOMETRY, HostOptions, Hosts, LoadSpec, capture_frames_max, wait,
};
use meloet_engine::{
    Effect, EffectContext, Instrument, InstrumentEvent, PluginBlock, PluginEffect,
    PluginInstrument, monotonic_us,
};
use meloet_model::{OutputSetting, PluginOutputs, PluginPlace, StripId, TrackId};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::HANG_MS;

use plughost::{alive, wait_for};

const RATE: u32 = 48_000;
const FRAMES: usize = 256;
const CTX: EffectContext = EffectContext { tempo_bpm: 120.0 };
const TRACK: PluginPlace = PluginPlace::Instrument(TrackId(1));
/// The fake instrument with four output sets of the plugin host: "a" a stereo pair, "b"
/// mono in the middle, "c" mono on the left, all three on, and "d" mono and off.
const FOUR_OUTPUTS: &str = "outs";

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
        outputs: Default::default(),
    }
}

fn seats(plugins: Vec<ExportPlugin>) -> Result<ExportSeats, ExportPluginError> {
    seats_with(&options(), plugins)
}

fn seats_with(
    options: &HostOptions,
    plugins: Vec<ExportPlugin>,
) -> Result<ExportSeats, ExportPluginError> {
    let want = ExportWant {
        plugins,
        sample_rate: RATE,
        max_frames: FRAMES as u32,
    };
    // A block whose deadline is long past, as an export's engine never sets one.
    export_seats(options, want, Arc::new(PluginBlock::default()))
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
            outputs: Default::default(),
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
#[ignore = "needs the plugin host's fake instrument with four output sets (the other half of \
            the output sets work); FOUR_OUTPUTS names it"]
fn an_export_keeps_the_sound_of_each_output_set_alone() {
    let mut four = plugin(TRACK, FOUR_OUTPUTS, Vec::new());
    let a_off = OutputSetting {
        on: Some(false),
        ..OutputSetting::new("a")
    };
    four.outputs = PluginOutputs::new(1, [a_off]);
    let mut seats = seats(vec![four]).unwrap();
    let ids: Vec<String> = (seats.output_sets(TRACK).unwrap().iter())
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(ids, ["a", "b", "c", "d"]);
    let too_long = capture_frames_max(ids.len()) + 1;
    assert!(seats.capture(TRACK, too_long).is_err());

    let mut instrument = take(&mut seats.instruments, TRACK);
    let blocks = 8;
    seats.capture(TRACK, (blocks * FRAMES) as u64).unwrap();
    let left = render(&mut *instrument, &mut [], blocks);
    let captured = seats.captured(TRACK).unwrap();
    let got: Vec<&str> = captured.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(got, ["a", "b", "c", "d"], "a set that is off is kept too");
    for sound in &captured {
        assert_eq!(sound.samples.len(), blocks * FRAMES * 2, "{}", sound.id);
        assert!(
            sound.samples.iter().any(|s| s.abs() > 1e-3),
            "{} is silent",
            sound.id
        );
    }
    // What is heard is the sets the settings and the rule leave on (b and c), as kept.
    let (b, c) = (&captured[1].samples, &captured[2].samples);
    for (i, heard) in left.iter().enumerate() {
        let kept = b[i * 2] + c[i * 2];
        assert!((heard - kept).abs() < 1e-4, "frame {i}: {heard} and {kept}");
    }
    drop(instrument);
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

const SURGE_VST3: (&str, &str) = (
    "vst3:ABCDEF019182FAEB566D624153675854",
    "/usr/lib/vst3/Surge XT.vst3",
);
const SURGE_LV2: (&str, &str) = (
    "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
    "/usr/lib/lv2/Surge XT.lv2",
);
const DRAGONFLY_LV2: (&str, &str) = (
    "lv2:https://github.com/michaelwillis/dragonfly-reverb",
    "/usr/lib/lv2/DragonflyHallReverb.lv2",
);

fn real(place: PluginPlace, plugin: (&str, &str), state: Vec<u8>) -> ExportPlugin {
    ExportPlugin {
        place,
        name: plugin.0.into(),
        key: plugin.0.parse().unwrap(),
        bundle: plugin.1.into(),
        state,
        outputs: Default::default(),
    }
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// Surge XT plays on the clock while an export runs two more of it and a reverb as fast
/// as they answer: the playing one misses no block, the export has the playing one's state,
/// and its processes are gone after it.
#[test]
#[ignore = "needs Surge XT (VST3 and LV2) and Dragonfly Reverb (LV2)"]
fn real_plugins_export_fast_while_the_playing_one_sounds() {
    if [SURGE_VST3, SURGE_LV2, DRAGONFLY_LV2]
        .iter()
        .any(|p| !std::path::Path::new(p.1).exists())
    {
        eprintln!("the plugins are not installed; nothing tested");
        return;
    }
    let options = HostOptions {
        env: Vec::new(),
        ..options()
    };
    let playing = Hosts::start(options.clone(), |_| {});
    let block = Arc::new(PluginBlock::default());
    let spec = LoadSpec {
        key: SURGE_VST3.0.parse().unwrap(),
        bundle: SURGE_VST3.1.into(),
        sample_rate: RATE,
        outputs: Default::default(),
    };
    let surge = wait(|done| playing.load(spec, block.clone(), done)).unwrap();
    // A state of its own: much quieter than a Surge XT just made.
    let params = wait(|done| playing.params(surge.id, done)).unwrap();
    let volume = params.iter().find(|p| p.name == "Global Volume").unwrap();
    wait(|done| playing.set_param(surge.id, volume.id, 0.3, done)).unwrap();
    let state = wait(|done| playing.state(surge.id, done)).unwrap();
    let playing_pids: Vec<u32> = playing.processes().iter().map(|p| p.pid).collect();
    let watch = surge.watch.clone();

    // The playing side: blocks of 1024 frames on the clock, each with the engine's deadline.
    let quit = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let player = {
        let (quit, block) = (quit.clone(), block.clone());
        let mut surge = PluginInstrument(surge.slot);
        std::thread::spawn(move || {
            let step = Duration::from_secs_f64(1024.0 / f64::from(RATE));
            let (mut left, mut right) = ([0.0f32; 1024], [0.0f32; 1024]);
            let mut heard = Vec::new();
            surge.event(InstrumentEvent::NoteOn {
                note: 60,
                velocity: 100,
            });
            let mut next = Instant::now();
            while !quit.load(std::sync::atomic::Ordering::Relaxed) {
                let wait_us = (step.as_micros() as f64 * 0.9) as u64;
                block.set_deadline((monotonic_us() + wait_us) * 1000);
                surge.render(&mut left, &mut right);
                heard.extend_from_slice(&left);
                next += step;
                std::thread::sleep(next.saturating_duration_since(Instant::now()));
            }
            heard
        })
    };
    std::thread::sleep(Duration::from_secs(2));
    let alone = watch.unanswered();

    let run = |seconds: usize| {
        let began = Instant::now();
        let mut seats = seats_with(
            &options,
            vec![
                real(TRACK, SURGE_VST3, state.clone()),
                real(insert(0), DRAGONFLY_LV2, Vec::new()),
                real(PluginPlace::Instrument(TrackId(2)), SURGE_LV2, Vec::new()),
            ],
        )
        .unwrap();
        let loaded = began.elapsed();
        let pids = seats.pids();
        assert_eq!(pids.len(), 3);
        assert!(pids.iter().all(|pid| !playing_pids.contains(pid)));
        let mut first = take(&mut seats.instruments, TRACK);
        let mut second = take(&mut seats.instruments, PluginPlace::Instrument(TrackId(2)));
        let mut reverb = take(&mut seats.effects, insert(0));
        let (mut left, mut right) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);
        let (mut l2, mut r2) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);
        let (mut quiet, mut fresh, mut wet) = (Vec::new(), Vec::new(), Vec::new());
        for instrument in [&mut first, &mut second] {
            instrument.event(InstrumentEvent::NoteOn {
                note: 60,
                velocity: 100,
            });
        }
        let began = Instant::now();
        let blocks = seconds * RATE as usize / FRAMES;
        for _ in 0..blocks {
            first.ask(FRAMES);
            second.ask(FRAMES);
            first.render(&mut left, &mut right);
            second.render(&mut l2, &mut r2);
            quiet.extend_from_slice(&left);
            fresh.extend_from_slice(&l2);
            reverb.process(&mut l2, &mut r2, &CTX);
            wet.extend_from_slice(&l2);
        }
        let took = began.elapsed();
        assert_eq!(seats.stopped(), None);
        for (_, watch) in &seats.watches {
            assert_eq!(watch.unanswered(), 0);
        }
        drop((first, second, reverb));
        drop(seats);
        assert!(gone(&pids), "an export's process is still there");
        println!(
            "export: loaded in {loaded:?}, {seconds} s of sound in {took:?} ({:.1} times as fast as it plays)",
            seconds as f64 / took.as_secs_f64()
        );
        (quiet, fresh, wet)
    };
    let (quiet, fresh, wet) = run(120);
    let (quiet_again, _, _) = run(20);
    std::thread::sleep(Duration::from_secs(1));
    let with_export = watch.unanswered();
    quit.store(true, std::sync::atomic::Ordering::Relaxed);
    let heard = player.join().unwrap();

    println!(
        "playing: {alone} blocks unanswered alone, {with_export} with the exports; waited {} ms in all",
        watch.waited_ns() / 1_000_000
    );
    assert_eq!((alone, with_export), (0, 0));
    assert!(!watch.stopped());
    // Past the first second (the patch is read in the first block, the note swells).
    let (from, to) = (RATE as usize, 3 * RATE as usize);
    let (playing_level, export_level) = (rms(&heard[from..to]), rms(&quiet[from..to]));
    let apart: Vec<f32> = (fresh[from..to].iter().zip(&wet[from..to]))
        .map(|(dry, wet)| wet - dry)
        .collect();
    let apart = rms(&apart);
    println!(
        "playing {playing_level}, export with its state {export_level} (again {}), a Surge XT just made {}, through the reverb {} apart from it",
        rms(&quiet_again[from..to]),
        rms(&fresh[from..to]),
        apart
    );
    assert!(playing_level > 0.001);
    assert!((export_level - playing_level).abs() < 0.1 * playing_level);
    assert!((rms(&quiet_again[from..to]) - export_level).abs() < 0.1 * export_level);
    assert!(rms(&fresh[from..to]) > 2.0 * export_level);
    assert!(
        apart > 0.05 * rms(&fresh[from..to]),
        "the reverb did nothing"
    );

    assert!(playing_pids.iter().all(|&pid| alive(pid)));
    drop(playing);
    assert!(gone(&playing_pids));
}
