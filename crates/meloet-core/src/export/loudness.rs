//! F1 「音の大きさ」: the true peak of what an export writes, and the one gain that brings a
//! file to the loudness the publishing sites play at (spec 3, round 12; docs/architecture.md 25).
//! The loudness itself is [`super::analysis::Listener`]'s.

use ebur128::{EbuR128, Mode};
use meloet_protocol::{
    EXPORT_MATCH_LUFS, EXPORT_MATCH_QUIETEST_LUFS, EXPORT_MATCH_TRUE_PEAK_DB, ExportLoudnessResult,
};

/// The true peak of interleaved stereo, as ITU-R BS.1770-4 Annex 2 finds it (4× oversampled
/// below 96 kHz), of the louder channel.
pub(crate) struct TruePeak {
    meter: EbuR128,
}

impl TruePeak {
    /// `None` at a rate the meter does not take.
    pub fn new(rate: u32) -> Option<TruePeak> {
        let meter = EbuR128::new(2, rate, Mode::TRUE_PEAK).ok()?;
        Some(TruePeak { meter })
    }

    /// ebur128 sets flush-to-zero in MXCSR while it filters and puts it back afterwards. Not
    /// inlined, so that no float work of the export's engine can be moved into that stretch.
    #[inline(never)]
    pub fn feed(&mut self, samples: &[f32]) {
        let samples = &samples[..samples.len() & !1];
        if samples.iter().all(|s| s.is_finite()) {
            let _ = self.meter.add_frames_f32(samples);
        } else {
            // One bad sample would stay in the filter for good.
            let finite: Vec<f32> = (samples.iter())
                .map(|s| if s.is_finite() { *s } else { 0.0 })
                .collect();
            let _ = self.meter.add_frames_f32(&finite);
        }
    }

    /// In dBTP; `None`: silence.
    pub fn db(&self) -> Option<f32> {
        let peak = (0..2)
            .filter_map(|c| self.meter.true_peak(c).ok())
            .fold(0.0f64, f64::max);
        (peak > 0.0).then(|| (20.0 * peak.log10()) as f32)
    }
}

/// What 「公開先に合わせる」 does to a file of this loudness (LUFS, the range without the tail)
/// and true peak (dBTP, the tail counted): the gain to [`EXPORT_MATCH_LUFS`], or less, so
/// that the true peak stays at [`EXPORT_MATCH_TRUE_PEAK_DB`]; it turns down too. `None`: the
/// loudness is not measured, or is below [`EXPORT_MATCH_QUIETEST_LUFS`].
pub(crate) fn match_gain(
    lufs: Option<f32>,
    true_peak_db: Option<f32>,
) -> Option<ExportLoudnessResult> {
    let lufs = lufs.filter(|l| l.is_finite() && *l >= EXPORT_MATCH_QUIETEST_LUFS)?;
    let wanted = EXPORT_MATCH_LUFS - lufs;
    let room = (true_peak_db.filter(|p| p.is_finite()))
        .map_or(f32::INFINITY, |p| EXPORT_MATCH_TRUE_PEAK_DB - p);
    let gain_db = wanted.min(room);
    Some(ExportLoudnessResult {
        gain_db,
        limited: wanted > room,
        lufs: lufs + gain_db,
    })
}

/// The factor of a gain in dB.
pub(crate) fn factor(gain_db: f32) -> f32 {
    10f64.powf(f64::from(gain_db) / 20.0) as f32
}

#[cfg(test)]
mod tests;
