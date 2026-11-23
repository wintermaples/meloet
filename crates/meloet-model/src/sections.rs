//! Whole sections of the song (round 12, A2): a copy of a section right after it, a section
//! taken out with what follows pulled up, and empty bars put in.
//!
//! All three are one rule, [`Splice`]: the new song is the old one up to `keep`, then `gap`
//! empty ticks, then the old one from `resume` on.
//!
//! - empty bars at `at`: keep = resume = `at`, gap = the bars
//! - a section `[from, to)` taken out: keep = `from`, resume = `to`
//! - a copy of `[from, to)` after it: keep = `to`, resume = `from` (what is between the two
//!   is there twice)
//!
//! Every cut is the head of a bar: a meter change is kept by its bar number, so time moves by
//! whole bars only.
//!
//! What has a place in the song follows the rule: clips (cut at `keep` and `resume` first),
//! the points of every lane (the master's too), markers, tempo points, meter changes and the
//! loop. Notes, control points, the offsets of takes, fades and what an audio take holds are
//! counted from their clip or take and stay as they are.
//!
//! At a seam the lines on both sides stay what they were. Where the two do not meet, a lane
//! (and a tempo line that ramps into the seam) gets a point one tick before the seam that
//! ends the left side, and a point on the seam that starts the right side. A point at a seam
//! that the line is the same without is left out, so putting bars in and taking the same
//! bars out gives back the points there were.

use std::sync::Arc;

use crate::arrange::split_clips;
use crate::automation::{AutoPoint, Lane, LaneId, line_at, on_line};
use crate::chunked::ChunkedVec;
use crate::edit::{Edit, EditError, Ids, MAX_TICK};
use crate::history::StepKind;
use crate::song::{Clip, ClipId, Doc, Marker, MarkerId};
use crate::tempo_map::{MeterChange, TempoMap, TempoPoint, Timeline};
use crate::time::{Tempo, TickRange};

/// The section of a marker: from it to the next marker that is later, or after the last one
/// to the song's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// Both ends are heads of bars. `bars`: how many bars it is long (1 or more).
    Usable { from: u64, to: u64, bars: u32 },
    /// An end is not the head of a bar.
    NotOnBars,
    /// Nothing from the marker on: the last marker, at the song's end or later.
    Empty,
}

/// The song's end for a section: the end of the last clip, up to the head of a bar; 0
/// without clips.
pub fn song_end(doc: &Doc, map: &TempoMap) -> u64 {
    (doc.tracks.iter())
        .filter_map(|t| t.clips.last())
        .map(|c| c.end)
        .max()
        .map_or(0, |end| map.bar_at_or_after(end))
}

/// The section that starts at `from`, where a marker is. `end`: [`song_end`].
pub fn section_from(doc: &Doc, map: &TempoMap, from: u64, end: u64) -> Section {
    let to = (doc.markers.iter().map(|m| m.tick))
        .find(|&tick| tick > from)
        .unwrap_or(end);
    if to <= from {
        return Section::Empty;
    }
    let on_bar = |tick: u64| map.bar_at_or_after(tick) == tick;
    let bars = u32::try_from(map.meter_at(to).bar - map.meter_at(from).bar);
    match bars {
        Ok(bars) if on_bar(from) && on_bar(to) => Section::Usable { from, to, bars },
        _ => Section::NotOnBars,
    }
}

/// The section of `marker`; `None` when the marker is not there.
pub fn section_of(doc: &Doc, marker: MarkerId) -> Option<Section> {
    let from = doc.marker(marker)?.tick;
    let map = TempoMap::new(&doc.timeline);
    Some(section_from(doc, &map, from, song_end(doc, &map)))
}

fn usable(doc: &Doc, marker: MarkerId) -> Result<(u64, u64, u32), EditError> {
    match section_of(doc, marker).ok_or(EditError::NoSuchPoint)? {
        Section::Usable { from, to, bars } => Ok((from, to, bars)),
        Section::NotOnBars | Section::Empty => Err(EditError::Unchanged),
    }
}

/// 「この区間をすぐ後ろに複製」. `NoSuchPoint` without the marker, `Unchanged` when its
/// section can not be used, `OffTheSong` when the song would get too long.
pub fn duplicate_section(doc: &Doc, marker: MarkerId) -> Result<Edit, EditError> {
    let (from, to, bars) = usable(doc, marker)?;
    Ok(Edit {
        doc: duplicate_range(doc, from, to)?,
        step: StepKind::DuplicateSection { marker, bars },
    })
}

/// 「この区間を消して詰める」. Errors as [`duplicate_section`].
pub fn delete_section(doc: &Doc, marker: MarkerId) -> Result<Edit, EditError> {
    let (from, to, bars) = usable(doc, marker)?;
    Ok(Edit {
        doc: remove_time(doc, from, to)?,
        step: StepKind::DeleteSection { marker, bars },
    })
}

/// 「この前に空きを入れる…」: `bars` bars (1 or more) of the meter at the marker, before it.
/// Errors as [`duplicate_section`].
pub fn insert_space(doc: &Doc, marker: MarkerId, bars: u32) -> Result<Edit, EditError> {
    let (at, _, _) = usable(doc, marker)?;
    Ok(Edit {
        doc: insert_time(doc, at, bars)?,
        step: StepKind::InsertSpace { marker, bars },
    })
}

/// `bars` empty bars at `at`, the head of a bar, in the meter there. What is at `at` or
/// later moves back; a clip over `at` is cut there. The values just before `at` stay all
/// through the empty bars.
pub fn insert_time(doc: &Doc, at: u64, bars: u32) -> Result<Doc, EditError> {
    let map = TempoMap::new(&doc.timeline);
    let here = map.meter_at(at);
    if bars == 0 || here.bar_tick != at {
        return Err(EditError::Unchanged);
    }
    let gap = (here.meter.ticks_per_bar())
        .checked_mul(u64::from(bars))
        .ok_or(EditError::OffTheSong)?;
    let cut = Splice {
        keep: at,
        gap,
        resume: at,
    };
    spliced(doc, cut, Bars::of(&map, cut, bars)?)
}

/// `[from, to)` goes, both heads of bars, and what is at `to` or later moves up to `from`.
/// Clips over an end are cut there. The values at `to` go on from `from`.
pub fn remove_time(doc: &Doc, from: u64, to: u64) -> Result<Doc, EditError> {
    let cut = Splice {
        keep: from,
        gap: 0,
        resume: to,
    };
    let map = TempoMap::new(&doc.timeline);
    if from >= to {
        return Err(EditError::Unchanged);
    }
    spliced(doc, cut, Bars::of(&map, cut, 0)?)
}

/// A copy of `[from, to)`, both heads of bars, at `to`; what is at `to` or later moves back
/// by its length. Clips over an end are cut there; the copies have new ids and the same
/// takes, the copied markers new ids and the same names.
pub fn duplicate_range(doc: &Doc, from: u64, to: u64) -> Result<Doc, EditError> {
    let cut = Splice {
        keep: to,
        gap: 0,
        resume: from,
    };
    let map = TempoMap::new(&doc.timeline);
    if from >= to {
        return Err(EditError::Unchanged);
    }
    spliced(doc, cut, Bars::of(&map, cut, 0)?)
}

/// The new song: the old one before `keep`, `gap` empty ticks, the old one from `resume` on.
/// `gap` is 0 unless `keep == resume`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Splice {
    keep: u64,
    gap: u64,
    resume: u64,
}

impl Splice {
    /// Where what was at `tick`, at `resume` or later, is now. `OffTheSong` past `MAX_TICK`.
    fn moved(self, tick: u64) -> Result<u64, EditError> {
        (tick - self.resume)
            .checked_add(self.keep)
            .and_then(|t| t.checked_add(self.gap))
            .filter(|&t| t <= MAX_TICK)
            .ok_or(EditError::OffTheSong)
    }

    /// Whether what was at `tick` is there after the seam, moved.
    fn resumes(self, tick: u64) -> bool {
        tick >= self.resume
    }

    /// Whether what was at `tick` stays where it is.
    fn keeps(self, tick: u64) -> bool {
        tick < self.keep
    }
}

/// The same splice in bars, for the meter changes.
#[derive(Debug, Clone, Copy)]
struct Bars {
    keep: u64,
    gap: u64,
    resume: u64,
}

impl Bars {
    /// `Unchanged` when a cut is not the head of a bar.
    fn of(map: &TempoMap, cut: Splice, gap: u32) -> Result<Bars, EditError> {
        let bar = |tick: u64| {
            let at = map.meter_at(tick);
            (at.bar_tick == tick)
                .then_some(at.bar)
                .ok_or(EditError::Unchanged)
        };
        Ok(Bars {
            keep: bar(cut.keep)?,
            gap: u64::from(gap),
            resume: bar(cut.resume)?,
        })
    }
}

fn spliced(doc: &Doc, cut: Splice, bars: Bars) -> Result<Doc, EditError> {
    let mut new = cut_clips(doc, cut.keep)?;
    if cut.resume != cut.keep {
        new = cut_clips(&new, cut.resume)?;
    }
    let mut ids = Ids(new.next_id);
    for track in &mut new.tracks {
        let clips = splice_clips(&track.clips, cut, &mut ids)?;
        let lanes = splice_lanes(&track.automation, cut, &mut ids)?;
        if clips.is_some() || lanes.is_some() {
            let t = Arc::make_mut(track);
            t.clips = clips.unwrap_or_else(|| std::mem::take(&mut t.clips));
            t.automation = lanes.unwrap_or_else(|| std::mem::take(&mut t.automation));
        }
    }
    if let Some(lanes) = splice_lanes(&new.master_lanes, cut, &mut ids)? {
        new.master_lanes = lanes;
    }
    new.markers = Arc::new(splice_markers(&doc.markers, cut, &mut ids)?);
    let timeline = splice_timeline(&doc.timeline, cut, bars)?;
    debug_assert_eq!(timeline.check(), Ok(()));
    if timeline != *doc.timeline {
        new.timeline = Arc::new(timeline);
    }
    new.loop_range = match doc.loop_range {
        Some(range) => splice_loop(range, cut)?,
        None => None,
    };
    new.next_id = ids.0;
    Ok(new)
}

/// Every clip that `at` is inside of, cut there (the rule of [`split_clips`]).
fn cut_clips(doc: &Doc, at: u64) -> Result<Doc, EditError> {
    let over: Vec<ClipId> = (doc.tracks.iter())
        .flat_map(|t| &t.clips)
        .filter(|c| c.start < at && at < c.end)
        .map(|c| c.id)
        .collect();
    match split_clips(doc, &over, at) {
        Ok(split) => Ok(split.edit.doc),
        Err(EditError::Unchanged) => Ok(doc.clone()),
        Err(e) => Err(e),
    }
}

/// `None`: the clips stay as they are. No clip is over `keep` or `resume`.
fn splice_clips(
    clips: &[Arc<Clip>],
    cut: Splice,
    ids: &mut Ids,
) -> Result<Option<Vec<Arc<Clip>>>, EditError> {
    if (clips.iter()).all(|c| cut.keeps(c.start) && !cut.resumes(c.start)) {
        return Ok(None);
    }
    let mut out: Vec<Arc<Clip>> = (clips.iter())
        .filter(|c| cut.keeps(c.start))
        .cloned()
        .collect();
    for c in clips.iter().filter(|c| cut.resumes(c.start)) {
        let start = cut.moved(c.start)?;
        let end = cut.moved(c.end)?;
        // One that also stays where it is: this is its copy.
        let id = match cut.keeps(c.start) {
            true => ClipId(ids.next()?),
            false => c.id,
        };
        out.push(Arc::new(Clip {
            id,
            start,
            end,
            ..Clip::clone(c)
        }));
    }
    Ok(Some(out))
}

fn splice_markers(
    markers: &[Marker],
    cut: Splice,
    ids: &mut Ids,
) -> Result<Vec<Marker>, EditError> {
    let mut out: Vec<Marker> = (markers.iter())
        .filter(|m| cut.keeps(m.tick))
        .cloned()
        .collect();
    for m in markers.iter().filter(|m| cut.resumes(m.tick)) {
        let id = match cut.keeps(m.tick) {
            true => MarkerId(ids.next()?),
            false => m.id,
        };
        out.push(Marker {
            id,
            tick: cut.moved(m.tick)?,
            name: m.name.clone(),
        });
    }
    out.sort_by_key(|m| (m.tick, m.id));
    Ok(out)
}

/// The loop is not a part of what is copied: it stays on the song it was put on. An end
/// inside what goes moves to the seam, and a loop all inside it goes.
fn splice_loop(range: TickRange, cut: Splice) -> Result<Option<TickRange>, EditError> {
    let (start, end) = (range.start(), range.end());
    if cut.resume > cut.keep {
        let (from, to) = (cut.keep, cut.resume);
        let at = |tick: u64| match tick {
            t if t <= from => t,
            t if t < to => from,
            t => t - (to - from),
        };
        return Ok(TickRange::new(at(start), at(end)));
    }
    // Time is put in at `keep`: as much as the gap, or as the copy is long.
    let by = cut.gap + (cut.keep - cut.resume);
    let after = |tick: u64| {
        tick.checked_add(by)
            .filter(|&t| t <= MAX_TICK)
            .ok_or(EditError::OffTheSong)
    };
    let start = if start >= cut.keep {
        after(start)?
    } else {
        start
    };
    let end = if end > cut.keep { after(end)? } else { end };
    Ok(TickRange::new(start, end))
}

/// How far a value of a lane may be from the line for a point to count as on it.
const ON_LINE: f32 = 1e-6;

/// `None`: every lane stays as it is. A lane whose points change gets a new id.
fn splice_lanes(
    lanes: &[Arc<Lane>],
    cut: Splice,
    ids: &mut Ids,
) -> Result<Option<Vec<Arc<Lane>>>, EditError> {
    let mut out = Vec::with_capacity(lanes.len());
    let mut changed = false;
    for lane in lanes {
        let old = lane.points.to_vec();
        let points = splice_points(&old, lane.step, cut)?;
        if points == old {
            out.push(lane.clone());
            continue;
        }
        changed = true;
        out.push(Arc::new(Lane {
            id: LaneId(ids.next()?),
            points: ChunkedVec::sharing(&points, &lane.points),
            ..Lane::clone(lane)
        }));
    }
    Ok(changed.then_some(out))
}

/// The points of a lane after the splice. First every point a seam could need is put in:
/// one a tick before the seam with the old value there, one on the seam with the value the
/// right side starts with (and, with a gap, one where the gap starts, holding the value from
/// before it). Then those the line is the same without go again.
fn splice_points(old: &[AutoPoint], step: bool, cut: Splice) -> Result<Vec<AutoPoint>, EditError> {
    if old.is_empty() {
        return Ok(Vec::new());
    }
    let value = |tick: u64| line_at(old, step, tick).expect("the lane has points");
    let x = cut.keep;
    let mut out: Vec<AutoPoint> = old.iter().filter(|p| cut.keeps(p.tick)).copied().collect();
    if x > 0 && out.last().is_none_or(|p| p.tick != x - 1) {
        out.push(AutoPoint {
            tick: x - 1,
            value: value(x - 1),
        });
    }
    if cut.gap > 0 {
        // The value that came up to the gap: in steps, the one held just before it.
        let before = if step && x > 0 { x - 1 } else { x };
        out.push(AutoPoint {
            tick: x,
            value: value(before),
        });
    }
    if !old.iter().any(|p| p.tick == cut.resume) {
        out.push(AutoPoint {
            tick: cut.moved(cut.resume)?,
            value: value(cut.resume),
        });
    }
    for p in old.iter().filter(|p| cut.resumes(p.tick)) {
        out.push(AutoPoint {
            tick: cut.moved(p.tick)?,
            value: p.value,
        });
    }
    let seams = [
        x.checked_sub(1),
        Some(x),
        (cut.gap > 0).then_some(x + cut.gap),
    ];
    for tick in seams.into_iter().flatten() {
        if let Ok(i) = out.binary_search_by_key(&tick, |p| p.tick)
            && not_needed(&out, i, step)
        {
            out.remove(i);
        }
    }
    Ok(out)
}

/// Whether the line is the same without the point at `i`.
fn not_needed(points: &[AutoPoint], i: usize, step: bool) -> bool {
    let p = points[i];
    let near = |value: f32| (value - p.value).abs() <= ON_LINE;
    let before = i.checked_sub(1).map(|b| points[b]);
    let after = points.get(i + 1).copied();
    match (before, after) {
        (Some(a), _) if step => near(a.value),
        (Some(a), Some(b)) => near(on_line(a, b, p.tick)),
        (Some(only), None) | (None, Some(only)) => near(only.value),
        (None, None) => false,
    }
}

/// The tempo points with the head as the first one, at tick 0.
fn tempo_line(t: &Timeline) -> Vec<TempoPoint> {
    let head = TempoPoint {
        tick: 0,
        tempo: t.tempo,
        ramp: t.head_ramp,
    };
    std::iter::once(head)
        .chain(t.tempo_points.iter().copied())
        .collect()
}

/// Whether the tempo moves from the point at `i` on: it ramps, and there is a point to ramp
/// to.
fn ramps(line: &[TempoPoint], i: usize) -> bool {
    line[i].ramp && i + 1 < line.len()
}

/// The tempo at `tick` on the straight line from `a` to `b`, in hundredths.
fn ramp_at(a: TempoPoint, b: TempoPoint, tick: u64) -> u32 {
    let (from, to) = (f64::from(a.tempo.centi()), f64::from(b.tempo.centi()));
    let f = (tick - a.tick) as f64 / (b.tick - a.tick) as f64;
    (from + f * (to - from)).round() as u32
}

fn tempo_of(centi: u32) -> Tempo {
    Tempo::from_centi(centi.clamp(Tempo::MIN_CENTI, Tempo::MAX_CENTI)).expect("in range")
}

/// A tempo is kept in hundredths, so a point made on a ramp is off the line by up to half
/// of one, and a point made from such a point by up to one.
fn near(tempo: Tempo, centi: u32) -> bool {
    tempo.centi().abs_diff(centi) <= 1
}

/// The tempo the line comes to at `tick` (1 or more) from before it.
fn tempo_before(line: &[TempoPoint], tick: u64) -> Tempo {
    let i = line.partition_point(|p| p.tick < tick) - 1;
    match ramps(line, i) {
        true => tempo_of(ramp_at(line[i], line[i + 1], tick)),
        false => line[i].tempo,
    }
}

/// What the line is from `tick` on, as a point there: its tempo, and the ramp of the point
/// in force. `.1`: whether the tempo moves from there on.
fn tempo_from(line: &[TempoPoint], tick: u64) -> (TempoPoint, bool) {
    let i = line.partition_point(|p| p.tick <= tick) - 1;
    let tempo = match ramps(line, i) && line[i].tick < tick {
        true => tempo_of(ramp_at(line[i], line[i + 1], tick)),
        false => line[i].tempo,
    };
    let point = TempoPoint {
        tick,
        tempo,
        ramp: line[i].ramp,
    };
    (point, ramps(line, i))
}

/// The tempo line (head first) after the splice.
///
/// A ramp always ends on the next point's tempo, so where a ramp comes to a seam and the
/// right side starts with another tempo, the ramp is ended one tick before the seam by a
/// point that does not ramp. Where the tempo before the seam stays, no point is needed
/// unless the right side starts with another tempo or moves.
fn splice_tempo(old: &[TempoPoint], cut: Splice) -> Result<Vec<TempoPoint>, EditError> {
    let x = cut.keep;
    let mut out: Vec<TempoPoint> = old.iter().filter(|p| cut.keeps(p.tick)).copied().collect();
    let (start, moves) = tempo_from(old, cut.resume);
    // The head is no point to keep: it is put only where the line needs it.
    let real = cut.resume > 0 && old.iter().any(|p| p.tick == cut.resume);
    if cut.gap > 0 {
        // The tempo that came up to the gap stays all through it.
        let held = match out.len() {
            0 => old[0].tempo,
            n if ramps(old, n - 1) => tempo_before(old, x),
            n => out[n - 1].tempo,
        };
        if out.is_empty() || ramps(old, out.len() - 1) {
            out.push(TempoPoint {
                tick: x,
                tempo: held,
                // A head that only says it ramps, with nothing to ramp to, keeps saying so.
                ramp: x == 0 && old[0].ramp && !moves,
            });
        }
        if real || start.tempo != held || moves {
            out.push(TempoPoint {
                tick: cut.moved(cut.resume)?,
                ..start
            });
        }
    } else if out.is_empty() {
        out.push(TempoPoint { tick: 0, ..start });
    } else {
        let last = out.len() - 1;
        if ramps(old, last) {
            if tempo_before(old, x) != start.tempo {
                if out[last].tick == x - 1 {
                    out[last].ramp = false;
                } else {
                    out.push(TempoPoint {
                        tick: x - 1,
                        tempo: tempo_of(ramp_at(old[last], old[last + 1], x - 1)),
                        ramp: false,
                    });
                }
            }
            out.push(TempoPoint { tick: x, ..start });
        } else if real || start.tempo != out[last].tempo || moves {
            out.push(TempoPoint { tick: x, ..start });
        }
    }
    for p in old.iter().filter(|p| p.tick > cut.resume) {
        out.push(TempoPoint {
            tick: cut.moved(p.tick)?,
            ..*p
        });
    }
    if cut.gap == 0 {
        tidy_tempo_seam(&mut out, x);
    }
    Ok(out)
}

/// Takes out the points at the seam `x` that the line is the same without:
/// - a point one tick before the seam that only ends a ramp, when the ramp comes to the
///   point on the seam anyway;
/// - the point on the seam, when it ramps on and is on the line from the point before it to
///   the one after it. When the point before it is the one tick before the seam, and does
///   not ramp, that one ramps again.
fn tidy_tempo_seam(line: &mut Vec<TempoPoint>, x: u64) {
    let Ok(mut at) = line.binary_search_by_key(&x, |p| p.tick) else {
        return;
    };
    if at >= 2 {
        let (before, end, seam) = (line[at - 2], line[at - 1], line[at]);
        if end.tick == x - 1
            && !end.ramp
            && before.ramp
            && near(end.tempo, ramp_at(before, seam, x - 1))
        {
            line.remove(at - 1);
            at -= 1;
        }
    }
    if at == 0 || !ramps(line, at) {
        return;
    }
    let (before, seam, after) = (line[at - 1], line[at], line[at + 1]);
    if !near(seam.tempo, ramp_at(before, after, x)) {
        return;
    }
    if before.ramp {
        line.remove(at);
    } else if before.tick == x - 1 {
        line[at - 1].ramp = true;
        line.remove(at);
    }
}

/// The meter changes with the head as the first one, at bar 0.
fn meter_line(t: &Timeline) -> Vec<MeterChange> {
    let head = MeterChange {
        bar: 0,
        meter: t.meter,
    };
    std::iter::once(head)
        .chain(t.meter_changes.iter().copied())
        .collect()
}

/// The meter changes (head first) after the splice. Empty bars are bars of the meter at the
/// cut, so a change on the cut stays and the later ones move. Else the changes from `resume`
/// on move to the seam, and a change is put on the seam when the meter there would not be
/// the one at `resume`.
fn splice_meter(old: &[MeterChange], bars: Bars) -> Result<Vec<MeterChange>, EditError> {
    let bar = |n: u64| u32::try_from(n).map_err(|_| EditError::OffTheSong);
    let moved = |c: &MeterChange| -> Result<MeterChange, EditError> {
        Ok(MeterChange {
            bar: bar(u64::from(c.bar) - bars.resume + bars.keep + bars.gap)?,
            meter: c.meter,
        })
    };
    if bars.gap > 0 {
        return (old.iter())
            .map(|c| match u64::from(c.bar) > bars.keep {
                true => moved(c),
                false => Ok(*c),
            })
            .collect();
    }
    let mut out: Vec<MeterChange> = (old.iter())
        .filter(|c| u64::from(c.bar) < bars.keep)
        .copied()
        .collect();
    let i = old.partition_point(|c| u64::from(c.bar) <= bars.resume) - 1;
    let real = bars.resume > 0 && u64::from(old[i].bar) == bars.resume;
    let meter = old[i].meter;
    if real || out.last().is_none_or(|c| c.meter != meter) {
        out.push(MeterChange {
            bar: bar(bars.keep)?,
            meter,
        });
    }
    for c in old.iter().filter(|c| u64::from(c.bar) > bars.resume) {
        out.push(moved(c)?);
    }
    Ok(out)
}

fn splice_timeline(t: &Timeline, cut: Splice, bars: Bars) -> Result<Timeline, EditError> {
    let tempo = splice_tempo(&tempo_line(t), cut)?;
    let meter = splice_meter(&meter_line(t), bars)?;
    Ok(Timeline {
        tempo: tempo[0].tempo,
        meter: meter[0].meter,
        head_ramp: tempo[0].ramp,
        tempo_points: tempo[1..].to_vec(),
        meter_changes: meter[1..].to_vec(),
    })
}

#[cfg(test)]
mod tests;
