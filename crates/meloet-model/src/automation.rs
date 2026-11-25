//! Automation (docs/architecture.md 21-5, 21-9): the lanes of a track, their points, and the
//! steps on them.
//!
//! A value is the place of the knob a person sees, 0.0..=1.0. Between two points the line is
//! straight over ticks; in a lane with `step` the value stays at the point before. Before the
//! first point it is the first point's value, after the last the last's. A lane with no point
//! moves nothing: what sounds is the value the hand put, as for a lane that rests.
//!
//! A lane whose points change gets a new `LaneId` from `Doc::next_id`, as a take does: a
//! command aimed at the points as they were finds no lane.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::chunked::ChunkedVec;
use crate::edit::{Edit, EditError, Ids, MAX_TICK, track_index};
use crate::history::StepKind;
use crate::mixer::{Bus, Gain, InsertId};
use crate::song::{Doc, Track};
use crate::tempo_map::TempoMap;
use crate::{InstrumentRef, TickRange, TrackId};

/// Where a lane lives: on a track, or on the master, which has one of its volume only. The
/// returns have no lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LaneHome {
    Track(TrackId),
    Master,
}

impl From<TrackId> for LaneHome {
    fn from(track: TrackId) -> LaneHome {
        LaneHome::Track(track)
    }
}

impl LaneHome {
    pub fn track(self) -> Option<TrackId> {
        match self {
            LaneHome::Track(track) => Some(track),
            LaneHome::Master => None,
        }
    }
}

/// What a lane moves. The master's lane moves its fader ([`AutoTarget::Volume`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoTarget {
    /// The fader.
    Volume,
    Pan,
    Send(Bus),
    Mute,
    /// A parameter of the plugin that is the track's instrument, by the plugin's own number.
    Instrument {
        param: u32,
    },
    /// A parameter of what sits in an insert slot of the track: its place in the table for an
    /// effect that comes with meloet, the plugin's own number for a plugin.
    Insert {
        insert: InsertId,
        param: u32,
    },
}

/// Names the points of a lane as they are; a new one whenever they change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LaneId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoPoint {
    pub tick: u64,
    /// 0.0..=1.0.
    pub value: f32,
}

/// A value in a lane is never NaN (`clamp_value`, `Doc::check_lanes`), so a point equals itself.
impl Eq for AutoPoint {}

impl Hash for AutoPoint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tick.hash(state);
        // 0.0 and -0.0 are equal and must hash alike.
        (self.value + 0.0).to_bits().hash(state);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lane {
    pub id: LaneId,
    pub target: AutoTarget,
    /// The value stays at the point before (a stepped value, a switch, the mute).
    pub step: bool,
    /// In the order of tick, each tick once. A jump is two points a tick apart. A lane made
    /// from another shares the chunks that did not change (docs/architecture.md 16, 21-5).
    pub points: ChunkedVec<AutoPoint>,
    /// 「休み」 (spec 3, 17-A): the lane moves nothing; the value is where the hand put it.
    pub resting: bool,
}

/// How far, as a place of the knob, a written line may stray from the points left out of it
/// (docs/architecture.md 21-9; to be settled by measuring).
pub const THIN_TOLERANCE: f32 = 0.005;

impl Lane {
    /// The value of the line at `tick`; `None` with no points.
    pub fn value_at(&self, tick: u64) -> Option<f32> {
        let first = self.points.first()?;
        let i = self.points.partition_point(|p| p.tick <= tick);
        if i == 0 {
            return Some(first.value);
        }
        Some(between(
            self.points[i - 1],
            self.points.get(i).copied(),
            self.step,
            tick,
        ))
    }

    /// Whether the lane moves its target: it does not rest and has points.
    pub fn moves(&self) -> bool {
        !self.resting && !self.points.is_empty()
    }
}

pub(crate) fn line_at(points: &[AutoPoint], step: bool, tick: u64) -> Option<f32> {
    let first = points.first()?;
    let i = points.partition_point(|p| p.tick <= tick);
    if i == 0 {
        return Some(first.value);
    }
    Some(between(points[i - 1], points.get(i).copied(), step, tick))
}

/// The value at `tick` of the straight line from `a` to `b`, at `a` or after it.
pub(crate) fn on_line(a: AutoPoint, b: AutoPoint, tick: u64) -> f32 {
    between(a, Some(b), false, tick)
}

/// The line's value at `tick`, at or after the point `a` and before the next one `b`.
fn between(a: AutoPoint, b: Option<AutoPoint>, step: bool, tick: u64) -> f32 {
    match b {
        Some(b) if !step && a.tick != tick => {
            let f = (tick - a.tick) as f64 / (b.tick - a.tick) as f64;
            (f64::from(a.value) + f * f64::from(b.value - a.value)) as f32
        }
        _ => a.value,
    }
}

impl Track {
    pub fn lane(&self, target: AutoTarget) -> Option<&Arc<Lane>> {
        self.automation.iter().find(|l| l.target == target)
    }

    /// Whether the track has what `target` moves: a plugin as its instrument, a knob its
    /// sound that comes with meloet uses (a vocal track has neither), an insert of that id
    /// (and for an effect of meloet, a parameter of that place in its table).
    pub fn has_target(&self, target: AutoTarget) -> bool {
        match target {
            AutoTarget::Volume | AutoTarget::Pan | AutoTarget::Send(_) | AutoTarget::Mute => true,
            AutoTarget::Instrument { .. } if self.kind.is_vocal() => false,
            AutoTarget::Instrument { param } => match &self.instrument {
                InstrumentRef::Plugin(_) => true,
                other => crate::knobs::SoundKnob::from_index(param)
                    .is_some_and(|knob| crate::knobs::knob_usable(other, knob)),
            },
            AutoTarget::Insert { insert, param } => {
                let held = self.channel.inserts.iter().flatten();
                held.filter(|i| i.id == insert).any(|i| match i.kind() {
                    Some(kind) => kind.param(param).is_some(),
                    None => true,
                })
            }
        }
    }
}

/// Lets go of the lanes whose target the track does not have any more.
pub(crate) fn drop_orphan_lanes(track: &mut Track) {
    if track.automation.iter().any(|l| !track.has_target(l.target)) {
        let lanes = std::mem::take(&mut track.automation);
        track.automation = lanes
            .into_iter()
            .filter(|l| track.has_target(l.target))
            .collect();
    }
}

impl Doc {
    /// The lane whose points are `id` now, and where it lives.
    pub fn lane(&self, id: LaneId) -> Option<(LaneHome, &Arc<Lane>)> {
        let of_tracks = (self.tracks.iter()).flat_map(|t| {
            let home = LaneHome::Track(t.id);
            t.automation.iter().map(move |l| (home, l))
        });
        let of_master = self.master_lanes.iter().map(|l| (LaneHome::Master, l));
        of_tracks.chain(of_master).find(|(_, l)| l.id == id)
    }

    /// The lanes of `home`; `None` for a track that is not there.
    pub fn lanes(&self, home: LaneHome) -> Option<&[Arc<Lane>]> {
        match home {
            LaneHome::Track(track) => self.track(track).map(|t| &t.automation[..]),
            LaneHome::Master => Some(&self.master_lanes),
        }
    }

    pub fn lane_of(&self, home: LaneHome, target: AutoTarget) -> Option<&Arc<Lane>> {
        self.lanes(home)?.iter().find(|l| l.target == target)
    }

    /// Whether `home` has what `target` moves ([`Track::has_target`]; the master: its
    /// volume).
    pub fn has_lane_target(&self, home: LaneHome, target: AutoTarget) -> bool {
        match home {
            LaneHome::Track(track) => self.track(track).is_some_and(|t| t.has_target(target)),
            LaneHome::Master => target == AutoTarget::Volume,
        }
    }

    /// What every `Doc` keeps of its lanes: each has its target, a track and the master have
    /// one lane for a target, the points are in order with each tick once and their values
    /// in 0..=1, and every lane id is below `next_id` and once in the song.
    pub fn check_lanes(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        let homes = (self.tracks.iter())
            .map(|t| (LaneHome::Track(t.id), &t.automation))
            .chain([(LaneHome::Master, &self.master_lanes)]);
        for (home, lanes) in homes {
            for (i, l) in lanes.iter().enumerate() {
                if !self.has_lane_target(home, l.target) {
                    return Err(format!("{home:?}: lane {:?} has no target", l.target));
                }
                if lanes[..i].iter().any(|o| o.target == l.target) {
                    return Err(format!("{home:?}: two lanes for {:?}", l.target));
                }
                if l.id.0 >= self.next_id || !ids.insert(l.id) {
                    return Err(format!("lane id {} given twice", l.id.0));
                }
                let ordered =
                    (l.points.iter().zip(l.points.iter().skip(1))).all(|(a, b)| a.tick < b.tick);
                let values = (l.points.iter()).all(|p| (0.0..=1.0).contains(&p.value));
                if !ordered || !values || l.points.last().is_some_and(|p| p.tick > MAX_TICK) {
                    return Err(format!("lane {}: points out of order or range", l.id.0));
                }
            }
        }
        Ok(())
    }
}

fn clamp_value(value: f32) -> Option<f32> {
    (!value.is_nan()).then(|| value.clamp(0.0, 1.0))
}

/// The lanes of `home` in a copy of `doc`, ready to change. `home` is there.
fn lanes_mut(doc: &mut Doc, home: LaneHome) -> &mut Vec<Arc<Lane>> {
    match home {
        LaneHome::Track(track) => {
            let ti = (doc.tracks.iter().position(|t| t.id == track)).expect("the lane's track");
            &mut Arc::make_mut(&mut doc.tracks[ti]).automation
        }
        LaneHome::Master => &mut doc.master_lanes,
    }
}

/// The lane at `li` of `home`, which is there.
fn lane_at(doc: &Doc, home: LaneHome, li: usize) -> &Arc<Lane> {
    &doc.lanes(home).expect("the lane's home")[li]
}

/// A copy of `doc` whose lane at `li` of `home` has `points` and a new id. The new lane
/// shares with the old one the chunks of points that are the same in both.
fn with_points(
    doc: &Doc,
    home: LaneHome,
    li: usize,
    points: Vec<AutoPoint>,
) -> Result<(Doc, LaneId), EditError> {
    let mut ids = Ids(doc.next_id);
    let id = LaneId(ids.next()?);
    let mut new = doc.clone();
    new.next_id = ids.0;
    let lanes = lanes_mut(&mut new, home);
    let mut lane = Lane::clone(&lanes[li]);
    if lane.points == points {
        return Err(EditError::Unchanged);
    }
    lane.points = ChunkedVec::sharing(&points, &lane.points);
    lane.id = id;
    lanes[li] = Arc::new(lane);
    Ok((new, id))
}

fn find(doc: &Doc, lane: LaneId) -> Result<(LaneHome, usize), EditError> {
    let (home, _) = doc.lane(lane).ok_or(EditError::NoSuchPoint)?;
    let lanes = doc.lanes(home).expect("the lane's home");
    let li = (lanes.iter().position(|l| l.id == lane)).expect("the lane is there");
    Ok((home, li))
}

fn find_target(doc: &Doc, home: LaneHome, target: AutoTarget) -> Result<usize, EditError> {
    if let LaneHome::Track(track) = home {
        track_index(doc, track)?;
    }
    let lanes = doc.lanes(home).expect("the home is there");
    (lanes.iter().position(|l| l.target == target)).ok_or(EditError::NoSuchPoint)
}

/// A new lane with no points, after the others. `NoSuchInsert` when `home` does not have
/// `target`, `Unchanged` when it has a lane for it.
pub fn add_lane(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
    step: bool,
) -> Result<(Edit, LaneId), EditError> {
    let home = home.into();
    if let LaneHome::Track(track) = home {
        track_index(doc, track)?;
    }
    if !doc.has_lane_target(home, target) {
        return Err(EditError::NoSuchInsert);
    }
    if doc.lane_of(home, target).is_some() {
        return Err(EditError::Unchanged);
    }
    let mut ids = Ids(doc.next_id);
    let id = LaneId(ids.next()?);
    let mut new = doc.clone();
    new.next_id = ids.0;
    lanes_mut(&mut new, home).push(Arc::new(Lane {
        id,
        target,
        step,
        points: ChunkedVec::default(),
        resting: false,
    }));
    Ok((
        Edit {
            doc: new,
            step: StepKind::AddLane { home, target },
        },
        id,
    ))
}

/// 「帯を消す」: the lane goes with its points.
pub fn delete_lane(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
) -> Result<Edit, EditError> {
    let home = home.into();
    let li = find_target(doc, home, target)?;
    let mut new = doc.clone();
    lanes_mut(&mut new, home).remove(li);
    Ok(Edit {
        doc: new,
        step: StepKind::DeleteLane { home, target },
    })
}

/// A point at `tick` (past `MAX_TICK` is `MAX_TICK`). `value`: without it, or not a number,
/// the line's value there, or `flat` on a lane with no points (nothing happens when that
/// is not a number either). A point at that tick takes the value. Returns the lane's new id
/// and the point's place.
pub fn add_point(
    doc: &Doc,
    lane: LaneId,
    tick: u64,
    value: Option<f32>,
    flat: f32,
) -> Result<(Edit, LaneId, usize), EditError> {
    let (home, li) = find(doc, lane)?;
    let l = lane_at(doc, home, li);
    let tick = tick.min(MAX_TICK);
    let value = (value.and_then(clamp_value))
        .or_else(|| l.value_at(tick))
        .or_else(|| clamp_value(flat))
        .ok_or(EditError::Unchanged)?;
    let mut points = l.points.to_vec();
    let at = match points.binary_search_by_key(&tick, |p| p.tick) {
        Ok(at) => {
            points[at].value = value;
            at
        }
        Err(at) => {
            points.insert(at, AutoPoint { tick, value });
            at
        }
    };
    let target = l.target;
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::AddAutoPoint { home, target },
        },
        id,
        at,
    ))
}

/// The places of `picked` among `len` points, in order, each once; `NoSuchPoint` for one that is not
/// there, `Unchanged` for none.
fn picked_places(len: usize, picked: &[usize]) -> Result<Vec<usize>, EditError> {
    let mut picked = picked.to_vec();
    picked.sort_unstable();
    picked.dedup();
    if picked.is_empty() {
        return Err(EditError::Unchanged);
    }
    if picked.last().is_some_and(|&i| i >= len) {
        return Err(EditError::NoSuchPoint);
    }
    Ok(picked)
}

/// One drag of points: each goes `d_tick` along and `d_value` up, the group stopping
/// together at the song's start, at `MAX_TICK`, and at 0 and 1. A point not dragged that has
/// the tick of a dragged one afterwards is gone. Returns the lane's new id and the places of
/// the moved points.
pub fn move_points(
    doc: &Doc,
    lane: LaneId,
    picked: &[usize],
    d_tick: i64,
    d_value: f32,
) -> Result<(Edit, LaneId, Vec<usize>), EditError> {
    let (home, li) = find(doc, lane)?;
    let l = lane_at(doc, home, li);
    let picked = picked_places(l.points.len(), picked)?;
    let moved = || picked.iter().map(|&i| l.points[i]);
    let (first, last) = (
        l.points[picked[0]].tick,
        l.points[picked[picked.len() - 1]].tick,
    );
    let d_tick = d_tick.clamp(-(first as i64), (MAX_TICK - last) as i64);
    let low = moved().map(|p| p.value).fold(1.0f32, f32::min);
    let high = moved().map(|p| p.value).fold(0.0f32, f32::max);
    let d_value = if d_value.is_nan() {
        0.0
    } else {
        d_value.clamp(-low, 1.0 - high)
    };
    if d_tick == 0 && d_value == 0.0 {
        return Err(EditError::Unchanged);
    }
    let shifted: Vec<AutoPoint> = moved()
        .map(|p| AutoPoint {
            tick: p.tick.saturating_add_signed(d_tick),
            value: (p.value + d_value).clamp(0.0, 1.0),
        })
        .collect();
    let mut points: Vec<(AutoPoint, bool)> = (l.points.iter().enumerate())
        .filter(|(i, p)| {
            picked.binary_search(i).is_err() && !shifted.iter().any(|s| s.tick == p.tick)
        })
        .map(|(_, p)| (*p, false))
        .chain(shifted.iter().map(|p| (*p, true)))
        .collect();
    points.sort_by_key(|(p, _)| p.tick);
    let places = (points.iter().enumerate())
        .filter(|(_, (_, m))| *m)
        .map(|(i, _)| i)
        .collect();
    let (target, count) = (l.target, picked.len());
    let points = points.into_iter().map(|(p, _)| p).collect();
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::MoveAutoPoints {
                home,
                target,
                count,
            },
        },
        id,
        places,
    ))
}

/// Two presses on a point, Delete with points selected.
pub fn delete_points(
    doc: &Doc,
    lane: LaneId,
    picked: &[usize],
) -> Result<(Edit, LaneId), EditError> {
    let (home, li) = find(doc, lane)?;
    let l = lane_at(doc, home, li);
    let picked = picked_places(l.points.len(), picked)?;
    let points = (l.points.iter().enumerate())
        .filter(|(i, _)| picked.binary_search(i).is_err())
        .map(|(_, p)| *p)
        .collect();
    let (target, count) = (l.target, picked.len());
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::DeleteAutoPoints {
                home,
                target,
                count,
            },
        },
        id,
    ))
}

/// Leaves out what the line needs not: on a stepped lane every point whose value is the one
/// before's; on any other, by Ramer–Douglas–Peucker over the value, the points within
/// `tolerance` of the straight line between the ones kept. The first and the last stay.
pub fn thin(points: &[AutoPoint], step: bool, tolerance: f32) -> Vec<AutoPoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    if step {
        let mut kept: Vec<AutoPoint> = Vec::with_capacity(points.len());
        for p in points {
            if kept.last().is_none_or(|k| k.value != p.value) {
                kept.push(*p);
            }
        }
        if kept.last() != points.last() {
            kept.push(points[points.len() - 1]);
        }
        return kept;
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut spans = vec![(0, points.len() - 1)];
    while let Some((a, b)) = spans.pop() {
        if b <= a + 1 {
            continue;
        }
        let ends = [points[a], points[b]];
        let (far, gap) = (a + 1..b)
            .map(|i| {
                (
                    i,
                    (points[i].value - line_at(&ends, false, points[i].tick).unwrap()).abs(),
                )
            })
            .fold((a, -1.0f32), |best, x| if x.1 > best.1 { x } else { best });
        if gap > tolerance {
            keep[far] = true;
            spans.push((a, far));
            spans.push((far, b));
        }
    }
    (points.iter().zip(keep))
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// Puts what one pass wrote into `points`: the points from a tick before the first written to
/// a tick after the last go, the written ones come in, and the two ticks around them get the
/// line's value as it was, so that the line goes back to it. `flat`: the value of a lane
/// with no points.
fn write_pass(points: &mut Vec<AutoPoint>, pass: &[AutoPoint], step: bool, flat: f32) {
    let (Some(first), Some(last)) = (pass.first(), pass.last()) else {
        return;
    };
    let line = |points: &[AutoPoint], tick: u64| line_at(points, step, tick).unwrap_or(flat);
    let before = first.tick.checked_sub(1).map(|tick| AutoPoint {
        tick,
        value: line(points, tick),
    });
    let after = (last.tick < MAX_TICK).then(|| AutoPoint {
        tick: last.tick + 1,
        value: line(points, last.tick + 1),
    });
    let from = before.map_or(first.tick, |p| p.tick);
    let to = after.map_or(last.tick, |p| p.tick);
    let head = points.partition_point(|p| p.tick < from);
    let tail = points.partition_point(|p| p.tick <= to);
    let written = before.into_iter().chain(pass.iter().copied()).chain(after);
    points.splice(head..tail, written);
}

/// The points of one pass as written: in the order of tick, each tick once (the later value
/// wins), inside the song and 0..=1, thinned.
fn written(pass: &[AutoPoint], step: bool) -> Vec<AutoPoint> {
    let mut sorted: Vec<AutoPoint> = (pass.iter())
        .filter_map(|p| {
            Some(AutoPoint {
                tick: p.tick.min(MAX_TICK),
                value: clamp_value(p.value)?,
            })
        })
        .collect();
    sorted.sort_by_key(|p| p.tick);
    let mut once: Vec<AutoPoint> = Vec::with_capacity(sorted.len());
    for p in sorted {
        match once.last_mut() {
            Some(last) if last.tick == p.tick => *last = p,
            _ => once.push(p),
        }
    }
    thin(&once, step, THIN_TOLERANCE)
}

/// 「● 書く」: what one press wrote, until it was let go or the song stopped (spec 3, 16-B;
/// docs/architecture.md 21-9). `passes`: the points written in each pass of a loop, the
/// oldest first; a later pass wins where they meet. `flat`: the value of a lane with no
/// points, where the hand put it; when the lane needs it and it is not a number, nothing
/// happens.
pub fn write(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
    passes: &[Vec<AutoPoint>],
    flat: f32,
) -> Result<(Edit, LaneId), EditError> {
    let home = home.into();
    let li = find_target(doc, home, target)?;
    let l = lane_at(doc, home, li);
    let flat = match clamp_value(flat) {
        Some(flat) => flat,
        None if l.points.is_empty() => return Err(EditError::Unchanged),
        // Not looked at: the line has a value everywhere.
        None => 0.0,
    };
    let mut points = l.points.to_vec();
    for pass in passes {
        write_pass(&mut points, &written(pass, l.step), l.step, flat);
    }
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::WriteAutomation { home, target },
        },
        id,
    ))
}

/// 「● 書く」 and a press that has no "while held" (the mute, a switch): one point at `tick`,
/// which the line keeps to the next point.
pub fn write_press(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
    tick: u64,
    value: f32,
) -> Result<(Edit, LaneId), EditError> {
    let home = home.into();
    let li = find_target(doc, home, target)?;
    let l = lane_at(doc, home, li);
    let point = AutoPoint {
        tick: tick.min(MAX_TICK),
        value: clamp_value(value).ok_or(EditError::Unchanged)?,
    };
    let mut points = l.points.to_vec();
    match points.binary_search_by_key(&point.tick, |p| p.tick) {
        Ok(at) => points[at] = point,
        Err(at) => points.insert(at, point),
    }
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::WriteAutomation { home, target },
        },
        id,
    ))
}

/// Points put into the lane of `target` as they are, in one step (the AI link writes them).
/// One at the tick of a point there takes its place; ticks past [`MAX_TICK`] are
/// `MAX_TICK`. `Unchanged` for a value that is not a number.
pub fn put_points(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
    put: &[AutoPoint],
) -> Result<(Edit, LaneId), EditError> {
    let home = home.into();
    let li = find_target(doc, home, target)?;
    let mut points = lane_at(doc, home, li).points.to_vec();
    for p in put {
        let point = AutoPoint {
            tick: p.tick.min(MAX_TICK),
            value: clamp_value(p.value).ok_or(EditError::Unchanged)?,
        };
        match points.binary_search_by_key(&point.tick, |p| p.tick) {
            Ok(at) => points[at] = point,
            Err(at) => points.insert(at, point),
        }
    }
    let (new, id) = with_points(doc, home, li, points)?;
    Ok((
        Edit {
            doc: new,
            step: StepKind::WriteAutomation { home, target },
        },
        id,
    ))
}

/// The volume lane of the track at `ti` with every point `db` lower
/// ([`crate::mixer::Gain::lowered_place`]). `Unchanged` when no point moves.
pub(crate) fn lower_volume_points(doc: &Doc, ti: usize, db: f64) -> Result<Doc, EditError> {
    let home = LaneHome::Track(doc.tracks[ti].id);
    let li = find_target(doc, home, AutoTarget::Volume)?;
    let points = (lane_at(doc, home, li).points.iter())
        .map(|p| AutoPoint {
            tick: p.tick,
            value: Gain::lowered_place(p.value, db),
        })
        .collect();
    with_points(doc, home, li, points).map(|(new, _)| new)
}

/// Puts the lane of `target` to rest, or back to moving its target. Returns whether it
/// changed; nothing for a home or lane that is not there.
pub fn set_resting(
    doc: &mut Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
    resting: bool,
) -> bool {
    let home = home.into();
    let Ok(li) = find_target(doc, home, target) else {
        return false;
    };
    if lane_at(doc, home, li).resting == resting {
        return false;
    }
    Arc::make_mut(&mut lanes_mut(doc, home)[li]).resting = resting;
    true
}

/// The lane rests and no value is set with it (a parameter moved in the plugin's own
/// window). `Unchanged` when it rests already.
pub fn rest_lane(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
) -> Result<Edit, EditError> {
    let home = home.into();
    resting_step(doc, home, target, true, StepKind::RestLane { home, target })
}

/// 「もどす」.
pub fn restore_lane(
    doc: &Doc,
    home: impl Into<LaneHome>,
    target: AutoTarget,
) -> Result<Edit, EditError> {
    let home = home.into();
    resting_step(
        doc,
        home,
        target,
        false,
        StepKind::RestoreLane { home, target },
    )
}

fn resting_step(
    doc: &Doc,
    home: LaneHome,
    target: AutoTarget,
    resting: bool,
    step: StepKind,
) -> Result<Edit, EditError> {
    find_target(doc, home, target)?;
    let mut new = doc.clone();
    if !set_resting(&mut new, home, target, resting) {
        return Err(EditError::Unchanged);
    }
    Ok(Edit { doc: new, step })
}

/// 「フェードアウトをつける」 (round 12, M5): the master's volume goes in a straight line
/// down to the bottom at the end of the last clip ([`crate::sections::last_clip_end`],
/// where the export's 「曲ぜんぶ」 ends), over the length of the `bars` bars before the bar
/// that end is in (from the song's head when the song is shorter). Points before the
/// start stay, points from the start on go, a lane that rests moves again, and a master
/// without a lane gets one. The start's value: where the hand put the fader, or on a lane
/// that moves the fader the lane's value there, or after the head of a fade-out that is
/// there ([`old_fade`]) the value at that head, the one from before that fade, so that a
/// shorter fade is shorter. That fade's end goes too when it lies before the start (the
/// song grew after it): the new fade takes its place. `NoTakes` in a song with no clip,
/// `Unchanged` for no bars or a fade that is there.
pub fn add_fade_out(doc: &Doc, bars: u32) -> Result<Edit, EditError> {
    const HOME: LaneHome = LaneHome::Master;
    let map = TempoMap::new(&doc.timeline);
    let end = crate::sections::last_clip_end(doc);
    if end == 0 {
        return Err(EditError::NoTakes);
    }
    if end > MAX_TICK {
        return Err(EditError::OffTheSong);
    }
    let bar = map.meter_at(end).bar;
    let start = match bar.checked_sub(u64::from(bars)) {
        Some(head) => end - (map.tick_of_bar(bar) - map.tick_of_bar(head)),
        None => 0,
    };
    let range = TickRange::new(start, end).ok_or(EditError::Unchanged)?;
    let old = doc.lane_of(HOME, AutoTarget::Volume);
    let mut points: Vec<AutoPoint> = (old.iter())
        .flat_map(|l| l.points.iter().copied())
        .take_while(|p| p.tick < start)
        .collect();
    let fade = old.and_then(|l| old_fade(l, start).map(|head| (l, head)));
    if let Some((l, _)) = fade {
        // Up to the head: the end of a fade that ends before the start goes with it.
        points.truncate(l.points.len() - 1);
    }
    let moving = old.filter(|l| l.moves());
    let from = match fade.filter(|_| moving.is_some()) {
        // The head is the last point before the start. It goes when the line comes to it
        // flat: then it changes nothing heard.
        Some((_, head)) => {
            let before = points.len().checked_sub(2).map(|i| points[i].value);
            if before.is_none_or(|v| v == head.value) {
                points.pop();
            }
            head.value
        }
        None => (moving.and_then(|l| l.value_at(start))).unwrap_or(doc.master.gain.position()),
    };
    points.push(AutoPoint {
        tick: start,
        value: from,
    });
    points.push(AutoPoint {
        tick: end,
        value: 0.0,
    });
    let mut new = match old {
        None => add_lane(doc, HOME, AutoTarget::Volume, false)?.0.doc,
        Some(_) => doc.clone(),
    };
    let li = find_target(&new, HOME, AutoTarget::Volume)?;
    let woke = set_resting(&mut new, HOME, AutoTarget::Volume, false);
    let doc = match with_points(&new, HOME, li, points) {
        Ok((with, _)) => with,
        Err(EditError::Unchanged) if woke => new,
        Err(e) => return Err(e),
    };
    Ok(Edit {
        doc,
        step: StepKind::AddFadeOut { range },
    })
}

/// The head of the fade-out that is there, when a new one from `start` takes its place: the
/// lane's last line when it goes down to the bottom and its head is before `start`,
/// wherever it ends and whoever drew it.
fn old_fade(lane: &Lane, start: u64) -> Option<AutoPoint> {
    let n = lane.points.len();
    let head = *lane.points.get(n.checked_sub(2)?)?;
    let foot = lane.points.get(n - 1)?;
    (foot.value == 0.0 && head.tick < start).then_some(head)
}

#[cfg(test)]
mod tests;
