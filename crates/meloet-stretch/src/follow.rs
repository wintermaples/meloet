//! Where to read the file while a clip follows a tempo map. Numbers only: no FFI, no audio.
//!
//! The caller gives `p`: output frame `j` of the clip (0 = the clip's first frame) → frame of the
//! file that must sound there, as a fraction. The stretcher works on spectral blocks: a block made
//! while output frame `J` is read has its centre `input latency` frames behind the newest input,
//! and that centre comes out `output latency` frames later. So for the file frame `p(j)` to come
//! out at output frame `j`, the input given before output frame `j` is read must end at
//! `round(p(j + output latency)) + input latency`.
//!
//! That end is always worked out from `p` itself, never by adding rounded block lengths, so no
//! error builds up, whatever the step size.
//!
//! The run starts `block + output latency` frames before the clip and throws that output away, so
//! every block that reaches into the clip's first frames has been made. The file is read there
//! too (frames before the clip's start, or silence before the file's start): a clip cut out of
//! the middle of a sound starts with what really came before, as it ends with what really comes
//! after. Outside `0..=out_frames`, `p` is continued in a straight line with the slope at that end.

/// The sizes of a configured stretcher that the arithmetic needs, in frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Latency {
    pub input: usize,
    pub output: usize,
    pub block: usize,
    /// How much past input the first block can use (`Stretcher::seek_length`).
    pub seek: usize,
}

/// The past input to give before the first step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seek {
    /// First file frame to read; may be negative (silence).
    pub src_start: i64,
    pub src_frames: usize,
    /// Input frames per output frame at the start.
    pub rate: f64,
}

/// One `process` call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// First file frame to read; may be negative or past the end of the file (silence).
    pub src_start: i64,
    pub src_frames: usize,
    pub out_frames: usize,
    /// How many of the first output frames lie before the clip and are thrown away.
    pub skip: usize,
}

pub struct Follower<P> {
    p: P,
    out_frames: u64,
    latency: Latency,
    step: usize,
    head: (f64, f64),
    tail: (f64, f64),
    start: i64,
    /// Clip frame of the next output frame.
    next: i64,
    /// File frame where the input given so far ends.
    fed: i64,
}

/// Frames over which the slope at each end of the clip is taken.
const SLOPE_SPAN: u64 = 64;

impl<P: Fn(u64) -> f64> Follower<P> {
    /// `p` must not go backwards. It is only called with `0..=out_frames`.
    /// `step` is the number of output frames per `process` call; inside one call the file
    /// position moves in a straight line.
    pub fn new(p: P, out_frames: u64, latency: Latency, step: usize) -> Self {
        assert!(step > 0, "follow: step");
        let span = SLOPE_SPAN.min(out_frames);
        let (p0, pn) = (p(0), p(out_frames));
        let slope = |a: f64, b: f64| {
            if span == 0 {
                1.0
            } else {
                ((b - a) / span as f64).max(0.0)
            }
        };
        let head = (p0, slope(p0, p(span)));
        let tail = (pn, slope(p(out_frames - span), pn));
        let start = if out_frames == 0 {
            0
        } else {
            -((latency.block + latency.output) as i64)
        };
        let mut this = Follower {
            p,
            out_frames,
            latency,
            step,
            head,
            tail,
            start,
            next: start,
            fed: 0,
        };
        this.fed = this.fed_before(start);
        this
    }

    /// The file frame for clip frame `j`, also outside the clip.
    pub fn source_at(&self, j: i64) -> f64 {
        if j < 0 {
            self.head.0 + j as f64 * self.head.1
        } else if j as u64 > self.out_frames {
            self.tail.0 + (j as u64 - self.out_frames) as f64 * self.tail.1
        } else {
            (self.p)(j as u64)
        }
    }

    fn fed_before(&self, j: i64) -> i64 {
        self.source_at(j + self.latency.output as i64).round() as i64 + self.latency.input as i64
    }

    /// The past input for `Stretcher::seek`, before the first [`Follower::next_step`].
    pub fn seek(&self) -> Seek {
        let end = self.fed_before(self.start);
        Seek {
            src_start: end - self.latency.seek as i64,
            src_frames: self.latency.seek,
            rate: self.head.1,
        }
    }

    pub fn next_step(&mut self) -> Option<Step> {
        let end = self.out_frames as i64;
        if self.next >= end {
            return None;
        }
        let to = (self.next + self.step as i64).min(end);
        let out_frames = (to - self.next) as usize;
        let fed = self.fed_before(to).max(self.fed);
        let step = Step {
            src_start: self.fed,
            src_frames: (fed - self.fed) as usize,
            out_frames,
            skip: (-self.next).clamp(0, out_frames as i64) as usize,
        };
        self.next = to;
        self.fed = fed;
        Some(step)
    }

    /// 0.0 at the start, 1.0 when every step has been handed out.
    pub fn done(&self) -> f64 {
        let all = self.out_frames as i64 - self.start;
        if all == 0 {
            1.0
        } else {
            (self.next - self.start) as f64 / all as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAT: Latency = Latency {
        input: 2880,
        output: 2880,
        block: 5760,
        seek: 7200,
    };

    fn run(p: impl Fn(u64) -> f64, n: u64, step: usize) -> (Seek, Vec<Step>) {
        let mut f = Follower::new(p, n, LAT, step);
        let seek = f.seek();
        let mut steps = Vec::new();
        while let Some(s) = f.next_step() {
            steps.push(s);
        }
        assert_eq!(f.done(), 1.0);
        (seek, steps)
    }

    /// A ramp: the rate goes from 0.8 to 1.25 over the clip.
    fn ramp(n: u64) -> impl Fn(u64) -> f64 {
        move |j| {
            let x = j as f64;
            100.0 + 0.8 * x + 0.45 * x * x / (2.0 * n as f64)
        }
    }

    #[test]
    fn the_steps_join_and_the_output_is_the_clip() {
        for step in [1, 64, 256, 1000, 100_000] {
            let n = 48_000;
            let (seek, steps) = run(ramp(n), n, step);
            let mut at = seek.src_start + seek.src_frames as i64;
            let mut kept = 0u64;
            let mut made = 0i64;
            for s in &steps {
                assert_eq!(s.src_start, at, "step {step}");
                at += s.src_frames as i64;
                kept += (s.out_frames - s.skip) as u64;
                made += s.out_frames as i64;
            }
            assert_eq!(kept, n);
            assert_eq!(made, n as i64 + (LAT.block + LAT.output) as i64);
        }
    }

    #[test]
    fn the_input_position_does_not_depend_on_the_step_size() {
        let n = 96_000u64;
        let p = ramp(n);
        for step in [1usize, 7, 64, 256, 4096] {
            let mut f = Follower::new(&p, n, LAT, step);
            let mut at = f.seek().src_start + LAT.seek as i64;
            let mut j = -((LAT.block + LAT.output) as i64);
            while let Some(s) = f.next_step() {
                at += s.src_frames as i64;
                j += s.out_frames as i64;
                // Not a sum of rounded lengths: the position after any number of steps is the formula.
                assert_eq!(
                    at,
                    f.source_at(j + LAT.output as i64).round() as i64 + LAT.input as i64,
                    "step {step} at {j}"
                );
            }
            assert_eq!(j, n as i64);
        }
    }

    #[test]
    fn outside_the_clip_the_line_goes_on_straight() {
        let n = 48_000u64;
        let f = Follower::new(ramp(n), n, LAT, 256);
        assert!(
            (f.source_at(-1000) - (100.0 - 1000.0 * (f.source_at(64) - 100.0) / 64.0)).abs() < 1e-6
        );
        let end = f.source_at(n as i64);
        let slope = (end - f.source_at(n as i64 - 64)) / 64.0;
        assert!((slope - 1.25).abs() < 0.001);
        assert!((f.source_at(n as i64 + 500) - (end + 500.0 * slope)).abs() < 1e-6);
    }

    #[test]
    fn rate_one_reads_the_file_frame_for_frame() {
        let (seek, steps) = run(|j| 5000.0 + j as f64, 10_000, 256);
        assert_eq!(seek.rate, 1.0);
        // Before the first output frame (clip frame -8640) the input ends at 5000 - 8640 + 2880 + 2880.
        assert_eq!(seek.src_start + seek.src_frames as i64, 5000 - 2880);
        assert!(steps.iter().all(|s| s.src_frames == s.out_frames));
    }

    #[test]
    fn an_empty_clip_has_no_steps_and_a_standing_p_reads_nothing() {
        assert!(run(|_| 0.0, 0, 256).1.is_empty());
        let (_, steps) = run(|_| 777.0, 1000, 256);
        assert!(steps.iter().all(|s| s.src_frames == 0));
    }
}
