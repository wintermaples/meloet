//! Changes to the tempo points, the meter changes, the markers and the loop's range. Each is
//! one step. Nothing here moves a note, a clip, a marker or a tempo point when the meter
//! changes: they keep their ticks, and only the bar lines move (a meter change keeps its bar).
//!
//! A tempo point is named by its tick and a meter change by its bar; 0 is the song's head,
//! which takes a value and can not be moved or deleted.

use std::sync::Arc;

use crate::edit::{Edit, EditError, MAX_TICK};
use crate::history::StepKind;
use crate::song::{Doc, Marker, MarkerId};
use crate::tempo_map::{MeterChange, TempoPoint, Timeline};
use crate::time::{Meter, Tempo, TickRange};

fn with_timeline(doc: &Doc, timeline: Timeline, step: StepKind) -> Edit {
    debug_assert_eq!(timeline.check(), Ok(()));
    Edit {
        doc: Doc {
            timeline: Arc::new(timeline),
            ..doc.clone()
        },
        step,
    }
}

fn point_index(timeline: &Timeline, tick: u64) -> Result<usize, EditError> {
    timeline
        .tempo_points
        .binary_search_by_key(&tick, |p| p.tick)
        .map_err(|_| EditError::NoSuchPoint)
}

fn change_index(timeline: &Timeline, bar: u32) -> Result<usize, EditError> {
    timeline
        .meter_changes
        .binary_search_by_key(&bar, |c| c.bar)
        .map_err(|_| EditError::NoSuchPoint)
}

/// The tempo at the song's head.
pub fn set_tempo(doc: &Doc, tempo: Tempo) -> Result<Edit, EditError> {
    set_tempo_at(doc, 0, tempo)
}

/// The meter at the song's head.
pub fn set_meter(doc: &Doc, meter: Meter) -> Result<Edit, EditError> {
    set_meter_at(doc, 0, meter)
}

/// The tempo of the point at `tick`; 0 is the head.
pub fn set_tempo_at(doc: &Doc, tick: u64, tempo: Tempo) -> Result<Edit, EditError> {
    let mut t = Timeline::clone(&doc.timeline);
    let (at, step) = if tick == 0 {
        let from = t.tempo;
        (&mut t.tempo, StepKind::Tempo { from, to: tempo })
    } else {
        let p = &mut t.tempo_points[point_index(&doc.timeline, tick)?];
        let from = p.tempo;
        let step = StepKind::SetTempoPoint {
            tick,
            from,
            to: tempo,
        };
        (&mut p.tempo, step)
    };
    if *at == tempo {
        return Err(EditError::Unchanged);
    }
    *at = tempo;
    Ok(with_timeline(doc, t, step))
}

/// A new tempo point. On the head or on a point, that one takes the tempo. A point put into
/// a ramp keeps the line going on to the next point.
pub fn add_tempo_point(doc: &Doc, tick: u64, tempo: Tempo) -> Result<Edit, EditError> {
    if tick == 0 || point_index(&doc.timeline, tick).is_ok() {
        return set_tempo_at(doc, tick, tempo);
    }
    if tick > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    let mut t = Timeline::clone(&doc.timeline);
    let i = t.tempo_points.partition_point(|p| p.tick < tick);
    let ramp = i < t.tempo_points.len()
        && match i.checked_sub(1) {
            Some(before) => t.tempo_points[before].ramp,
            None => t.head_ramp,
        };
    t.tempo_points.insert(i, TempoPoint { tick, tempo, ramp });
    Ok(with_timeline(
        doc,
        t,
        StepKind::AddTempoPoint { tick, tempo },
    ))
}

/// The point at `tick` goes to `to` and says `tempo`; its ramp stays. A point already at `to`
/// goes. The head (`tick` 0) only takes the tempo; no point goes to the head.
pub fn move_tempo_point(doc: &Doc, tick: u64, to: u64, tempo: Tempo) -> Result<Edit, EditError> {
    if tick == 0 || to == tick {
        return set_tempo_at(doc, tick, tempo);
    }
    if to == 0 || to > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    let mut t = Timeline::clone(&doc.timeline);
    let moved = t.tempo_points.remove(point_index(&doc.timeline, tick)?);
    t.tempo_points.retain(|p| p.tick != to);
    let i = t.tempo_points.partition_point(|p| p.tick < to);
    t.tempo_points.insert(
        i,
        TempoPoint {
            tick: to,
            tempo,
            ..moved
        },
    );
    let step = StepKind::MoveTempoPoint {
        from: tick,
        to,
        tempo,
    };
    Ok(with_timeline(doc, t, step))
}

/// Whether the tempo goes in a straight line from the point at `tick` (0: the head) to the
/// next one.
pub fn set_tempo_ramp(doc: &Doc, tick: u64, on: bool) -> Result<Edit, EditError> {
    let mut t = Timeline::clone(&doc.timeline);
    let ramp = if tick == 0 {
        &mut t.head_ramp
    } else {
        &mut t.tempo_points[point_index(&doc.timeline, tick)?].ramp
    };
    if *ramp == on {
        return Err(EditError::Unchanged);
    }
    *ramp = on;
    Ok(with_timeline(doc, t, StepKind::SetTempoRamp { tick, on }))
}

pub fn delete_tempo_point(doc: &Doc, tick: u64) -> Result<Edit, EditError> {
    let mut t = Timeline::clone(&doc.timeline);
    t.tempo_points.remove(point_index(&doc.timeline, tick)?);
    Ok(with_timeline(doc, t, StepKind::DeleteTempoPoint { tick }))
}

/// The meter of the change at the head of `bar` (from 0); 0 is the song's head.
pub fn set_meter_at(doc: &Doc, bar: u32, meter: Meter) -> Result<Edit, EditError> {
    let mut t = Timeline::clone(&doc.timeline);
    let (at, step) = if bar == 0 {
        let from = t.meter;
        (&mut t.meter, StepKind::Meter { from, to: meter })
    } else {
        let c = &mut t.meter_changes[change_index(&doc.timeline, bar)?];
        let from = c.meter;
        let step = StepKind::SetMeterChange {
            bar,
            from,
            to: meter,
        };
        (&mut c.meter, step)
    };
    if *at == meter {
        return Err(EditError::Unchanged);
    }
    *at = meter;
    Ok(with_timeline(doc, t, step))
}

/// A new meter change at the head of `bar`. On bar 0 or on a change, that one takes the meter.
pub fn add_meter_change(doc: &Doc, bar: u32, meter: Meter) -> Result<Edit, EditError> {
    if bar == 0 || change_index(&doc.timeline, bar).is_ok() {
        return set_meter_at(doc, bar, meter);
    }
    let mut t = Timeline::clone(&doc.timeline);
    let i = t.meter_changes.partition_point(|c| c.bar < bar);
    t.meter_changes.insert(i, MeterChange { bar, meter });
    Ok(with_timeline(
        doc,
        t,
        StepKind::AddMeterChange { bar, meter },
    ))
}

/// The change at `bar` goes to the head of bar `to`. A change already there goes. Bar 0 does
/// not move, and no change goes to it.
pub fn move_meter_change(doc: &Doc, bar: u32, to: u32) -> Result<Edit, EditError> {
    let at = change_index(&doc.timeline, bar)?;
    if to == bar {
        return Err(EditError::Unchanged);
    }
    if to == 0 {
        return Err(EditError::OffTheSong);
    }
    let mut t = Timeline::clone(&doc.timeline);
    let moved = t.meter_changes.remove(at);
    t.meter_changes.retain(|c| c.bar != to);
    let i = t.meter_changes.partition_point(|c| c.bar < to);
    t.meter_changes.insert(i, MeterChange { bar: to, ..moved });
    Ok(with_timeline(
        doc,
        t,
        StepKind::MoveMeterChange { from: bar, to },
    ))
}

pub fn delete_meter_change(doc: &Doc, bar: u32) -> Result<Edit, EditError> {
    let mut t = Timeline::clone(&doc.timeline);
    t.meter_changes.remove(change_index(&doc.timeline, bar)?);
    Ok(with_timeline(doc, t, StepKind::DeleteMeterChange { bar }))
}

fn with_markers(doc: &Doc, mut markers: Vec<Marker>, step: StepKind) -> Edit {
    markers.sort_by_key(|m| (m.tick, m.id));
    Edit {
        doc: Doc {
            markers: Arc::new(markers),
            ..doc.clone()
        },
        step,
    }
}

fn marker_index(doc: &Doc, id: MarkerId) -> Result<usize, EditError> {
    doc.markers
        .iter()
        .position(|m| m.id == id)
        .ok_or(EditError::NoSuchPoint)
}

/// The name as it is kept: without the spaces around it. `None` when nothing is left.
pub fn marker_name(name: &str) -> Option<&str> {
    Some(name.trim()).filter(|n| !n.is_empty())
}

/// A new marker at `tick`. Returns the edit and the marker's id.
pub fn add_marker(doc: &Doc, tick: u64, name: &str) -> Result<(Edit, MarkerId), EditError> {
    let name = marker_name(name).ok_or(EditError::Unchanged)?;
    if tick > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    let id = MarkerId(doc.next_id);
    let next_id = doc.next_id.checked_add(1).ok_or(EditError::OutOfIds)?;
    let mut markers = Vec::clone(&doc.markers);
    markers.push(Marker {
        id,
        tick,
        name: name.to_owned(),
    });
    let mut edit = with_markers(doc, markers, StepKind::AddMarker { marker: id });
    edit.doc.next_id = next_id;
    Ok((edit, id))
}

/// An empty name, or the name the marker has, is `Unchanged`.
pub fn rename_marker(doc: &Doc, id: MarkerId, name: &str) -> Result<Edit, EditError> {
    let i = marker_index(doc, id)?;
    let name = marker_name(name).ok_or(EditError::Unchanged)?;
    if doc.markers[i].name == name {
        return Err(EditError::Unchanged);
    }
    let mut markers = Vec::clone(&doc.markers);
    markers[i].name = name.to_owned();
    Ok(with_markers(
        doc,
        markers,
        StepKind::RenameMarker { marker: id },
    ))
}

pub fn move_marker(doc: &Doc, id: MarkerId, tick: u64) -> Result<Edit, EditError> {
    let i = marker_index(doc, id)?;
    if doc.markers[i].tick == tick {
        return Err(EditError::Unchanged);
    }
    if tick > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    let mut markers = Vec::clone(&doc.markers);
    markers[i].tick = tick;
    Ok(with_markers(
        doc,
        markers,
        StepKind::MoveMarker { marker: id },
    ))
}

pub fn delete_marker(doc: &Doc, id: MarkerId) -> Result<Edit, EditError> {
    let i = marker_index(doc, id)?;
    let mut markers = Vec::clone(&doc.markers);
    markers.remove(i);
    Ok(with_markers(
        doc,
        markers,
        StepKind::DeleteMarker { marker: id },
    ))
}

/// The loop's range. Whether the loop is on is not a part of the `Doc`.
pub fn set_loop(doc: &Doc, range: TickRange) -> Result<Edit, EditError> {
    if doc.loop_range == Some(range) {
        return Err(EditError::Unchanged);
    }
    if range.end() > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    Ok(Edit {
        doc: Doc {
            loop_range: Some(range),
            ..doc.clone()
        },
        step: StepKind::SetLoop { range },
    })
}

/// The song's end for a loop that starts at `from`: the end of the last clip, up to the head
/// of a bar; `bars` bars after `from` when no clip ends after it.
pub fn loop_end_from(doc: &Doc, from: u64, bars: u64) -> u64 {
    let map = crate::tempo_map::TempoMap::new(&doc.timeline);
    let last = doc
        .tracks
        .iter()
        .filter_map(|t| t.clips.last())
        .map(|c| c.end)
        .max()
        .map_or(0, |end| map.bar_at_or_after(end));
    if last > from {
        return last;
    }
    let bar = map.meter_at(from).bar;
    let span = map.tick_of_bar(bar + bars) - map.tick_of_bar(bar);
    from.saturating_add(span).min(MAX_TICK)
}

#[cfg(test)]
mod tests;
