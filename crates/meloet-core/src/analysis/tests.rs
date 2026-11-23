use std::time::Instant;

use meloet_engine::{
    BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, Instrument, InstrumentEvent,
    MixStrip,
};

use super::*;

/// The same value on every frame.
struct Dc;

impl Instrument for Dc {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.25);
        right.fill(0.5);
    }
    fn reset(&mut self) {}
}

fn engine() -> EngineParts {
    let mut parts = Engine::new(EngineConfig::default());
    assert!(parts.handle.swap_instrument(0, Box::new(Dc)).is_ok());
    parts
}

fn blocks(parts: &mut EngineParts, count: usize) {
    let mut out = [0.0f32; 512];
    for _ in 0..count {
        parts.engine.process(&mut out, BlockTiming::default());
    }
}

#[test]
fn the_thread_takes_what_the_engine_writes_and_ends_with_its_owner() {
    let mut parts = engine();
    let analysis = Analysis::spawn(parts.handle.take_analysis());
    let readings = analysis.readings().clone();
    // Four times what the ring holds, a quarter of it at a time: the thread empties it.
    let burst = meloet_engine::ANALYSIS_RING_FRAMES / 4 / 256;
    let until = Instant::now() + Duration::from_secs(10);
    let mut written = 0;
    for _ in 0..16 {
        blocks(&mut parts, burst);
        written += burst as u64 * 256;
        while readings.frames() < written && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    assert_eq!(readings.frames(), written);
    // The bands are off until told; M3 fills the rest, until then they say none.
    assert_eq!(readings.spectrum(), [0; SPECTRUM_BANDS]);
    assert_eq!(
        (
            readings.lufs_short(),
            readings.lufs_integrated(),
            readings.true_peak()
        ),
        (TELEMETRY_NO_VALUE, TELEMETRY_NO_VALUE, TELEMETRY_NO_VALUE)
    );
    drop(analysis);
    // The reader went with the thread: the engine writes no more.
    blocks(&mut parts, 100);
    assert_eq!(readings.frames(), written);
}

#[test]
fn a_reader_of_an_engine_made_anew_takes_the_place_of_the_old_one() {
    let mut parts = engine();
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    blocks(&mut parts, 3);
    assert_eq!(side.pump(), 768);
    assert_eq!(side.pump(), 0);

    blocks(&mut parts, 2);
    let (engine, _) = parts.handle.rebuild();
    parts.engine = engine;
    assert!(parts.handle.swap_instrument(0, Box::new(Dc)).is_ok());
    side.set_reader(
        parts
            .handle
            .take_analysis()
            .expect("the new engine's reader"),
    );
    blocks(&mut parts, 1);
    assert_eq!(side.pump(), 256, "only the new engine's");
    assert_eq!(side.readings.frames(), 768 + 256);
}

#[test]
fn an_engine_whose_reader_is_not_taken_writes_nothing() {
    let mut parts = engine();
    blocks(&mut parts, 100);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    assert_eq!(side.pump(), 0, "nothing from before");
}

/// A sine at `hz`, both sides, full scale.
struct Sine {
    hz: f32,
    rate: f32,
    at: f32,
}

impl Instrument for Sine {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.rate = sample_rate;
    }
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right) {
            *l = (std::f32::consts::TAU * self.at).sin();
            *r = *l;
            self.at = (self.at + self.hz / self.rate).fract();
        }
    }
    fn reset(&mut self) {}
}

fn sine_engine(hz: f32) -> EngineParts {
    let mut parts = Engine::new(EngineConfig::default());
    let sine = Sine {
        hz,
        rate: 48_000.0,
        at: 0.0,
    };
    assert!(parts.handle.swap_instrument(0, Box::new(sine)).is_ok());
    parts
}

/// 1 kHz is in band 12 (800 Hz to 1027 Hz).
const KHZ_BAND: usize = 12;

#[test]
fn the_bands_are_worked_only_while_the_spectrum_is_on() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    blocks(&mut parts, 40);
    assert_eq!(side.pump(), 40 * 256);
    assert_eq!(side.spectrum.ffts(), 0, "off: no FFT");
    assert_eq!(side.readings.spectrum(), [0; SPECTRUM_BANDS]);

    side.set_spectrum(true);
    blocks(&mut parts, 40);
    side.pump();
    let ffts = side.spectrum.ffts();
    assert_eq!(ffts, 10, "one every 1024 frames");
    let bars = side.readings.spectrum();
    assert!(bars[KHZ_BAND] >= 245, "{bars:?}");
    assert_eq!(bars.iter().max(), Some(&bars[KHZ_BAND]));

    side.set_spectrum(false);
    assert_eq!(
        side.readings.spectrum(),
        [0; SPECTRUM_BANDS],
        "off reads 0 at once"
    );
    blocks(&mut parts, 40);
    side.pump();
    assert_eq!(side.spectrum.ffts(), ffts);
}

#[test]
fn when_no_frames_come_the_bars_fall_to_nothing() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    side.set_spectrum(true);
    blocks(&mut parts, 40);
    side.pump();
    let start = side.readings.spectrum()[KHZ_BAND];
    // A few wakes with nothing (a large block on its way) change nothing.
    for _ in 0..STALLED_WAKES {
        side.pump();
    }
    assert_eq!(side.readings.spectrum()[KHZ_BAND], start);
    // Then the stream is taken as stopped: 30 dB a second by the time.
    for _ in 0..100 {
        side.pump();
    }
    let fallen = side.readings.spectrum()[KHZ_BAND];
    assert!(
        (f64::from(start) - 30.0 * 255.0 / 72.0 - f64::from(fallen)).abs() <= 1.0,
        "{start} → {fallen}"
    );
    for _ in 0..300 {
        side.pump();
    }
    assert_eq!(side.readings.spectrum(), [0; SPECTRUM_BANDS]);
}

#[test]
fn a_new_reader_or_lost_frames_start_the_bands_from_silence() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_spectrum(true);
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    blocks(&mut parts, 40);
    side.pump();
    assert!(side.readings.spectrum()[KHZ_BAND] > 0);

    // The ring overflows: what it holds came before the gap and is let go of; the bands start
    // again with what comes after.
    let ffts = side.spectrum.ffts();
    blocks(&mut parts, 100);
    assert_eq!(side.pump(), meloet_engine::ANALYSIS_RING_FRAMES);
    assert_eq!(side.readings.spectrum(), [0; SPECTRUM_BANDS]);
    assert_eq!(side.spectrum.ffts(), ffts);
    blocks(&mut parts, 40);
    side.pump();
    assert!(side.readings.spectrum()[KHZ_BAND] >= 245);

    let (engine, _) = parts.handle.rebuild();
    parts.engine = engine;
    side.set_reader(parts.handle.take_analysis().expect("the new reader"));
    assert_eq!(side.readings.spectrum(), [0; SPECTRUM_BANDS]);
}

/// Renders `seconds` of the engine, taking what the ring holds every 16 blocks.
fn hear(parts: &mut EngineParts, side: &mut Side, seconds: f64) {
    let total = (seconds * 48_000.0 / 256.0).round() as usize;
    let mut done = 0;
    while done < total {
        let n = 16.min(total - done);
        blocks(parts, n);
        side.pump();
        done += n;
    }
}

fn tell(parts: &mut EngineParts, command: EngineCommand) {
    assert!(parts.handle.send(command).is_ok());
}

/// Short-term, integrated, true peak.
fn loudness(side: &Side) -> [i16; 3] {
    let r = &side.readings;
    [r.lufs_short(), r.lufs_integrated(), r.true_peak()]
}

/// Within 0.2 dB of `want`.
fn reads(value: i16, want: f64) -> bool {
    value != TELEMETRY_NO_VALUE && (f64::from(value) / 100.0 - want).abs() <= 0.2
}

const NONE: [i16; 3] = [TELEMETRY_NO_VALUE; 3];

#[test]
fn the_loudness_is_worked_only_while_it_is_wanted() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    hear(&mut parts, &mut side, 1.0);
    assert!(
        side.loudness.is_none(),
        "no meters while nothing shows them"
    );
    assert_eq!(loudness(&side), NONE);

    side.set_loudness(true);
    hear(&mut parts, &mut side, 4.0);
    // A full-scale 1 kHz sine on both sides is 0 LUFS; the song does not play.
    let [short, integrated, peak] = loudness(&side);
    assert!(reads(short, 0.0), "{short}");
    assert_eq!((integrated, peak), (TELEMETRY_NO_VALUE, TELEMETRY_NO_VALUE));

    side.set_loudness(false);
    assert!(side.loudness.is_none());
    assert_eq!(loudness(&side), NONE, "none at once");
    hear(&mut parts, &mut side, 1.0);
    assert_eq!(loudness(&side), NONE);
}

#[test]
fn a_play_is_measured_from_its_start_and_its_values_stay_after_it_stops() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    side.set_loudness(true);
    hear(&mut parts, &mut side, 1.0);

    tell(&mut parts, EngineCommand::Play { from: 0 });
    hear(&mut parts, &mut side, 5.0);
    let [short, integrated, peak] = loudness(&side);
    assert!(reads(short, 0.0) && reads(integrated, 0.0) && reads(peak, 0.0));

    // Stopped, 12 dB quieter: what is heard now changes, the play's values stay.
    tell(&mut parts, EngineCommand::Stop { t_us: 0 });
    let quieter = EngineCommand::SetStrip {
        strip: MixStrip::Master,
        gain: 0.25,
        pan: 0.0,
    };
    tell(&mut parts, quieter);
    hear(&mut parts, &mut side, 4.0);
    let [short, integrated, peak] = loudness(&side);
    assert!(reads(short, -12.04), "{short}");
    assert!(reads(integrated, 0.0) && reads(peak, 0.0));

    // The next play starts anew: only what it plays counts.
    tell(&mut parts, EngineCommand::Play { from: 0 });
    hear(&mut parts, &mut side, 0.1);
    assert_eq!(loudness(&side)[1], TELEMETRY_NO_VALUE, "under 400 ms");
    hear(&mut parts, &mut side, 5.0);
    let [_, integrated, peak] = loudness(&side);
    assert!(reads(integrated, -12.04), "{integrated}");
    assert!(reads(peak, -12.04), "{peak}");
}

#[test]
fn when_no_frames_come_what_is_heard_now_goes_and_the_play_stays() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    side.set_loudness(true);
    tell(&mut parts, EngineCommand::Play { from: 0 });
    hear(&mut parts, &mut side, 5.0);
    for _ in 0..STALLED_WAKES {
        side.pump();
    }
    assert!(reads(loudness(&side)[0], 0.0), "a large block on its way");
    side.pump();
    let [short, integrated, peak] = loudness(&side);
    assert_eq!(short, TELEMETRY_NO_VALUE);
    assert!(reads(integrated, 0.0) && reads(peak, 0.0));
}

#[test]
fn a_new_reader_starts_the_loudness_from_silence() {
    let mut parts = sine_engine(1000.0);
    let mut side = Side::new(Arc::default());
    side.set_loudness(true);
    side.set_reader(parts.handle.take_analysis().expect("the reader"));
    tell(&mut parts, EngineCommand::Play { from: 0 });
    hear(&mut parts, &mut side, 5.0);
    assert!(reads(loudness(&side)[1], 0.0));
    let (engine, _) = parts.handle.rebuild();
    parts.engine = engine;
    side.set_reader(parts.handle.take_analysis().expect("the new reader"));
    assert_eq!(loudness(&side), NONE);
}
