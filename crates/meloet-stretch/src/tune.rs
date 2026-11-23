//! 「音程を寄せる」: how far to move the pitch at each moment of a clip so that a voice comes
//! nearer to the notes of a scale. Numbers only: no FFI, no audio.
//!
//! The curve is laid on the clip's own time (one point every 10 ms of what is heard), so its
//! smoothing is the same however the clip is stretched.

use crate::pitch::PitchTrack;

/// Goes into the key of made sounds: another number when the same points and settings may give
/// another curve.
pub const TUNE_VERSION: u32 = 1;

/// Seconds between two points of the curve.
const POINT_SECONDS: f64 = 0.01;
/// The curve is the wanted move under `exp(−|t| / SMOOTH_SECONDS)`: a sudden change of the
/// wanted move is spread over about 50 ms (from 18 % to 82 % of it in 50 ms), half before and
/// half after.
const SMOOTH_SECONDS: f64 = 0.025;
/// Points without a voice this near to a voice move with it (the spectral blocks that make the
/// start and the end of a note reach this far out); further away nothing moves.
const HOLD_POINTS: usize = 6;
/// A move under half a cent is none.
const NOTHING: f32 = 0.005;
/// No scale has two notes further apart than three semitones, so no move is longer than 1.5;
/// this only stops a wrong number from going further.
const MOST_SEMITONES: f32 = 2.0;

/// What a clip is tuned to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tune {
    /// The notes of the scale: `[0]` is C, `[11]` is B.
    pub scale: [bool; 12],
    /// 0.0 (nothing) ..= 1.0 (all the way to the note).
    pub strength: f32,
    /// What the clip is transposed by in the same run, in semitones: the voice is tuned as it
    /// sounds after it.
    pub transpose: f32,
}

impl Tune {
    /// From `note` (a MIDI note number with its fraction) to the nearest note of the scale, in
    /// semitones; the lower one when two are as near. 0.0 for a scale without notes.
    pub fn off_by(&self, note: f32) -> f32 {
        let base = note.floor() as i32;
        let mut best: Option<f32> = None;
        for n in base - 3..=base + 4 {
            if self.scale[n.rem_euclid(12) as usize] {
                let d = n as f32 - note;
                if best.is_none_or(|b| d.abs() < b.abs()) {
                    best = Some(d);
                }
            }
        }
        best.unwrap_or(0.0)
    }
}

/// The move of the pitch over a clip, in semitones, on top of the clip's transpose.
#[derive(Clone, Debug, PartialEq)]
pub struct TuneCurve {
    /// Clip frames from one point to the next.
    step: f64,
    points: Vec<f32>,
}

impl TuneCurve {
    /// The move at `frame` of the clip; before the first point and after the last, theirs.
    pub fn at(&self, frame: f64) -> f32 {
        let Some(&last) = self.points.last() else {
            return 0.0;
        };
        let x = frame / self.step;
        if x <= 0.0 {
            return self.points[0];
        }
        let i = x.floor() as usize;
        if i + 1 >= self.points.len() {
            return last;
        }
        let part = (x - i as f64) as f32;
        self.points[i] + (self.points[i + 1] - self.points[i]) * part
    }

    pub fn points(&self) -> &[f32] {
        &self.points
    }

    /// Whether the curve moves nothing anywhere.
    pub fn is_flat(&self) -> bool {
        self.points.iter().all(|&p| p == 0.0)
    }
}

/// The curve for a clip of `out_frames` frames at `sample_rate` whose frame `j` plays file frame
/// `p(j)` (as for [`crate::render`]; called only with `0..=out_frames`), from the pitch of the
/// file.
///
/// Where a voice sounds, the wanted move is from its pitch (after `tune.transpose`) to the
/// nearest note of the scale, times the strength. Where none sounds it is nothing. The curve is
/// the wanted move smoothed over 50 ms, so a slide from note to note stays a slide and a
/// vibrato keeps part of its depth.
pub fn tune_curve(
    track: &PitchTrack,
    tune: &Tune,
    sample_rate: u32,
    out_frames: u64,
    p: impl Fn(u64) -> f64,
) -> TuneCurve {
    let step = f64::from(sample_rate) * POINT_SECONDS;
    let count = if out_frames == 0 {
        0
    } else {
        (out_frames as f64 / step).ceil() as usize + 1
    };
    let strength = if tune.strength.is_nan() {
        0.0
    } else {
        tune.strength.clamp(0.0, 1.0)
    };
    let wanted: Vec<Option<f32>> = (0..count)
        .map(|k| {
            let frame = ((k as f64 * step).round() as u64).min(out_frames);
            let note = track.note_at(p(frame))? + tune.transpose;
            Some((tune.off_by(note) * strength).clamp(-MOST_SEMITONES, MOST_SEMITONES))
        })
        .collect();

    let mut points = held(&wanted);
    let a = (1.0 - (-POINT_SECONDS / SMOOTH_SECONDS).exp()) as f32;
    let mut y = points.first().copied().unwrap_or(0.0);
    for v in points.iter_mut() {
        y += (*v - y) * a;
        *v = y;
    }
    let mut y = points.last().copied().unwrap_or(0.0);
    for v in points.iter_mut().rev() {
        y += (*v - y) * a;
        *v = y;
    }
    // Far from any voice the curve is nothing, not a very small number: such blocks are then
    // made the same way as in a clip that is not tuned.
    for v in points.iter_mut().filter(|v| v.abs() < NOTHING) {
        *v = 0.0;
    }
    TuneCurve { step, points }
}

/// The wanted moves with each gap of no voice filled: the points within `HOLD_POINTS` of a
/// voice take the move of the nearest voiced point (the earlier one when two are as near), the
/// others 0.
fn held(wanted: &[Option<f32>]) -> Vec<f32> {
    let n = wanted.len();
    let mut out = vec![0.0f32; n];
    // The nearest voiced point at or before each point, then at or after.
    let mut before: Vec<Option<usize>> = vec![None; n];
    let mut last = None;
    for i in 0..n {
        if wanted[i].is_some() {
            last = Some(i);
        }
        before[i] = last;
    }
    let mut next = None;
    for i in (0..n).rev() {
        if wanted[i].is_some() {
            next = Some(i);
        }
        let b = before[i].filter(|&b| i - b <= HOLD_POINTS);
        let a = next.filter(|&a| a - i <= HOLD_POINTS);
        let from = match (b, a) {
            (Some(b), Some(a)) => Some(if i - b <= a - i { b } else { a }),
            (b, a) => b.or(a),
        };
        out[i] = from.and_then(|j| wanted[j]).unwrap_or(0.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const C_MAJOR: [bool; 12] = [
        true, false, true, false, true, true, false, true, false, true, false, true,
    ];

    fn tune(strength: f32) -> Tune {
        Tune {
            scale: C_MAJOR,
            strength,
            transpose: 0.0,
        }
    }

    /// A track at 48 kHz with one point every 480 frames.
    fn track(notes: Vec<f32>) -> PitchTrack {
        PitchTrack {
            first: 0,
            hop: 480,
            notes,
        }
    }

    fn curve(notes: Vec<f32>, t: &Tune) -> TuneCurve {
        let frames = (notes.len() as u64 - 1) * 480;
        tune_curve(&track(notes), t, 48_000, frames, |j| j as f64)
    }

    #[test]
    fn the_nearest_note_of_the_scale() {
        let t = tune(1.0);
        assert_eq!(t.off_by(60.0), 0.0);
        assert!((t.off_by(59.7) - 0.3).abs() < 1e-5);
        assert!((t.off_by(60.3) + 0.3).abs() < 1e-5);
        // C♯ is as far from C as from D: the lower one.
        assert_eq!(t.off_by(61.0), -1.0);
        assert!((t.off_by(61.1) - 0.9).abs() < 1e-5);
        // Below 0 and far up.
        assert!((t.off_by(-0.2) - 0.2).abs() < 1e-5);
        assert!((t.off_by(127.4) + 0.4).abs() < 1e-4);
        // A pentatonic scale has steps of three semitones.
        let mut penta = [false; 12];
        for n in [0, 2, 4, 7, 9] {
            penta[n] = true;
        }
        let p = Tune {
            scale: penta,
            ..tune(1.0)
        };
        assert!((p.off_by(65.4) + 1.4).abs() < 1e-5);
        assert!((p.off_by(65.6) - 1.4).abs() < 1e-5);
        let none = Tune {
            scale: [false; 12],
            ..tune(1.0)
        };
        assert_eq!(none.off_by(60.3), 0.0);
    }

    #[test]
    fn a_steady_note_moves_by_how_far_it_is_off_times_the_strength() {
        let flat = vec![59.7f32; 100];
        for (strength, want) in [(1.0, 0.3), (0.5, 0.15), (0.0, 0.0)] {
            let c = curve(flat.clone(), &tune(strength));
            assert_eq!(c.points().len(), 100);
            for p in c.points() {
                assert!((p - want).abs() < 1e-5, "{p} at {strength}");
            }
        }
        // A note of the scale stays.
        assert!(curve(vec![64.0; 50], &tune(1.0)).is_flat());
        // A note outside it goes to the nearer neighbour.
        let c = curve(vec![63.2; 50], &tune(1.0));
        assert!(c.points().iter().all(|p| (p - 0.8).abs() < 1e-5));
    }

    #[test]
    fn the_transpose_comes_first() {
        // 59.7 up 0.3 is a C already; down 0.5 it is 59.2, nearest to B (59).
        let t = Tune {
            transpose: 0.3,
            ..tune(1.0)
        };
        assert!(
            curve(vec![59.7; 50], &t)
                .points()
                .iter()
                .all(|p| p.abs() < 1e-5)
        );
        let t = Tune {
            transpose: -0.5,
            ..tune(1.0)
        };
        let c = curve(vec![59.7; 50], &t);
        assert!(c.points().iter().all(|p| (p + 0.2).abs() < 1e-4));
    }

    #[test]
    fn no_voice_moves_nothing_and_a_voice_takes_its_edges_with_it() {
        let mut notes = vec![f32::NAN; 100];
        notes[40..60].fill(59.6);
        let c = curve(notes, &tune(1.0));
        let p = c.points();
        // The whole note has the whole move, from its first point to its last.
        for (i, v) in p.iter().enumerate().take(60).skip(40) {
            assert!((v - 0.4).abs() < 0.02, "point {i}: {v}");
        }
        // 200 ms away nothing moves, exactly.
        assert!(p[..20].iter().all(|v| *v == 0.0));
        assert!(p[80..].iter().all(|v| *v == 0.0));
        // In between it comes down without a jump.
        for w in p.windows(2) {
            assert!((w[1] - w[0]).abs() < 0.15);
        }
        assert!(curve(vec![f32::NAN; 80], &tune(1.0)).is_flat());
    }

    #[test]
    fn a_short_gap_inside_a_note_is_bridged() {
        let mut notes = vec![59.7f32; 60];
        notes[30..33].fill(f32::NAN);
        let c = curve(notes, &tune(1.0));
        assert!(c.points().iter().all(|p| (p - 0.3).abs() < 1e-5));
    }

    #[test]
    fn a_slide_between_two_notes_is_not_made_a_step() {
        // From C (60) to D (62) in one second. Where the nearest note changes the wanted move
        // jumps from −1 to +1; the curve takes about 50 ms for most of it.
        let notes: Vec<f32> = (0..=100).map(|i| 60.0 + 2.0 * i as f32 / 100.0).collect();
        let c = curve(notes.clone(), &tune(1.0));
        let heard: Vec<f32> = notes.iter().zip(c.points()).map(|(n, p)| n + p).collect();
        let most = heard
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        // Not tuned it climbs 0.02 a point; a step would be 2.0 in one point.
        assert!(most < 0.75, "{most}");
        assert!(heard.windows(2).all(|w| w[1] >= w[0] - 1e-4));
        // 20 % to 80 % of the way takes 40 ms or more.
        let low = heard.iter().position(|h| *h > 60.4).unwrap();
        let high = heard.iter().position(|h| *h > 61.6).unwrap();
        assert!(high - low >= 4, "{low}..{high}");
        // Away from the change it sits on the notes.
        assert!((heard[20] - 60.0).abs() < 0.05);
        assert!((heard[80] - 62.0).abs() < 0.05);
        // At half strength the slide keeps half of its way everywhere.
        let half = curve(notes.clone(), &tune(0.5));
        for (h, f) in half.points().iter().zip(c.points()) {
            assert!((h - f / 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn a_vibrato_keeps_part_of_its_depth() {
        // ±40 cents at 5.5 Hz around a note 20 cents under E.
        let notes: Vec<f32> = (0..200)
            .map(|i| 63.8 + 0.4 * (2.0 * std::f32::consts::PI * 5.5 * i as f32 * 0.01).sin())
            .collect();
        let c = curve(notes.clone(), &tune(1.0));
        let heard: Vec<f32> = (notes.iter().zip(c.points()))
            .map(|(n, p)| n + p)
            .skip(30)
            .take(140)
            .collect();
        let mean = heard.iter().sum::<f32>() / heard.len() as f32;
        let depth = heard.iter().map(|h| (h - mean).abs()).fold(0.0, f32::max);
        assert!((mean - 64.0).abs() < 0.03, "{mean}");
        assert!((0.1..0.3).contains(&depth), "{depth}");
    }

    #[test]
    fn the_curve_is_laid_on_what_is_heard() {
        // The clip plays the file at half speed from frame 4800: a note change at file frame
        // 14400 is heard at clip frame 19200.
        let mut notes = vec![59.8f32; 30];
        notes.extend(vec![62.3f32; 31]);
        let c = tune_curve(&track(notes), &tune(1.0), 48_000, 48_000, |j| {
            4800.0 + j as f64 / 2.0
        });
        assert_eq!(c.points().len(), 101);
        assert!((c.at(9600.0) - 0.2).abs() < 0.01);
        assert!((c.at(30_000.0) + 0.3).abs() < 0.01);
        assert!((c.at(18_480.0) + 0.05).abs() < 0.05);
        // Between points a straight line; outside the clip the ends.
        let mid = c.at(19_200.0 + 240.0);
        assert!((mid - (c.at(19_200.0) + c.at(19_680.0)) / 2.0).abs() < 1e-5);
        assert_eq!(c.at(-5000.0), c.points()[0]);
        assert_eq!(c.at(1e9), c.points()[100]);
    }

    #[test]
    fn an_empty_clip_and_an_empty_track() {
        let c = tune_curve(&track(Vec::new()), &tune(1.0), 48_000, 0, |_| 0.0);
        assert!(c.points().is_empty());
        assert_eq!(c.at(10.0), 0.0);
        let c = tune_curve(&track(Vec::new()), &tune(1.0), 48_000, 4800, |j| j as f64);
        assert_eq!(c.points().len(), 11);
        assert!(c.is_flat());
    }
}
