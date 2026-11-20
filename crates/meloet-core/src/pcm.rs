//! Where the two halves of playing a vocal clip meet (docs/architecture.md 22-8, 22-17):
//! the read-ahead thread, which lays the audio of a track out in the order it sounds, and
//! the audio files, which are found, decoded and brought to the engine's sample rate
//! somewhere else. The thread sees a file only as a [`PcmFile`] it gets from a
//! [`PcmStore`], and a clip only as a [`FeedClip`].

use std::io;

use meloet_engine::song_frame;
use meloet_model::TempoMap;
use meloet_model::song::Clip;

/// One audio file as plain samples **at the engine's sample rate**, whatever the file on
/// the disk is (a wav at that rate as it is, or what was made of it in the cache).
pub trait PcmFile: Send {
    /// 1 or 2.
    fn channels(&self) -> u8;
    /// How many frames it has, at the engine's rate.
    fn frames(&self) -> u64;
    /// Reads frames from `frame` on into `out`, interleaved by `channels`, as many as `out`
    /// holds and the file has. Returns how many frames were read; fewer only at the end.
    fn read(&mut self, frame: u64, out: &mut [f32]) -> io::Result<usize>;
}

/// Opens the song's audio for the read-ahead thread. Called on that thread, never on the
/// audio thread.
pub trait PcmStore: Send + Sync {
    /// The audio with this `uid` (`AudioSource::uid`) at `sample_rate`. `None` while it is
    /// not found or not made yet (`AudioClipState::Missing`, `Loading`): its clips are
    /// silence, and the thread asks again when it is told the store changed.
    fn open(&self, uid: &[u8; 16], sample_rate: u32) -> Option<Box<dyn PcmFile>>;
}

/// One clip as the read-ahead thread plays it: `[start, end)` in song frames, as the
/// engine's `ClipRegion` of the same clip, and where in the file that is. The fades and the
/// volume are not its business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedClip {
    pub start: u64,
    pub end: u64,
    pub uid: [u8; 16],
    /// The frame of the `PcmFile` that sounds at `start`. Below 0, or from the file's end
    /// on, there is silence.
    pub file_frame: i64,
    /// The file has no frames for the clip past this one (the end of the take).
    pub file_end: i64,
}

/// The clip as the read-ahead thread plays it; `None` for a keys clip. The audio sounds at
/// its own speed: the take's tick 0 is at the song frame of that tick, and from there one
/// frame of the file is one frame of the song.
pub fn feed_clip(map: &TempoMap, sample_rate: u32, clip: &Clip) -> Option<FeedClip> {
    let take = &clip.active_audio()?.take;
    let rate = f64::from(sample_rate);
    let to_engine = |frames: u64| {
        (frames as f64 * rate / f64::from(take.source.sample_rate.max(1))).round() as i64
    };
    let start = song_frame(map, clip.start as f64, rate);
    // The take's tick 0 may be before the song's head.
    let zero = (map.seconds_at(clip.active_zero() as f64) * rate).round() as i64;
    let first = to_engine(take.first_frame);
    Some(FeedClip {
        start,
        end: song_frame(map, clip.end as f64, rate),
        uid: take.source.uid,
        file_frame: first + (start as i64 - zero),
        file_end: first + to_engine(take.frames),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use meloet_model::song::{ClipId, ClipTake, ClipTakes, TakeId};
    use meloet_model::{AudioId, AudioShape, AudioSource, AudioTake, Meter, Tempo, Timeline};

    use super::*;

    fn clip(start: u64, end: u64, offset: i64, source_rate: u32) -> Clip {
        let source = Arc::new(AudioSource {
            id: AudioId(31),
            uid: [9; 16],
            file: "歌 31.wav".to_owned(),
            frames: 10 * u64::from(source_rate),
            sample_rate: source_rate,
            channels: 1,
            bits: Some(24),
            sha256: None,
        });
        Clip {
            id: ClipId(9),
            name: "歌 テイク 1".to_owned(),
            start,
            end,
            takes: ClipTakes::Vocal {
                takes: vec![ClipTake {
                    take: Arc::new(AudioTake {
                        id: TakeId(40),
                        source,
                        first_frame: u64::from(source_rate) / 4,
                        frames: 8 * u64::from(source_rate),
                        sync: None,
                    }),
                    offset,
                }],
                shape: AudioShape::RECORDED,
            },
            active: 0,
        }
    }

    #[test]
    fn a_clip_plays_its_file_from_where_its_start_is_in_the_take() {
        // 120 a minute at 48 kHz: a quarter note is 24000 frames.
        let tempo = Tempo::from_centi(12_000).unwrap();
        let map = TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR));
        // The take's tick 0 is at the clip's start: the file plays from its first frame.
        let fed = feed_clip(&map, 48_000, &clip(7_680, 15_360, 0, 48_000)).unwrap();
        assert_eq!((fed.start, fed.end), (96_000, 192_000));
        assert_eq!((fed.file_frame, fed.file_end), (12_000, 12_000 + 384_000));
        assert_eq!(fed.uid, [9; 16]);
        // The right half of a cut: the take's tick 0 is a quarter note before the start.
        let fed = feed_clip(&map, 48_000, &clip(7_680, 15_360, -1_920, 48_000)).unwrap();
        assert_eq!(fed.file_frame, 12_000 + 24_000);
        // The start pulled out to before the take: silence first.
        let fed = feed_clip(&map, 48_000, &clip(0, 15_360, 1_920, 48_000)).unwrap();
        assert_eq!(fed.file_frame, 12_000 - 24_000);
        // The take's tick 0 before the song's head.
        let fed = feed_clip(&map, 48_000, &clip(0, 15_360, -3_840, 48_000)).unwrap();
        assert_eq!(fed.file_frame, 12_000 + 48_000);
        // A file at another rate is counted in the engine's frames.
        let fed = feed_clip(&map, 48_000, &clip(7_680, 15_360, 0, 44_100)).unwrap();
        assert_eq!((fed.file_frame, fed.file_end), (12_000, 12_000 + 384_000));

        let keys = Clip {
            takes: ClipTakes::Keys(Vec::new()),
            ..clip(0, 1, 0, 48_000)
        };
        assert_eq!(feed_clip(&map, 48_000, &keys), None);
    }

    #[test]
    fn a_store_hands_out_files() {
        struct Ramp(u64);
        impl PcmFile for Ramp {
            fn channels(&self) -> u8 {
                1
            }
            fn frames(&self) -> u64 {
                self.0
            }
            fn read(&mut self, frame: u64, out: &mut [f32]) -> io::Result<usize> {
                let count = (self.0.saturating_sub(frame) as usize).min(out.len());
                for (i, sample) in out[..count].iter_mut().enumerate() {
                    *sample = (frame + i as u64) as f32;
                }
                Ok(count)
            }
        }
        struct Store;
        impl PcmStore for Store {
            fn open(&self, uid: &[u8; 16], _: u32) -> Option<Box<dyn PcmFile>> {
                (uid[0] == 9).then(|| Box::new(Ramp(100)) as Box<dyn PcmFile>)
            }
        }
        let store: Arc<dyn PcmStore> = Arc::new(Store);
        assert!(store.open(&[1; 16], 48_000).is_none());
        let mut file = store.open(&[9; 16], 48_000).unwrap();
        let mut out = [0.0f32; 8];
        assert_eq!(file.read(96, &mut out).unwrap(), 4);
        assert_eq!(out[..4], [96.0, 97.0, 98.0, 99.0]);
        assert_eq!((file.channels(), file.frames()), (1, 100));
    }
}
