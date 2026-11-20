//! Vocal tracks and the audio they hold (docs/architecture.md 22-10). Names and numbers only:
//! the samples are in files this crate never opens.

use std::sync::Arc;

use crate::song::TakeId;
use crate::tempo_map::TempoPoint;
use crate::time::Tempo;

/// The two kinds of track (spec 3, 23-A). A keys track holds clips of notes and sounds with
/// its instrument; a vocal track holds clips of audio and records from an input.
///
/// A vocal track has no instrument: `Track::instrument` of one is the sine wave, which
/// nothing reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrackKind {
    Keys,
    Vocal { input: VocalInput },
}

impl TrackKind {
    pub fn is_vocal(self) -> bool {
        matches!(self, TrackKind::Vocal { .. })
    }

    pub fn input(self) -> Option<VocalInput> {
        match self {
            TrackKind::Keys => None,
            TrackKind::Vocal { input } => Some(input),
        }
    }
}

/// The name a new vocal track gets ("歌", then "歌 2"; spec 3, 23-A). Part of the song, as
/// the names of keys tracks are.
pub const VOCAL_TRACK_NAME: &str = "歌";

/// Which channels of the input a vocal track records (spec 3 「歌のトラックの見出し」):
/// 「入口 1 ・ モノ」, 「入口 2 ・ モノ」, 「入口 1 ・ 2 ・ ステレオ」. Whether the PC has an
/// input with those channels is not the song's to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VocalInput {
    #[default]
    Mono1,
    Mono2,
    Stereo12,
}

impl VocalInput {
    pub const ALL: [VocalInput; 3] = [VocalInput::Mono1, VocalInput::Mono2, VocalInput::Stereo12];

    /// The channels of the input it takes, from 0, left first.
    pub fn channels(self) -> &'static [u8] {
        match self {
            VocalInput::Mono1 => &[0],
            VocalInput::Mono2 => &[1],
            VocalInput::Stereo12 => &[0, 1],
        }
    }

    /// Whether an input with `count` channels has all of them.
    pub fn fits(self, count: u16) -> bool {
        self.channels().iter().all(|&c| u16::from(c) < count)
    }
}

/// A number of the song's audio table. From `Doc::next_id`, never given twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioId(pub u32);

/// One audio file of the song. Never changed once made, and neither is the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    pub id: AudioId,
    /// Given by meloet: the key of what is kept beside the file (peaks, stretched audio).
    pub uid: [u8; 16],
    /// The file's name inside the song's audio folder.
    pub file: String,
    pub frames: u64,
    pub sample_rate: u32,
    /// 1 or 2.
    pub channels: u8,
    /// Of a file brought in, to tell whether a file found later holds the same audio.
    pub sha256: Option<[u8; 32]>,
}

/// The tempo the audio was played or sung at: the tempo points from the take's tick 0 on.
/// Recorded audio keeps the song's map as it was; a file brought in has one tempo, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTempo {
    pub tempo: Tempo,
    pub head_ramp: bool,
    /// Ticks from the take's tick 0, in order.
    pub points: Vec<TempoPoint>,
}

/// A take of a vocal clip. Never changed once made, as a keys take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTake {
    pub id: TakeId,
    pub source: Arc<AudioSource>,
    /// The frame of the file that sounds at the take's tick 0 (docs/architecture.md 22-6).
    pub first_frame: u64,
    /// How many frames from there.
    pub frames: u64,
    /// `None`: the audio has no tempo (a file brought in, 「—」).
    pub sync: Option<SourceTempo>,
}

/// A vocal clip's volume: −60.00 to +12.00 dB in steps of 0.01 (spec 3 「クリップの音量」).
/// Not the mixer's `Gain`, which ends at +6 and has −∞.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ClipGain(i16);

impl ClipGain {
    pub const UNITY: ClipGain = ClipGain(0);
    pub const MIN_DB: f64 = -60.0;
    pub const MAX_DB: f64 = 12.0;

    /// The nearest step inside the range; `None` for a value that is not a number.
    pub fn from_db(db: f64) -> Option<ClipGain> {
        (!db.is_nan())
            .then(|| ClipGain((db.clamp(Self::MIN_DB, Self::MAX_DB) * 100.0).round() as i16))
    }

    /// `None` outside the range.
    pub fn from_centi(centi: i32) -> Option<ClipGain> {
        (-6000..=1200)
            .contains(&centi)
            .then_some(ClipGain(centi as i16))
    }

    pub fn centi(self) -> i32 {
        i32::from(self.0)
    }

    pub fn db(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

/// What only a vocal clip has. Changing any of it is one step; the file stays as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioShape {
    /// Ticks from the clip's start, and before its end.
    pub fade_in: u64,
    pub fade_out: u64,
    pub gain: ClipGain,
    /// 「曲のテンポについていく」: on for recorded audio, off for a file brought in.
    pub follow: bool,
    /// −12..=12.
    pub semitones: i8,
    /// −50..=50.
    pub cents: i8,
    /// 「声の感じを保つ」.
    pub keep_formant: bool,
}

impl AudioShape {
    /// Of a clip just recorded.
    pub const RECORDED: AudioShape = AudioShape {
        fade_in: 0,
        fade_out: 0,
        gain: ClipGain::UNITY,
        follow: true,
        semitones: 0,
        cents: 0,
        keep_formant: true,
    };

    pub fn is_valid(&self) -> bool {
        (-12..=12).contains(&self.semitones) && (-50..=50).contains(&self.cents)
    }
}

pub mod record;

#[cfg(test)]
mod clip_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_channels() {
        assert!(VocalInput::Mono1.fits(1));
        assert!(!VocalInput::Mono2.fits(1));
        assert!(!VocalInput::Stereo12.fits(1));
        assert!(VocalInput::ALL.iter().all(|i| i.fits(2)));
        assert!(!VocalInput::Mono1.fits(0));
        assert_eq!(VocalInput::default(), VocalInput::Mono1);
    }

    #[test]
    fn clip_gain_range() {
        assert_eq!(ClipGain::from_db(-3.0).map(ClipGain::centi), Some(-300));
        assert_eq!(ClipGain::from_db(40.0).map(ClipGain::db), Some(12.0));
        assert_eq!(ClipGain::from_db(-90.0).map(ClipGain::db), Some(-60.0));
        assert_eq!(ClipGain::from_db(f64::NAN), None);
        assert_eq!(ClipGain::from_centi(1201), None);
        assert_eq!(ClipGain::from_centi(-6000).map(ClipGain::db), Some(-60.0));
        assert_eq!(ClipGain::default(), ClipGain::UNITY);
    }

    #[test]
    fn shape_range() {
        assert!(AudioShape::RECORDED.is_valid());
        assert!(
            !AudioShape {
                semitones: 13,
                ..AudioShape::RECORDED
            }
            .is_valid()
        );
        assert!(
            !AudioShape {
                cents: -51,
                ..AudioShape::RECORDED
            }
            .is_valid()
        );
    }
}
