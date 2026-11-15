//! Positions in the song: integer ticks, 480 to a quarter note.

/// Ticks in one quarter note.
pub const TICKS_PER_QUARTER: u64 = 480;

/// What the "tick" of "bar . beat . tick" counts: one beat cut into this many.
pub const DISPLAY_TICKS_PER_BEAT: u64 = 480;

/// Time signature. One beat is a `den`th note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Meter {
    num: u8,
    den: u8,
}

impl Meter {
    pub const FOUR_FOUR: Meter = Meter { num: 4, den: 4 };
    pub const NUM_MAX: u8 = 12;
    pub const DENS: [u8; 4] = [2, 4, 8, 16];

    /// `num` 1 to 12, `den` 2, 4, 8 or 16.
    pub fn new(num: u8, den: u8) -> Option<Meter> {
        ((1..=Self::NUM_MAX).contains(&num) && Self::DENS.contains(&den))
            .then_some(Meter { num, den })
    }

    pub fn num(self) -> u8 {
        self.num
    }

    pub fn den(self) -> u8 {
        self.den
    }

    pub fn ticks_per_beat(self) -> u64 {
        TICKS_PER_QUARTER * 4 / u64::from(self.den)
    }

    pub fn ticks_per_bar(self) -> u64 {
        u64::from(self.num) * self.ticks_per_beat()
    }
}

/// Beats (as the meter counts them) per minute, in steps of 0.01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tempo(u32);

impl Tempo {
    /// 30.00
    pub const MIN_CENTI: u32 = 3_000;
    /// 300.00
    pub const MAX_CENTI: u32 = 30_000;

    /// Hundredths of a beat per minute: 9600 is 96.00.
    pub fn from_centi(centi: u32) -> Option<Tempo> {
        (Self::MIN_CENTI..=Self::MAX_CENTI)
            .contains(&centi)
            .then_some(Tempo(centi))
    }

    /// Rounded to the nearest 0.01.
    pub fn from_bpm(bpm: f64) -> Option<Tempo> {
        let centi = (bpm * 100.0).round();
        if !centi.is_finite() || centi < 0.0 || centi > f64::from(u32::MAX) {
            return None;
        }
        Self::from_centi(centi as u32)
    }

    pub fn centi(self) -> u32 {
        self.0
    }

    pub fn bpm(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

/// `[start, end)`, never empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TickRange {
    start: u64,
    end: u64,
}

impl TickRange {
    pub fn new(start: u64, end: u64) -> Option<TickRange> {
        (end > start).then_some(TickRange { start, end })
    }

    pub fn start(self) -> u64 {
        self.start
    }

    pub fn end(self) -> u64 {
        self.end
    }

    /// Ranges that only touch at an end do not overlap.
    pub fn overlaps(self, other: TickRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    pub fn union(self, other: TickRange) -> TickRange {
        TickRange {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// "bar . beat . tick" as shown. Bar and beat count from 1; tick is 0 to 479.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarBeatTick {
    pub bar: u64,
    pub beat: u32,
    pub tick: u32,
}

/// With one meter for the whole song (meter changes are not used yet).
pub fn bar_beat_tick(tick: u64, meter: Meter) -> BarBeatTick {
    let per_beat = meter.ticks_per_beat();
    let per_bar = meter.ticks_per_bar();
    let in_bar = tick % per_bar;
    BarBeatTick {
        bar: tick / per_bar + 1,
        beat: (in_bar / per_beat) as u32 + 1,
        tick: ((in_bar % per_beat) * DISPLAY_TICKS_PER_BEAT / per_beat) as u32,
    }
}

/// With the tempo and meter at the head of the song (tempo changes are not used yet).
pub fn ticks_to_seconds(tick: u64, tempo: Tempo, meter: Meter) -> f64 {
    tick as f64 / meter.ticks_per_beat() as f64 * 60.0 / tempo.bpm()
}

#[cfg(test)]
mod tests;
