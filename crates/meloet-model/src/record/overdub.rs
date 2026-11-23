//! 「重ねる」: what a recording on a keys track played goes into the shown take of the clip
//! there, as an edit of that take (a new take id in its place; the clip keeps its number of
//! takes).
//!
//! The clips "there" are the ones that hold the start of a recorded note, or lie between such
//! clips. With none, the recording makes a new clip as without 「重ねる」, but only in the empty
//! place around the notes: it does not take in the clips next to it. With one, the notes go
//! into its shown take; with more, they are made one clip, as [`crate::edit::add_recording`]
//! merges the clips a recording overlaps: the earliest gives its id and name, every take of
//! them is kept, and the shown takes become one, where the earliest one's was. Only what each
//! clip shows inside its range goes into it, a note running past its clip's end cut there.
//!
//! Notes that start or end outside make the clip grow by whole bars, as written notes do
//! (B-82), but not over another clip: it stops where the next one starts, and a note reaching
//! past that stays in the take, trimmed off.

use std::collections::HashSet;
use std::sync::Arc;

use crate::TrackId;
use crate::chunked::ChunkedVec;
use crate::edit::{
    Edit, EditError, Ids, MAX_TICK, NewTake, RecordingAdded, add_recording, track_index, with_track,
};
use crate::history::StepKind;
use crate::song::{
    Clip, ClipTake, ClipTakes, ControlPoint, ControlValue, Doc, Note, Take, TakeId, TakeOrigin,
    Track,
};
use crate::tempo_map::TempoMap;

/// Adds what `take` holds to the clips of `track` there (see the module). The step is a
/// `StepKind::Record` that adds no take.
pub fn overdub_recording(
    doc: &Doc,
    track: TrackId,
    take: NewTake,
) -> Result<(Edit, RecordingAdded), EditError> {
    let ti = track_index(doc, track)?;
    let old = &doc.tracks[ti];
    if old.kind.is_vocal() {
        return Err(EditError::NotKeys);
    }
    if !take.is_valid() {
        return Err(EditError::BadTake(0));
    }
    if take.notes.is_empty() {
        return Err(EditError::NoTakes);
    }
    let origin = take.origin as i64;
    let first = origin + take.notes.iter().map(|n| n.start).min().expect("notes") as i64;
    let last_start = origin + take.notes.iter().map(|n| n.start).max().expect("notes") as i64;
    let last_end = origin + take.notes.iter().map(|n| n.end()).max().expect("notes") as i64;
    let hit: Vec<usize> = (old.clips.iter().enumerate())
        .filter(|(_, c)| (c.start as i64) <= last_start && first < c.end as i64)
        .map(|(i, _)| i)
        .collect();
    let (Some(&h0), Some(&hn)) = (hit.first(), hit.last()) else {
        return add_recording(
            doc,
            track,
            vec![in_the_gap(old, take, first, last_start)],
            0,
        );
    };

    let map = TempoMap::new(&doc.timeline);
    let (u_start, u_end) = (old.clips[h0].start, old.clips[hn].end);
    let mut start = if (first as u64) < u_start {
        map.meter_at(first as u64).bar_tick
    } else {
        u_start
    };
    let mut end = if last_end as u64 > u_end {
        map.bar_at_or_after(last_end as u64).min(MAX_TICK)
    } else {
        u_end
    };
    for (i, c) in old.clips.iter().enumerate() {
        if hit.contains(&i) {
            continue;
        }
        if c.end <= u_start {
            start = start.max(c.end);
        } else if c.start >= u_end {
            end = end.min(c.start);
        }
    }

    let primary = &old.clips[h0];
    let single = hit.len() == 1;
    let mut notes: Vec<(i64, Note)> = Vec::new();
    let mut controls: Vec<(i64, ControlValue)> = Vec::new();
    for &i in &hit {
        let c = &old.clips[i];
        let shown = c.active_take();
        let zero = c.zero(shown);
        for n in shown.take.notes.iter() {
            let at = zero + n.start as i64;
            if single {
                notes.push((at, *n));
            } else if c.holds(n.start) {
                let length = n.length.min((c.end as i64 - at) as u64);
                notes.push((
                    at,
                    Note {
                        length,
                        from: None,
                        ..*n
                    },
                ));
            }
        }
        for p in shown.take.controls.iter() {
            if single || c.holds(p.tick) {
                controls.push((zero + p.tick as i64, p.value));
            }
        }
    }
    let known: HashSet<(i64, ControlValue)> = controls.iter().copied().collect();
    notes.extend((take.notes.iter()).map(|n| (origin + n.start as i64, Note { from: None, ..*n })));
    controls.extend(
        (take.controls.iter())
            .map(|p| (origin + p.tick as i64, p.value))
            .filter(|p| !known.contains(p)),
    );
    notes.sort_by_key(|(at, n)| (*at, n.key));
    controls.sort_by_key(|(at, _)| *at);

    let shown = primary.active_take();
    let old_zero = primary.zero(shown);
    let content_start = (notes.iter().map(|(at, _)| *at))
        .chain(controls.iter().map(|(at, _)| *at))
        .min()
        .expect("notes");
    let content_end = (notes.iter().map(|(at, n)| at + n.length as i64))
        .chain(controls.iter().map(|(at, _)| at + 1))
        .max()
        .expect("notes");
    let (zero, reach) = if single {
        (
            content_start.min(old_zero),
            content_end.max(old_zero + shown.take.length as i64),
        )
    } else {
        (content_start.min(start as i64), content_end.max(end as i64))
    };

    let mut ids = Ids(doc.next_id);
    let id = TakeId(ids.next()?);
    let shift = (old_zero - zero) as u64;
    let take_origin = match &shown.take.origin {
        Some(o) if single && shift > 0 => Some(TakeOrigin {
            take: Arc::new(shifted(&o.take, shift, TakeId(ids.next()?))),
            settings: o.settings,
        }),
        o if single => o.clone(),
        _ => None,
    };
    let notes: Vec<Note> = (notes.into_iter())
        .map(|(at, n)| Note {
            start: (at - zero) as u64,
            ..n
        })
        .collect();
    let controls: Vec<ControlPoint> = (controls.into_iter())
        .map(|(at, value)| ControlPoint {
            tick: (at - zero) as u64,
            value,
        })
        .collect();
    let (notes, controls) = if single {
        (
            ChunkedVec::sharing(&notes, &shown.take.notes),
            ChunkedVec::sharing(&controls, &shown.take.controls),
        )
    } else {
        (notes.into(), controls.into())
    };
    let new_take = ClipTake {
        take: Arc::new(Take {
            id,
            length: (reach - zero) as u64,
            notes,
            controls,
            origin: take_origin,
        }),
        offset: zero - start as i64,
    };

    let mut takes: Vec<ClipTake> = Vec::new();
    let mut active = 0;
    let mut new_take = Some(new_take);
    for &i in &hit {
        let c = &old.clips[i];
        for (k, ct) in c.keys().iter().enumerate() {
            if k == c.active {
                if let Some(t) = new_take.take() {
                    active = takes.len();
                    takes.push(t);
                }
                continue;
            }
            takes.push(ClipTake {
                take: ct.take.clone(),
                offset: c.zero(ct) - start as i64,
            });
        }
    }

    let clip = Arc::new(Clip {
        id: primary.id,
        name: primary.name.clone(),
        start,
        end,
        takes: ClipTakes::Keys(takes),
        active,
        comp: Vec::new(),
    });
    let mut clips: Vec<Arc<Clip>> = (old.clips.iter().enumerate())
        .filter(|(i, _)| !hit.contains(i))
        .map(|(_, c)| c.clone())
        .collect();
    let at = clips.partition_point(|c| c.start < start);
    clips.insert(at, clip);
    let mut new = with_track(
        doc,
        ti,
        Track {
            clips,
            ..Track::clone(old)
        },
    );
    new.next_id = ids.0;
    Ok((
        Edit {
            doc: new,
            step: StepKind::Record {
                track,
                clip: primary.id,
                takes: 0,
            },
        },
        RecordingAdded {
            clip: primary.id,
            active,
        },
    ))
}

/// `take` cut to the empty place of the track around its notes, `first` to `last_start`.
fn in_the_gap(track: &Track, take: NewTake, first: i64, last_start: i64) -> NewTake {
    let low = (track.clips.iter())
        .filter(|c| c.end as i64 <= first)
        .map(|c| c.end)
        .max()
        .unwrap_or(0)
        .max(take.origin);
    let high = (track.clips.iter())
        .filter(|c| c.start as i64 > last_start)
        .map(|c| c.start)
        .min()
        .unwrap_or(u64::MAX)
        .min(take.origin + take.length);
    let cut = low - take.origin;
    let length = high - low;
    NewTake {
        origin: low,
        length,
        notes: (take.notes.into_iter())
            .map(|n| {
                let start = n.start - cut;
                Note {
                    start,
                    length: n.length.min(length - start),
                    ..n
                }
            })
            .collect(),
        controls: (take.controls.into_iter())
            .filter(|p| p.tick >= cut && p.tick - cut < length)
            .map(|p| ControlPoint {
                tick: p.tick - cut,
                ..p
            })
            .collect(),
    }
}

fn shifted(take: &Take, shift: u64, id: TakeId) -> Take {
    Take {
        id,
        length: take.length + shift,
        notes: (take.notes.iter())
            .map(|n| Note {
                start: n.start + shift,
                ..*n
            })
            .collect(),
        controls: (take.controls.iter())
            .map(|c| ControlPoint {
                tick: c.tick + shift,
                ..*c
            })
            .collect(),
        origin: None,
    }
}

#[cfg(test)]
mod tests;
