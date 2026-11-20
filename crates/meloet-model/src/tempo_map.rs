//! Tempo and meter over the song, and the one place that turns ticks into seconds and back.
//!
//! A tempo is beats per minute, where a beat is the lower number of the meter in force there:
//! ticks per minute = tempo × ticks per beat. A ramp between two tempo points is a straight
//! line over ticks. Before tick 0 the tempo and meter of the head go on backwards, without a
//! ramp.
//!
//! Nothing in [`TempoMap`] allocates after [`TempoMap::new`]; the audio thread uses it.

use crate::time::{BarBeatTick, DISPLAY_TICKS_PER_BEAT, Meter, TICKS_PER_QUARTER, Tempo};

/// A tempo from `tick` on. With `ramp`, the tempo goes in a straight line to the next point;
/// on the last point `ramp` does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempoPoint {
    pub tick: u64,
    pub tempo: Tempo,
    pub ramp: bool,
}

/// A meter from the head of bar `bar` on (bars count from 0 here; the song's head is bar 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterChange {
    pub bar: u32,
    pub meter: Meter,
}

/// What the song keeps. The head's tempo and meter are apart from the changes.
///
/// Well formed ([`Timeline::check`]): `tempo_points` in the order of `tick`, each tick 1 or
/// more and only once; `meter_changes` in the order of `bar`, each bar 1 or more and only once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    pub tempo: Tempo,
    pub meter: Meter,
    /// The tempo goes in a straight line from the head to the first point.
    pub head_ramp: bool,
    pub tempo_points: Vec<TempoPoint>,
    pub meter_changes: Vec<MeterChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineError {
    /// A tempo point at tick 0: that is `Timeline::tempo`.
    TempoPointAtHead,
    /// Tempo points out of order, or two at one tick.
    TempoPointsOrder,
    /// A meter change at bar 0: that is `Timeline::meter`.
    MeterChangeAtHead,
    /// Meter changes out of order, or two at one bar.
    MeterChangesOrder,
}

impl Timeline {
    /// One tempo and one meter for the whole song.
    pub fn new(tempo: Tempo, meter: Meter) -> Timeline {
        Timeline {
            tempo,
            meter,
            head_ramp: false,
            tempo_points: Vec::new(),
            meter_changes: Vec::new(),
        }
    }

    pub fn check(&self) -> Result<(), TimelineError> {
        if self.tempo_points.first().is_some_and(|p| p.tick == 0) {
            return Err(TimelineError::TempoPointAtHead);
        }
        if self.tempo_points.windows(2).any(|w| w[0].tick >= w[1].tick) {
            return Err(TimelineError::TempoPointsOrder);
        }
        if self.meter_changes.first().is_some_and(|c| c.bar == 0) {
            return Err(TimelineError::MeterChangeAtHead);
        }
        if self.meter_changes.windows(2).any(|w| w[0].bar >= w[1].bar) {
            return Err(TimelineError::MeterChangesOrder);
        }
        Ok(())
    }
}

/// The meter at a tick, and the bar that tick is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterAt {
    pub meter: Meter,
    /// The bar's number, from 0.
    pub bar: u64,
    /// The tick of the bar's head.
    pub bar_tick: u64,
}

/// The head of a beat. Negative before the song's head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Beat {
    pub tick: i64,
    /// The first beat of a bar.
    pub downbeat: bool,
}

/// A stretch with one meter and one tempo line. `u` below is ticks from `tick0`.
#[derive(Debug, Clone, Copy)]
struct Piece {
    tick0: f64,
    sec0: f64,
    /// Ticks per beat.
    tpb: f64,
    /// Tempo at `tick0`.
    a: f64,
    /// Tempo gained per tick; 0 when the tempo stays.
    s: f64,
}

/// `ln(1 + x) / x`, which goes to 1 at 0 without a step.
fn ln_1p_over(x: f64) -> f64 {
    if x == 0.0 { 1.0 } else { x.ln_1p() / x }
}

/// `(exp(y) − 1) / y`, which goes to 1 at 0 without a step.
fn exp_m1_over(y: f64) -> f64 {
    if y == 0.0 { 1.0 } else { y.exp_m1() / y }
}

fn seconds_of(u: f64, tpb: f64, a: f64, s: f64) -> f64 {
    u * 60.0 / (tpb * a) * ln_1p_over(s * u / a)
}

fn ticks_of(seconds: f64, tpb: f64, a: f64, s: f64) -> f64 {
    let minutes = seconds / 60.0;
    a * tpb * minutes * exp_m1_over(s * tpb * minutes)
}

impl Piece {
    fn seconds(&self, u: f64) -> f64 {
        seconds_of(u, self.tpb, self.a, self.s)
    }

    fn ticks(&self, seconds: f64) -> f64 {
        ticks_of(seconds, self.tpb, self.a, self.s)
    }
}

/// A stretch of whole bars with one meter.
#[derive(Debug, Clone, Copy)]
struct Region {
    tick0: u64,
    bar0: u64,
    meter: Meter,
}

/// Made from a [`Timeline`], never changed.
#[derive(Debug, Clone)]
pub struct TempoMap {
    /// Never empty; the first starts at tick 0 and second 0.
    pieces: Vec<Piece>,
    /// Never empty; the first is the head's meter at tick 0.
    regions: Vec<Region>,
}

impl TempoMap {
    /// A timeline that is not well formed still gives a map, the same one each time: points
    /// and changes are put in order, of two at one place the later in the list stays, and a
    /// point at tick 0 or a change at bar 0 is left out (the head has its own fields).
    pub fn new(timeline: &Timeline) -> TempoMap {
        let mut regions = vec![Region {
            tick0: 0,
            bar0: 0,
            meter: timeline.meter,
        }];
        let mut changes: Vec<MeterChange> = timeline
            .meter_changes
            .iter()
            .copied()
            .filter(|c| c.bar > 0)
            .collect();
        changes.sort_by_key(|c| c.bar);
        for (i, change) in changes.iter().enumerate() {
            if changes
                .get(i + 1)
                .is_some_and(|next| next.bar == change.bar)
            {
                continue;
            }
            let last = regions[regions.len() - 1];
            let bar0 = u64::from(change.bar);
            regions.push(Region {
                tick0: last.tick0 + (bar0 - last.bar0) * last.meter.ticks_per_bar(),
                bar0,
                meter: change.meter,
            });
        }

        let mut points = vec![TempoPoint {
            tick: 0,
            tempo: timeline.tempo,
            ramp: timeline.head_ramp,
        }];
        let mut rest: Vec<TempoPoint> = timeline
            .tempo_points
            .iter()
            .copied()
            .filter(|p| p.tick > 0)
            .collect();
        rest.sort_by_key(|p| p.tick);
        for (i, point) in rest.iter().enumerate() {
            if rest.get(i + 1).is_none_or(|next| next.tick != point.tick) {
                points.push(*point);
            }
        }

        let mut cuts: Vec<u64> = points
            .iter()
            .map(|p| p.tick)
            .chain(regions.iter().map(|r| r.tick0))
            .collect();
        cuts.sort_unstable();
        cuts.dedup();

        let mut pieces: Vec<Piece> = Vec::with_capacity(cuts.len());
        let (mut pi, mut ri) = (0, 0);
        let mut sec0 = 0.0;
        for (i, &cut) in cuts.iter().enumerate() {
            while points.get(pi + 1).is_some_and(|p| p.tick <= cut) {
                pi += 1;
            }
            while regions.get(ri + 1).is_some_and(|r| r.tick0 <= cut) {
                ri += 1;
            }
            let point = points[pi];
            let s = match points.get(pi + 1) {
                Some(next) if point.ramp => {
                    (next.tempo.bpm() - point.tempo.bpm()) / (next.tick - point.tick) as f64
                }
                _ => 0.0,
            };
            let piece = Piece {
                tick0: cut as f64,
                sec0,
                tpb: regions[ri].meter.ticks_per_beat() as f64,
                a: point.tempo.bpm() + s * (cut - point.tick) as f64,
                s,
            };
            if let Some(&next) = cuts.get(i + 1) {
                sec0 += piece.seconds((next - cut) as f64);
            }
            pieces.push(piece);
        }
        TempoMap { pieces, regions }
    }

    /// How many stretches the map is cut into (for measuring).
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    fn piece_at_tick(&self, tick: f64) -> &Piece {
        let i = self.pieces.partition_point(|p| p.tick0 <= tick);
        &self.pieces[i.max(1) - 1]
    }

    fn region_at_tick(&self, tick: u64) -> &Region {
        let i = self.regions.partition_point(|r| r.tick0 <= tick);
        &self.regions[i.max(1) - 1]
    }

    /// Seconds from the song's head to `tick`. Negative before the head.
    pub fn seconds_at(&self, tick: f64) -> f64 {
        if tick < 0.0 {
            let head = &self.pieces[0];
            return seconds_of(tick, head.tpb, head.a, 0.0);
        }
        let p = self.piece_at_tick(tick);
        p.sec0 + p.seconds(tick - p.tick0)
    }

    /// The tick `seconds` after the song's head.
    pub fn tick_at(&self, seconds: f64) -> f64 {
        if seconds < 0.0 {
            let head = &self.pieces[0];
            return ticks_of(seconds, head.tpb, head.a, 0.0);
        }
        let i = self.pieces.partition_point(|p| p.sec0 <= seconds);
        let p = &self.pieces[i.max(1) - 1];
        p.tick0 + p.ticks(seconds - p.sec0)
    }

    /// The tempo as the transport shows it: beats of the meter there, per minute.
    pub fn tempo_at(&self, tick: f64) -> f64 {
        if tick < 0.0 {
            return self.pieces[0].a;
        }
        let p = self.piece_at_tick(tick);
        p.a + p.s * (tick - p.tick0)
    }

    /// Quarter notes per minute, for plugins and the delay.
    pub fn quarter_bpm_at(&self, tick: f64) -> f64 {
        let tpb = if tick < 0.0 {
            self.pieces[0].tpb
        } else {
            self.piece_at_tick(tick).tpb
        };
        self.tempo_at(tick) * tpb / TICKS_PER_QUARTER as f64
    }

    pub fn meter_at(&self, tick: u64) -> MeterAt {
        let r = self.region_at_tick(tick);
        let per_bar = r.meter.ticks_per_bar();
        let bars = (tick - r.tick0) / per_bar;
        MeterAt {
            meter: r.meter,
            bar: r.bar0 + bars,
            bar_tick: r.tick0 + bars * per_bar,
        }
    }

    /// The tick of the head of bar `bar` (from 0).
    pub fn tick_of_bar(&self, bar: u64) -> u64 {
        let i = self.regions.partition_point(|r| r.bar0 <= bar);
        let r = &self.regions[i.max(1) - 1];
        (bar - r.bar0)
            .saturating_mul(r.meter.ticks_per_bar())
            .saturating_add(r.tick0)
    }

    /// "bar . beat . tick" as shown: bar and beat from 1, the tick a 480th of a beat.
    pub fn bar_beat_tick(&self, tick: u64) -> BarBeatTick {
        let r = self.region_at_tick(tick);
        let per_beat = r.meter.ticks_per_beat();
        let per_bar = r.meter.ticks_per_bar();
        let from = tick - r.tick0;
        let in_bar = from % per_bar;
        BarBeatTick {
            bar: r.bar0 + from / per_bar + 1,
            beat: (in_bar / per_beat) as u32 + 1,
            tick: ((in_bar % per_beat) * DISPLAY_TICKS_PER_BEAT / per_beat) as u32,
        }
    }

    /// The head of the beat nearest to `tick`; of two as near, the later one.
    pub fn nearest_beat(&self, tick: u64) -> u64 {
        let r = self.region_at_tick(tick);
        let per_beat = r.meter.ticks_per_beat();
        // A region is whole bars, so a beat found at its end is the next region's first.
        let beats = (tick - r.tick0).saturating_add(per_beat / 2) / per_beat;
        r.tick0.saturating_add(beats.saturating_mul(per_beat))
    }

    /// The first head of a bar at `tick` or after it.
    pub fn bar_at_or_after(&self, tick: u64) -> u64 {
        let at = self.meter_at(tick);
        if at.bar_tick == tick {
            tick
        } else {
            self.tick_of_bar(at.bar + 1)
        }
    }

    /// The stretch of one meter `tick` is in: where it starts, its meter, and where the next
    /// one starts.
    pub fn meter_stretch(&self, tick: u64) -> (u64, Meter, Option<u64>) {
        let i = self.regions.partition_point(|r| r.tick0 <= tick).max(1) - 1;
        let r = &self.regions[i];
        (r.tick0, r.meter, self.regions.get(i + 1).map(|n| n.tick0))
    }

    /// The first head of a beat at `tick` or after it. Before the song's head, the beats of
    /// the head's meter go on backwards.
    pub fn next_beat(&self, tick: f64) -> Beat {
        let i = self.regions.partition_point(|r| r.tick0 as f64 <= tick);
        let r = &self.regions[i.max(1) - 1];
        let per_beat = r.meter.ticks_per_beat() as i64;
        // A region is whole bars, so a beat found past its end is the next region's first.
        let beats = ((tick - r.tick0 as f64) / per_beat as f64).ceil() as i64;
        Beat {
            tick: (r.tick0 as i64).saturating_add(beats.saturating_mul(per_beat)),
            downbeat: beats.rem_euclid(i64::from(r.meter.num())) == 0,
        }
    }

    /// The first tick after `tick` where a tempo point or a meter change is (where what a
    /// listener is told of tempo and meter may jump; inside a ramp there is none).
    pub fn next_change(&self, tick: f64) -> Option<u64> {
        let i = self.pieces.partition_point(|p| p.tick0 <= tick);
        self.pieces.get(i).map(|p| p.tick0 as u64)
    }

    /// Counting frames from `from_seconds`, the first frame at `tick` or past it.
    pub fn frame_of(&self, tick: f64, from_seconds: f64, sample_rate: f64) -> u64 {
        // A position that is exactly on a frame must not come out one frame late for the last
        // bits of the f64.
        let frames = (self.seconds_at(tick) - from_seconds) * sample_rate - 1e-6;
        frames.ceil().max(0.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slope_near_zero_joins_the_straight_line() {
        let (tpb, a) = (480.0, 104.0);
        for u in [1.0, 480.0, 7680.0, 1.0e6] {
            let flat = seconds_of(u, tpb, a, 0.0);
            let mut exp = -3;
            while exp >= -300 {
                for sign in [1.0, -1.0] {
                    let s = sign * 10f64.powi(exp);
                    let x = s * u / a;
                    if x.abs() > 0.5 {
                        continue;
                    }
                    let want = flat * (1.0 - x / 2.0 + x * x / 3.0 - x * x * x / 4.0);
                    let got = seconds_of(u, tpb, a, s);
                    assert!(
                        (got - want).abs() <= flat * (x.powi(4).abs() + 4.0 * f64::EPSILON),
                        "u {u} s {s}: {got} {want}"
                    );
                    let back = ticks_of(got, tpb, a, s);
                    assert!(
                        (back - u).abs() <= u * 8.0 * f64::EPSILON,
                        "u {u} s {s}: {back}"
                    );
                }
                exp -= 1;
            }
        }
        // Too small to change the last bit: the very same number as the straight line.
        assert_eq!(
            seconds_of(7680.0, tpb, a, 1e-300),
            seconds_of(7680.0, tpb, a, 0.0)
        );
        assert_eq!(ticks_of(8.5, tpb, a, 1e-300), ticks_of(8.5, tpb, a, 0.0));
        assert_eq!(
            seconds_of(7680.0, tpb, a, 5e-324),
            seconds_of(7680.0, tpb, a, 0.0)
        );
    }

    #[test]
    fn a_broken_timeline_still_gives_one_map() {
        let t = |c| Tempo::from_centi(c).unwrap();
        let point = |tick, c| TempoPoint {
            tick,
            tempo: t(c),
            ramp: false,
        };
        let m = |n, d| Meter::new(n, d).unwrap();
        let broken = Timeline {
            tempo: t(9600),
            meter: Meter::FOUR_FOUR,
            head_ramp: false,
            tempo_points: vec![
                point(1920, 12000),
                point(0, 20000),
                point(960, 10000),
                point(1920, 11000),
            ],
            meter_changes: vec![
                MeterChange {
                    bar: 2,
                    meter: m(3, 4),
                },
                MeterChange {
                    bar: 0,
                    meter: m(7, 8),
                },
                MeterChange {
                    bar: 2,
                    meter: m(6, 8),
                },
            ],
        };
        assert_eq!(broken.check(), Err(TimelineError::TempoPointsOrder));
        let fixed = Timeline {
            tempo_points: vec![point(960, 10000), point(1920, 11000)],
            meter_changes: vec![MeterChange {
                bar: 2,
                meter: m(6, 8),
            }],
            ..broken.clone()
        };
        assert_eq!(fixed.check(), Ok(()));
        let (a, b) = (TempoMap::new(&broken), TempoMap::new(&fixed));
        assert_eq!(a.piece_count(), b.piece_count());
        for tick in [0u64, 500, 960, 1919, 1920, 3840, 5000, 100_000] {
            assert_eq!(a.seconds_at(tick as f64), b.seconds_at(tick as f64));
            assert_eq!(a.tempo_at(tick as f64), b.tempo_at(tick as f64));
            assert_eq!(a.meter_at(tick), b.meter_at(tick));
        }
        assert_eq!(a.tempo_at(0.0), 96.0);
        assert_eq!(a.meter_at(0).meter, Meter::FOUR_FOUR);
    }
}
