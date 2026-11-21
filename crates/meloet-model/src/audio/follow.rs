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
use crate::tempo_map::{TempoMap, Timeline};
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
    use crate::tempo_map::TempoPoint;

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
}
