//! Recorded audio goes into the song (docs/architecture.md 22-6, 22-10): one file for one
//! vocal track, a take for every lap, each take a stretch of the file.

use std::sync::Arc;

use super::comp::comp_joined;
use super::{AudioId, AudioShape, AudioSource, AudioTake, SourceTempo};
use crate::TrackId;
use crate::edit::{Edit, EditError, Ids, MAX_TICK, RecordingAdded, with_track};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Doc, TakeId, Track};
use crate::tempo_map::{TempoPoint, Timeline};
use crate::time::TickRange;

/// The file a recording wrote, before it is in the audio table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAudio {
    pub uid: [u8; 16],
    pub file: String,
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits: Option<u8>,
}

/// One lap of a recording: `length` ticks from `origin`, which sound the file from
/// `first_frame` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewAudioTake {
    pub origin: u64,
    pub length: u64,
    pub first_frame: u64,
    pub frames: u64,
}

/// What one vocal track recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocalRecording {
    pub track: TrackId,
    pub audio: NewAudio,
    /// In the order of the laps. Never empty.
    pub takes: Vec<NewAudioTake>,
}

impl SourceTempo {
    /// The song's tempo from `origin` on, as a take that starts there keeps it.
    pub fn from_timeline(timeline: &Timeline, origin: u64) -> SourceTempo {
        let points = &timeline.tempo_points;
        let before = points.iter().rev().find(|p| p.tick <= origin);
        let (tempo, head_ramp) = match before {
            Some(p) => (p.tempo, p.ramp),
            None => (timeline.tempo, timeline.head_ramp),
        };
        // Inside a ramp the take starts at the tempo the ramp had got to, to the nearest
        // hundredth.
        let next = points.iter().find(|p| p.tick > origin);
        let tempo = match (head_ramp, next) {
            (true, Some(next)) => {
                let from_tick = before.map_or(0, |p| p.tick);
                let along = (origin - from_tick) as f64 / (next.tick - from_tick) as f64;
                let centi = f64::from(tempo.centi())
                    + (f64::from(next.tempo.centi()) - f64::from(tempo.centi())) * along;
                crate::time::Tempo::from_centi(centi.round() as u32).unwrap_or(tempo)
            }
            _ => tempo,
        };
        SourceTempo {
            tempo,
            head_ramp: head_ramp && next.is_some(),
            points: (points.iter().filter(|p| p.tick > origin))
                .map(|p| TempoPoint {
                    tick: p.tick - origin,
                    ..*p
                })
                .collect(),
        }
    }
}

fn check(recording: &VocalRecording) -> Result<(), EditError> {
    if recording.takes.is_empty() {
        return Err(EditError::NoTakes);
    }
    let audio = &recording.audio;
    let good = |t: &NewAudioTake| {
        t.length > 0
            && t.frames > 0
            && t.origin
                .checked_add(t.length)
                .is_some_and(|e| e <= MAX_TICK)
            && (t.first_frame.checked_add(t.frames)).is_some_and(|e| e <= audio.frames)
    };
    if let Some(bad) = recording.takes.iter().position(|t| !good(t)) {
        return Err(EditError::BadTake(bad));
    }
    if audio.sample_rate == 0 || !(1..=2).contains(&audio.channels) || audio.file.is_empty() {
        return Err(EditError::BadTake(0));
    }
    Ok(())
}

/// Puts what the vocal tracks recorded into `doc`: each file into the audio table, and its
/// takes on its track, the last lap shown. Clips that overlap the takes are merged with
/// them into one clip, as [`crate::edit::add_recording`] does for keys; the merged clip
/// keeps the id, the name and the shape of the earliest of them, and when one of them has
/// 「いいとこ取り」 it sounds as they did (the new takes are there to be chosen).
///
/// The step is `step` when given (the keys recorded at the same time made it: one
/// recording is one step), else a `StepKind::Record` of the first track here.
pub fn add_vocal_recordings(
    doc: &Doc,
    recordings: Vec<VocalRecording>,
    step: Option<StepKind>,
) -> Result<(Edit, Vec<RecordingAdded>), EditError> {
    if recordings.is_empty() {
        return Err(EditError::NoTakes);
    }
    let mut new = doc.clone();
    let mut ids = Ids(doc.next_id);
    let mut table: Vec<Arc<AudioSource>> = doc.audio.as_ref().clone();
    let mut added = Vec::new();
    let mut own_step = None;
    // Every file gets its id before anything else does: a file is named with its id when
    // the recording starts, as the next ids of the `Doc`, in this order.
    let mut sources = Vec::new();
    for recording in &recordings {
        check(recording)?;
        sources.push(Arc::new(AudioSource {
            id: AudioId(ids.next()?),
            uid: recording.audio.uid,
            file: recording.audio.file.clone(),
            frames: recording.audio.frames,
            sample_rate: recording.audio.sample_rate,
            channels: recording.audio.channels,
            bits: recording.audio.bits,
            sha256: None,
        }));
    }
    for (recording, source) in recordings.into_iter().zip(sources) {
        let ti = (new.tracks.iter())
            .position(|t| t.id == recording.track)
            .ok_or(EditError::NoSuchTrack(recording.track))?;
        if !new.tracks[ti].kind.is_vocal() {
            return Err(EditError::WrongKind);
        }
        table.push(source.clone());

        let ranges = (recording.takes.iter())
            .map(|t| TickRange::new(t.origin, t.origin + t.length).expect("checked"));
        let new_range = ranges.reduce(TickRange::union).expect("not empty");
        let old = new.tracks[ti].clone();
        let (hit, mut clips): (Vec<_>, Vec<_>) =
            (old.clips.iter().cloned()).partition(|c| c.range().overlaps(new_range));
        let range = (hit.iter().map(|c| c.range())).fold(new_range, TickRange::union);
        let (id, name, shape) = match hit.first() {
            Some(c) => (
                c.id,
                c.name.clone(),
                c.takes.shape().copied().unwrap_or(AudioShape::RECORDED),
            ),
            None => (ClipId(ids.next()?), old.name.clone(), AudioShape::RECORDED),
        };
        let mut merged: Vec<ClipTake<AudioTake>> = hit
            .iter()
            .flat_map(|c| {
                c.takes.vocal().iter().map(|ct| ClipTake {
                    take: ct.take.clone(),
                    offset: c.zero(ct) - range.start() as i64,
                })
            })
            .collect();
        let count = recording.takes.len();
        for t in recording.takes {
            merged.push(ClipTake {
                offset: (t.origin - range.start()) as i64,
                take: Arc::new(AudioTake {
                    id: TakeId(ids.next()?),
                    source: source.clone(),
                    first_frame: t.first_frame,
                    frames: t.frames,
                    sync: Some(SourceTempo::from_timeline(&doc.timeline, t.origin)),
                }),
            });
        }
        let active = merged.len() - 1;
        let clip = Arc::new(Clip {
            id,
            name,
            start: range.start(),
            end: range.end(),
            takes: ClipTakes::Vocal {
                takes: merged,
                shape,
            },
            active,
            comp: comp_joined(&hit, range.start()),
        });
        let at = clips.partition_point(|c| c.start < clip.start);
        clips.insert(at, clip);
        new = with_track(
            &new,
            ti,
            Track {
                clips,
                ..Track::clone(&old)
            },
        );
        own_step.get_or_insert(StepKind::Record {
            track: recording.track,
            clip: id,
            takes: count,
        });
        added.push(RecordingAdded { clip: id, active });
    }
    new.audio = Arc::new(table);
    new.next_id = ids.0;
    Ok((
        Edit {
            doc: new,
            step: step.or(own_step).expect("not empty"),
        },
        added,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit;
    use crate::time::Tempo;

    fn audio(frames: u64) -> NewAudio {
        NewAudio {
            uid: [7; 16],
            file: "歌 9.wav".into(),
            frames,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
        }
    }

    fn with_vocal() -> (Doc, TrackId) {
        let doc = Doc::new_default();
        let (e, track) = edit::add_vocal_track(&doc, None).unwrap();
        (e.doc, track)
    }

    #[test]
    fn a_recording_becomes_a_source_a_clip_and_a_take_for_every_lap() {
        let (doc, track) = with_vocal();
        let lap = |first_frame| NewAudioTake {
            origin: 7_680,
            length: 7_680,
            first_frame,
            frames: 120_000,
        };
        let recording = VocalRecording {
            track,
            audio: audio(400_000),
            takes: vec![lap(1_000), lap(121_000), lap(241_000)],
        };
        let (e, added) = add_vocal_recordings(&doc, vec![recording], None).unwrap();
        assert_eq!(e.doc.audio.len(), 1);
        let source = &e.doc.audio[0];
        assert_eq!((source.frames, source.channels), (400_000, 1));
        let t = e.doc.track(track).unwrap();
        assert_eq!(t.clips.len(), 1);
        let clip = &t.clips[0];
        assert_eq!((clip.start, clip.end, clip.active), (7_680, 15_360, 2));
        assert_eq!(clip.takes.shape(), Some(&AudioShape::RECORDED));
        let takes = clip.takes.vocal();
        assert_eq!(takes.len(), 3);
        assert!(
            takes
                .iter()
                .all(|t| Arc::ptr_eq(&t.take.source, source) && t.offset == 0)
        );
        assert_eq!(takes[1].take.first_frame, 121_000);
        assert!(
            takes[0]
                .take
                .sync
                .as_ref()
                .is_some_and(|s| s.points.is_empty())
        );
        assert_eq!(
            added,
            [RecordingAdded {
                clip: clip.id,
                active: 2
            }]
        );
        assert!(matches!(e.step, StepKind::Record { takes: 3, .. }));
        assert_eq!(
            source.id.0, doc.next_id,
            "the file's id is the first one given out"
        );
        // Every id is new and the next one is past them all.
        let ids = [
            source.id.0,
            clip.id.0,
            takes[0].take.id.0,
            takes[2].take.id.0,
        ];
        assert!(
            ids.iter()
                .all(|id| *id >= doc.next_id && *id < e.doc.next_id)
        );

        // A second recording over the clip joins it: its take is the fourth, and shown.
        let again = VocalRecording {
            track,
            audio: NewAudio {
                file: "歌 20.wav".into(),
                ..audio(50_000)
            },
            takes: vec![NewAudioTake {
                origin: 3_840,
                length: 5_760,
                first_frame: 0,
                frames: 50_000,
            }],
        };
        let (e2, added) = add_vocal_recordings(&e.doc, vec![again], None).unwrap();
        let clip2 = &e2.doc.track(track).unwrap().clips[0];
        assert_eq!((clip2.id, clip2.start, clip2.end), (clip.id, 3_840, 15_360));
        assert_eq!(clip2.takes.len(), 4);
        assert_eq!(clip2.takes.vocal()[0].offset, 3_840);
        assert_eq!(added[0].active, 3);
        assert_eq!(e2.doc.audio.len(), 2);
    }

    #[test]
    fn what_does_not_fit_is_turned_away() {
        let (doc, track) = with_vocal();
        let take = NewAudioTake {
            origin: 0,
            length: 1_920,
            first_frame: 10,
            frames: 100,
        };
        let rec = |track, frames, takes| VocalRecording {
            track,
            audio: audio(frames),
            takes,
        };
        assert_eq!(
            add_vocal_recordings(&doc, vec![rec(track, 109, vec![take])], None).err(),
            Some(EditError::BadTake(0)),
            "the take is longer than the file"
        );
        assert_eq!(
            add_vocal_recordings(&doc, vec![rec(track, 200, vec![])], None).err(),
            Some(EditError::NoTakes)
        );
        assert_eq!(
            add_vocal_recordings(&doc, vec![rec(TrackId(1), 200, vec![take])], None).err(),
            Some(EditError::WrongKind),
            "a keys track"
        );
        assert_eq!(
            add_vocal_recordings(&doc, vec![], None).err(),
            Some(EditError::NoTakes)
        );
    }

    #[test]
    fn a_take_keeps_the_tempo_it_was_sung_at() {
        let tempo = |centi| Tempo::from_centi(centi).unwrap();
        let mut timeline = Timeline::new(tempo(12_000), crate::time::Meter::FOUR_FOUR);
        timeline.tempo_points = vec![
            TempoPoint {
                tick: 1_920,
                tempo: tempo(10_000),
                ramp: true,
            },
            TempoPoint {
                tick: 3_840,
                tempo: tempo(8_000),
                ramp: false,
            },
        ];
        let head = SourceTempo::from_timeline(&timeline, 0);
        assert_eq!((head.tempo, head.head_ramp), (tempo(12_000), false));
        assert_eq!(head.points, timeline.tempo_points);

        // In the middle of the ramp from 100 to 80.
        let mid = SourceTempo::from_timeline(&timeline, 2_880);
        assert_eq!((mid.tempo, mid.head_ramp), (tempo(9_000), true));
        assert_eq!(
            mid.points,
            [TempoPoint {
                tick: 960,
                tempo: tempo(8_000),
                ramp: false
            }]
        );

        let after = SourceTempo::from_timeline(&timeline, 3_840);
        assert_eq!((after.tempo, after.head_ramp), (tempo(8_000), false));
        assert!(after.points.is_empty());
    }
}
