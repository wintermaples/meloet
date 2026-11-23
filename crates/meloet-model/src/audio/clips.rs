//! Changes to vocal clips and to the song's audio table (docs/architecture.md 22-10): the
//! volume and the fades of a clip, audio files brought in, a file taken out of the song.
//! The files themselves are never touched.

use std::sync::Arc;

use super::comp::comp_renumbered;
use super::{AudioId, AudioShape, AudioSource, AudioTake, ClipGain};
use crate::edit::{Edit, EditError, Ids, MAX_TICK, add_vocal_track, clip_index, with_track};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Doc, TakeId, Track};
use crate::tempo_map::TempoMap;
use crate::time::TickRange;
use crate::{NewAudio, TrackId};

/// The shape with its fades no longer than a clip of `length` ticks: the fade-out gives way
/// first, as the fade-in is what is heard first.
pub fn fit_fades(shape: AudioShape, length: u64) -> AudioShape {
    let fade_in = shape.fade_in.min(length);
    AudioShape {
        fade_in,
        fade_out: shape.fade_out.min(length - fade_in),
        ..shape
    }
}

pub(super) fn with_shape(
    doc: &Doc,
    clip: ClipId,
    change: impl FnOnce(&AudioShape, u64) -> AudioShape,
    step: StepKind,
) -> Result<Edit, EditError> {
    let (ti, ci) = clip_index(doc, clip)?;
    let track = &doc.tracks[ti];
    let c = &track.clips[ci];
    let ClipTakes::Vocal { takes, shape } = &c.takes else {
        return Err(EditError::WrongKind);
    };
    let new = change(shape, c.end - c.start);
    if new == *shape {
        return Err(EditError::Unchanged);
    }
    let mut clips = track.clips.clone();
    clips[ci] = Arc::new(Clip {
        takes: ClipTakes::Vocal {
            takes: takes.clone(),
            shape: new,
        },
        ..Clip::clone(c)
    });
    Ok(Edit {
        doc: with_track(
            doc,
            ti,
            Track {
                clips,
                ..Track::clone(track)
            },
        ),
        step,
    })
}

/// The volume of a vocal clip. `WrongKind` for a keys clip.
pub fn set_clip_gain(doc: &Doc, clip: ClipId, gain: ClipGain) -> Result<Edit, EditError> {
    with_shape(
        doc,
        clip,
        |shape, _| AudioShape { gain, ..*shape },
        StepKind::SetClipGain { clip },
    )
}

/// One change to the shapes of several vocal clips (spec 3, 39-B and 40-A), as one step:
/// `step` is given how many clips changed. Ids that are no vocal clip's are passed by, and
/// so is a clip `change` leaves as it is. `Unchanged` when no clip changes.
pub fn set_clips_shape(
    doc: &Doc,
    clips: &[ClipId],
    change: impl Fn(&AudioShape) -> AudioShape,
    step: impl FnOnce(usize) -> StepKind,
) -> Result<Edit, EditError> {
    let mut new = doc.clone();
    let mut count = 0;
    for (ti, track) in doc.tracks.iter().enumerate() {
        let mut changed = None;
        for (ci, c) in track.clips.iter().enumerate() {
            let ClipTakes::Vocal { takes, shape } = &c.takes else {
                continue;
            };
            if !clips.contains(&c.id) {
                continue;
            }
            let shape = change(shape);
            if c.takes.shape() == Some(&shape) {
                continue;
            }
            let list = changed.get_or_insert_with(|| track.clips.clone());
            list[ci] = Arc::new(Clip {
                takes: ClipTakes::Vocal {
                    takes: takes.clone(),
                    shape,
                },
                ..Clip::clone(c)
            });
            count += 1;
        }
        if let Some(clips) = changed {
            new.tracks[ti] = Arc::new(Track {
                clips,
                ..Track::clone(track)
            });
        }
    }
    if count == 0 {
        return Err(EditError::Unchanged);
    }
    Ok(Edit {
        doc: new,
        step: step(count),
    })
}

/// Which fade of a vocal clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeEnd {
    In,
    Out,
}

/// A fade of a vocal clip, in ticks: no longer than the clip less the other fade.
pub fn set_clip_fade(doc: &Doc, clip: ClipId, end: FadeEnd, ticks: u64) -> Result<Edit, EditError> {
    with_shape(
        doc,
        clip,
        |shape, length| match end {
            FadeEnd::In => AudioShape {
                fade_in: ticks.min(length - shape.fade_out.min(length)),
                ..*shape
            },
            FadeEnd::Out => AudioShape {
                fade_out: ticks.min(length - shape.fade_in.min(length)),
                ..*shape
            },
        },
        StepKind::SetClipFade { clip },
    )
}

/// An audio file that was brought in, before it is in the audio table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAudio {
    pub audio: NewAudio,
    pub sha256: [u8; 32],
    /// The name its clip gets.
    pub name: String,
}

/// Where audio files brought in together go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportPlan {
    /// A new vocal track for each file, every clip from the same tick.
    Separate,
    /// On the track (a new vocal track for `None`), one after the other in the order given.
    /// A file that would overlap a clip there goes to one more new vocal track instead.
    InRow(Option<TrackId>),
}

/// A clip that [`import_audio`] made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportedClip {
    pub track: TrackId,
    pub clip: ClipId,
    pub audio: AudioId,
    pub start: u64,
}

/// Puts the files into the audio table and a clip for each on a vocal track, from `tick`
/// on, as one step. A clip is as long as its file sounds at the song's tempo there; the
/// audio does not follow the tempo (`AudioShape::follow` off) and has none of its own.
/// New tracks go below the last track. `WrongKind` when the track of the plan is a keys
/// track.
pub fn import_audio(
    doc: &Doc,
    files: Vec<ImportedAudio>,
    plan: ImportPlan,
    tick: u64,
    language: crate::Language,
) -> Result<(Edit, Vec<ImportedClip>), EditError> {
    if files.is_empty() {
        return Err(EditError::Unchanged);
    }
    if let ImportPlan::InRow(Some(track)) = plan {
        let t = doc.track(track).ok_or(EditError::NoSuchTrack(track))?;
        if !t.kind.is_vocal() {
            return Err(EditError::WrongKind);
        }
    }
    let good = |f: &ImportedAudio| {
        f.audio.frames > 0
            && f.audio.sample_rate > 0
            && (1..=2).contains(&f.audio.channels)
            && !f.audio.file.is_empty()
    };
    if let Some(bad) = files.iter().position(|f| !good(f)) {
        return Err(EditError::BadTake(bad));
    }
    let map = TempoMap::new(&doc.timeline);
    let mut new = doc.clone();
    let mut table: Vec<Arc<AudioSource>> = doc.audio.as_ref().clone();
    let mut made = Vec::with_capacity(files.len());
    // The track files go to when they cannot go where the plan says.
    let mut spare: Option<TrackId> = None;
    let mut cursor = tick;
    for file in files {
        let seconds = file.audio.frames as f64 / f64::from(file.audio.sample_rate);
        // Up to the tick the file ends in; a hair over a tick is the arithmetic, not audio.
        let end = (map.tick_at(map.seconds_at(cursor as f64) + seconds) - 1e-6).ceil() as u64;
        let end = end.max(cursor.saturating_add(1));
        if end > MAX_TICK {
            return Err(EditError::OffTheSong);
        }
        let range = TickRange::new(cursor, end).expect("not empty");
        let free = |doc: &Doc, track: TrackId| {
            (doc.track(track)).is_some_and(|t| !t.clips.iter().any(|c| c.range().overlaps(range)))
        };
        let wanted = match plan {
            ImportPlan::Separate => None,
            ImportPlan::InRow(track) => {
                (track.filter(|&t| free(&new, t))).or_else(|| spare.filter(|&t| free(&new, t)))
            }
        };
        let track = match wanted {
            Some(track) => track,
            None => {
                let (e, track) = add_vocal_track(&new, None, language)?;
                new = e.doc;
                spare = Some(track);
                track
            }
        };
        let mut ids = Ids(new.next_id);
        let source = Arc::new(AudioSource {
            id: AudioId(ids.next()?),
            uid: file.audio.uid,
            file: file.audio.file,
            frames: file.audio.frames,
            sample_rate: file.audio.sample_rate,
            channels: file.audio.channels,
            bits: file.audio.bits,
            sha256: Some(file.sha256),
        });
        table.push(source.clone());
        let clip = Arc::new(Clip {
            id: ClipId(ids.next()?),
            name: file.name,
            start: cursor,
            end,
            takes: ClipTakes::Vocal {
                takes: vec![ClipTake {
                    take: Arc::new(AudioTake {
                        id: TakeId(ids.next()?),
                        first_frame: 0,
                        frames: source.frames,
                        source: source.clone(),
                        sync: None,
                    }),
                    offset: 0,
                }],
                shape: AudioShape {
                    follow: false,
                    ..AudioShape::RECORDED
                },
            },
            active: 0,
            comp: Vec::new(),
        });
        made.push(ImportedClip {
            track,
            clip: clip.id,
            audio: source.id,
            start: cursor,
        });
        let ti = (new.tracks.iter().position(|t| t.id == track)).expect("found or made above");
        let old = new.tracks[ti].clone();
        let mut clips = old.clips.clone();
        clips.insert(clips.partition_point(|c| c.start < cursor), clip);
        new = with_track(
            &new,
            ti,
            Track {
                clips,
                ..Track::clone(&old)
            },
        );
        new.next_id = ids.0;
        if matches!(plan, ImportPlan::InRow(_)) {
            cursor = end;
        }
    }
    new.audio = Arc::new(table);
    let step = StepKind::ImportAudio { count: made.len() };
    Ok((Edit { doc: new, step }, made))
}

/// 「曲から外す」: every clip whose takes all play the file is gone, and so is every take of
/// it in the clips that keep other takes (the shown take then is the one before, or the
/// first; a comp with a piece of such a take goes). The file leaves the audio table. `Unchanged` when the table has no such file.
pub fn remove_audio(doc: &Doc, audio: AudioId) -> Result<Edit, EditError> {
    if doc.audio(audio).is_none() {
        return Err(EditError::Unchanged);
    }
    let plays = |ct: &ClipTake<AudioTake>| ct.take.source.id == audio;
    let mut gone = 0u32;
    let tracks = (doc.tracks.iter())
        .map(|t| {
            if !t.clips.iter().any(|c| c.takes.vocal().iter().any(plays)) {
                return t.clone();
            }
            let mut clips = Vec::with_capacity(t.clips.len());
            for c in &t.clips {
                let ClipTakes::Vocal { takes, shape } = &c.takes else {
                    clips.push(c.clone());
                    continue;
                };
                if !takes.iter().any(plays) {
                    clips.push(c.clone());
                    continue;
                }
                let kept: Vec<_> = takes.iter().filter(|ct| !plays(ct)).cloned().collect();
                if kept.is_empty() {
                    gone += 1;
                    continue;
                }
                let before = takes[..c.active].iter().filter(|ct| !plays(ct)).count();
                let active = if plays(&takes[c.active]) {
                    before.saturating_sub(1)
                } else {
                    before
                };
                let mut places = 0..;
                let places: Vec<Option<usize>> = (takes.iter())
                    .map(|ct| (!plays(ct)).then(|| places.next()).flatten())
                    .collect();
                clips.push(Arc::new(Clip {
                    takes: ClipTakes::Vocal {
                        takes: kept,
                        shape: *shape,
                    },
                    active,
                    comp: comp_renumbered(&c.comp, &places),
                    ..Clip::clone(c)
                }));
            }
            Arc::new(Track {
                clips,
                ..Track::clone(t)
            })
        })
        .collect();
    Ok(Edit {
        doc: Doc {
            tracks,
            audio: Arc::new(
                (doc.audio.iter())
                    .filter(|a| a.id != audio)
                    .cloned()
                    .collect(),
            ),
            ..doc.clone()
        },
        step: StepKind::RemoveAudio { audio, clips: gone },
    })
}

#[cfg(test)]
mod tests;
