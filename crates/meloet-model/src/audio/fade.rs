//! The one shape of a vocal clip's fades (spec 3, 27-A: 「線は曲がった形 1 つ（大きさが
//! へこまない形）」).
//!
//! The shape is a quarter of a sine: `sin(x · π/2)` going in, and the same read backwards
//! going out. Of the curves a fade can have it is the one whose two halves keep the power
//! at 1 wherever they meet (`in² + out² = 1`): a clip that fades out while the next one
//! fades in does not get quieter in the middle, as two straight lines would (−3 dB there).
//! It starts steep and lands flat, so a fade-in is heard from its first moment.
//!
//! The engine, the waveform and the line the screen draws all use these functions; the
//! screen's copy is `ui/src/components/arrange/fade.ts`, checked against the table
//! `tests/fade.rs` writes.

use std::f64::consts::FRAC_PI_2;

/// The gain of a fade-in at `x`: 0 at its start (`x` 0), 1 at its end (`x` 1) and after.
/// A fade-out at `x` of its length is `fade_gain(1.0 - x)`.
pub fn fade_gain(x: f64) -> f64 {
    if x >= 1.0 {
        1.0
    } else if x > 0.0 {
        (x * FRAC_PI_2).sin()
    } else {
        // Also what is not a number: silence.
        0.0
    }
}

/// What the two fades of a clip make of a sample `at` from the clip's start. Every number
/// is in the same unit (frames for the engine, ticks or px for the screen); `length` is the
/// clip's. Outside the clip it is 0. The clip's volume is not in here.
pub fn clip_fade_gain(at: f64, length: f64, fade_in: f64, fade_out: f64) -> f64 {
    if !(at >= 0.0 && at < length) {
        return 0.0;
    }
    let going_in = if fade_in > 0.0 {
        fade_gain(at / fade_in)
    } else {
        1.0
    };
    let going_out = if fade_out > 0.0 {
        fade_gain((length - at) / fade_out)
    } else {
        1.0
    };
    going_in * going_out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ends_and_the_way_between() {
        assert_eq!(fade_gain(0.0), 0.0);
        assert_eq!(fade_gain(1.0), 1.0);
        assert_eq!(fade_gain(-3.0), 0.0);
        assert_eq!(fade_gain(7.0), 1.0);
        assert_eq!(fade_gain(f64::NAN), 0.0);
        let mut last = 0.0;
        for i in 1..=1000 {
            let g = fade_gain(f64::from(i) / 1000.0);
            assert!(g > last && g <= 1.0);
            last = g;
        }
        // Above the straight line all the way: it does not sag.
        assert!((1..1000).all(|i| fade_gain(f64::from(i) / 1000.0) > f64::from(i) / 1000.0));
    }

    #[test]
    fn a_fade_out_over_a_fade_in_keeps_the_power() {
        for i in 0..=1000 {
            let x = f64::from(i) / 1000.0;
            let power = fade_gain(x).powi(2) + fade_gain(1.0 - x).powi(2);
            assert!((power - 1.0).abs() < 1e-12, "{x}: {power}");
        }
    }

    #[test]
    fn a_clip_with_both_fades() {
        let g = |at| clip_fade_gain(at, 1000.0, 100.0, 200.0);
        assert_eq!(g(0.0), 0.0);
        assert_eq!(g(100.0), 1.0);
        assert_eq!(g(500.0), 1.0);
        assert_eq!(g(800.0), 1.0);
        assert_eq!(g(900.0), fade_gain(0.5));
        assert!(g(999.0) > 0.0 && g(999.0) < 0.01);
        assert_eq!(g(1000.0), 0.0);
        assert_eq!(g(-1.0), 0.0);
        // No fades: the whole clip as it is.
        assert_eq!(clip_fade_gain(0.0, 10.0, 0.0, 0.0), 1.0);
        assert_eq!(clip_fade_gain(9.0, 10.0, 0.0, 0.0), 1.0);
        // Fades that meet in the middle multiply.
        assert_eq!(
            clip_fade_gain(50.0, 100.0, 100.0, 100.0),
            fade_gain(0.5) * fade_gain(0.5)
        );
    }
}
