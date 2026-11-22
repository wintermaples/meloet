//! Where a recorded voice lands, on a PipeWire of the test's own with the quantum forced to
//! 64, 128, 256, 512 and 1024 frames in turn (docs/architecture.md 22-6). The sink's monitor
//! is the input, so the vocal track records the metronome: every click must lie in the file
//! where the take says its beat is. At one tempo, in 6/8, on a timeline with tempo points,
//! ramps and meter changes, and over a loop. Before the recordings, 「遅れを測る」 runs at
//! every quantum.
//!
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber`, `pw-link` and
//! `pw-metadata`. `MELOET_VTIMING_QUANTA=64,256` and `MELOET_VTIMING_RUNS=probe,tempo,meter,
//! map,loop` narrow it. `MELOET_VTIMING_DELAY_MS=10` puts a `pw-loopback` with that delay
//! and a second sink between the output and the input, a box PipeWire may not know of: the
//! clicks are then recorded once as they come and once with the measured number in use.
//! `MELOET_VTIMING_SECOND_SINK=1` links the output's monitor straight into a second sink
//! and records that sink's monitor.
//! See `tools/measure/vocal-timing.sh`.
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use meloet_core::{Core, CoreOptions};
use meloet_model::TempoMap;
use meloet_protocol::{AppState, Command, LatencyMeasure, TrackKindView, TransportState};
use record_timing::{map_of, wait_for};

const RATE: f64 = 48_000.0;
const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const DELAY_SINK: &str = "meloet-test-delay-sink";

fn env_list(name: &str, default: &[u32]) -> Vec<u32> {
    std::env::var(name).map_or_else(
        |_| default.to_vec(),
        |v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect(),
    )
}

fn out_dir() -> PathBuf {
    std::env::var_os("MELOET_VTIMING_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/measure/vocal-timing"),
        PathBuf::from,
    )
}

fn pw_tool(pw: &private_pw::PrivatePipeWire, program: &str) -> Process {
    let mut command = Process::new(program);
    command
        .env_remove("PIPEWIRE_REMOTE")
        .env("PIPEWIRE_RUNTIME_DIR", pw.dir())
        .env("XDG_RUNTIME_DIR", pw.dir());
    command
}

fn force_quantum(pw: &private_pw::PrivatePipeWire, quantum: u32) {
    let run = |args: &[&str]| {
        let out = pw_tool(pw, "pw-metadata")
            .args(["-n", "settings", "0"])
            .args(args)
            .output()
            .expect("pw-metadata runs");
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    run(&["clock.force-quantum", &quantum.to_string()]);
    let now = run(&[]);
    assert!(
        now.contains(&format!("key:'clock.force-quantum' value:'{quantum}'")),
        "the private daemon took no quantum: {now}"
    );
}

/// A second sink, and a `pw-loopback` that plays the first sink's monitor into it `delay_s`
/// later. The monitor of the second sink is then the input.
struct DelayBox {
    children: Vec<Child>,
}

impl DelayBox {
    fn start(pw: &private_pw::PrivatePipeWire, delay_s: f64) -> DelayBox {
        add_sink(pw, DELAY_SINK);
        let loopback = pw_tool(pw, "pw-loopback")
            .args(["--delay", &delay_s.to_string()])
            .args(["--capture", private_pw::SINK])
            .args(["--capture-props", "stream.capture.sink=true"])
            .args(["--playback", DELAY_SINK])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("pw-loopback starts");
        std::thread::sleep(Duration::from_millis(800));
        DelayBox {
            children: vec![loopback],
        }
    }
}

impl Drop for DelayBox {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// One tempo, 4/4.
    Tempo(f64),
    /// One tempo in this meter.
    Meter(f64, u8, u8),
    /// Tempo points, two ramps, 3/4 and 6/8.
    Map,
    /// Bars 2 to 4 at 120, round and round.
    Loop,
}

struct Plan {
    label: String,
    shape: Shape,
    /// How long the song rolls before it is stopped, in seconds.
    seconds: f64,
}

/// 4/4 at 96; 104 from bar 5, going in a straight line to 120 at bar 9; 3/4 from bar 9; 6/8
/// from bar 13; 150 from bar 14, going down to 90 at bar 16 (as `record_timing_private`).
fn set_changing_timeline(core: &Core) {
    use Command as C;
    core.send(C::SetTempo { bpm: 96.0 });
    for command in [
        C::AddTempoPoint {
            tick: 16 * Q,
            bpm: 104.0,
        },
        C::AddTempoPoint {
            tick: 32 * Q,
            bpm: 120.0,
        },
        C::SetTempoRamp {
            tick: 16 * Q,
            on: true,
        },
        C::AddMeterChange {
            bar: 8,
            num: 3,
            den: 4,
        },
        C::AddMeterChange {
            bar: 12,
            num: 6,
            den: 8,
        },
        C::AddTempoPoint {
            tick: 47 * Q,
            bpm: 150.0,
        },
        C::AddTempoPoint {
            tick: 53 * Q,
            bpm: 90.0,
        },
        C::SetTempoRamp {
            tick: 47 * Q,
            on: true,
        },
    ] {
        core.send(command);
    }
    let t = core.snapshot().state.timeline;
    assert_eq!(t.tempo_points.len(), 4);
    assert_eq!(t.meter_changes.len(), 2);
}

fn start(config: &Path, state: &Path) -> Core {
    Core::start(CoreOptions {
        config_dir: Some(config.to_owned()),
        measure_dir: None,
        midi: None,
        devices: true,
        audio: true,
        songs_dir: None,
        state_dir: Some(state.to_owned()),
        sounds_dir: None,
        plugins: None,
        sf2_dir: None,
        cache_dir: None,
    })
}

fn write_settings(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let sink = private_pw::SINK;
    std::fs::write(
        dir.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{sink}", "output_name": "{sink}", "scale": 100,
                 "metronome": true, "count_in_bars": 1, "record_bits": 32 }}"#
        ),
    )
    .unwrap();
}

/// A core with its output and input running at `quantum`, a vocal track armed.
fn ready(config: &Path, state: &Path, quantum: u32) -> (Core, f32) {
    let core = start(config, state);
    wait_for(&core, "the output runs", Duration::from_secs(10), |s| {
        s.audio.running && s.audio.actual_frames.is_some()
    });
    assert_eq!(
        core.snapshot().state.audio.actual_frames,
        Some(quantum),
        "the stream runs at the fixed quantum"
    );
    core.send(Command::AddVocalTrack);
    wait_for(&core, "the input opens", Duration::from_secs(10), |s| {
        s.inputs.open
            && (s.song.tracks.iter()).any(|t| {
                matches!(
                    t.kind,
                    TrackKindView::Vocal {
                        input_ready: true,
                        armed: true,
                        ..
                    }
                )
            })
    });
    wait_for(
        &core,
        "the PC's delays are read",
        Duration::from_secs(5),
        |s| s.audio.latency.reported_ms.is_some(),
    );
    // The streams run before WirePlumber has linked them.
    std::thread::sleep(Duration::from_millis(1_200));
    let reported = core.snapshot().state.audio.latency.reported_ms.unwrap();
    (core, reported)
}

fn vocal_clips(state: &AppState) -> usize {
    (state.song.tracks.iter())
        .filter(|t| matches!(t.kind, TrackKindView::Vocal { .. }))
        .map(|t| t.clips.len())
        .sum()
}

/// A 32 bit float wav file as `vocalrec` writes it: the first channel.
fn read_wav(path: &Path) -> Vec<f32> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let channels = usize::from(u16::from_le_bytes([bytes[22], bytes[23]]));
    assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 32);
    (bytes[44..].chunks_exact(4 * channels))
        .map(|f| f32::from_le_bytes([f[0], f[1], f[2], f[3]]))
        .collect()
}

fn kept_song(state: &Path) -> serde_json::Value {
    let file = state.join("recovery.json");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let song = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .map(|whole| whole["song"].clone())
            .filter(|song| song["audioTakes"].as_array().is_some_and(|t| !t.is_empty()));
        if let Some(song) = song {
            return song;
        }
        assert!(Instant::now() < until, "no recovery file with a vocal take");
        std::thread::sleep(Duration::from_millis(50));
    }
}

struct Click {
    lap: usize,
    tick: u64,
    /// Where the click lies in the file less where the take says its tick is, in frames.
    error: f64,
}

struct Outcome {
    label: String,
    quantum: u32,
    reported_ms: f32,
    clicks: Vec<Click>,
    takes: usize,
    file_frames: usize,
    /// Clicks that are loud where a quiet one belongs, or the other way.
    wrong_accents: usize,
}

impl Outcome {
    fn worst(&self) -> f64 {
        (self.clicks.iter()).fold(
            0.0,
            |m, c| if c.error.abs() > m { c.error.abs() } else { m },
        )
    }

    /// [`Outcome::worst`] of the clicks that are not alone: on a busy PC the output may
    /// miss cycles, and the recording leaves out what was sung while the song was not heard
    /// (docs/architecture.md 22-26). A click whose head fell there is found late, and the
    /// clicks on each side of it are where they belong. Those are counted, not measured.
    fn worst_but_lone(&self) -> (f64, usize) {
        let near =
            |c: Option<&Click>, lap: usize| c.is_none_or(|c| c.lap != lap || c.error.abs() <= 1.5);
        let mut lone = 0;
        let mut worst = 0.0f64;
        for (i, c) in self.clicks.iter().enumerate() {
            let alone = c.error.abs() > 1.5
                && c.error.abs() < 1_000.0
                && near(i.checked_sub(1).and_then(|j| self.clicks.get(j)), c.lap)
                && near(self.clicks.get(i + 1), c.lap);
            if alone {
                lone += 1;
            } else {
                worst = worst.max(c.error.abs());
            }
        }
        (worst, lone)
    }

    fn mean(&self) -> f64 {
        self.clicks.iter().map(|c| c.error).sum::<f64>() / self.clicks.len().max(1) as f64
    }

    fn row(&self) -> String {
        format!(
            "| {} | {} | {:.1} | {} | {} | {} | {:+.2} | {:.2} | {:.3} |",
            self.label,
            self.quantum,
            self.reported_ms,
            self.takes,
            self.clicks.len(),
            self.wrong_accents,
            self.mean(),
            self.worst(),
            self.worst() / RATE * 1000.0
        )
    }

    fn json(&self) -> String {
        let clicks: Vec<String> = (self.clicks.iter())
            .map(|c| {
                format!(
                    r#"{{"lap":{},"tick":{},"errorFrames":{:.3}}}"#,
                    c.lap, c.tick, c.error
                )
            })
            .collect();
        format!(
            r#"{{"label":"{}","quantum":{},"reportedMs":{:.1},"takes":{},"fileFrames":{},"wrongAccents":{},"clicks":[{}]}}"#,
            self.label,
            self.quantum,
            self.reported_ms,
            self.takes,
            self.file_frames,
            self.wrong_accents,
            clicks.join(",")
        )
    }
}

const HEAD: &str = "\
| run | quantum | PC says ms | takes | clicks | wrong accents | mean frames | max abs frames | max abs ms |
|---|---|---|---|---|---|---|---|---|";

/// Records the metronome on the vocal track and reads where each click lies.
fn record(root: &Path, quantum: u32, plan: &Plan, x_us: Option<i32>, tag: &str) -> Outcome {
    let name = format!("q{quantum}-{tag}");
    let (config, state) = (
        root.join(format!("config-{name}")),
        root.join(format!("state-{name}")),
    );
    write_settings(&config);
    let (core, reported_ms) = ready(&config, &state, quantum);
    if let Some(us) = x_us {
        // 「手で決める…」 is the whole way round: what the PC says and the rest.
        let ms = f64::from(reported_ms) + f64::from(us) / 1000.0;
        core.send(Command::SettingsSetManualLatency {
            ms: Some(ms as f32),
        });
    }
    match plan.shape {
        Shape::Tempo(bpm) => {
            core.send(Command::SetTempo { bpm });
        }
        Shape::Meter(bpm, num, den) => {
            core.send(Command::SetTempo { bpm });
            core.send(Command::SetMeter { num, den });
        }
        Shape::Map => set_changing_timeline(&core),
        Shape::Loop => {
            core.send(Command::SetTempo { bpm: 120.0 });
            core.send(Command::SetLoop {
                from_bar: 2,
                to_bar: 4,
            });
            assert!(core.snapshot().state.song.loop_on);
        }
    }
    let view = core.snapshot().state;
    let map = map_of(&view.timeline);

    core.send(Command::Record);
    wait_for(&core, "it rolls", Duration::from_secs(20), |s| {
        s.transport.state == TransportState::Recording && !s.transport.recording_vocals.is_empty()
    });
    std::thread::sleep(Duration::from_secs_f64(plan.seconds));
    core.send(Command::PlayStop);
    wait_for(
        &core,
        "the voice is in the song",
        Duration::from_secs(10),
        |s| vocal_clips(s) == 1 && s.transport.recording_vocals.is_empty(),
    );
    let song = kept_song(&state);
    if std::env::var_os("MELOET_VTIMING_DUMP").is_some() {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "tracks": song["song"]["tracks"], "audio": song["song"]["audio"],
                "audioTakes": song["audioTakes"] }))
            .unwrap()
        );
    }
    let outcome = read_clicks(&song, &state, &map, plan, quantum, reported_ms);
    core.stop();
    outcome
}

fn read_clicks(
    song: &serde_json::Value,
    state: &Path,
    map: &TempoMap,
    plan: &Plan,
    quantum: u32,
    reported_ms: f32,
) -> Outcome {
    let file = song["song"]["audio"][0]["file"].as_str().expect("a file");
    let heard = read_wav(&state.join("work/音").join(file));
    let takes = song["audioTakes"].as_array().expect("takes");
    let clip = (song["song"]["tracks"].as_array().expect("tracks").iter())
        .filter(|t| t["kind"] == "vocal")
        .flat_map(|t| t["clips"].as_array().expect("clips"))
        .next()
        .expect("a vocal clip");
    let clip_start = clip["start"].as_i64().unwrap();
    let mut clicks = Vec::new();
    let mut wrong_accents = 0;
    for (lap, in_clip) in clip["takes"].as_array().unwrap().iter().enumerate() {
        let take = (takes.iter())
            .find(|t| t["id"] == in_clip["take"])
            .expect("the clip's take");
        let first_frame = take["firstFrame"].as_u64().unwrap() as f64;
        let frames = take["frames"].as_u64().unwrap() as f64;
        // The take's tick 0 in the song.
        let origin = (clip_start + in_clip["offset"].as_i64().unwrap()) as f64;
        let origin_s = map.seconds_at(origin);
        let mut click = map.next_click(origin);
        loop {
            let tick = click.tick;
            let expected = first_frame + (map.seconds_at(tick as f64) - origin_s) * RATE;
            // The whole click (35 ms) must be inside the take.
            if expected + 2_000.0 > first_frame + frames {
                break;
            }
            let from = (expected - 2_000.0).max(0.0) as usize;
            let to = ((expected + 2_000.0) as usize).min(heard.len());
            let onset = (heard[from..to].iter())
                .position(|s| s.abs() > 0.02)
                .map(|i| from + i);
            // The head of a bar is the louder click (0.17 against 0.13): a recording that
            // lies whole beats off shows here.
            if onset.is_some_and(|at| (heard[at].abs() > 0.15) != click.downbeat) {
                wrong_accents += 1;
            }
            clicks.push(Click {
                lap,
                tick: tick as u64,
                error: onset.map_or(f64::NAN, |at| at as f64 - expected),
            });
            click = map.next_click(tick as f64 + 1.0);
        }
    }
    Outcome {
        label: plan.label.clone(),
        quantum,
        reported_ms,
        clicks,
        takes: takes.len(),
        file_frames: heard.len(),
        wrong_accents,
    }
}

/// 「遅れを測る」 once: (what the PC says, the measured number as the dialog shows it, and
/// what is kept in the settings: the way round less what the PC says, in microseconds).
fn measure(root: &Path, quantum: u32, tag: &str) -> (f32, Option<f32>, Option<i64>) {
    let name = format!("q{quantum}-{tag}");
    let (config, state) = (
        root.join(format!("config-{name}")),
        root.join(format!("state-{name}")),
    );
    write_settings(&config);
    let (core, reported) = ready(&config, &state, quantum);
    core.send(Command::OpenLatencyMeasure);
    core.send(Command::StartLatencyMeasure);
    wait_for(
        &core,
        "the clicks come back",
        Duration::from_secs(20),
        |s| {
            (s.workspace.latency_dialog.as_ref()).is_some_and(|d| {
                matches!(
                    d.measure,
                    LatencyMeasure::Done { .. } | LatencyMeasure::Failed { .. }
                )
            })
        },
    );
    let dialog = core.snapshot().state.workspace.latency_dialog.unwrap();
    let measured = match dialog.measure {
        LatencyMeasure::Done { measured_ms } => Some(measured_ms),
        other => {
            eprintln!("q{quantum}: {other:?}");
            None
        }
    };
    core.send(Command::UseMeasuredLatency);
    core.send(Command::CloseLatencyMeasure);
    std::thread::sleep(Duration::from_millis(300));
    let reported_after = core.snapshot().state.audio.latency.reported_ms;
    core.stop();
    let kept: Option<i64> = std::fs::read(config.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["measured_latency"][0]["x_us"].as_i64());
    eprintln!(
        "q{quantum} {tag}: PC says {reported} ms (after: {reported_after:?}), measured \
         {measured:?} ms, kept x_us {kept:?}"
    );
    (reported, measured, kept)
}

#[test]
#[ignore = "starts a PipeWire daemon of its own and records for minutes"]
fn a_recorded_voice_lies_where_it_was_heard_at_every_quantum() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let delay_ms: Option<f64> = std::env::var("MELOET_VTIMING_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok());
    let _delay = delay_ms.map(|ms| DelayBox::start(&pw, ms / 1000.0));
    // `MELOET_VTIMING_SECOND_SINK=1`: the output's monitor is linked straight into a second
    // sink, whose monitor is the input. Nothing is between them, but the input is then not
    // the monitor of the sink that drives the graph.
    let second = delay_ms.is_none() && std::env::var_os("MELOET_VTIMING_SECOND_SINK").is_some();
    if second {
        add_sink(&pw, DELAY_SINK);
        for side in ["FL", "FR"] {
            let done = pw_tool(&pw, "pw-link")
                .arg(format!("{}:monitor_{side}", private_pw::SINK))
                .arg(format!("{DELAY_SINK}:playback_{side}"))
                .status()
                .unwrap();
            assert!(done.success(), "the sinks are linked");
        }
    }
    let input_sink = if delay_ms.is_some() || second {
        DELAY_SINK
    } else {
        private_pw::SINK
    };
    // SAFETY: no core runs yet, so no other thread reads the environment.
    unsafe { std::env::set_var("MELOET_INPUT_SINK_MONITOR", input_sink) };
    let quanta = env_list("MELOET_VTIMING_QUANTA", &[64, 128, 256, 512, 1024]);
    let wanted = std::env::var("MELOET_VTIMING_RUNS")
        .unwrap_or_else(|_| "probe,tempo,meter,map,loop".into());
    let wants = |what: &str| wanted.split(',').any(|w| w.trim() == what);
    let root = pw.dir().join("vtiming");

    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut probes: Vec<String> = Vec::new();
    for quantum in quanta {
        force_quantum(&pw, quantum);
        let mut x_us = None;
        if wants("probe") {
            for round in 0..3 {
                let (reported, measured, kept) = measure(&root, quantum, &format!("probe{round}"));
                probes.push(format!(
                    "| {quantum} | {round} | {reported:.1} | {} | {} |",
                    measured.map_or("-".into(), |m| format!("{m:.1}")),
                    kept.map_or("-".into(), |k| format!(
                        "{k} µs = {:.2} frames",
                        k as f64 * RATE / 1e6
                    )),
                ));
                x_us = x_us.or(kept.map(|k| k as i32));
            }
        }
        let mut plans = Vec::new();
        if wants("tempo") {
            for bpm in [96.0, 137.5] {
                plans.push(Plan {
                    label: format!("{bpm} 4/4"),
                    shape: Shape::Tempo(bpm),
                    seconds: 12.0,
                });
            }
        }
        if wants("meter") {
            plans.push(Plan {
                label: "120 6/8".into(),
                shape: Shape::Meter(120.0, 6, 8),
                seconds: 12.0,
            });
        }
        if wants("map") {
            plans.push(Plan {
                label: "map".into(),
                shape: Shape::Map,
                seconds: 34.0,
            });
        }
        if wants("loop") {
            plans.push(Plan {
                label: "loop".into(),
                shape: Shape::Loop,
                seconds: 15.0,
            });
        }
        for (i, plan) in plans.iter().enumerate() {
            let mut o = record(&root, quantum, plan, None, &format!("r{i}"));
            eprintln!("{}", o.row());
            if delay_ms.is_some() {
                o.label = format!("{} (box, as it comes)", o.label);
                outcomes.push(o);
                if let Some(x) = x_us {
                    let mut used = record(&root, quantum, plan, Some(x), &format!("x{i}"));
                    used.label = format!("{} (box, measured number in use)", plan.label);
                    eprintln!("{}", used.row());
                    outcomes.push(used);
                }
            } else {
                outcomes.push(o);
            }
        }
    }

    let dir = out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let rows: Vec<String> = outcomes.iter().map(Outcome::row).collect();
    let all: Vec<String> = outcomes.iter().map(Outcome::json).collect();
    let summary = format!(
        "Private PipeWire (null sink, quantum set by clock.force-quantum){}; the vocal track \
         records the metronome from the sink's monitor, 32 bit float. Error = where a click \
         lies in the file − where the take says its tick is.\n\n{HEAD}\n{}\n\n\
         「遅れを測る」:\n\n| quantum | round | PC says ms | measured ms | kept (measured − PC says) |\n\
         |---|---|---|---|---|\n{}\n",
        delay_ms.map_or(
            if second {
                ", the output's monitor linked into a second sink whose monitor is the input"
                    .to_string()
            } else {
                String::new()
            },
            |ms| format!(", a pw-loopback of {ms} ms and a second sink before the input")
        ),
        rows.join("\n"),
        probes.join("\n")
    );
    std::fs::write(dir.join("summary.md"), &summary).unwrap();
    std::fs::write(dir.join("runs.json"), format!("[{}]\n", all.join(",\n"))).unwrap();
    eprintln!("{summary}\nwritten to {}", dir.display());

    if delay_ms.is_none() && !second {
        for o in &outcomes {
            assert!(
                o.clicks.len() >= 8,
                "{}: {} clicks",
                o.label,
                o.clicks.len()
            );
            assert_eq!(o.wrong_accents, 0, "{} at {}", o.label, o.quantum);
            let (worst, lone) = o.worst_but_lone();
            assert!(
                worst <= 1.5,
                "{} at {}: a click is {worst} frames off",
                o.label,
                o.quantum,
            );
            if lone > 0 {
                eprintln!("{} at {}: {lone} clicks alone off", o.label, o.quantum);
            }
        }
    }
}

// ---- the monitor: from the input to the output ----

fn add_sink(pw: &private_pw::PrivatePipeWire, name: &str) {
    let sink = pw_tool(pw, "pw-cli")
        .args([
            "create-node",
            "adapter",
            &format!(
                "{{ factory.name=support.null-audio-sink node.name={name} \
                 media.class=Audio/Sink audio.position=[FL FR] audio.rate=48000 \
                 object.linger=true }}"
            ),
        ])
        .stdout(Stdio::null())
        .status()
        .expect("pw-cli runs");
    assert!(sink.success(), "the second sink");
    std::thread::sleep(Duration::from_millis(500));
}

/// Two seconds of nothing, then a click of one frame at 0.5 every 0.4 s.
fn write_clicks(path: &Path, clicks: usize) {
    let frames = 96_000 + clicks * 19_200 + 24_000;
    let mut bytes = Vec::with_capacity(44 + frames * 4);
    let data = (frames * 4) as u32;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&48_000u32.to_le_bytes());
    bytes.extend_from_slice(&(48_000u32 * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for frame in 0..frames {
        let on =
            frame >= 96_000 && (frame - 96_000) % 19_200 < 8 && frame < 96_000 + clicks * 19_200;
        let sample: i16 = if on { 16_384 } else { 0 };
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

/// The channels of a 16 bit wav file whose head may not know its length.
fn read_channels(path: &Path, channels: usize) -> Vec<Vec<f32>> {
    let bytes = std::fs::read(path).unwrap();
    let data = (bytes.windows(4).position(|w| w == b"data")).expect("a data chunk") + 8;
    let mut out = vec![Vec::new(); channels];
    for frame in bytes[data..].chunks_exact(2 * channels) {
        for (c, s) in frame.as_chunks::<2>().0.iter().enumerate() {
            out[c].push(f32::from(i16::from_le_bytes(*s)) / 32_768.0);
        }
    }
    out
}

fn onsets(samples: &[f32], level: f32) -> Vec<usize> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < samples.len() {
        if samples[i].abs() > level {
            found.push(i);
            i += 9_600;
        } else {
            i += 1;
        }
    }
    found
}

/// How long a sound takes from the input through a vocal track's monitor to the output
/// (docs/architecture.md 22-5): a second sink stands for the microphone, and one recorder
/// hears that sink and the output's sink in the same cycles.
#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn the_monitor_is_as_late_as_the_status_line_says() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    add_sink(&pw, DELAY_SINK);
    // SAFETY: no core runs yet, so no other thread reads the environment.
    unsafe { std::env::set_var("MELOET_INPUT_SINK_MONITOR", DELAY_SINK) };
    let quanta = env_list("MELOET_VTIMING_QUANTA", &[64, 128, 256, 512, 1024]);
    let root = pw.dir().join("vmonitor");
    std::fs::create_dir_all(&root).unwrap();
    let clicks_file = root.join("clicks.wav");
    write_clicks(&clicks_file, 20);
    let link = |from: &str, to: &str| {
        let done = pw_tool(&pw, "pw-link").args([from, to]).output().unwrap();
        if !done.status.success() {
            let ports = pw_tool(&pw, "pw-link").arg("-io").output().unwrap();
            panic!(
                "{from} -> {to}: {}\n{}\n{}",
                String::from_utf8_lossy(&done.stderr),
                String::from_utf8_lossy(&ports.stdout),
                std::fs::read_to_string(root.join("pw-record.log")).unwrap_or_default()
            );
        }
    };

    let mut rows = Vec::new();
    for quantum in quanta {
        force_quantum(&pw, quantum);
        let (config, state) = (
            root.join(format!("config-{quantum}")),
            root.join(format!("state-{quantum}")),
        );
        write_settings(&config);
        let (core, reported) = ready(&config, &state, quantum);
        let track = (core.snapshot().state.song.tracks.iter())
            .find(|t| matches!(t.kind, TrackKindView::Vocal { .. }))
            .map(|t| t.id)
            .unwrap();
        core.send(Command::ToggleMonitor { track });
        let said = match core.snapshot().state.status.message {
            Some(meloet_protocol::StatusMessage::MonitorFirstOn { latency_ms }) => latency_ms,
            other => panic!("{other:?}"),
        };

        let heard_file = root.join(format!("heard-{quantum}.wav"));
        let mut recorder = pw_tool(&pw, "pw-record")
            .args(["--target", "0", "--channels", "2"])
            .args(["--format", "s16", "--rate", "48000"])
            .args(["-P", "node.name=meloet-test-ears"])
            .arg(&heard_file)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(root.join("pw-record.log")).unwrap())
            .spawn()
            .expect("pw-record starts");
        std::thread::sleep(Duration::from_millis(700));
        link(
            &format!("{DELAY_SINK}:monitor_FL"),
            "meloet-test-ears:input_FL",
        );
        link(
            &format!("{}:monitor_FL", private_pw::SINK),
            "meloet-test-ears:input_FR",
        );
        let played = pw_tool(&pw, "pw-play")
            .args(["--target", DELAY_SINK])
            .arg(&clicks_file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("pw-play runs");
        assert!(played.success());
        std::thread::sleep(Duration::from_millis(500));
        let _ = Process::new("kill")
            .args(["-INT", &recorder.id().to_string()])
            .status();
        let _ = recorder.wait();
        assert_eq!(
            core.snapshot().state.audio.actual_frames,
            Some(quantum),
            "the quantum held"
        );
        core.stop();

        // Left: the sink that stands for the microphone. Right: meloet's output.
        let heard = read_channels(&heard_file, 2);
        let (sung, out) = (onsets(&heard[0], 0.1), onsets(&heard[1], 0.1));
        let late: Vec<i64> = (sung.iter())
            .filter_map(|&s| {
                (out.iter())
                    .find(|&&o| o >= s && o < s + 9_600)
                    .map(|&o| o as i64 - s as i64)
            })
            .collect();
        let (least, most) = (late.iter().min(), late.iter().max());
        let mut sorted = late.clone();
        sorted.sort_unstable();
        let middle = sorted.get(sorted.len() / 2).copied().unwrap_or(-1);
        let ms = |frames: i64| frames as f64 / 48.0;
        let row = format!(
            "| {quantum} | {reported:.1} | {} | {} | {} | {middle} = {:.1} ms = {:.2} blocks | {} .. {} |",
            said.map_or("-".into(), |m| format!("{m:.1}")),
            sung.len(),
            late.len(),
            ms(middle),
            middle as f64 / f64::from(quantum),
            least.map_or("-".into(), |f| f.to_string()),
            most.map_or("-".into(), |f| f.to_string()),
        );
        eprintln!("{row}  each: {late:?}");
        rows.push(row);
    }
    let dir = out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let summary = format!(
        "Private PipeWire, two null sinks: clicks are played into the second one, whose monitor \
         is meloet's input; a vocal track with its monitor on sounds them to the first one. One \
         recorder hears both sinks.\n\n\
         | quantum | PC says (round trip) ms | status line says ms | clicks in | clicks out | late: middle | least .. most frames |\n\
         |---|---|---|---|---|---|---|\n{}\n",
        rows.join("\n")
    );
    std::fs::write(dir.join("monitor.md"), &summary).unwrap();
    eprintln!("{summary}\nwritten to {}", dir.display());
}
