//! Shared by the tests: sounds, rendering from memory, and simple measurements.
#![allow(dead_code)]

use meloet_stretch::{Follower, Preset, RenderError, Stretcher, render};
use std::convert::Infallible;
use std::f64::consts::TAU;
use std::sync::atomic::AtomicBool;

pub const SR: f64 = 48_000.0;

pub fn stretcher(channels: usize, preset: Preset) -> Stretcher {
    Stretcher::new(channels, SR as f32, preset).expect("stretcher")
}

pub fn sine(freq: f64, frames: usize, amp: f32) -> Vec<f32> {
    (0..frames)
        .map(|i| amp * (TAU * freq * i as f64 / SR).sin() as f32)
        .collect()
}

/// A click: a few milliseconds of a decaying 3 kHz burst starting at `at`.
pub fn add_click(buf: &mut [f32], at: usize) {
    for i in 0..96 {
        let t = i as f64 / SR;
        buf[at + i] += (0.9 * (-t * 2000.0).exp() * (TAU * 3000.0 * t).cos()) as f32;
    }
}

/// Copies file frames, zeros outside the file.
pub fn read_from(
    file: &[Vec<f32>],
) -> impl FnMut(i64, &mut [&mut [f32]]) -> Result<(), Infallible> + '_ {
    move |start, channels| {
        for (c, out) in channels.iter_mut().enumerate() {
            for (i, v) in out.iter_mut().enumerate() {
                let at = start + i as i64;
                *v = if at >= 0 && (at as usize) < file[c].len() {
                    file[c][at as usize]
                } else {
                    0.0
                };
            }
        }
        Ok(())
    }
}

pub fn render_all(
    st: &mut Stretcher,
    file: &[Vec<f32>],
    out_frames: u64,
    p: impl Fn(u64) -> f64,
) -> Vec<Vec<f32>> {
    let mut out = vec![Vec::with_capacity(out_frames as usize); file.len()];
    let write = |block: &[&[f32]]| {
        for (o, b) in out.iter_mut().zip(block) {
            o.extend_from_slice(b);
        }
        Ok(())
    };
    let done: Result<(), RenderError<Infallible>> = render(
        st,
        out_frames,
        p,
        read_from(file),
        write,
        |_| {},
        &AtomicBool::new(false),
    );
    done.expect("render");
    out
}

/// The same as `render`, with another number of output frames per `process` call.
pub fn render_with_step(
    st: &mut Stretcher,
    file: &[Vec<f32>],
    out_frames: u64,
    p: impl Fn(u64) -> f64,
    step: usize,
) -> Vec<Vec<f32>> {
    let channels = file.len();
    let mut read = read_from(file);
    let mut follower = Follower::new(p, out_frames, st.latency(), step);
    let mut out = vec![Vec::new(); channels];
    let mut fetch = |start: i64, frames: usize| {
        let mut input = vec![vec![0.0f32; frames]; channels];
        let mut refs: Vec<&mut [f32]> = input.iter_mut().map(|c| c.as_mut_slice()).collect();
        read(start, &mut refs).unwrap();
        input
    };
    st.reset();
    let seek = follower.seek();
    let past = fetch(seek.src_start, seek.src_frames);
    st.seek(
        &past.iter().map(|c| c.as_slice()).collect::<Vec<_>>(),
        seek.rate,
    )
    .unwrap();
    while let Some(s) = follower.next_step() {
        let input = fetch(s.src_start, s.src_frames);
        let mut made = vec![vec![0.0f32; s.out_frames]; channels];
        let mut refs: Vec<&mut [f32]> = made.iter_mut().map(|c| c.as_mut_slice()).collect();
        st.process(
            &input.iter().map(|c| c.as_slice()).collect::<Vec<_>>(),
            &mut refs,
        )
        .unwrap();
        for (o, m) in out.iter_mut().zip(&made) {
            o.extend_from_slice(&m[s.skip..]);
        }
    }
    out
}

fn magnitude(x: &[f32], freq: f64) -> f64 {
    let n = x.len();
    let (step_re, step_im) = ((TAU * freq / SR).cos(), (TAU * freq / SR).sin());
    let (mut re, mut im) = (1.0f64, 0.0f64);
    let (mut sum_re, mut sum_im) = (0.0, 0.0);
    for (i, &v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (TAU * i as f64 / n as f64).cos();
        sum_re += w * v as f64 * re;
        sum_im += w * v as f64 * im;
        (re, im) = (re * step_re - im * step_im, re * step_im + im * step_re);
    }
    (sum_re * sum_re + sum_im * sum_im).sqrt() / n as f64
}

/// The strongest frequency between `lo` and `hi`, to about 0.02 Hz, over a Hann window.
pub fn peak_freq(x: &[f32], lo: f64, hi: f64) -> f64 {
    let scan = |from: f64, to: f64, step: f64| {
        let mut best = (from, 0.0);
        let mut f = from;
        while f <= to {
            let m = magnitude(x, f);
            if m > best.1 {
                best = (f, m);
            }
            f += step;
        }
        best.0
    };
    let coarse = scan(lo, hi, 1.0);
    scan(coarse - 1.0, coarse + 1.0, 0.02)
}

/// Energy between two frequencies (a sum over a 5 Hz grid; only for comparing bands).
pub fn band_energy(x: &[f32], lo: f64, hi: f64) -> f64 {
    let mut sum = 0.0;
    let mut f = lo;
    while f < hi {
        let m = magnitude(x, f);
        sum += m * m;
        f += 5.0;
    }
    sum
}

pub fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

/// The frame where a short click sits: the centre of energy of `x` around its largest sample.
pub fn click_at(x: &[f32]) -> f64 {
    let top = (0..x.len())
        .max_by(|&a, &b| x[a].abs().total_cmp(&x[b].abs()))
        .unwrap();
    let (from, to) = (top.saturating_sub(480), (top + 480).min(x.len()));
    let (mut weight, mut moment) = (0.0f64, 0.0f64);
    for (i, &v) in x[from..to].iter().enumerate() {
        let e = v as f64 * v as f64;
        weight += e;
        moment += e * (from + i) as f64;
    }
    moment / weight
}

/// The output frame `j` with `p(j) = target`, for a `p` that only goes up.
pub fn inverse(p: impl Fn(f64) -> f64, target: f64, hi: f64) -> f64 {
    let (mut a, mut b) = (0.0, hi);
    for _ in 0..80 {
        let mid = 0.5 * (a + b);
        if p(mid) < target { a = mid } else { b = mid }
    }
    0.5 * (a + b)
}
