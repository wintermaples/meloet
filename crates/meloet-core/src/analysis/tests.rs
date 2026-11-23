use std::time::Instant;

use meloet_engine::{BlockTiming, Engine, EngineConfig, EngineParts, Instrument, InstrumentEvent};

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
    // E7 and M3 fill these; until then they say none.
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
