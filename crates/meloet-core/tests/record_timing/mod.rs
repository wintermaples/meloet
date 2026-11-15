//! Where a recorded key lands, measured through the real ALSA sequencer and a real PipeWire.
//!
//! A virtual keyboard (a sequencer client of its own) presses each key at the moment a beat
//! reaches the DAC, as the core's telemetry says: `sent + (beat − tick) / ticksPerUs`. The take
//! that comes out should then hold every note on its beat. The difference is what this measures.
//!
//! The take keeps whole ticks, so one note alone cannot show less than half a tick. To see
//! below that, every press is moved by a known part of a tick (`dither`, −0.5 to +0.5, spread
//! evenly), and the error is taken against `beat + dither`: the mean then shows a bias well
//! below one tick.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_core::Core;
use meloet_midi::vkeys::VirtualKeyboard;
use meloet_midi::{MidiEvent, MidiOptions, monotonic_us};
use meloet_protocol::{AppState, Command, TakeNotes, Telemetry, TransportReading, TransportState};

pub const TICKS_PER_BEAT: f64 = 480.0;
/// The first beat pressed; the beats before it let the recording settle.
const FIRST_BEAT: u32 = 4;
const NOTE: u8 = 72;
/// Quiet: the placeholder sound is barely heard.
const VELOCITY: u8 = 1;

/// A virtual keyboard name no other run on this PC uses, and the MIDI options that take only it.
pub fn private_keyboard(tag: &str) -> (String, MidiOptions) {
    let name = format!("meloet-vkeys-timing-{tag}-{}", std::process::id());
    let options = MidiOptions {
        accept_virtual: true,
        only_virtual: true,
        virtual_names: vec![name.clone()],
        fail_after_events: None,
    };
    (name, options)
}

/// The settings file of a core that opens `output` at once, with the metronome on and no
/// count-in.
pub fn write_settings(dir: &std::path::Path, output: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{output}", "output_name": "{output}", "scale": 100, "metronome": true, "count_in_bars": 0 }}"#
        ),
    )
    .unwrap();
}

pub fn out_dir() -> PathBuf {
    std::env::var_os("MELOET_TIMING_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/measure/record-timing"),
        PathBuf::from,
    )
}

pub fn wait_for(core: &Core, what: &str, limit: Duration, done: impl Fn(&AppState) -> bool) {
    let started = Instant::now();
    loop {
        let state = core.snapshot().state;
        if done(&state) {
            return;
        }
        assert!(
            started.elapsed() < limit,
            "{what}: {:?} {:?} {:?}",
            state.status,
            state.audio,
            state.transport
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The newest telemetry reading with a position, kept by the telemetry sink.
#[derive(Clone, Copy, Debug)]
pub struct Reading {
    pub sent_us: f64,
    pub transport: TransportReading,
}

pub fn listen(core: &Core) -> Arc<Mutex<Option<Reading>>> {
    let latest = Arc::new(Mutex::new(None));
    let sink = latest.clone();
    core.subscribe(
        Box::new(|_| true),
        Box::new(move |bytes| {
            if let Ok(t) = Telemetry::decode(bytes)
                && let Some(transport) = t.transport
            {
                *sink.lock().unwrap() = Some(Reading {
                    sent_us: t.sent_ns as f64 / 1000.0,
                    transport,
                });
            }
            true
        }),
    );
    latest
}

#[derive(Clone, Debug)]
pub struct Run {
    pub label: String,
    pub bpm: f64,
    pub beats: u32,
    /// Pressed this much after the beat on purpose.
    pub late_ms: f64,
}

#[derive(Clone, Debug)]
pub struct Press {
    pub beat: u32,
    /// The tick the key should land on: the beat, the dither and `late_ms`.
    pub expected_tick: f64,
    pub target_us: f64,
    /// When the virtual keyboard sent it, minus `target_us`.
    pub send_late_us: f64,
    /// Where the listener was when the key was sent, by a reading taken after it. It differs
    /// from `expected_tick` (plus the send delay) when the map from time to song position moved
    /// in between, as after an xrun.
    pub heard_at_send: f64,
    pub recorded_tick: Option<u64>,
}

impl Press {
    /// The map moved by more than a quarter tick between the plan and the press.
    pub fn map_moved(&self, ticks_per_us: f64) -> bool {
        (self.heard_at_send - (self.expected_tick + self.send_late_us * ticks_per_us)).abs() > 0.25
    }
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub run: Run,
    pub actual_frames: Option<u32>,
    pub latency_ms: Option<f32>,
    pub presses: Vec<Press>,
    pub notes_in_take: usize,
}

pub struct Stats {
    pub mean: f64,
    pub p95_abs: f64,
    pub max_abs: f64,
}

pub fn stats(values: &[f64]) -> Stats {
    let mut abs: Vec<f64> = values.iter().map(|v| v.abs()).collect();
    abs.sort_by(f64::total_cmp);
    let n = abs.len().max(1);
    let p95 = abs
        .get(((n as f64 * 0.95).ceil() as usize).clamp(1, n) - 1)
        .copied()
        .unwrap_or(f64::NAN);
    Stats {
        mean: values.iter().sum::<f64>() / n as f64,
        p95_abs: p95,
        max_abs: abs.last().copied().unwrap_or(f64::NAN),
    }
}

impl Outcome {
    pub fn ticks_per_ms(&self) -> f64 {
        TICKS_PER_BEAT * self.run.bpm / 60_000.0
    }

    /// Recorded minus expected, in ticks.
    pub fn errors_ticks(&self) -> Vec<f64> {
        self.presses
            .iter()
            .filter_map(|p| p.recorded_tick.map(|t| t as f64 - p.expected_tick))
            .collect()
    }

    /// Recorded minus where the listener was at the send, in ticks.
    pub fn errors_heard_ticks(&self) -> Vec<f64> {
        self.presses
            .iter()
            .filter_map(|p| p.recorded_tick.map(|t| t as f64 - p.heard_at_send))
            .collect()
    }

    pub fn moved(&self) -> usize {
        let tpu = self.ticks_per_ms() / 1000.0;
        self.presses.iter().filter(|p| p.map_moved(tpu)).count()
    }

    /// Recorded minus the beat, in ms: where the note sits against the beat line.
    pub fn offsets_from_beat_ms(&self) -> Vec<f64> {
        self.presses
            .iter()
            .filter_map(|p| {
                p.recorded_tick
                    .map(|t| (t as f64 - f64::from(p.beat) * TICKS_PER_BEAT) / self.ticks_per_ms())
            })
            .collect()
    }

    pub fn send_late_us(&self) -> Vec<f64> {
        self.presses.iter().map(|p| p.send_late_us).collect()
    }

    pub fn json(&self) -> String {
        let presses: Vec<String> = self
            .presses
            .iter()
            .map(|p| {
                format!(
                    r#"{{"beat":{},"expectedTick":{:.4},"recordedTick":{},"targetUs":{:.1},"sendLateUs":{:.1},"heardAtSend":{:.4}}}"#,
                    p.beat,
                    p.expected_tick,
                    p.recorded_tick
                        .map_or_else(|| "null".to_string(), |t| t.to_string()),
                    p.target_us,
                    p.send_late_us,
                    p.heard_at_send
                )
            })
            .collect();
        format!(
            r#"{{"label":"{}","bpm":{},"beats":{},"lateMs":{},"actualFrames":{},"latencyMs":{},"notesInTake":{},"presses":[{}]}}"#,
            self.run.label,
            self.run.bpm,
            self.run.beats,
            self.run.late_ms,
            self.actual_frames
                .map_or_else(|| "null".into(), |f| f.to_string()),
            self.latency_ms
                .map_or_else(|| "null".into(), |l| format!("{l:.3}")),
            self.notes_in_take,
            presses.join(",\n")
        )
    }

    pub fn table_row(&self) -> String {
        let e = stats(&self.errors_ticks());
        let tpm = self.ticks_per_ms();
        let b = stats(&self.offsets_from_beat_ms());
        let s = stats(&self.send_late_us());
        let h = stats(&self.errors_heard_ticks());
        format!(
            "| {} | {} | {:.2} | {:.1} | {} | {:+.3} ms / {:+.3} t | {:.3} ms / {:.3} t | {:.3} ms / {:.3} t | {:+.3} ms | {:.0} µs | {} | {:+.3} ms / {:.3} ms |",
            self.run.label,
            self.actual_frames.map_or("?".into(), |f| f.to_string()),
            self.run.bpm,
            self.run.late_ms,
            self.notes_in_take,
            e.mean / tpm,
            e.mean,
            e.p95_abs / tpm,
            e.p95_abs,
            e.max_abs / tpm,
            e.max_abs,
            b.mean,
            s.max_abs,
            self.moved(),
            h.mean / tpm,
            h.max_abs / tpm,
        )
    }
}

pub const TABLE_HEAD: &str = "\
| run | quantum | bpm | late ms | notes | mean error | 95 % abs error | max abs error | mean from beat | max send late | map moved | vs heard at send: mean / max abs |
|---|---|---|---|---|---|---|---|---|---|---|---|";

/// Low-discrepancy dither in −0.5..0.5 ticks.
fn dither(index: u32) -> f64 {
    (f64::from(index) * 0.618_033_988_749_895).fract() - 0.5
}

fn reading(latest: &Mutex<Option<Reading>>) -> Option<Reading> {
    *latest.lock().unwrap()
}

/// Waits to `target_us` on CLOCK_MONOTONIC: sleeps to 2 ms before, then spins.
fn wait_until(target_us: f64) {
    let now = monotonic_us() as f64;
    if target_us - now > 2_000.0 {
        std::thread::sleep(Duration::from_micros((target_us - now - 2_000.0) as u64));
    }
    while (monotonic_us() as f64) < target_us {
        std::hint::spin_loop();
    }
}

/// Records one take on track 1 from the song start and reads where each key landed. The core
/// must have its output running, the metronome on, no count-in, and the keyboard `keys`.
pub fn record(
    core: &Core,
    latest: &Mutex<Option<Reading>>,
    keys: &VirtualKeyboard,
    run: Run,
) -> Outcome {
    core.send(Command::SetTempo { bpm: run.bpm });
    core.send(Command::GoToStart);
    let state = core.snapshot().state;
    assert!(
        (state.song.tempo_bpm - run.bpm).abs() < 1e-9,
        "tempo {}",
        state.song.tempo_bpm
    );
    assert_eq!(state.song.meter_den, 4);
    assert!(state.transport.metronome);
    assert_eq!(state.transport.count_in_bars, 0);
    let actual_frames = state.audio.actual_frames;
    let latency_ms = state.audio.latency_ms;

    let asked = monotonic_us() as f64;
    core.send(Command::Record);
    let deadline = Instant::now() + Duration::from_secs(3);
    // Right after the last take stopped, the DAC still plays its end for one latency: wait for
    // the new recording from tick 0 to be heard.
    while !reading(latest).is_some_and(|r| {
        r.sent_us > asked
            && r.transport.state == TransportState::Recording
            && r.transport.tick < TICKS_PER_BEAT
    }) {
        assert!(Instant::now() < deadline, "recording did not start");
        std::thread::sleep(Duration::from_millis(2));
    }

    let ticks_per_ms = TICKS_PER_BEAT * run.bpm / 60_000.0;
    let beat_ms = 60_000.0 / run.bpm;
    let mut presses = Vec::with_capacity(run.beats as usize);
    for i in 0..run.beats {
        let beat = FIRST_BEAT + i;
        let r = reading(latest).expect("a reading");
        assert_eq!(r.transport.state, TransportState::Recording);
        assert!(r.transport.ticks_per_us > 0.0);
        let expected_tick =
            f64::from(beat) * TICKS_PER_BEAT + dither(i) + run.late_ms * ticks_per_ms;
        let target_us = r.sent_us + (expected_tick - r.transport.tick) / r.transport.ticks_per_us;
        assert!(
            target_us > monotonic_us() as f64 + 1_000.0,
            "{}: beat {beat} is already past: the reading is {} µs old, at tick {}, {:?}",
            run.label,
            monotonic_us() as f64 - r.sent_us,
            r.transport.tick,
            core.snapshot().state.transport
        );
        wait_until(target_us);
        let sent = keys
            .send(MidiEvent::NoteOn {
                note: NOTE,
                velocity: VELOCITY,
            })
            .expect("send the key");
        std::thread::sleep(Duration::from_secs_f64(beat_ms * 0.4 / 1000.0));
        keys.send(MidiEvent::NoteOff {
            note: NOTE,
            velocity: 0,
        })
        .expect("send the release");
        let after = reading(latest).expect("a reading");
        assert!(after.sent_us > sent as f64, "no reading since the key");
        let heard_at_send =
            after.transport.tick + (sent as f64 - after.sent_us) * after.transport.ticks_per_us;
        presses.push(Press {
            beat,
            expected_tick,
            target_us,
            send_late_us: sent as f64 - target_us,
            heard_at_send,
            recorded_tick: None,
        });
        // Wait into the next beat's half before reading again, so that the next target is ahead.
        std::thread::sleep(Duration::from_secs_f64(beat_ms * 0.2 / 1000.0));
    }
    std::thread::sleep(Duration::from_secs_f64(beat_ms * 0.5 / 1000.0));
    core.send(Command::Record);
    wait_for(
        core,
        "the take is in the song",
        Duration::from_secs(3),
        |s| s.transport.state == TransportState::Stopped && s.transport.recording.is_none(),
    );

    let state = core.snapshot().state;
    let clip_id = state
        .arrange
        .selected_clip
        .expect("the new clip is selected");
    let clip = state
        .song
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == clip_id)
        .expect("the selected clip");
    assert_eq!(clip.take, clip.takes, "the new take is shown");
    let bytes = core.take_notes(clip.take_id).expect("the take's notes");
    let take = TakeNotes::decode(&bytes).expect("a take binary");
    let base = clip.start + clip.take_offset;
    let notes_in_take = take.notes.len();
    if notes_in_take == presses.len() {
        for (press, note) in presses.iter_mut().zip(&take.notes) {
            press.recorded_tick = Some(base + u64::from(note.start));
        }
    } else {
        eprintln!(
            "{}: {} notes in the take for {} keys",
            run.label,
            notes_in_take,
            presses.len()
        );
    }
    Outcome {
        run,
        actual_frames,
        latency_ms,
        presses,
        notes_in_take,
    }
}

/// Writes every run's raw JSON and a summary table under `out_dir()/name`.
pub fn write(name: &str, outcomes: &[Outcome], notes: &str) {
    let dir = out_dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let all: Vec<String> = outcomes.iter().map(Outcome::json).collect();
    std::fs::write(dir.join("runs.json"), format!("[{}]\n", all.join(",\n"))).unwrap();
    let rows: Vec<String> = outcomes.iter().map(Outcome::table_row).collect();
    let table = format!("{TABLE_HEAD}\n{}\n", rows.join("\n"));
    std::fs::write(dir.join("summary.md"), format!("{notes}\n\n{table}")).unwrap();
    eprintln!("{table}\nwritten to {}", dir.display());
}

/// The checks every run must pass. The bounds are loose on purpose: they catch a broken path
/// (a key placed by a whole buffer off, a missing key), not tenths of a millisecond.
pub fn check(o: &Outcome) {
    assert_eq!(
        o.notes_in_take,
        o.presses.len(),
        "{}: a key went missing",
        o.run.label
    );
    // Against where the listener was at the send: a press whose plan went stale (the map moved,
    // as after an xrun) is still judged by what was heard then.
    let e = stats(&o.errors_heard_ticks());
    let tpm = o.ticks_per_ms();
    assert!(
        e.mean.abs() / tpm < 0.5,
        "{}: mean error {} ms",
        o.run.label,
        e.mean / tpm
    );
    // Half a tick from the rounding to whole ticks, plus one millisecond.
    assert!(
        e.max_abs < 0.5 + tpm,
        "{}: worst error {} ticks",
        o.run.label,
        e.max_abs
    );
}
