//! A measuring tool (docs/history/measurements/2026-11-21-round7-step3.md, chapter 6): are the blocks
//! of the input that come late blocks after a hole, or only late? On a PipeWire daemon of the
//! test's own, the sink's monitor is the input. Every block's time and frame count is kept
//! for some seconds, and the rule the disk writer fills holes by (a block that starts more
//! than half a block after the one before it ended; `meloet-core`, `vocalrec.rs`) is tried
//! on them. A hole stays: every block after it is late by as much. A late wake does not: the
//! blocks after it are on the line again. Run with `-- --ignored --nocapture`, with
//! something else keeping the PC busy.
//!
//! It also says how the output kept up: which frame of the engine reached the DAC when each
//! block of the input was captured, less the count of the input's frames. That number
//! stays while both go on, grows when the input alone misses cycles, falls when the output
//! alone does, and stays when both miss the same ones (a recording counts the input's
//! frames from its head, so where the number has moved, the song and the file are apart).
//!
//! `MELOET_GAPS_QUANTUM` (64), `MELOET_GAPS_SECONDS` (20), `MELOET_GAPS_OUT` (a JSON file
//! with every block), `MELOET_GAPS_ENGINE_OUT` (the same with the engine's frame of each).
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
    let clock = parts.handle.clock().clone();
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
    // Of each block: the engine's frame at the DAC when its first frame was captured.
    let mut engine_frames: Vec<f64> = Vec::new();
    let mut into = vec![0.0f32; 2 * 8_192];
    let until = Instant::now() + Duration::from_secs(u64::from(seconds));
    while Instant::now() < until {
        while let Some(block) = record.pop(&mut into) {
            engine_frames.push(
                clock
                    .frame_at(block.first_frame_us(rate))
                    .unwrap_or(f64::NAN),
            );
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
    // The engine's frame less the input's count, against the same of the blocks before.
    let apart: Vec<f64> = (blocks.iter().zip(&engine_frames))
        .map(|(b, e)| e - b.first_frame as f64)
        .collect();
    let half = f64::from(quantum) / 2.0;
    // The middle of five: a time that alone is off for a block or two is not a move.
    let calm: Vec<f64> = apart.windows(5).map(middle).collect();
    let (mut input_alone, mut output_alone, mut shown) = ((0u32, 0.0f64), (0u32, 0.0f64), 0);
    for (i, pair) in calm.windows(2).enumerate() {
        let step = pair[1] - pair[0];
        // NaN: the engine's clock had nothing for the block.
        if step.abs() <= half || step.is_nan() {
            continue;
        }
        let side = if step > 0.0 {
            &mut input_alone
        } else {
            &mut output_alone
        };
        *side = (side.0 + 1, side.1 + step.abs());
        if shown < 12 {
            shown += 1;
            eprintln!(
                "block {}: the engine's frame less the input's count moves by {step:.0}",
                i + 3
            );
        }
    }
    eprintln!(
        "the engine's frame at the DAC less the input's count: {:.0} more at the end than at \
         the start; the input alone missed {:.0} frames in {} places, the output alone {:.0} \
         in {}",
        apart.last().unwrap_or(&0.0) - apart.first().unwrap_or(&0.0),
        input_alone.1,
        input_alone.0,
        output_alone.1,
        output_alone.0
    );
    if let Ok(path) = std::env::var("MELOET_GAPS_ENGINE_OUT") {
        let rows: Vec<String> = (blocks.iter().zip(&engine_frames))
            .map(|(b, e)| format!("[{},{},{},{e:.1}]", b.first_frame, b.frames, b.capture_us))
            .collect();
        std::fs::write(path, format!("[{}]", rows.join(","))).expect("the blocks are written");
    }
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
