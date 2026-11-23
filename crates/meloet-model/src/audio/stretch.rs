//! The group 「のび縮み ・ 音の高さ」 of a vocal clip (spec 3, 28-A and Y22;
//! docs/architecture.md 22-19): following the song's tempo, the tempo of the audio itself,
//! semitones, cents and 「声の感じを保つ」. Each change is one step, and the file stays as
//! it is.

use std::sync::Arc;

use super::clips::with_shape;
use super::{AudioShape, AudioTake, SourceTempo, TUNE_STRENGTH_MAX, Tune};
use crate::edit::{Edit, EditError, Ids, clip_index, with_track};
use crate::history::StepKind;
use crate::roll::ScaleMode;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Doc, TakeId, Track};
use crate::tempo_map::TempoMap;
use crate::time::{Meter, TICKS_PER_QUARTER, Tempo};

/// 「曲のテンポについていく」. A clip whose take has no tempo (「—」) keeps the switch and
/// plays at the file's speed all the same.
pub fn set_clip_follow(doc: &Doc, clip: ClipId, on: bool) -> Result<Edit, EditError> {
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            follow: on,
            ..*shape
        },
        StepKind::SetClipFollow { clip },
    )
}

/// The semitones of a vocal clip, put into −12..=12.
pub fn set_clip_semitones(doc: &Doc, clip: ClipId, semitones: i32) -> Result<Edit, EditError> {
    let most = i32::from(AudioShape::SEMITONES_MAX);
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            semitones: semitones.clamp(-most, most) as i8,
            ..*shape
        },
        StepKind::SetClipPitch { clip },
    )
}

/// The cents of a vocal clip, put into −50..=50.
pub fn set_clip_cents(doc: &Doc, clip: ClipId, cents: i32) -> Result<Edit, EditError> {
    let most = i32::from(AudioShape::CENTS_MAX);
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            cents: cents.clamp(-most, most) as i8,
            ..*shape
        },
        StepKind::SetClipPitch { clip },
    )
}

/// 「声の感じを保つ」.
pub fn set_clip_keep_formant(doc: &Doc, clip: ClipId, on: bool) -> Result<Edit, EditError> {
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            keep_formant: on,
            ..*shape
        },
        StepKind::SetClipPitch { clip },
    )
}

/// 「音程を寄せる」, the strength in percent, put into 0..=100. At 0 the clip is not tuned and
/// keeps no scale. Leaving 0 it takes `root` and `mode` (the song's scale at that moment);
/// between two strengths above 0 it keeps the scale it has.
pub fn set_clip_tune_strength(
    doc: &Doc,
    clip: ClipId,
    percent: i32,
    root: u8,
    mode: ScaleMode,
) -> Result<Edit, EditError> {
    if root > 11 {
        return Err(EditError::Unchanged);
    }
    let strength = percent.clamp(0, i32::from(TUNE_STRENGTH_MAX)) as u8;
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            tune: (strength > 0).then_some(match shape.tune {
                Some(tune) => Tune { strength, ..tune },
                None => Tune {
                    root,
                    mode,
                    strength,
                },
            }),
            ..*shape
        },
        StepKind::SetClipTune { clip },
    )
}

/// 「いまのスケールにする」: a tuned clip takes `root` and `mode`. `Unchanged` for a clip that
/// is not tuned (strength 0) and for the scale it has.
pub fn set_clip_tune_scale(
    doc: &Doc,
    clip: ClipId,
    root: u8,
    mode: ScaleMode,
) -> Result<Edit, EditError> {
    if root > 11 {
        return Err(EditError::Unchanged);
    }
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape {
            tune: shape.tune.map(|tune| Tune { root, mode, ..tune }),
            ..*shape
        },
        StepKind::SetClipTune { clip },
    )
}

/// 「この音のテンポ」: the audio of the shown take was played at `tempo`, all along. A take
/// is never changed, so the clip gets a take with a new id in the same place, with the same
/// audio and the same range of it; another clip that shows the old take keeps it. Returns
/// the new take's id. `Unchanged` when the take has that one tempo already.
pub fn set_source_tempo(
    doc: &Doc,
    clip: ClipId,
    tempo: Tempo,
) -> Result<(Edit, TakeId), EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let ClipTakes::Vocal { takes, shape } = &c.takes else {
        return Err(EditError::WrongKind);
    };
    let shown = &takes[c.active];
    let sync = SourceTempo::steady(tempo);
    if shown.take.sync.as_ref() == Some(&sync) {
        return Err(EditError::Unchanged);
    }
    let mut ids = Ids(doc.next_id);
    let id = TakeId(ids.next()?);
    let mut takes = takes.clone();
    takes[c.active] = ClipTake {
        take: Arc::new(AudioTake {
            id,
            sync: Some(sync),
            ..AudioTake::clone(&shown.take)
        }),
        offset: shown.offset,
    };
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        takes: ClipTakes::Vocal {
            takes,
            shape: *shape,
        },
        ..Clip::clone(c)
    });
    let mut new = with_track(
        doc,
        ti,
        Track {
            clips,
            ..Track::clone(track)
        },
    );
    new.next_id = ids.0;
    let step = StepKind::SetSourceTempo { clip };
    Ok((Edit { doc: new, step }, id))
}

/// A tempo the audio may have been played at: with it the audio is `bars` bars long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TempoGuess {
    pub bars: u8,
    pub tempo: Tempo,
}

/// The lengths 「さがす」 tries, in bars.
pub const GUESS_BARS: [u8; 4] = [1, 2, 4, 8];
/// The tempos it takes, to the hundredth: 60.00..=180.00.
pub const GUESS_MIN: u32 = 6_000;
pub const GUESS_MAX: u32 = 18_000;

/// 「さがす」: the tempos at which audio of `frames` frames is 1, 2, 4 or 8 bars of `meter`
/// long, those of them from 60 to 180 (docs/architecture.md 22-8), slowest first. Each bar
/// count has twice the tempo of the one before, so there are two at most, and none for
/// audio shorter than a bar at 180 or longer than eight at 60.
pub fn source_tempo_guesses(frames: u64, sample_rate: u32, meter: Meter) -> Vec<TempoGuess> {
    if frames == 0 || sample_rate == 0 {
        return Vec::new();
    }
    let minutes = frames as f64 / f64::from(sample_rate) / 60.0;
    let quarters = meter.ticks_per_bar() as f64 / TICKS_PER_QUARTER as f64;
    (GUESS_BARS.iter())
        .filter_map(|&bars| {
            let tempo = Tempo::from_bpm(f64::from(bars) * quarters / minutes)?;
            (GUESS_MIN..=GUESS_MAX)
                .contains(&tempo.centi())
                .then_some(TempoGuess { bars, tempo })
        })
        .collect()
}

/// The guess nearest to `bpm` as a ratio (120 is as far from 60 as from 240); of two as
/// near, the slower.
pub fn pick_guess(guesses: &[TempoGuess], bpm: f64) -> Option<TempoGuess> {
    let far = |g: &TempoGuess| (g.tempo.bpm() / bpm).ln().abs();
    (guesses.iter().copied()).reduce(|best, g| if far(&g) < far(&best) { g } else { best })
}

/// What 「さがす」 finds for a vocal clip: the length of its shown take as bars of the meter
/// at the clip's start, and of the tempos that fit the one nearest to the song's tempo
/// there. `Ok(None)`: no tempo fits. `WrongKind` for a keys clip.
pub fn guess_source_tempo(doc: &Doc, clip: ClipId) -> Result<Option<TempoGuess>, EditError> {
    let (_, c) = doc.clip(clip).ok_or(EditError::NoSuchClip(clip))?;
    let shown = c.active_audio().ok_or(EditError::WrongKind)?;
    let map = TempoMap::new(&doc.timeline);
    let guesses = source_tempo_guesses(
        shown.take.frames,
        shown.take.source.sample_rate,
        map.meter_at(c.start).meter,
    );
    Ok(pick_guess(&guesses, map.tempo_at(c.start as f64)))
}

#[cfg(test)]
mod tests;
