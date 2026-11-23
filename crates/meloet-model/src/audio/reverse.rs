//! 「ノーマライズ」 and 「逆にする」 of vocal clips (round 12, A9): which frames of its file a
//! clip plays, the volume that brings the loudest of them to −1.0 dBFS, and the take that
//! plays them backwards. The samples are read and written outside this crate; the file a
//! clip plays is never changed.

use std::sync::Arc;

use super::clips::with_shape;
use super::follow::tempo_piece;
use super::{AudioId, AudioShape, AudioSource, AudioTake, ClipGain, SourceTempo};
use crate::NewAudio;
use crate::edit::{Edit, EditError, Ids, clip_index, with_track};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Doc, TakeId, Track};
use crate::tempo_map::{TempoMap, Timeline};
use crate::time::{Meter, Tempo};

/// Where 「ノーマライズ」 puts the loudest sample, in dBFS.
pub const NORMALIZE_TO_DB: f64 = -1.0;

/// The frames of its file that a vocal clip plays from its start to its end, with the take
/// it shows. Two are equal only when the clip plays the same frames of the same file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayedFrames {
    pub clip: ClipId,
    pub take: TakeId,
    pub source: Arc<AudioSource>,
    /// The frame of the file at the clip's start and at its end. The clip is silent where
    /// they are below 0 or from `audio_end` on.
    pub from: i64,
    pub to: i64,
    /// The take, or the file, has no frames from here on.
    pub audio_end: u64,
}

impl PlayedFrames {
    /// The frames that sound: `from..to` inside `0..audio_end`. Empty for a silent clip.
    pub fn sounding(&self) -> std::ops::Range<u64> {
        let start = self.from.max(0) as u64;
        let end = (self.to.max(0) as u64).min(self.audio_end).max(start);
        start..end
    }

    /// How many frames the backwards file has: from the clip's end back to its start, or to
    /// the file's frame 0. What is silent at the clip's head is past the backwards file's
    /// end, where its clip is silent anyway; what is silent at the clip's end is written.
    pub fn backwards_frames(&self) -> u64 {
        (self.to - self.from.max(0)).max(0) as u64
    }
}

/// The frames the clip plays, as the read-ahead thread lays them out: each end of the clip
/// and the take's tick 0 rounded to frames of the file by itself (`pcm::feed_clip`). `None`
/// for a keys clip or a clip that is not there.
pub fn played_frames(doc: &Doc, clip: ClipId) -> Option<PlayedFrames> {
    let (_, c) = doc.clip(clip)?;
    let shown = c.active_audio()?;
    let take = &shown.take;
    let follow = c.takes.shape()?.follow;
    let rate = f64::from(take.source.sample_rate);
    let zero = c.active_zero();
    let frame = |seconds: f64| (seconds * rate).round() as i64;
    let first = i64::try_from(take.first_frame).ok()?;
    let (from, to) = match take.sync.as_ref().filter(|_| follow) {
        // The ticks of the song are ticks of the take.
        Some(sync) => {
            let own = sync.map();
            let at = |tick: u64| frame(own.seconds_at((tick as i64 - zero) as f64));
            (first + at(c.start), first + at(c.end))
        }
        None => {
            let map = TempoMap::new(&doc.timeline);
            let zero = frame(map.seconds_at(zero as f64));
            let at = |tick: u64| frame(map.seconds_at(tick as f64)) - zero;
            (first + at(c.start), first + at(c.end))
        }
    };
    Some(PlayedFrames {
        clip,
        take: take.id,
        source: take.source.clone(),
        from,
        to: to.max(from),
        audio_end: (take.first_frame.saturating_add(take.frames)).min(take.source.frames),
    })
}

/// The volume that brings audio whose loudest sample is `peak` (of full scale) to
/// [`NORMALIZE_TO_DB`], put into the range of a clip's volume; and whether it had to be put
/// in. `None` for silence, and for a peak that is not a number.
pub fn normalize_gain(peak: f32) -> Option<(ClipGain, bool)> {
    if !(peak > 0.0 && peak.is_finite()) {
        return None;
    }
    let db = NORMALIZE_TO_DB - 20.0 * f64::from(peak).log10();
    let centi = (db * 100.0).round();
    let limited = !(ClipGain::MIN_DB * 100.0..=ClipGain::MAX_DB * 100.0).contains(&centi);
    Some((ClipGain::from_db(db)?, limited))
}

/// 「ノーマライズ」 as one step: each clip that still plays the frames it was measured at
/// gets its volume. A clip that is gone, shows another take or plays other frames now is
/// passed over. `Unchanged` when no clip changes.
pub fn normalize_clips(doc: &Doc, gains: &[(PlayedFrames, ClipGain)]) -> Result<Edit, EditError> {
    let mut new = doc.clone();
    let mut count = 0;
    for (played, gain) in gains {
        if played_frames(&new, played.clip).as_ref() != Some(played) {
            continue;
        }
        let change = |shape: &AudioShape, _| AudioShape {
            gain: *gain,
            ..*shape
        };
        match with_shape(
            &new,
            played.clip,
            change,
            StepKind::NormalizeClips { count },
        ) {
            Ok(e) => {
                new = e.doc;
                count += 1;
            }
            Err(EditError::Unchanged) => {}
            Err(e) => return Err(e),
        }
    }
    if count == 0 {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: new,
        step: StepKind::NormalizeClips { count },
    })
}

/// A file 「逆にする」 made: the frames `played` backwards, `played.backwards_frames()` of
/// them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reversed {
    pub played: PlayedFrames,
    pub audio: NewAudio,
}

/// What [`reverse_clips`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipsReversed {
    pub edit: Edit,
    /// Clips that followed the song's tempo and do not now: the tempo of their audio
    /// changes along them.
    pub follow_off: u32,
}

/// The tempo the backwards audio of the clip's shown take was played at, and whether the
/// clip follows the song's tempo with it. A take with one tempo all along the clip keeps it;
/// a take whose tempo changes there gives audio without a tempo, which does not follow.
fn backwards_tempo(c: &Clip, take: &AudioTake, follow: bool) -> (Option<SourceTempo>, bool) {
    let Some(sync) = &take.sync else {
        return (None, follow);
    };
    // Before the take's tick 0 the head's tempo goes on: the piece is looked at from 0.
    let zero = c.active_zero();
    let from = u64::try_from(c.start as i64 - zero).unwrap_or(0);
    let to = u64::try_from(c.end as i64 - zero)
        .unwrap_or(0)
        .max(from + 1);
    let own = Timeline {
        tempo: sync.tempo,
        meter: Meter::FOUR_FOUR,
        head_ramp: sync.head_ramp,
        tempo_points: sync.points.clone(),
        meter_changes: Vec::new(),
    };
    match tempo_piece(&own, from, to)[..] {
        [(_, centi, false)] => match Tempo::from_centi(centi) {
            Some(tempo) => (Some(SourceTempo::steady(tempo)), follow),
            None => (None, false),
        },
        _ => (None, false),
    }
}

/// 「逆にする」 as one step: each file goes into the audio table, and its clip shows, in place
/// of the take the file was made of, a new take that plays the file from the clip's start
/// (its tick 0 there, from the file's first frame). The clip's other takes, fades and volume
/// stay. A file for a clip that is gone, shows another take or plays other frames now (it
/// was moved where the tempo is another, or trimmed) is passed over. `Unchanged` when that is
/// every file.
pub fn reverse_clips(doc: &Doc, made: &[Reversed]) -> Result<ClipsReversed, EditError> {
    let mut new = doc.clone();
    let mut table: Vec<Arc<AudioSource>> = doc.audio.as_ref().clone();
    let mut ids = Ids(doc.next_id);
    let mut count = 0;
    let mut follow_off = 0;
    for Reversed { played, audio } in made {
        let fits = audio.frames == played.backwards_frames()
            && audio.frames > 0
            && audio.sample_rate > 0
            && (1..=2).contains(&audio.channels)
            && !audio.file.is_empty();
        if !fits || played_frames(&new, played.clip).as_ref() != Some(played) {
            continue;
        }
        let (ti, ci) = clip_index(&new, played.clip)?;
        let track = new.tracks[ti].clone();
        let c = &track.clips[ci];
        let ClipTakes::Vocal { takes, shape } = &c.takes else {
            continue;
        };
        let (sync, follow) = backwards_tempo(c, &takes[c.active].take, shape.follow);
        follow_off += u32::from(shape.follow && !follow);
        let source = Arc::new(AudioSource {
            id: AudioId(ids.next()?),
            uid: audio.uid,
            file: audio.file.clone(),
            frames: audio.frames,
            sample_rate: audio.sample_rate,
            channels: audio.channels,
            bits: audio.bits,
            sha256: None,
        });
        table.push(source.clone());
        let mut takes = takes.clone();
        takes[c.active] = ClipTake {
            take: Arc::new(AudioTake {
                id: TakeId(ids.next()?),
                source,
                first_frame: 0,
                frames: audio.frames,
                sync,
            }),
            offset: 0,
        };
        let mut clips = track.clips.clone();
        clips[ci] = Arc::new(Clip {
            takes: ClipTakes::Vocal {
                takes,
                shape: AudioShape { follow, ..*shape },
            },
            ..Clip::clone(c)
        });
        new = with_track(
            &new,
            ti,
            Track {
                clips,
                ..Track::clone(&track)
            },
        );
        count += 1;
    }
    if count == 0 {
        return Err(EditError::Unchanged);
    }
    new.audio = Arc::new(table);
    new.next_id = ids.0;
    Ok(ClipsReversed {
        edit: Edit {
            doc: new,
            step: StepKind::ReverseClips { count },
        },
        follow_off,
    })
}

#[cfg(test)]
mod tests;
