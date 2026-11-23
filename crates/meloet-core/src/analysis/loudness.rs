//! The loudness of the master as it is heard, for 「仕上げのメーター」 (round 12 M3), by
//! ITU-R BS.1770-4 through `ebur128` (the crate the export's true peak uses, F1):
//!
//! - the short-term loudness (3 s) of whatever is heard, the song or keys played while stopped;
//! - the integrated loudness (gated) and the true peak of what was heard since the song last
//!   started to play. They are fed only while it plays and hold their values after it stops.
//!
//! Every buffer is made in [`Loudness::new`]; `Mode::HISTOGRAM` keeps the gating blocks as
//! 1000 counters, so a song played for hours takes no more memory than one played for a second.

use ebur128::{EbuR128, Mode};

/// Short-term readings under the absolute gate of BS.1770 are taken as silence.
const QUIETEST_LUFS: f64 = -70.0;

pub struct Loudness {
    /// Fed everything heard.
    now: EbuR128,
    /// Fed while the song plays, from the start of the play.
    play: EbuR128,
    rate: u32,
}

impl Loudness {
    /// `None` at a rate the meters do not take.
    pub fn new(rate: u32) -> Option<Loudness> {
        let now = EbuR128::new(2, rate, Mode::S | Mode::HISTOGRAM).ok()?;
        let play = EbuR128::new(2, rate, Mode::I | Mode::TRUE_PEAK | Mode::HISTOGRAM).ok()?;
        Some(Loudness { now, play, rate })
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// The song started to play: its loudness and true peak are measured anew.
    pub fn start_play(&mut self) {
        self.play.reset();
    }

    /// Nothing is heard any more (the stream stopped): the short-term loudness starts from
    /// silence; what the play measured stays.
    pub fn stop_hearing(&mut self) {
        self.now.reset();
    }

    /// What was heard before does not join what comes next.
    pub fn restart(&mut self) {
        self.now.reset();
        self.play.reset();
    }

    /// Frames in the order they were heard; `playing`: the song plays over them. ebur128 sets
    /// flush-to-zero in MXCSR while it filters and puts it back afterwards; not inlined, so
    /// that no float work of the caller is moved into that stretch.
    #[inline(never)]
    pub fn feed(&mut self, frames: &[[f32; 2]], playing: bool) {
        let samples = frames.as_flattened();
        let _ = self.now.add_frames_f32(samples);
        if playing {
            let _ = self.play.add_frames_f32(samples);
        }
    }

    /// The last 3 s, in LUFS; `None`: silence.
    pub fn short_term(&self) -> Option<f64> {
        let lufs = self.now.loudness_shortterm().ok()?;
        (lufs.is_finite() && lufs > QUIETEST_LUFS).then_some(lufs)
    }

    /// Since the play started, gated, in LUFS; `None`: no block over the absolute gate.
    pub fn integrated(&self) -> Option<f64> {
        let lufs = self.play.loudness_global().ok()?;
        lufs.is_finite().then_some(lufs)
    }

    /// The most since the play started, of the louder side, in dBTP; `None`: silence.
    pub fn true_peak_db(&self) -> Option<f64> {
        let peak = (0..2)
            .filter_map(|c| self.play.true_peak(c).ok())
            .fold(0.0f64, f64::max);
        (peak > 0.0).then(|| 20.0 * peak.log10())
    }
}

#[cfg(test)]
mod tests;
