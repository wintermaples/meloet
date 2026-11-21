//! Where in its file a vocal clip is at a tick of the song (docs/architecture.md 22-8 and
//! 22-19).
//!
//! The take's tick 0 is at `clip.start + offset` in the song, and there the file is at
//! `first_frame`. From there on:
//!
//! - a clip that does not follow the tempo plays at the file's own speed: the seconds the
//!   song takes from the take's tick 0 to the tick, times the sample rate;
//! - a clip that follows it is stuck to the song's ticks: the tick of the song is a tick of
//!   the take, and the take's own tempo ([`SourceTempo`]) says how many seconds of the file
//!   that is. The song's tempo is not in it at all, which is why such a clip stretches when
//!   the song's tempo changes.
//!
//! A clip follows only when it is asked to (`AudioShape::follow`) and its take has a tempo.
//!
//! The one who makes the stretched audio, the engine's table of clips and the waveform all
//! use [`FileClock`]; the screen's copy is `ui/src/components/arrange/follow.ts`, checked
//! against the table `tests/follow.rs` writes.

use super::SourceTempo;
use crate::song::Clip;
use crate::tempo_map::{TempoMap, TempoPoint, Timeline};
use crate::time::{Meter, Tempo};

impl SourceTempo {
    /// One tempo all along: what a typed or found 「この音のテンポ」 makes.
    pub fn steady(tempo: Tempo) -> SourceTempo {
        SourceTempo {
            tempo,
            head_ramp: false,
            points: Vec::new(),
        }
    }

    /// Ticks of the take to seconds of the audio, as a map from the take's tick 0. Before
    /// it the head's tempo goes on backwards.
    pub fn map(&self) -> TempoMap {
        TempoMap::new(&Timeline {
            tempo: self.tempo,
            // A tempo is quarter notes a minute whatever the meter; nothing reads this one.
            meter: Meter::FOUR_FOUR,
            head_ramp: self.head_ramp,
            tempo_points: self.points.clone(),
            meter_changes: Vec::new(),
        })
    }
}

/// A tempo from a tick on, and whether it bends towards the next one.
type Node = (u64, Tempo, bool);

impl SourceTempo {
    fn nodes(&self) -> impl Iterator<Item = Node> + '_ {
        let last = self.points.len();
        std::iter::once((0, self.tempo, self.head_ramp && last > 0)).chain(
            (self.points.iter().enumerate())
                .map(move |(i, p)| (p.tick, p.tempo, p.ramp && i + 1 < last)),
        )
    }

    /// What of it says how the first `end` ticks of the take go, written one way: points
    /// that change nothing are left out, and so is everything past `end` but the point a
    /// ramp over `end` bends towards. Two takes whose first `end` ticks take the same
    /// seconds give the same.
    pub fn until(&self, end: u64) -> SourceTempo {
        let mut kept: Vec<Node> = Vec::new();
        for (tick, tempo, ramp) in self.nodes() {
            match kept.last() {
                Some(&(_, _, bends)) if tick >= end => {
                    if bends {
                        kept.push((tick, tempo, false));
                    }
                    break;
                }
                Some(&(_, before, false)) if before == tempo && !ramp => {}
                _ => kept.push((tick, tempo, ramp)),
            }
        }
        SourceTempo {
            tempo: self.tempo,
            head_ramp: kept[0].2,
            points: (kept[1..].iter())
                .map(|&(tick, tempo, ramp)| TempoPoint { tick, tempo, ramp })
                .collect(),
        }
    }

    /// Whether audio with this tempo, its tick 0 at `zero` of the song, was played to the
    /// song's own map as far as `end` ticks of the take: following the song then changes
    /// nothing, and the file sounds as it is.
    ///
    /// Asked of the take a recording made (`from_timeline` at the tick it started from)
    /// in a song whose map is as it was, the answer is always yes: both sides come out of
    /// the same function, the rounding inside a ramp too (docs/architecture.md 22-20).
    pub fn is_the_songs(&self, timeline: &Timeline, zero: i64, end: u64) -> bool {
        let songs = match u64::try_from(zero) {
            Ok(origin) => SourceTempo::from_timeline(timeline, origin),
            // The take starts before the song's head, where the head's tempo goes on
            // backwards.
            Err(_) => {
                let shift = zero.unsigned_abs();
                let ramp = timeline.head_ramp && !timeline.tempo_points.is_empty();
                let head = ramp.then_some(TempoPoint {
                    tick: shift,
                    tempo: timeline.tempo,
                    ramp: true,
                });
                let rest = timeline.tempo_points.iter().map(|p| TempoPoint {
                    tick: p.tick.saturating_add(shift),
                    ..*p
                });
                SourceTempo {
                    tempo: timeline.tempo,
                    head_ramp: false,
                    points: head.into_iter().chain(rest).collect(),
                }
            }
        };
        self.until(end) == songs.until(end)
    }
}

/// The song's tempo from tick `from` to tick `to`, as numbers that are the same for the
/// same tempo wherever in the song it is: `(ticks from `from`, hundredths, bends towards
/// the next)`. The first is the tempo at `from` (at 0, or before `from` where a ramp
/// began); the last may be past `to`, where a ramp over `to` ends. Part of the key audio
/// made for a clip is kept under.
pub fn tempo_piece(timeline: &Timeline, from: u64, to: u64) -> Vec<(i64, u32, bool)> {
    let all = SourceTempo {
        tempo: timeline.tempo,
        head_ramp: timeline.head_ramp,
        points: timeline.tempo_points.clone(),
    };
    let mut kept: Vec<(i64, u32, bool)> = Vec::new();
    for (tick, tempo, ramp) in all.nodes() {
        let at = tick as i64 - from as i64;
        let node = (at, tempo.centi(), ramp);
        match kept.last().copied() {
            _ if tick <= from => {
                kept.clear();
                // Where a steady tempo began does not matter.
                kept.push(if ramp { node } else { (0, node.1, false) });
            }
            Some((_, _, bends)) if tick >= to => {
                if bends {
                    kept.push((at, node.1, false));
                }
                break;
            }
            Some((_, before, false)) if before == node.1 && !ramp => {}
            _ => kept.push(node),
        }
    }
    kept
}

/// Ticks of the song to frames of the file, for one take in one clip.
#[derive(Debug, Clone)]
pub struct FileClock {
    zero: f64,
    first_frame: f64,
    sample_rate: f64,
    /// `Some`: the clip follows the song's tempo.
    source: Option<TempoMap>,
}

impl FileClock {
    /// `zero`: where the take's tick 0 is in the song (`Clip::active_zero`). `first_frame`
    /// and `sample_rate` in the same frames: the file's own, or those of a copy of it at
    /// another rate. `source`: the take's tempo when the clip follows the song's, else
    /// `None`.
    pub fn new(
        zero: i64,
        first_frame: f64,
        sample_rate: f64,
        source: Option<&SourceTempo>,
    ) -> FileClock {
        FileClock {
            zero: zero as f64,
            first_frame,
            sample_rate,
            source: source.map(SourceTempo::map),
        }
    }

    /// Of the shown take of a vocal clip, in frames of the file as it is on the disk.
    /// `None` for a keys clip.
    pub fn of_clip(clip: &Clip) -> Option<FileClock> {
        let shown = clip.active_audio()?;
        let follow = clip.takes.shape()?.follow;
        Some(FileClock::new(
            clip.active_zero(),
            shown.take.first_frame as f64,
            f64::from(shown.take.source.sample_rate),
            shown.take.sync.as_ref().filter(|_| follow),
        ))
    }

    pub fn follows(&self) -> bool {
        self.source.is_some()
    }

    /// The frame of the file that sounds at `tick` of the song, with its fraction. Below 0
    /// and past the take's end where the clip has no audio.
    pub fn frame_at(&self, song: &TempoMap, tick: f64) -> f64 {
        let seconds = match &self.source {
            Some(source) => source.seconds_at(tick - self.zero),
            None => song.seconds_at(tick) - song.seconds_at(self.zero),
        };
        self.first_frame + seconds * self.sample_rate
    }

    /// How much faster than recorded the audio plays at `tick`: the song's tempo there over
    /// the take's ("×1.25"). `None` for a clip that does not follow.
    pub fn ratio_at(&self, song: &TempoMap, tick: f64) -> Option<f64> {
        let source = self.source.as_ref()?;
        Some(song.tempo_at(tick) / source.tempo_at(tick - self.zero))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_QUARTER;

    const Q: u64 = TICKS_PER_QUARTER;
    const BAR: u64 = 4 * Q;

    fn tempo(centi: u32) -> Tempo {
        Tempo::from_centi(centi).unwrap()
    }

    fn point(tick: u64, centi: u32, ramp: bool) -> TempoPoint {
        TempoPoint {
            tick,
            tempo: tempo(centi),
            ramp,
        }
    }

    fn song(centi: u32, points: Vec<TempoPoint>, head_ramp: bool) -> Timeline {
        Timeline {
            head_ramp,
            tempo_points: points,
            ..Timeline::new(tempo(centi), Meter::FOUR_FOUR)
        }
    }

    #[test]
    fn a_clip_that_does_not_follow_plays_at_the_files_speed() {
        // 120 a minute: a bar is two seconds.
        let map = TempoMap::new(&song(12_000, Vec::new(), false));
        let clock = FileClock::new(BAR as i64, 1_000.0, 48_000.0, None);
        assert!(!clock.follows());
        assert_eq!(clock.frame_at(&map, BAR as f64), 1_000.0);
        assert_eq!(clock.frame_at(&map, 2.0 * BAR as f64), 97_000.0);
        assert_eq!(clock.frame_at(&map, 0.0), -95_000.0);
        assert_eq!(clock.ratio_at(&map, BAR as f64), None);
        // At 96 the same ticks are more of the file.
        let slow = TempoMap::new(&song(9_600, Vec::new(), false));
        assert_eq!(clock.frame_at(&slow, 2.0 * BAR as f64), 121_000.0);
    }

    #[test]
    fn a_clip_that_follows_is_stuck_to_the_ticks() {
        let source = SourceTempo::steady(tempo(9_600));
        let clock = FileClock::new(BAR as i64, 0.0, 48_000.0, Some(&source));
        assert!(clock.follows());
        // Whatever the song's tempo, a bar of the song is a bar of the take: 2.5 seconds
        // of the file at 96.
        for centi in [6_000, 9_600, 12_000, 30_000] {
            let map = TempoMap::new(&song(centi, Vec::new(), false));
            assert_eq!(clock.frame_at(&map, 2.0 * BAR as f64), 120_000.0);
            assert_eq!(clock.frame_at(&map, 0.0), -120_000.0);
            assert_eq!(
                clock.ratio_at(&map, BAR as f64),
                Some(f64::from(centi) / 9_600.0)
            );
        }
        let map = TempoMap::new(&song(12_000, Vec::new(), false));
        assert_eq!(clock.ratio_at(&map, 0.0), Some(1.25));
    }

    #[test]
    fn audio_recorded_in_this_map_follows_without_stretching() {
        // 96, a ramp from bar 3 to 120 at bar 5, then 90 from bar 7.
        let timeline = song(
            9_600,
            vec![
                point(2 * BAR, 9_600, true),
                point(4 * BAR, 12_000, false),
                point(6 * BAR, 9_000, false),
            ],
            false,
        );
        let map = TempoMap::new(&timeline);
        // Recorded from bar 2, where no ramp is: the take keeps the map exactly.
        let origin = BAR;
        let source = SourceTempo::from_timeline(&timeline, origin);
        let follow = FileClock::new(origin as i64, 500.0, 48_000.0, Some(&source));
        let free = FileClock::new(origin as i64, 500.0, 48_000.0, None);
        for i in 0..=400 {
            let tick = (origin + i * BAR / 50) as f64;
            let (a, b) = (follow.frame_at(&map, tick), free.frame_at(&map, tick));
            assert!((a - b).abs() < 1e-5, "{tick}: {a} {b}");
            let ratio = follow.ratio_at(&map, tick).unwrap();
            assert!((ratio - 1.0).abs() < 1e-12, "{tick}: {ratio}");
        }
    }

    #[test]
    fn ramps_in_both_maps() {
        // The song goes from 96 to 144 over bars 1 to 4; the take was sung to a ramp from
        // 120 down to 90 over its first two bars, then 90.
        let map = TempoMap::new(&song(9_600, vec![point(4 * BAR, 14_400, false)], true));
        let source = SourceTempo {
            tempo: tempo(12_000),
            head_ramp: true,
            points: vec![point(2 * BAR, 9_000, false)],
        };
        let zero = BAR;
        let clock = FileClock::new(zero as i64, 0.0, 48_000.0, Some(&source));
        let own = source.map();
        // The file's frames are the take's own seconds, whatever the song does.
        for tick in [BAR, BAR + 7, 2 * BAR, 3 * BAR, 3 * BAR + 1, 5 * BAR] {
            let expected = own.seconds_at((tick - zero) as f64) * 48_000.0;
            assert_eq!(clock.frame_at(&map, tick as f64), expected);
        }
        // Two bars of a ramp from 120 to 90: 8 beats at the mean of the beat's length.
        let two_bars = 8.0 * 60.0 / 30.0 * (120.0f64 / 90.0).ln();
        let got = clock.frame_at(&map, 3.0 * BAR as f64) / 48_000.0;
        assert!((got - two_bars).abs() < 1e-9, "{got} {two_bars}");
        // The ratio moves along both ramps: 108 over 120 at the take's head, 120 over 105
        // a bar later, 144 over 90 once both ramps are over.
        let ratio = |tick: u64| clock.ratio_at(&map, tick as f64).unwrap();
        assert!((ratio(BAR) - 108.0 / 120.0).abs() < 1e-12);
        assert!((ratio(2 * BAR) - 120.0 / 105.0).abs() < 1e-12);
        assert!((ratio(5 * BAR) - 144.0 / 90.0).abs() < 1e-12);
        // The frames go on forwards all the way.
        let mut last = f64::NEG_INFINITY;
        for i in 0..=600 {
            let at = clock.frame_at(&map, (i * BAR / 100) as f64);
            assert!(at > last);
            last = at;
        }
    }

    #[test]
    fn a_recording_is_the_songs_until_the_map_under_it_changes() {
        // 96, a ramp from bar 3 to 120 at bar 5, then 90 from bar 7.
        let points = vec![
            point(2 * BAR, 9_600, true),
            point(4 * BAR, 12_000, false),
            point(6 * BAR, 9_000, false),
        ];
        let timeline = song(9_600, points.clone(), false);
        // From every tick, inside the ramp too, where the head's tempo is rounded.
        for origin in (0..8 * BAR).step_by(331) {
            let sync = SourceTempo::from_timeline(&timeline, origin);
            for end in [1, BAR, 3 * BAR, 9 * BAR] {
                assert!(
                    sync.is_the_songs(&timeline, origin as i64, end),
                    "{origin} {end}"
                );
            }
        }
        // Recorded over bars 4 and 5 (inside the ramp, to its end).
        let origin = 3 * BAR;
        let sync = SourceTempo::from_timeline(&timeline, origin);
        let with = |points: Vec<TempoPoint>, centi: u32| song(centi, points, false);
        // A change past the clip's end is nothing to it; one inside it is.
        let mut later = points.clone();
        later[2].tempo = tempo(6_000);
        assert!(sync.is_the_songs(&with(later.clone(), 9_600), origin as i64, 2 * BAR));
        assert!(!sync.is_the_songs(&with(later, 9_600), origin as i64, 4 * BAR));
        // Where the ramp ends matters to a clip that ends inside it.
        let mut bent = points.clone();
        bent[1].tick = 5 * BAR;
        assert!(!sync.is_the_songs(&with(bent, 9_600), origin as i64, BAR));
        let mut faster = points.clone();
        faster[1].tempo = tempo(12_001);
        assert!(!sync.is_the_songs(&with(faster, 9_600), origin as i64, BAR));
        // The clip moved: the map under it is another one.
        assert!(!sync.is_the_songs(&timeline, (origin + BAR) as i64, BAR));
        // A point that changes nothing is no change.
        let mut same = points.clone();
        same.push(point(7 * BAR, 9_000, false));
        same.insert(0, point(BAR, 9_600, false));
        let early = SourceTempo::from_timeline(&timeline, 0);
        assert!(early.is_the_songs(&with(same, 9_600), 0, 9 * BAR));
        assert!(!early.is_the_songs(&with(points, 9_601), 0, 9 * BAR));
    }

    #[test]
    fn a_take_that_starts_before_the_songs_head() {
        let steady = SourceTempo::steady(tempo(12_000));
        let plain = song(12_000, vec![point(4 * BAR, 9_000, false)], false);
        assert!(steady.is_the_songs(&plain, -(BAR as i64), 5 * BAR));
        assert!(!steady.is_the_songs(&plain, -(BAR as i64), 5 * BAR + 1));
        // The head's ramp starts at the song's tick 0, a bar into the take.
        let ramped = song(12_000, vec![point(4 * BAR, 9_000, false)], true);
        assert!(steady.is_the_songs(&ramped, -(BAR as i64), BAR));
        assert!(!steady.is_the_songs(&ramped, -(BAR as i64), BAR + 1));
        let sung = SourceTempo {
            tempo: tempo(12_000),
            head_ramp: false,
            points: vec![point(BAR, 12_000, true), point(5 * BAR, 9_000, false)],
        };
        assert!(sung.is_the_songs(&ramped, -(BAR as i64), 9 * BAR));
    }

    #[test]
    fn the_piece_of_a_tempo_is_the_same_wherever_it_is() {
        let timeline = song(
            9_600,
            vec![
                point(2 * BAR, 9_600, true),
                point(4 * BAR, 12_000, false),
                point(6 * BAR, 12_000, false),
                point(8 * BAR, 9_000, false),
            ],
            false,
        );
        let piece = |from: u64, to: u64| tempo_piece(&timeline, from, to);
        assert_eq!(piece(0, BAR), [(0, 9_600, false)]);
        assert_eq!(piece(BAR, 2 * BAR), piece(0, BAR));
        assert_eq!(piece(4 * BAR, 8 * BAR), [(0, 12_000, false)]);
        assert_eq!(piece(5 * BAR, 7 * BAR), piece(6 * BAR, 8 * BAR));
        // Inside the ramp: where it began and where it ends.
        let b = BAR as i64;
        assert_eq!(
            piece(3 * BAR, 3 * BAR + 9),
            [(-b, 9_600, true), (b, 12_000, false)]
        );
        assert_eq!(
            piece(BAR, 9 * BAR),
            [
                (0, 9_600, false),
                (b, 9_600, true),
                (3 * b, 12_000, false),
                (7 * b, 9_000, false)
            ]
        );
    }
}
