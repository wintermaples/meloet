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

    /// The audio that was made for a clip and kept under `key` (`made::MadeKey`), at
    /// `sample_rate`. `None` when it is not there.
    fn open_made(&self, _key: &[u8; 32], _sample_rate: u32) -> Option<Box<dyn PcmFile>> {
        None
    }

    /// A number that goes up whenever the store may open what it could not before
    /// (`AudioStore::generation`); a store whose files never change keeps it at 0.
    fn generation(&self) -> u64 {
        0
    }
}

/// One clip as the read-ahead thread plays it: `[start, end)` in song frames, as the
/// engine's `ClipRegion` of the same clip, and where in the file that is. The fades and the
/// volume are not its business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedClip {
    pub start: u64,
    pub end: u64,
    pub uid: [u8; 16],
    /// `Some`: the clip plays the audio made for it (stretched, at another pitch), which
    /// the store keeps under this key, and not the file of `uid`.
    pub made: Option<[u8; 32]>,
    /// The frame of the `PcmFile` that sounds at `start`. Below 0, or from the file's end
    /// on, there is silence.
    pub file_frame: i64,
    /// The file has no frames for the clip past this one (the end of the take).
    pub file_end: i64,
    /// A piece of 「いいとこ取り」 comes in over its first `lead` frames and goes out over
    /// its last `tail`, in a straight line, while the piece next to it does the opposite
    /// over the same frames: the two always add up to one. 0 and 0 for a whole clip.
    pub lead: u32,
    pub tail: u32,
}

impl FeedClip {
    /// What a frame of the piece at song frame `frame` (inside it) is multiplied by.
    pub fn seam_gain(&self, frame: u64) -> f32 {
        let (lead, tail) = (u64::from(self.lead), u64::from(self.tail));
        let into = frame - self.start;
        if into < lead {
            return (into + 1) as f32 / (lead + 1) as f32;
        }
        if frame + tail >= self.end {
            return 1.0 - (frame + tail - self.end + 1) as f32 / (tail + 1) as f32;
        }
        1.0
    }
}

/// How long the seam between two pieces of 「いいとこ取り」 is: 10 ms.
pub fn seam_frames(sample_rate: u32) -> u64 {
    u64::from(sample_rate / 100)
}

/// The clip as the read-ahead thread plays its shown take; `None` for a keys clip.
pub fn feed_clip(map: &TempoMap, sample_rate: u32, clip: &Clip) -> Option<FeedClip> {
    feed_take(map, sample_rate, clip, clip.active)
}

/// The clip as the read-ahead thread plays it when take `index` sounds all along; `None`
/// for a keys clip or a take the clip does not have. The audio sounds at its own speed: the
/// take's tick 0 is at the song frame of that tick, and from there one frame of the file is
/// one frame of the song.
pub fn feed_take(map: &TempoMap, sample_rate: u32, clip: &Clip, index: usize) -> Option<FeedClip> {
    let shown = clip.takes.vocal().get(index)?;
    let take = &shown.take;
    let rate = f64::from(sample_rate);
    let to_engine = |frames: u64| {
        (frames as f64 * rate / f64::from(take.source.sample_rate.max(1))).round() as i64
    };
    let start = song_frame(map, clip.start as f64, rate);
    // The take's tick 0 may be before the song's head.
    let zero = (map.seconds_at(clip.zero(shown) as f64) * rate).round() as i64;
    let first = to_engine(take.first_frame);
    Some(FeedClip {
        start,
        end: song_frame(map, clip.end as f64, rate),
        uid: take.source.uid,
        made: None,
        file_frame: first + (start as i64 - zero),
        file_end: first + to_engine(take.frames),
        lead: 0,
        tail: 0,
    })
}

/// The clip as the read-ahead thread plays it, with the take each part is of: the shown
/// take whole, or the pieces of 「いいとこ取り」. Every take is where it is as the shown one
/// ([`feed_take`]). Two pieces next to each other lie over one another around their seam,
/// for 10 ms or for half the shorter of the two when that is less, so no seam reaches the
/// one after it; a piece that has no frame of its own is left out.
pub fn feed_pieces(map: &TempoMap, sample_rate: u32, clip: &Clip) -> Vec<(FeedClip, usize)> {
    if clip.comp.is_empty() {
        return (feed_clip(map, sample_rate, clip).map(|feed| (feed, clip.active)))
            .into_iter()
            .collect();
    }
    let rate = f64::from(sample_rate);
    let (start, end) = (
        song_frame(map, clip.start as f64, rate),
        song_frame(map, clip.end as f64, rate),
    );
    // Each piece from its first frame; the one before it ends there.
    let mut parts: Vec<(u64, usize)> = Vec::with_capacity(clip.comp.len());
    for (i, piece) in clip.comp.iter().enumerate() {
        let from = match i {
            0 => start,
            _ => song_frame(map, clip.start.saturating_add(piece.from) as f64, rate),
        };
        let from = from.clamp(start, end);
        let until = (clip.comp.get(i + 1))
            .map_or(end, |next| {
                song_frame(map, clip.start.saturating_add(next.from) as f64, rate)
            })
            .clamp(start, end);
        if until > from && parts.last().is_none_or(|last| last.1 != piece.take) {
            parts.push((from, piece.take));
        }
    }
    let half = |at: usize| -> u64 {
        // The seam before part `at`; none before the first and after the last.
        if at == 0 || at >= parts.len() {
            return 0;
        }
        let length = |i: usize| parts.get(i + 1).map_or(end, |p| p.0) - parts[i].0;
        (seam_frames(sample_rate) / 2)
            .min(length(at - 1) / 2)
            .min(length(at) / 2)
    };
    (parts.iter().enumerate())
        .filter_map(|(i, &(from, take))| {
            let whole = feed_take(map, sample_rate, clip, take)?;
            let until = parts.get(i + 1).map_or(end, |p| p.0);
            let (before, after) = (half(i), half(i + 1));
            let first = from - before;
            let piece = FeedClip {
                start: first,
                end: until + after,
                file_frame: whole.file_frame + (first - whole.start) as i64,
                lead: 2 * before as u32,
                tail: 2 * after as u32,
                ..whole
            };
            Some((piece, take))
        })
        .collect()
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
            comp: Vec::new(),
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

    /// A clip over ticks 7680 to 15360 (frames 96000 to 192000 at 120 a minute and 48 kHz)
    /// with three takes of three files, each with its own tick 0, and a comp.
    fn comped(comp: &[(u64, usize)]) -> Clip {
        let one = clip(7_680, 15_360, 0, 48_000);
        let take = |n: u8, offset: i64| {
            let ClipTakes::Vocal { takes, .. } = &one.takes else {
                unreachable!()
            };
            let source = AudioSource {
                uid: [n; 16],
                ..AudioSource::clone(&takes[0].take.source)
            };
            ClipTake {
                take: Arc::new(AudioTake {
                    id: TakeId(40 + u32::from(n)),
                    source: Arc::new(source),
                    ..AudioTake::clone(&takes[0].take)
                }),
                offset,
            }
        };
        Clip {
            takes: ClipTakes::Vocal {
                takes: vec![take(1, 0), take(2, -1_920), take(3, 960)],
                shape: AudioShape::RECORDED,
            },
            active: 2,
            comp: (comp.iter())
                .map(|&(from, take)| meloet_model::CompPiece { from, take })
                .collect(),
            ..one
        }
    }

    #[test]
    fn the_pieces_of_a_comp_lie_over_each_other_for_ten_ms_each_take_in_its_place() {
        let tempo = Tempo::from_centi(12_000).unwrap();
        let map = TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR));
        // No comp: the shown take, whole, as `feed_clip`.
        let plain = comped(&[]);
        let whole = feed_clip(&map, 48_000, &plain).unwrap();
        assert_eq!(feed_pieces(&map, 48_000, &plain), [(whole, 2)]);
        assert_eq!((whole.uid, whole.lead, whole.tail), ([3; 16], 0, 0));
        assert_eq!(seam_frames(48_000), 480);

        // Seams at ticks 1920 and 3840 of the clip: frames 120000 and 144000.
        let clip = comped(&[(0, 0), (1_920, 1), (3_840, 2)]);
        let pieces = feed_pieces(&map, 48_000, &clip);
        let spans: Vec<_> = (pieces.iter())
            .map(|(p, take)| (p.start, p.end, p.lead, p.tail, *take, p.uid[0]))
            .collect();
        assert_eq!(
            spans,
            [
                (96_000, 120_240, 0, 480, 0, 1),
                (119_760, 144_240, 480, 480, 1, 2),
                (143_760, 192_000, 480, 0, 2, 3),
            ]
        );
        // Every piece plays the frames its take plays there as the shown take.
        for (piece, take) in &pieces {
            let whole = feed_take(&map, 48_000, &clip, *take).unwrap();
            assert_eq!((whole.start, whole.end), (96_000, 192_000));
            assert_eq!(
                piece.file_frame - whole.file_frame,
                (piece.start - whole.start) as i64
            );
            assert_eq!((piece.file_end, piece.made), (whole.file_end, None));
        }
        assert_eq!(pieces[0].0.file_frame, 12_000);
        assert_eq!(pieces[1].0.file_frame, 12_000 + 24_000 + 23_760);
        // The third take's tick 0 is a quarter note after the clip's start.
        assert_eq!(pieces[2].0.file_frame, 47_760);

        // Around a seam the two pieces add up to one, and each goes in a straight line.
        let (out, into) = (pieces[0].0, pieces[1].0);
        assert_eq!((out.seam_gain(96_000), out.seam_gain(119_759)), (1.0, 1.0));
        assert_eq!(into.seam_gain(120_240), 1.0);
        let mut last = 0.0;
        for frame in 119_760..120_240 {
            let (a, b) = (out.seam_gain(frame), into.seam_gain(frame));
            assert!((a + b - 1.0).abs() < 1e-6, "frame {frame}: {a} + {b}");
            assert!(
                b > last && b < 1.0 && b - last < 1.0 / 480.0,
                "frame {frame}"
            );
            last = b;
        }

        // Pieces shorter than 10 ms: the seams take half of the shorter neighbour each, so
        // no two of them meet. 16 ticks are 200 frames.
        let clip = comped(&[(0, 0), (1_920, 1), (1_936, 2)]);
        let pieces = feed_pieces(&map, 48_000, &clip);
        let spans: Vec<_> = (pieces.iter())
            .map(|(p, _)| (p.start, p.end, p.lead, p.tail))
            .collect();
        assert_eq!(
            spans,
            [
                (96_000, 120_100, 0, 200),
                (119_900, 120_300, 200, 200),
                (120_100, 192_000, 200, 0),
            ]
        );
        for frame in 119_900..120_300 {
            let sum: f32 = (pieces.iter())
                .filter(|(p, _)| (p.start..p.end).contains(&frame))
                .map(|(p, _)| p.seam_gain(frame))
                .sum();
            assert!((sum - 1.0).abs() < 1e-6, "frame {frame}: {sum}");
        }
        // One take all along is that take whole, with no seam.
        let clip = comped(&[(0, 1)]);
        let whole = feed_take(&map, 48_000, &clip, 1).unwrap();
        assert_eq!(feed_pieces(&map, 48_000, &clip), [(whole, 1)]);
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
