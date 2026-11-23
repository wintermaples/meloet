//! 「音が大きすぎます」 (round 12, M2): finding, from the master's readings the telemetry thread
//! takes about 60 times a second, that the song is too loud while it plays.
//!
//! - With a limiter on the master that is not passed by: for half of the last 4 s or more of
//!   the play, it held the sound down by 3 dB or more. The amount is the mean over that time.
//! - Without one: the master went over 0 dBFS in 3 readings or more of the last 4 s. The
//!   amount is the largest overshoot.
//!
//! Once a play at most. Not looked at while stopped or counting in, nor while the test tone or
//! the click of the latency measurement sounds.

use std::collections::VecDeque;
use std::time::Duration;

use meloet_protocol::StatusMessage;

/// The stretch of the play that is looked at.
pub const WINDOW: Duration = Duration::from_secs(4);
/// What a limiter holds the sound down by, at least, to count.
pub const HELD_DB: f32 = 3.0;
/// Readings over 0 dBFS in the window that are too many.
pub const OVERS: usize = 3;
/// The most the button 「全部のトラックを N dB 下げる」 lowers by.
pub const LOWER_MAX_DB: u8 = 12;
/// A reading after a longer gap (the PC was stopped) counts for this much time only.
const LONGEST_GAP: Duration = Duration::from_millis(100);

/// What the telemetry thread read once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// The time since any fixed moment.
    pub at: Duration,
    /// The song plays or records (not stopped, not counting in).
    pub playing: bool,
    /// The engine ran a block since the reading before: without one the meters say nothing.
    pub fresh: bool,
    /// The test tone or the latency click sounded since the reading before.
    pub aside: bool,
    /// The master has a limiter that is not passed by.
    pub limiter: bool,
    /// The most the master's limiter held the sound down since the reading before, in dB.
    pub reduction_db: f32,
    /// The master's peak since the reading before (1.0 = 0 dBFS).
    pub peak: f32,
}

/// What was found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Found {
    pub by_db: f32,
    pub limiter: bool,
}

#[derive(Debug, Clone, Copy)]
struct Kept {
    at: Duration,
    /// The time this reading stands for.
    span: Duration,
    reduction_db: f32,
    /// How far over 0 dBFS, in dB; `None`: not over.
    over_db: Option<f32>,
}

#[derive(Debug, Default)]
pub struct Watch {
    kept: VecDeque<Kept>,
    playing: bool,
    /// Found in this play already.
    found: bool,
    /// The reading before stands for the time up to here.
    last: Option<Duration>,
    /// The reading after one with the test tone is not looked at either.
    skip_next: bool,
}

impl Watch {
    pub fn read(&mut self, r: Reading) -> Option<Found> {
        if !r.playing {
            self.playing = false;
            self.kept.clear();
            self.last = None;
            return None;
        }
        if !self.playing {
            self.playing = true;
            self.found = false;
            self.skip_next = false;
            self.last = Some(r.at);
            return None;
        }
        if !r.fresh {
            return None;
        }
        let span =
            r.at.saturating_sub(self.last.unwrap_or(r.at))
                .min(LONGEST_GAP);
        self.last = Some(r.at);
        let skip = r.aside || self.skip_next;
        self.skip_next = r.aside;
        if self.found || skip {
            return None;
        }
        let over_db = (r.peak > 1.0).then(|| 20.0 * r.peak.log10());
        let reduction_db = if r.reduction_db.is_finite() {
            r.reduction_db.max(0.0)
        } else {
            0.0
        };
        self.kept.push_back(Kept {
            at: r.at,
            span,
            reduction_db,
            over_db,
        });
        while (self.kept.front()).is_some_and(|k| r.at.saturating_sub(k.at) >= WINDOW) {
            self.kept.pop_front();
        }
        let found = if r.limiter { self.held() } else { self.overs() };
        let found = found.map(|by_db| Found {
            by_db,
            limiter: r.limiter,
        });
        self.found = found.is_some();
        found
    }

    /// The mean of what the limiter held down while it held 3 dB or more, when that was half
    /// of the window or more.
    fn held(&self) -> Option<f32> {
        let (mut time, mut sum) = (0.0f64, 0.0f64);
        for k in self.kept.iter().filter(|k| k.reduction_db >= HELD_DB) {
            let s = k.span.as_secs_f64();
            time += s;
            sum += s * f64::from(k.reduction_db);
        }
        (time >= WINDOW.as_secs_f64() / 2.0 && time > 0.0).then(|| (sum / time) as f32)
    }

    fn overs(&self) -> Option<f32> {
        let overs = self.kept.iter().filter_map(|k| k.over_db);
        (overs.clone().count() >= OVERS).then(|| overs.fold(0.0, f32::max))
    }
}

/// The notice for what was found: the amount to a tenth of a dB, and the button's amount
/// rounded up from it into 1..=12.
pub fn notice(by_db: f32, limiter: bool) -> StatusMessage {
    let by_db = if by_db.is_finite() {
        (by_db.max(0.0) * 10.0).round() / 10.0
    } else {
        0.0
    };
    let lower_db = (by_db.ceil() as u8).clamp(1, LOWER_MAX_DB);
    StatusMessage::TooLoud {
        by_db,
        lower_db,
        limiter,
    }
}

#[cfg(test)]
mod tests;
