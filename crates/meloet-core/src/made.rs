//! Which vocal clips need audio made for them, and the name that audio is kept under
//! (docs/architecture.md 22-8, 22-19 pass 2).
//!
//! A clip plays its file as it is unless it is stretched (it follows the song's tempo and
//! the song's tempo under it is not the one its audio was played to) or its pitch is
//! changed. For those a [`MadePlan`] says what to make: which frame of the file sounds at
//! each frame of the clip, the pitch, and the key of the result in the cache. The thread
//! that makes it is [`crate::maker`].

use std::collections::HashSet;
use std::sync::Arc;

use meloet_engine::ClipTable;
use meloet_model::song::Clip;
use meloet_model::{FileClock, TempoMap, Timeline, tempo_piece};
use meloet_stretch::{
    PITCH_VERSION, Preset, RENDER_STEP, STRETCH_VERSION, Stretcher, TUNE_VERSION, Tune,
};
use sha2::{Digest, Sha256};

use crate::pcm::FeedClip;
use crate::play::clip_tables_of;

/// The name audio made for a clip is kept under: a digest of everything it is made from.
pub type MadeKey = [u8; 32];

/// Goes into every key: another number when meloet makes the audio another way.
const RECIPE: u32 = 1;
/// How the part that stretches is set up. Each is part of the key by its name here.
const PRESET: (Preset, &str) = (Preset::Cheaper, "cheaper");
/// The fundamental the formant step is told (`Stretcher::set_formant_base`), as a
/// frequency in Hz; `None`: the part looks for it block by block.
const FORMANT_BASE_HZ: Option<f32> = None;

/// How the part that stretches is set up for a clip that is tuned (`Preset::for_tuning`),
/// by its name in the key.
const TUNE_PRESET: &str = "tune-120-20";

/// 「音程を寄せる」 of a clip: the notes it is brought toward and how far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TunePlan {
    /// The notes of the scale the clip keeps: `[0]` is C, `[11]` is B.
    pub scale: [bool; 12],
    /// 1..=100 percent.
    pub strength: u8,
}

/// Where in the file a clip is at each of its frames.
#[derive(Debug, Clone)]
pub enum Place {
    /// At the file's own speed: frame `j` of the clip is frame `file_frame + j`.
    Straight { file_frame: i64 },
    /// Stuck to the song's ticks.
    Follow {
        clock: FileClock,
        map: Arc<TempoMap>,
        /// Where the clip starts on `map`.
        start_sec: f64,
    },
}

/// The audio to make for one clip, in frames at the rate the engine runs at.
#[derive(Debug, Clone)]
pub struct MadePlan {
    pub key: MadeKey,
    /// `ClipView::id` and `AudioClipView::audio`, for 「音を作っています」.
    pub clip: u32,
    pub audio: u32,
    pub uid: [u8; 16],
    pub sample_rate: u32,
    /// The clip in ticks of the song: the one nearest to the play line is made first.
    pub start: u64,
    pub end: u64,
    /// How many frames the clip has.
    pub frames: u64,
    pub place: Place,
    /// The file has no frames for the clip from this one on (the end of the take).
    pub file_end: i64,
    /// Semitones, the cents as their fraction.
    pub semitones: f32,
    pub keep_formant: bool,
    /// `None`: the pitch is the same all along the clip.
    pub tune: Option<TunePlan>,
}

impl MadePlan {
    /// The frame of the file, with its fraction, that sounds at frame `j` of the clip.
    pub fn file_frame_at(&self, j: u64) -> f64 {
        match &self.place {
            Place::Straight { file_frame } => *file_frame as f64 + j as f64,
            Place::Follow {
                clock,
                map,
                start_sec,
            } => {
                let seconds = start_sec + j as f64 / f64::from(self.sample_rate);
                clock.frame_at(map, map.tick_at(seconds))
            }
        }
    }

    /// The part that stretches, set up for this plan with `channels` channels.
    pub fn stretcher(&self, channels: usize) -> Result<Stretcher, meloet_stretch::Error> {
        let rate = self.sample_rate as f32;
        let preset = match self.tune {
            Some(_) => Preset::for_tuning(rate),
            None => PRESET.0,
        };
        let mut stretcher = Stretcher::new(channels, rate, preset)?;
        stretcher.set_transpose_semitones(self.semitones, None);
        if self.keep_formant {
            stretcher.set_formant_factor(1.0, true);
            if let Some(hz) = FORMANT_BASE_HZ {
                stretcher.set_formant_base(hz / rate);
            }
        }
        Ok(stretcher)
    }

    /// What `meloet_stretch::render_tuned` is told for a tuned plan: the voice is brought
    /// to the scale as it sounds after the clip's semitones and cents.
    pub fn tuning(&self) -> Option<Tune> {
        self.tune.map(|tune| Tune {
            scale: tune.scale,
            strength: f32::from(tune.strength) / 100.0,
            transpose: self.semitones,
        })
    }
}

struct Digest32(Sha256);

impl Digest32 {
    fn bytes(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }
    fn int(&mut self, n: i64) {
        self.0.update(n.to_le_bytes());
    }
}

/// What to make for `clip`, whose plain audio the read-ahead thread plays as `feed`, with
/// the song's tempo as `timeline` (and `map`, made of it) at `sample_rate`. `None`: the
/// clip plays its file as it is (and so does a keys clip).
pub fn plan(
    timeline: &Timeline,
    map: &Arc<TempoMap>,
    sample_rate: u32,
    clip: &Clip,
    feed: &FeedClip,
) -> Option<MadePlan> {
    let take = &clip.active_audio()?.take;
    let shape = clip.takes.shape()?;
    let zero = clip.active_zero();
    let pitched = shape.semitones != 0 || shape.cents != 0;
    let tune = shape.tune.map(|tune| {
        let mut scale = [false; 12];
        for step in tune.mode.steps() {
            scale[usize::from((tune.root + step) % 12)] = true;
        }
        TunePlan {
            scale,
            strength: tune.strength,
        }
    });
    let take_ticks = u64::try_from(clip.end as i64 - zero).unwrap_or(0);
    // Audio that was played to the song's own tempo is where it belongs already.
    let sync = (take.sync.as_ref())
        .filter(|sync| shape.follow && !sync.is_the_songs(timeline, zero, take_ticks));
    let frames = feed.end.saturating_sub(feed.start);
    if (!pitched && sync.is_none() && tune.is_none()) || frames == 0 {
        return None;
    }
    // The formant step does nothing while the pitch stays.
    let keep_formant = shape.keep_formant && (pitched || tune.is_some());
    let rate = f64::from(sample_rate);

    let mut key = Digest32(Sha256::new());
    key.bytes(b"meloet made audio");
    key.int(i64::from(RECIPE));
    key.bytes(STRETCH_VERSION.as_bytes());
    key.bytes(PRESET.1.as_bytes());
    key.int(RENDER_STEP as i64);
    key.int(FORMANT_BASE_HZ.map_or(-1, |hz| i64::from(hz.to_bits())));
    key.bytes(&take.source.uid);
    key.int(i64::from(sample_rate));
    key.int(frames as i64);
    key.int(feed.file_end);
    key.int(i64::from(shape.semitones));
    key.int(i64::from(shape.cents));
    key.int(i64::from(keep_formant));
    let place = match sync {
        None => {
            key.bytes(b"straight");
            key.int(feed.file_frame);
            Place::Straight {
                file_frame: feed.file_frame,
            }
        }
        Some(sync) => {
            key.bytes(b"follow");
            key.int(take.first_frame as i64);
            key.int(i64::from(take.source.sample_rate));
            key.int(i64::from(sync.tempo.centi()));
            key.int(i64::from(sync.head_ramp));
            key.int(sync.points.len() as i64);
            for p in &sync.points {
                key.int(p.tick as i64);
                key.int(i64::from(p.tempo.centi()));
                key.int(i64::from(p.ramp));
            }
            // Where the clip is in the take, how long it is, and the song's tempo under it.
            key.int(clip.start as i64 - zero);
            key.int((clip.end - clip.start) as i64);
            let piece = tempo_piece(timeline, clip.start, clip.end);
            key.int(piece.len() as i64);
            for (tick, centi, ramp) in piece {
                key.int(tick);
                key.int(i64::from(centi));
                key.int(i64::from(ramp));
            }
            let first = take.first_frame as f64 * rate / f64::from(take.source.sample_rate.max(1));
            Place::Follow {
                clock: FileClock::new(zero, first, rate, Some(sync)),
                map: Arc::clone(map),
                start_sec: map.seconds_at(clip.start as f64),
            }
        }
    };
    // Only a tuned clip has these in its key: the key of every other clip is what it was.
    // The notes and not the scale's name: two scales of the same notes are the same audio.
    if let Some(tune) = &tune {
        key.bytes(b"tune");
        key.bytes(TUNE_PRESET.as_bytes());
        key.int(i64::from(PITCH_VERSION));
        key.int(i64::from(TUNE_VERSION));
        key.int((0..12).fold(0, |bits, n| bits | (i64::from(tune.scale[n]) << n)));
        key.int(i64::from(tune.strength));
    }
    Some(MadePlan {
        key: key.0.finalize().into(),
        clip: clip.id.0,
        audio: take.source.id.0,
        uid: take.source.uid,
        sample_rate,
        start: clip.start,
        end: clip.end,
        frames,
        place,
        file_end: feed.file_end,
        semitones: f32::from(shape.semitones) + f32::from(shape.cents) / 100.0,
        keep_formant,
        tune,
    })
}

/// What the engine and the read-ahead thread get for the vocal clips of one track, and
/// what is to be made for them.
pub struct SeatAudio {
    pub table: ClipTable,
    pub feeds: Vec<FeedClip>,
    pub plans: Vec<MadePlan>,
}

/// The vocal clips of one track for a stream at `sample_rate`. A clip whose audio is to be
/// made plays what was made when it is there (`ready`), and until then its file as it is,
/// not stretched and at its own pitch.
pub fn seat_audio(
    timeline: Option<&Timeline>,
    map: &Arc<TempoMap>,
    sample_rate: u32,
    clips: &[Arc<Clip>],
    ready: &HashSet<MadeKey>,
) -> SeatAudio {
    let (table, mut feeds, kept) = clip_tables_of(map, sample_rate, clips);
    let mut plans = Vec::new();
    for (feed, clip) in feeds.iter_mut().zip(&kept) {
        let Some(plan) = timeline.and_then(|t| plan(t, map, sample_rate, clip, feed)) else {
            continue;
        };
        if ready.contains(&plan.key) {
            *feed = made_feed(feed, &plan);
        }
        plans.push(plan);
    }
    SeatAudio {
        table,
        feeds,
        plans,
    }
}

/// Whether `new` are the clips of `old` in the same places and only what some of them
/// play is other audio (made audio for the file, or the other way): such a change is
/// laid over what sounds (`ReadAhead::splice_clips`), with no new generation.
pub fn only_audio_differs(old: &[FeedClip], new: &[FeedClip]) -> bool {
    old.len() == new.len()
        && old != new
        && (old.iter().zip(new))
            .all(|(a, b)| (a.start, a.end) == (b.start, b.end) && (a == b || a.made != b.made))
}

/// The clip as the read-ahead thread plays it once the audio of `plan` is there: the made
/// file from its first frame.
pub fn made_feed(feed: &FeedClip, plan: &MadePlan) -> FeedClip {
    FeedClip {
        made: Some(plan.key),
        file_frame: 0,
        file_end: plan.frames as i64,
        ..*feed
    }
}

#[cfg(test)]
mod tests;
