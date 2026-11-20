//! The clips of a vocal track as the audio thread needs them (docs/architecture.md 22-8,
//! 22-17): where each starts and ends, how long its fades are, and its volume.
//!
//! The read-ahead thread puts the plain audio of the clips, and silence between them, into
//! the track's ring ([`crate::StreamReader`]); the engine multiplies each frame by what the
//! table says for it. So a fade or a volume that changes while the song plays needs no new
//! read-ahead.
//!
//! Everything is counted in **song frames**: frame 0 is the song's tick 0, and a tick is at
//! [`song_frame`], the nearest frame at the engine's sample rate. The core makes the table
//! again when the tempo map, the clips or the sample rate change. The engine's own place in
//! song frames is [`song_frame_at`] of its anchor.

use meloet_model::{TempoMap, clip_fade_gain};

/// The frame of the song at `tick`: the nearest one. The table, the read-ahead thread and
/// the engine all place audio with this one function.
pub fn song_frame(map: &TempoMap, tick: f64, sample_rate: f64) -> u64 {
    (map.seconds_at(tick) * sample_rate).round().max(0.0) as u64
}

/// The song frame of the engine's position: `frames` after an anchor at `anchor_sec` on the
/// map. Notes and clicks are placed from the anchor itself (`TempoMap::frame_of`), so audio
/// may sit up to a frame from a note of the same tick.
pub fn song_frame_at(anchor_sec: f64, frames: u64, sample_rate: f64) -> u64 {
    (anchor_sec * sample_rate).round().max(0.0) as u64 + frames
}

/// One vocal clip. `[start, end)` in song frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipRegion {
    pub start: u64,
    pub end: u64,
    /// Frames from `start`, and before `end`.
    pub fade_in: u64,
    pub fade_out: u64,
    /// The clip's volume, linear (1.0 for 0 dB).
    pub gain: f32,
}

impl ClipRegion {
    /// From the clip as the song has it: ticks, and the volume in dB.
    pub fn of(
        map: &TempoMap,
        sample_rate: f64,
        start: u64,
        end: u64,
        fade_in: u64,
        fade_out: u64,
        gain_db: f64,
    ) -> ClipRegion {
        let frame = |tick: u64| song_frame(map, tick as f64, sample_rate);
        let (first, last) = (frame(start), frame(end));
        ClipRegion {
            start: first,
            end: last,
            fade_in: frame(start.saturating_add(fade_in)).min(last) - first,
            fade_out: last - frame(end.saturating_sub(fade_out)).max(first),
            gain: 10f64.powf(gain_db / 20.0) as f32,
        }
    }

    /// What a sample of the clip at song frame `frame` is multiplied by: the fades and the
    /// volume. 0 outside the clip.
    pub fn gain_at(&self, frame: u64) -> f32 {
        if frame < self.start || frame >= self.end {
            return 0.0;
        }
        let fades = clip_fade_gain(
            (frame - self.start) as f64,
            (self.end - self.start) as f64,
            self.fade_in as f64,
            self.fade_out as f64,
        );
        fades as f32 * self.gain
    }
}

/// The clips of one vocal track, in the order they play. Made off the audio thread; the
/// engine gets it boxed and hands the old one back on a retired ring, as a `Sequence`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ClipTable {
    regions: Box<[ClipRegion]>,
}

impl ClipTable {
    /// `None` when the regions are not in order, overlap, or one ends before it starts
    /// (clips of a track never overlap; spec 3, 82). Regions with no frames are left out.
    pub fn new(regions: Vec<ClipRegion>) -> Option<ClipTable> {
        let regions: Vec<ClipRegion> = regions.into_iter().filter(|r| r.end != r.start).collect();
        let in_order = regions.windows(2).all(|w| w[0].end <= w[1].start);
        let whole = regions.iter().all(|r| r.start < r.end);
        (in_order && whole).then(|| ClipTable {
            regions: regions.into_boxed_slice(),
        })
    }

    pub fn regions(&self) -> &[ClipRegion] {
        &self.regions
    }

    /// The index of the first region that ends after `frame`: the one `frame` is in, or the
    /// next one. Takes no memory.
    pub fn from(&self, frame: u64) -> usize {
        self.regions.partition_point(|r| r.end <= frame)
    }

    /// What a sample at song frame `frame` is multiplied by; 0 between the clips.
    pub fn gain_at(&self, frame: u64) -> f32 {
        (self.regions.get(self.from(frame))).map_or(0.0, |r| r.gain_at(frame))
    }
}

#[cfg(test)]
mod tests {
    use meloet_model::{Meter, Tempo, Timeline, fade_gain};

    use super::*;

    fn map(bpm_centi: u32) -> TempoMap {
        let tempo = Tempo::from_centi(bpm_centi).unwrap();
        TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR))
    }

    #[test]
    fn ticks_become_song_frames() {
        // 120 a minute: a quarter note is half a second.
        let map = map(12_000);
        assert_eq!(song_frame(&map, 0.0, 48_000.0), 0);
        assert_eq!(song_frame(&map, 1_920.0, 48_000.0), 24_000);
        assert_eq!(song_frame(&map, 1_920.0, 44_100.0), 22_050);
        assert_eq!(song_frame_at(0.5, 10, 48_000.0), 24_010);
        let r = ClipRegion::of(&map, 48_000.0, 7_680, 15_360, 1_920, 960, -6.0);
        assert_eq!((r.start, r.end), (96_000, 192_000));
        assert_eq!((r.fade_in, r.fade_out), (24_000, 12_000));
        assert!((r.gain - 0.501_187).abs() < 1e-5);
        // Fades longer than the clip stop at its ends.
        let r = ClipRegion::of(&map, 48_000.0, 0, 1_920, 5_000, 5_000, 0.0);
        assert_eq!((r.fade_in, r.fade_out, r.gain), (24_000, 24_000, 1.0));
    }

    #[test]
    fn the_gain_of_a_frame_is_the_fades_and_the_volume() {
        let r = ClipRegion {
            start: 1_000,
            end: 2_000,
            fade_in: 100,
            fade_out: 200,
            gain: 0.5,
        };
        assert_eq!(r.gain_at(999), 0.0);
        assert_eq!(r.gain_at(1_000), 0.0);
        assert_eq!(r.gain_at(1_050), (fade_gain(0.5) * 0.5) as f32);
        assert_eq!(r.gain_at(1_100), 0.5);
        assert_eq!(r.gain_at(1_800), 0.5);
        assert_eq!(r.gain_at(1_900), (fade_gain(0.5) * 0.5) as f32);
        assert_eq!(r.gain_at(2_000), 0.0);
    }

    #[test]
    fn a_table_finds_the_clip_of_a_frame() {
        let region = |start, end, gain| ClipRegion {
            start,
            end,
            fade_in: 0,
            fade_out: 0,
            gain,
        };
        let table = ClipTable::new(vec![
            region(100, 200, 1.0),
            region(200, 300, 0.5),
            region(400, 400, 9.0),
            region(1_000, 2_000, 2.0),
        ])
        .unwrap();
        assert_eq!(table.regions().len(), 3);
        assert_eq!(
            [0, 100, 199, 200, 299, 300, 999, 1_000, 1_999, 2_000].map(|f| table.gain_at(f)),
            [0.0, 1.0, 1.0, 0.5, 0.5, 0.0, 0.0, 2.0, 2.0, 0.0]
        );
        assert_eq!(
            [0, 150, 200, 300, 2_000].map(|f| table.from(f)),
            [0, 0, 1, 2, 3]
        );
        assert_eq!(ClipTable::default().gain_at(5), 0.0);
        assert_eq!(ClipTable::new(vec![region(5, 3, 1.0)]), None);
        assert_eq!(
            ClipTable::new(vec![region(0, 10, 1.0), region(9, 20, 1.0)]),
            None
        );
    }
}
