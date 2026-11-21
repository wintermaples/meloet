//! A measuring tool (docs/measurements/2026-11-21-round7-step3.md, chapter 6): are the blocks
//! of the input that come late blocks after a hole, or only late? On a PipeWire daemon of the
//! test's own, the sink's monitor is the input. Every block's time and frame count is kept
//! for some seconds, and the rule the disk writer fills holes by (a block that starts more
//! than half a block after the one before it ended; `meloet-core`, `vocalrec.rs`) is tried
//! on them. A hole stays: every block after it is late by as much. A late wake does not: the
//! blocks after it are on the line again. Run with `-- --ignored --nocapture`, with
//! something else keeping the PC busy.
//!
//! `MELOET_GAPS_QUANTUM` (64), `MELOET_GAPS_SECONDS` (20), `MELOET_GAPS_OUT` (a JSON file
//! with every block).
#![allow(
    clippy::disallowed_methods,
    reason = "the test thread is the control side; it may sleep"
)]

mod common;
mod private_pw;

use std::thread::sleep;
use std::time::{Duration, Instant};

use meloet_engine::{
    AudioInput, AudioOutput, Engine, EngineConfig, InputBlock, InputTarget, OutputTarget,
};

fn env_number(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn middle(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn late_blocks_of_the_input_are_holes_or_only_late() {
    let Some(_pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let quantum = env_number("MELOET_GAPS_QUANTUM", 64);
    let seconds = env_number("MELOET_GAPS_SECONDS", 20);
    let parts = Engine::new(EngineConfig::default());
    let meters = parts.handle.meters().clone();
    let output = AudioOutput::open(
        &OutputTarget {
            node_name: private_pw::SINK.into(),
        },
        quantum,
        parts.engine,
    )
    .expect("the output opens");
    let rate = output.sample_rate();
    let mut input = AudioInput::open(
        &InputTarget {
            node_name: private_pw::SINK.into(),
            sink_monitor: true,
        },
        rate,
        quantum,
    )
    .expect("the sink's monitor opens as an input");
    let in_meters = input.meters().clone();
    let mut record = input.take_record().expect("the record ring");
    let deadline = Instant::now() + Duration::from_secs(5);
    while meters.callbacks() < 20 || in_meters.frames() < 20 * u64::from(quantum) {
        assert!(Instant::now() < deadline, "the streams do not run");
        sleep(Duration::from_millis(5));
    }
    sleep(Duration::from_millis(500));

    in_meters.set_recording(true);
    let (out_blocks, began) = (meters.callbacks(), Instant::now());
    let mut blocks: Vec<InputBlock> = Vec::new();
    let mut into = vec![0.0f32; 2 * 8_192];
    let until = Instant::now() + Duration::from_secs(u64::from(seconds));
    while Instant::now() < until {
        while let Some(block) = record.pop(&mut into) {
            blocks.push(block);
        }
        sleep(Duration::from_millis(2));
    }
    let (xruns_in, dropped) = (in_meters.xruns(), in_meters.record_dropped());
    // The output's side of the same seconds: the blocks the engine made against the clock.
    let out_blocks = meters.callbacks() - out_blocks;
    let out_due = began.elapsed().as_secs_f64() * f64::from(rate) / f64::from(quantum);
    input.close();
    drop(output);

    let frame_us = 1e6 / f64::from(rate);
    // How late each block's first frame is against the count of frames.
    let off: Vec<f64> = (blocks.iter())
        .map(|b| b.first_frame_us(rate) - b.first_frame as f64 * frame_us)
        .collect();
    let (mut holes, mut only_late, mut filled, mut filled_wrongly) = (0u32, 0u32, 0u64, 0u64);
    let mut shown = 0;
    let mut odd_sizes = 0u32;
    for i in 1..blocks.len() {
        let (before, block) = (blocks[i - 1], blocks[i]);
        odd_sizes += u32::from(block.frames != quantum);
        let due_us = before.first_frame_us(rate) + f64::from(before.frames) * frame_us;
        let late_us = block.first_frame_us(rate) - due_us;
        if late_us <= f64::from(block.frames) * frame_us / 2.0 {
            continue;
        }
        let missed = (late_us / frame_us).round() as u64;
        filled += missed;
        let earlier = middle(&off[i.saturating_sub(30)..i]);
        let later = &off[(i + 5).min(off.len())..(i + 35).min(off.len())];
        let stays =
            !later.is_empty() && middle(later) - earlier > f64::from(quantum) * frame_us / 2.0;
        let counted = block.first_frame - (before.first_frame + u64::from(before.frames));
        if stays {
            holes += 1;
        } else {
            only_late += 1;
            filled_wrongly += missed;
        }
        if shown < 12 {
            shown += 1;
            eprintln!(
                "block {i}: {:.0} us late ({missed} frames by the rule), frames between by \
                 the count {counted}, the blocks after are {:.0} us off the line before: {}",
                late_us,
                if later.is_empty() {
                    f64::NAN
                } else {
                    middle(later) - earlier
                },
                if stays { "a hole" } else { "only late" }
            );
        }
    }
    let span_s = blocks
        .last()
        .map_or(0.0, |b| (b.capture_us - blocks[0].capture_us) as f64 / 1e6);
    let frames: u64 = blocks.iter().map(|b| u64::from(b.frames)).sum();
    eprintln!(
        "quantum {quantum}, {} blocks in {span_s:.2} s ({frames} frames, {:.1} by the clock), \
         {odd_sizes} of another size, xruns {xruns_in}, ring dropped {dropped}: the rule fills \
         {filled} frames in {} places; {holes} are holes, {only_late} only late \
         ({filled_wrongly} frames filled with nothing missing)",
        blocks.len(),
        span_s * f64::from(rate),
        holes + only_late
    );
    eprintln!(
        "the output made {out_blocks} blocks where the clock has {out_due:.0}: {:.0} less",
        out_due - out_blocks as f64
    );
    if let Ok(path) = std::env::var("MELOET_GAPS_OUT") {
        let rows: Vec<String> = (blocks.iter())
            .map(|b| {
                format!(
                    "[{},{},{},{}]",
                    b.first_frame, b.frames, b.capture_us, b.callback_us
                )
            })
            .collect();
        std::fs::write(path, format!("[{}]", rows.join(","))).expect("the blocks are written");
    }
}
