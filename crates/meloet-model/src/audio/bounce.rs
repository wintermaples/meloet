//! 「音のクリップにする」 (round 12, A7): the audio a keys track was turned into goes into the
//! song, as one step. The audio itself is made outside this crate.

use std::sync::Arc;

use super::{AudioId, AudioShape, AudioSource, AudioTake, TrackKind, VocalInput};
use crate::NewAudio;
use crate::automation::{AutoTarget, Lane, LaneId};
use crate::edit::{Edit, EditError, MAX_TICK, track_ids, track_index};
use crate::history::StepKind;
use crate::mixer::Channel;
use crate::song::{Clip, ClipId, ClipTake, ClipTakes, Doc, TakeId, Track};
use crate::tempo_map::TempoMap;
use crate::{InstrumentRef, Language, TrackColor, TrackId};

/// What the name of the new track has after the name of the track it was made from (in
/// Japanese; English has " (audio)").
pub const BOUNCED_SUFFIX: &str = "（音）";

/// What [`bounce_track`] made.
#[derive(Debug, Clone, PartialEq)]
pub struct Bounced {
    pub edit: Edit,
    /// The new vocal track.
    pub track: TrackId,
    pub clip: ClipId,
}

/// Lanes the new track takes over: what it copies of the strip.
fn of_the_strip(target: AutoTarget) -> bool {
    matches!(
        target,
        AutoTarget::Volume | AutoTarget::Pan | AutoTarget::Send(_)
    )
}

/// `audio`, which `from` sounded from `start` on through its instrument and inserts, goes on
/// a new vocal track right below `from`, as one clip from `start`, as long as the audio
/// sounds at the song's tempo there, not following the tempo. The new track is named as
/// `from` with [`BOUNCED_SUFFIX`] (and a number from 2 when a track has that name); it has
/// the fader, the pan and the sends of `from` and their lanes, and no inserts. `from` is
/// muted, with its mute lane at rest when it moves (so that the mute is heard), and keeps
/// everything else. The audio goes into the audio table as a recording does.
/// `WrongKind` for a vocal track; `BadTake(0)` for audio that cannot be.
pub fn bounce_track(
    doc: &Doc,
    from: TrackId,
    audio: NewAudio,
    start: u64,
    language: Language,
) -> Result<Bounced, EditError> {
    let ti = track_index(doc, from)?;
    let source_track = &doc.tracks[ti];
    if source_track.kind.is_vocal() {
        return Err(EditError::WrongKind);
    }
    let good = audio.frames > 0
        && audio.sample_rate > 0
        && (1..=2).contains(&audio.channels)
        && !audio.file.is_empty();
    if !good {
        return Err(EditError::BadTake(0));
    }
    let map = TempoMap::new(&doc.timeline);
    let seconds = audio.frames as f64 / f64::from(audio.sample_rate);
    // Up to the tick the audio ends in; a hair over a tick is the arithmetic, not audio.
    let end = (map.tick_at(map.seconds_at(start as f64) + seconds) - 1e-6).ceil() as u64;
    let end = end.max(start.saturating_add(1));
    if end > MAX_TICK {
        return Err(EditError::OffTheSong);
    }

    let mut ids = track_ids(doc)?;
    let source = Arc::new(AudioSource {
        id: AudioId(ids.next()?),
        uid: audio.uid,
        file: audio.file,
        frames: audio.frames,
        sample_rate: audio.sample_rate,
        channels: audio.channels,
        bits: audio.bits,
        sha256: None,
    });
    let track = TrackId(ids.next()?);
    let name = bounced_name(doc, &source_track.name, language);
    let clip = Arc::new(Clip {
        id: ClipId(ids.next()?),
        name: name.clone(),
        start,
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
    let mut automation = Vec::new();
    for lane in source_track.automation.iter() {
        if of_the_strip(lane.target) {
            automation.push(Arc::new(Lane {
                id: LaneId(ids.next()?),
                ..Lane::clone(lane)
            }));
        }
    }
    let clip_id = clip.id;
    let at = ti + 1;
    let new_track = Track {
        id: track,
        name,
        color: TrackColor::for_index(at),
        kind: TrackKind::Vocal {
            input: VocalInput::default(),
        },
        instrument: InstrumentRef::SINE,
        channel: Channel {
            inserts: Default::default(),
            ..source_track.channel.clone()
        },
        sends: source_track.sends,
        muted: false,
        clips: vec![clip],
        automation,
        arp: Default::default(),
        knobs: Default::default(),
    };
    let mut new = doc.clone();
    new.tracks[ti] = Arc::new(Track {
        muted: true,
        ..Track::clone(source_track)
    });
    if source_track
        .lane(AutoTarget::Mute)
        .is_some_and(|l| l.moves())
    {
        crate::automation::set_resting(&mut new, from, AutoTarget::Mute, true);
    }
    new.tracks.insert(at, Arc::new(new_track));
    let mut table = doc.audio.as_ref().clone();
    table.push(source);
    new.audio = Arc::new(table);
    new.next_id = ids.0;
    Ok(Bounced {
        edit: Edit {
            doc: new,
            step: StepKind::Bounce { track },
        },
        track,
        clip: clip_id,
    })
}

/// 「（name）（音）」, with the smallest number from 2 after it when a track has that name.
fn bounced_name(doc: &Doc, name: &str, language: Language) -> String {
    let base = format!("{name}{}", language.pick(BOUNCED_SUFFIX, " (audio)"));
    let taken = |n: &str| doc.tracks.iter().any(|t| t.name == n);
    if !taken(&base) {
        return base;
    }
    (2u32..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken(n))
        .expect("fewer tracks than numbers")
}

#[cfg(test)]
mod tests;
