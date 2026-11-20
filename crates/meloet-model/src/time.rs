//! Positions in the song: integer ticks, 1920 to a quarter note.

/// Ticks in one quarter note.
pub const TICKS_PER_QUARTER: u64 = 1920;

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

    /// From one click of the metronome to the next: a dotted quarter in 6/8, 9/8 and 12/8,
    /// a beat in every other meter.
    pub fn ticks_per_click(self) -> u64 {
        if self.den == 8 && matches!(self.num, 6 | 9 | 12) {
            3 * self.ticks_per_beat()
        } else {
            self.ticks_per_beat()
        }
    }

    /// How many times the metronome sounds in one bar.
    pub fn clicks_per_bar(self) -> u32 {
        (self.ticks_per_bar() / self.ticks_per_click()) as u32
    }
}

/// Quarter notes per minute, whatever the meter, in steps of 0.01.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tempo(u32);

impl Tempo {
    /// 30.00
    pub const MIN_CENTI: u32 = 3_000;
    /// 300.00
    pub const MAX_CENTI: u32 = 30_000;

    /// Hundredths of a quarter note per minute: 9600 is 96.00.
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

/// "bar . beat . tick" as shown. Bar and beat count from 1; tick is the ticks into the beat
/// (0 to 1919 when the beat is a quarter note).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarBeatTick {
    pub bar: u64,
    pub beat: u32,
    pub tick: u32,
}

#[cfg(test)]
mod tests;
