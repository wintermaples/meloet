use std::sync::Arc;

use super::*;
use crate::audio::{AudioId, AudioShape, AudioSource, AudioTake, SourceTempo, TrackKind};
use crate::automation::AutoTarget;
use crate::edit::{NewTake, add_recording};
use crate::history::History;
use crate::id_book::{IdBook, held};
use crate::playback::track_events;
use crate::song::{ClipTake, ClipTakes, Note, TakeId, Track};
use crate::time::Meter;
use crate::{InstrumentRef, TICKS_PER_QUARTER, TrackColor, TrackId, VocalInput};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
const T1: TrackId = TrackId(1);
const T2: TrackId = TrackId(2);
const T3: TrackId = TrackId(3);

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn point(tick: u64, centi: u32, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick,
        tempo: tempo(centi),
        ramp,
    }
}

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity: 80,
    }
}

fn record(doc: &mut Doc, track: TrackId, start: u64, end: u64, notes: Vec<Note>) -> ClipId {
    let take = NewTake {
        origin: start,
        length: end - start,
        notes,
        controls: Vec::new(),
    };
    let (e, added) = add_recording(doc, track, vec![take], 0).unwrap();
    *doc = e.doc;
    added.clip
}

fn lane(doc: &mut Doc, target: AutoTarget, step: bool, points: &[(u64, f32)]) -> Arc<Lane> {
    let id = LaneId(doc.next_id);
    doc.next_id += 1;
    Arc::new(Lane {
        id,
        target,
        step,
        points: points
            .iter()
            .map(|&(tick, value)| AutoPoint { tick, value })
            .collect(),
        resting: false,
    })
}

fn track_mut(doc: &mut Doc, id: TrackId) -> &mut Track {
    let i = doc.tracks.iter().position(|t| t.id == id).unwrap();
    Arc::make_mut(&mut doc.tracks[i])
}

fn mark(doc: &mut Doc, tick: u64, name: &str) -> MarkerId {
    let (e, id) = crate::timeline::add_marker(doc, tick, name).unwrap();
    *doc = e.doc;
    id
}

/// A song in 4/4 with `points`, markers at the heads of bars 0, 2 and 4, and no clips.
fn plain(head: u32, head_ramp: bool, points: &[TempoPoint]) -> Doc {
    let mut doc = Doc::new_default();
    doc.timeline = Arc::new(Timeline {
        tempo: tempo(head),
        head_ramp,
        tempo_points: points.to_vec(),
        ..Timeline::clone(&doc.timeline)
    });
    doc
}

fn tempos(doc: &Doc) -> Vec<(u64, u32, bool)> {
    let t = &doc.timeline;
    std::iter::once((0, t.tempo.centi(), t.head_ramp))
        .chain((t.tempo_points.iter()).map(|p| (p.tick, p.tempo.centi(), p.ramp)))
        .collect()
}

fn meters(doc: &Doc) -> Vec<(u32, u8, u8)> {
    let t = &doc.timeline;
    std::iter::once((0, t.meter.num(), t.meter.den()))
        .chain((t.meter_changes.iter()).map(|c| (c.bar, c.meter.num(), c.meter.den())))
        .collect()
}

fn with_meters(doc: &mut Doc, head: (u8, u8), changes: &[(u32, u8, u8)]) {
    doc.timeline = Arc::new(Timeline {
        meter: Meter::new(head.0, head.1).unwrap(),
        meter_changes: changes
            .iter()
            .map(|&(bar, num, den)| MeterChange {
                bar,
                meter: Meter::new(num, den).unwrap(),
            })
            .collect(),
        ..Timeline::clone(&doc.timeline)
    });
}

fn bar(doc: &Doc, n: u64) -> u64 {
    TempoMap::new(&doc.timeline).tick_of_bar(n)
}

fn lane_points(l: &Lane) -> Vec<(u64, f32)> {
    l.points.iter().map(|p| (p.tick, p.value)).collect()
}

fn spans(doc: &Doc, track: TrackId) -> Vec<(u32, u64, u64)> {
    (doc.track(track).unwrap().clips.iter())
        .map(|c| (c.id.0, c.start, c.end))
        .collect()
}

fn marks(doc: &Doc) -> Vec<(u64, &str)> {
    (doc.markers.iter())
        .map(|m| (m.tick, m.name.as_str()))
        .collect()
}

/// The doc with what a round trip may renew left out: `next_id` and the ids of the lanes.
fn sans_ids(doc: &Doc) -> Doc {
    let mut doc = held(doc);
    let plain = |lanes: &mut Vec<Arc<Lane>>| {
        for l in lanes {
            Arc::make_mut(l).id = LaneId(0);
        }
    };
    for t in &mut doc.tracks {
        plain(&mut Arc::make_mut(t).automation);
    }
    plain(&mut doc.master_lanes);
    doc
}

/// What every `Doc` keeps: the timeline and the lanes are well formed, the clips of a track
/// are in order and do not overlap, the markers are in order.
fn check(doc: &Doc) {
    assert_eq!(doc.timeline.check(), Ok(()));
    doc.check_lanes().unwrap();
    for l in &doc.master_lanes {
        assert!(
            l.points
                .iter()
                .zip(l.points.iter().skip(1))
                .all(|(a, b)| a.tick < b.tick)
        );
        assert!(l.id.0 < doc.next_id);
    }
    for t in &doc.tracks {
        assert!(t.clips.iter().all(|c| c.start < c.end && c.end <= MAX_TICK));
        assert!(t.clips.windows(2).all(|w| w[0].end <= w[1].start));
    }
    assert!((doc.markers.windows(2)).all(|w| (w[0].tick, w[0].id) < (w[1].tick, w[1].id)));
}

// ---- tempo ----

#[test]
fn a_flat_tempo_needs_no_points() {
    let doc = plain(12_000, false, &[]);
    for new in [
        insert_time(&doc, 2 * BAR, 4).unwrap(),
        remove_time(&doc, 2 * BAR, 4 * BAR).unwrap(),
        duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap(),
        insert_time(&doc, 0, 1).unwrap(),
        remove_time(&doc, 0, BAR).unwrap(),
    ] {
        assert_eq!(tempos(&new), [(0, 12_000, false)]);
    }
}

#[test]
fn a_tempo_point_on_the_cut_moves_with_what_follows() {
    let doc = plain(12_000, false, &[point(2 * BAR, 14_000, false)]);
    let put = insert_time(&doc, 2 * BAR, 2).unwrap();
    assert_eq!(tempos(&put), [(0, 12_000, false), (4 * BAR, 14_000, false)]);
    let back = remove_time(&put, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(tempos(&back), tempos(&doc));
    // Later points move, earlier ones stay.
    let put = insert_time(&doc, BAR, 1).unwrap();
    assert_eq!(tempos(&put), [(0, 12_000, false), (3 * BAR, 14_000, false)]);
    let put = insert_time(&doc, 3 * BAR, 1).unwrap();
    assert_eq!(tempos(&put), tempos(&doc));
}

#[test]
fn bars_put_into_a_ramp_hold_the_tempo_there_and_the_ramp_goes_on_after_them() {
    let doc = plain(10_000, true, &[point(4 * BAR, 12_000, false)]);
    let put = insert_time(&doc, 2 * BAR, 1).unwrap();
    assert_eq!(
        tempos(&put),
        [
            (0, 10_000, true),
            (2 * BAR, 11_000, false),
            (3 * BAR, 11_000, true),
            (5 * BAR, 12_000, false),
        ]
    );
    let back = remove_time(&put, 2 * BAR, 3 * BAR).unwrap();
    assert_eq!(tempos(&back), tempos(&doc));
}

#[test]
fn bars_put_where_a_ramp_ends_hold_the_tempo_it_came_to() {
    let doc = plain(10_000, true, &[point(2 * BAR, 12_000, false)]);
    let put = insert_time(&doc, 2 * BAR, 1).unwrap();
    assert_eq!(
        tempos(&put),
        [
            (0, 10_000, true),
            (2 * BAR, 12_000, false),
            (3 * BAR, 12_000, false),
        ]
    );
    let back = remove_time(&put, 2 * BAR, 3 * BAR).unwrap();
    assert_eq!(tempos(&back), tempos(&doc));
}

#[test]
fn bars_put_at_the_head_leave_the_head_and_move_its_ramp() {
    let doc = plain(10_000, true, &[point(4 * BAR, 12_000, false)]);
    let put = insert_time(&doc, 0, 2).unwrap();
    assert_eq!(
        tempos(&put),
        [
            (0, 10_000, false),
            (2 * BAR, 10_000, true),
            (6 * BAR, 12_000, false),
        ]
    );
    let back = remove_time(&put, 0, 2 * BAR).unwrap();
    assert_eq!(tempos(&back), tempos(&doc));
    // A head that only says it ramps keeps saying so.
    let lone = plain(10_000, true, &[]);
    let put = insert_time(&lone, 0, 2).unwrap();
    assert_eq!(tempos(&put), [(0, 10_000, true)]);
    assert_eq!(
        tempos(&remove_time(&put, 0, 2 * BAR).unwrap()),
        tempos(&lone)
    );
}

#[test]
fn a_ramp_into_what_goes_ends_a_tick_before_the_seam() {
    // 100 at the head to 120 at bar 4, then 150 from bar 4 on.
    let doc = plain(
        10_000,
        true,
        &[
            point(4 * BAR - Q, 12_000, false),
            point(4 * BAR, 15_000, false),
        ],
    );
    let new = remove_time(&doc, 2 * BAR, 4 * BAR).unwrap();
    let on_ramp = ramp_at(
        point(0, 10_000, true),
        point(4 * BAR - Q, 12_000, false),
        2 * BAR - 1,
    );
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, true),
            (2 * BAR - 1, on_ramp, false),
            (2 * BAR, 15_000, false),
        ]
    );
    // The time of what is before the seam stays (the tempo is kept in hundredths).
    let (old, now) = (TempoMap::new(&doc.timeline), TempoMap::new(&new.timeline));
    for tick in [Q, BAR, 2 * BAR - Q, 2 * BAR] {
        let (a, b) = (old.seconds_at(tick as f64), now.seconds_at(tick as f64));
        assert!((a - b).abs() < 5e-4, "{tick}: {a} {b}");
    }
}

#[test]
fn a_cut_out_of_one_ramp_leaves_both_ends_of_the_ramp_as_they_were() {
    let doc = plain(10_000, true, &[point(8 * BAR, 18_000, false)]);
    let new = remove_time(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, true),
            (2 * BAR - 1, 12_000, false),
            (2 * BAR, 14_000, true),
            (6 * BAR, 18_000, false),
        ]
    );
}

#[test]
fn what_goes_from_the_head_gives_the_head_the_tempo_that_follows() {
    let doc = plain(
        10_000,
        false,
        &[point(BAR, 11_000, true), point(3 * BAR, 13_000, false)],
    );
    let new = remove_time(&doc, 0, 2 * BAR).unwrap();
    assert_eq!(tempos(&new), [(0, 12_000, true), (BAR, 13_000, false)]);
    let new = remove_time(&doc, 0, BAR).unwrap();
    assert_eq!(tempos(&new), [(0, 11_000, true), (2 * BAR, 13_000, false)]);
}

#[test]
fn a_copy_starts_with_the_tempo_of_the_section_and_what_follows_keeps_its_own() {
    // Bars 0 to 2 at 100, bars 2 to 4 at 140, from bar 4 on 90.
    let doc = plain(
        10_000,
        false,
        &[point(2 * BAR, 14_000, false), point(4 * BAR, 9_000, false)],
    );
    let new = duplicate_range(&doc, 0, 2 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, false),
            (4 * BAR, 14_000, false),
            (6 * BAR, 9_000, false),
        ]
    );
    let new = duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, false),
            (2 * BAR, 14_000, false),
            (4 * BAR, 14_000, false),
            (6 * BAR, 9_000, false),
        ]
    );
    assert_eq!(
        tempos(&remove_time(&new, 4 * BAR, 6 * BAR).unwrap()),
        tempos(&doc)
    );
    // A copy of the first bars after a change: the copy goes back to the head's tempo.
    let doc = plain(10_000, false, &[point(BAR, 14_000, false)]);
    let new = duplicate_range(&doc, 0, 2 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, false),
            (BAR, 14_000, false),
            (2 * BAR, 10_000, false),
            (3 * BAR, 14_000, false),
        ]
    );
    assert_eq!(
        tempos(&remove_time(&new, 2 * BAR, 4 * BAR).unwrap()),
        tempos(&doc)
    );
}

#[test]
fn a_copy_out_of_one_ramp_and_back() {
    let doc = plain(10_000, true, &[point(8 * BAR, 18_000, false)]);
    let new = duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, true),
            (4 * BAR - 1, 14_000, false),
            (4 * BAR, 12_000, true),
            (10 * BAR, 18_000, false),
        ]
    );
    assert_eq!(
        tempos(&remove_time(&new, 4 * BAR, 6 * BAR).unwrap()),
        tempos(&doc)
    );
    // Tempos that do not fall on hundredths: the points made are off the line by a
    // hundredth at most, and still go again.
    for (head, last, bars) in [
        (10_001, 17_333, 7u64),
        (9_999, 13_337, 9),
        (3_000, 30_000, 11),
    ] {
        let doc = plain(head, true, &[point(bars * BAR, last, false)]);
        for (from, to) in [(1, 2), (2, 5), (1, 6), (3, 4)] {
            let new = duplicate_range(&doc, from * BAR, to * BAR).unwrap();
            assert_eq!(new.timeline.check(), Ok(()));
            let back = remove_time(&new, to * BAR, (2 * to - from) * BAR).unwrap();
            assert_eq!(tempos(&back), tempos(&doc), "{head} {last} {from} {to}");
            let put = insert_time(&doc, from * BAR, (to - from) as u32).unwrap();
            let back = remove_time(&put, from * BAR, to * BAR).unwrap();
            assert_eq!(tempos(&back), tempos(&doc), "{head} {last} {from} {to}");
        }
    }
}

#[test]
fn a_ramp_from_the_tick_before_the_seam_is_ended_on_its_own_point_and_comes_back() {
    let doc = plain(
        10_000,
        false,
        &[
            point(4 * BAR - 1, 11_000, true),
            point(7 * BAR, 14_000, false),
        ],
    );
    let new = duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        tempos(&new),
        [
            (0, 10_000, false),
            (4 * BAR - 1, 11_000, false),
            (4 * BAR, 10_000, false),
            (6 * BAR - 1, 11_000, true),
            (9 * BAR, 14_000, false),
        ]
    );
    assert_eq!(
        tempos(&remove_time(&new, 4 * BAR, 6 * BAR).unwrap()),
        tempos(&doc)
    );
}

// ---- meter ----

#[test]
fn empty_bars_take_the_meter_at_the_cut_and_later_changes_move() {
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (4, 4), &[(2, 3, 4), (6, 6, 8)]);
    let at = bar(&doc, 2);
    let new = insert_time(&doc, at, 3).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (2, 3, 4), (9, 6, 8)]);
    // Three bars of 3/4.
    assert_eq!(bar(&new, 9), bar(&doc, 6) + 3 * 3 * Q);
    assert_eq!(
        meters(&remove_time(&new, at, at + 9 * Q).unwrap()),
        meters(&doc)
    );
    let new = insert_time(&doc, bar(&doc, 1), 2).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (4, 3, 4), (8, 6, 8)]);
    let new = insert_time(&doc, 0, 2).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (4, 3, 4), (8, 6, 8)]);
}

#[test]
fn bars_taken_out_take_their_meter_changes_and_the_meter_that_follows_goes_on() {
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (4, 4), &[(2, 3, 4), (4, 6, 8), (8, 4, 4)]);
    // Bars 2 and 3 (3/4) go: the 6/8 comes up to bar 2.
    let new = remove_time(&doc, bar(&doc, 2), bar(&doc, 4)).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (2, 6, 8), (6, 4, 4)]);
    // Bars 1 to 5 go: bar 5 was in 6/8, which no change says any more.
    let new = remove_time(&doc, bar(&doc, 1), bar(&doc, 5)).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (1, 6, 8), (4, 4, 4)]);
    // Bars 3 to 8: from bar 8 on it is 4/4, after 3/4.
    let new = remove_time(&doc, bar(&doc, 3), bar(&doc, 8)).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (2, 3, 4), (3, 4, 4)]);
    // The meter that follows is the one before: no change is needed.
    let mut same = Doc::new_default();
    with_meters(&mut same, (4, 4), &[(2, 3, 4), (4, 4, 4)]);
    let new = remove_time(&same, bar(&same, 2), bar(&same, 3)).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (2, 3, 4), (3, 4, 4)]);
    let new = remove_time(&same, bar(&same, 1), bar(&same, 5)).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4)]);
    // From the head: the head takes the meter.
    let new = remove_time(&doc, 0, bar(&doc, 3)).unwrap();
    assert_eq!(meters(&new), [(0, 3, 4), (1, 6, 8), (5, 4, 4)]);
}

#[test]
fn a_copy_starts_with_the_meter_of_the_section() {
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (4, 4), &[(2, 3, 4), (4, 6, 8)]);
    // Bars 2 and 3, with the change at their head.
    let (from, to) = (bar(&doc, 2), bar(&doc, 4));
    let new = duplicate_range(&doc, from, to).unwrap();
    assert_eq!(meters(&new), [(0, 4, 4), (2, 3, 4), (4, 3, 4), (6, 6, 8)]);
    assert_eq!(
        meters(&remove_time(&new, to, to + (to - from)).unwrap()),
        meters(&doc)
    );
    // Bars 1 to 3: 4/4 then 3/4. The copy goes back to 4/4, and bar 3 on stays 3/4.
    let (from, to) = (bar(&doc, 1), bar(&doc, 3));
    let new = duplicate_range(&doc, from, to).unwrap();
    assert_eq!(
        meters(&new),
        [(0, 4, 4), (2, 3, 4), (3, 4, 4), (4, 3, 4), (6, 6, 8)]
    );
    assert_eq!(bar(&new, 5), to + (to - from));
    assert_eq!(
        meters(&remove_time(&new, to, to + (to - from)).unwrap()),
        meters(&doc)
    );
    // From the head.
    let new = duplicate_range(&doc, 0, to).unwrap();
    assert_eq!(
        meters(&new),
        [(0, 4, 4), (2, 3, 4), (3, 4, 4), (5, 3, 4), (7, 6, 8)]
    );
}

#[test]
fn a_cut_that_is_not_the_head_of_a_bar_does_nothing() {
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (4, 4), &[(2, 3, 4)]);
    // Bar 2 is in 3/4: 2 bars and 4 beats is inside its second bar.
    let off = 2 * BAR + 4 * Q;
    assert_eq!(insert_time(&doc, off, 1), Err(EditError::Unchanged));
    assert_eq!(
        remove_time(&doc, off, off + 3 * Q),
        Err(EditError::Unchanged)
    );
    assert_eq!(remove_time(&doc, BAR, off), Err(EditError::Unchanged));
    assert_eq!(duplicate_range(&doc, BAR, off), Err(EditError::Unchanged));
    assert_eq!(insert_time(&doc, BAR, 0), Err(EditError::Unchanged));
    assert_eq!(remove_time(&doc, BAR, BAR), Err(EditError::Unchanged));
    assert_eq!(
        duplicate_range(&doc, 2 * BAR, BAR),
        Err(EditError::Unchanged)
    );
}

// ---- lanes ----

fn volume(doc: &mut Doc, step: bool, points: &[(u64, f32)]) {
    let target = if step {
        AutoTarget::Mute
    } else {
        AutoTarget::Volume
    };
    let l = lane(doc, target, step, points);
    track_mut(doc, T1).automation = vec![l];
}

fn lane_of(doc: &Doc) -> Vec<(u64, f32)> {
    lane_points(&doc.tracks[0].automation[0])
}

#[test]
fn empty_bars_hold_the_value_of_a_lane_and_its_line_goes_on_after_them() {
    let mut doc = Doc::new_default();
    volume(
        &mut doc,
        false,
        &[(BAR, 0.25), (3 * BAR, 0.75), (5 * BAR, 0.25)],
    );
    let put = insert_time(&doc, 2 * BAR, 2).unwrap();
    assert_eq!(
        lane_of(&put),
        [
            (BAR, 0.25),
            (2 * BAR, 0.5),
            (4 * BAR, 0.5),
            (5 * BAR, 0.75),
            (7 * BAR, 0.25)
        ]
    );
    assert_ne!(
        put.tracks[0].automation[0].id,
        doc.tracks[0].automation[0].id
    );
    assert_eq!(
        lane_of(&remove_time(&put, 2 * BAR, 4 * BAR).unwrap()),
        lane_of(&doc)
    );
    // On a point: the point moves, and its value is held from where it was.
    let put = insert_time(&doc, 3 * BAR, 1).unwrap();
    assert_eq!(
        lane_of(&put),
        [
            (BAR, 0.25),
            (3 * BAR, 0.75),
            (4 * BAR, 0.75),
            (6 * BAR, 0.25)
        ]
    );
    assert_eq!(
        lane_of(&remove_time(&put, 3 * BAR, 4 * BAR).unwrap()),
        lane_of(&doc)
    );
    // Before the first point and after the last one the line is flat: no point is needed,
    // and the lane keeps its id when nothing in it moves.
    let put = insert_time(&doc, 0, 1).unwrap();
    assert_eq!(
        lane_of(&put),
        [(2 * BAR, 0.25), (4 * BAR, 0.75), (6 * BAR, 0.25)]
    );
    for at in [5 * BAR, 6 * BAR] {
        let put = insert_time(&doc, at, 1).unwrap();
        assert_eq!(lane_of(&put), lane_of(&doc));
        assert!(Arc::ptr_eq(
            &put.tracks[0].automation[0],
            &doc.tracks[0].automation[0]
        ));
        assert_eq!(put.next_id, doc.next_id);
    }
    // A jump on the cut stays where it is, and the bars hold the value it jumps to.
    let mut jump = Doc::new_default();
    volume(
        &mut jump,
        false,
        &[(2 * BAR - 1, 0.25), (2 * BAR, 0.75), (3 * BAR, 0.5)],
    );
    let put = insert_time(&jump, 2 * BAR, 1).unwrap();
    assert_eq!(
        lane_of(&put),
        [
            (2 * BAR - 1, 0.25),
            (2 * BAR, 0.75),
            (3 * BAR, 0.75),
            (4 * BAR, 0.5)
        ]
    );
    assert_eq!(
        lane_of(&remove_time(&put, 2 * BAR, 3 * BAR).unwrap()),
        lane_of(&jump)
    );
}

#[test]
fn a_stepped_lane_needs_no_points_for_empty_bars() {
    let mut doc = Doc::new_default();
    volume(&mut doc, true, &[(BAR, 1.0), (3 * BAR, 0.0)]);
    let put = insert_time(&doc, 2 * BAR, 2).unwrap();
    assert_eq!(lane_of(&put), [(BAR, 1.0), (5 * BAR, 0.0)]);
    // On a step: the value before it is held, and the step comes after the bars.
    let put = insert_time(&doc, 3 * BAR, 2).unwrap();
    assert_eq!(lane_of(&put), [(BAR, 1.0), (5 * BAR, 0.0)]);
    assert_eq!(
        lane_of(&remove_time(&put, 3 * BAR, 5 * BAR).unwrap()),
        lane_of(&doc)
    );
}

#[test]
fn where_bars_go_a_lane_jumps_from_the_value_before_them_to_the_value_after_them() {
    let mut doc = Doc::new_default();
    volume(
        &mut doc,
        false,
        &[(0, 0.0), (4 * BAR, 0.5), (6 * BAR, 0.25)],
    );
    let before = doc.tracks[0].automation[0].value_at(2 * BAR - 1).unwrap();
    assert!(before > 0.249 && before < 0.25);
    let new = remove_time(&doc, 2 * BAR, 5 * BAR).unwrap();
    assert_eq!(
        lane_of(&new),
        [
            (0, 0.0),
            (2 * BAR - 1, before),
            (2 * BAR, 0.375),
            (3 * BAR, 0.25)
        ]
    );
    // A point on the end of what goes moves to the seam.
    let new = remove_time(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        lane_of(&new),
        [
            (0, 0.0),
            (2 * BAR - 1, before),
            (2 * BAR, 0.5),
            (4 * BAR, 0.25)
        ]
    );
    // All the points go: the value before and the value that follows stay.
    let mut inside = Doc::new_default();
    volume(&mut inside, false, &[(2 * BAR, 0.25), (3 * BAR, 0.75)]);
    let new = remove_time(&inside, BAR, 4 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(BAR - 1, 0.25), (BAR, 0.75)]);
    let new = remove_time(&inside, 0, 4 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(0, 0.75)]);
    // Steps: the value in force after what goes is in force from the seam.
    let mut steps = Doc::new_default();
    volume(
        &mut steps,
        true,
        &[(BAR, 1.0), (3 * BAR, 0.0), (5 * BAR, 1.0)],
    );
    let new = remove_time(&steps, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(BAR, 1.0), (2 * BAR, 0.0), (3 * BAR, 1.0)]);
    let new = remove_time(&steps, 2 * BAR, 6 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(BAR, 1.0)]);
    let new = remove_time(&steps, 0, 2 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(0, 1.0), (BAR, 0.0), (3 * BAR, 1.0)]);
    let new = remove_time(&steps, 0, 4 * BAR).unwrap();
    assert_eq!(lane_of(&new), [(0, 0.0), (BAR, 1.0)]);
}

#[test]
fn a_copy_has_the_points_of_the_section_and_starts_with_its_value() {
    let mut doc = Doc::new_default();
    volume(
        &mut doc,
        false,
        &[(BAR, 0.25), (3 * BAR, 0.75), (6 * BAR, 1.0)],
    );
    let end = doc.tracks[0].automation[0].value_at(4 * BAR - 1).unwrap();
    let new = duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        lane_of(&new),
        [
            (BAR, 0.25),
            (3 * BAR, 0.75),
            (4 * BAR - 1, end),
            (4 * BAR, 0.5),
            (5 * BAR, 0.75),
            (8 * BAR, 1.0),
        ]
    );
    assert_eq!(
        lane_of(&remove_time(&new, 4 * BAR, 6 * BAR).unwrap()),
        lane_of(&doc)
    );
}

#[test]
fn the_masters_lane_moves_as_a_tracks_does() {
    let mut doc = Doc::new_default();
    let l = lane(
        &mut doc,
        AutoTarget::Volume,
        false,
        &[(0, 0.75), (4 * BAR, 0.25)],
    );
    doc.master_lanes = vec![l];
    let put = insert_time(&doc, 2 * BAR, 2).unwrap();
    assert_eq!(
        lane_points(&put.master_lanes[0]),
        [(0, 0.75), (2 * BAR, 0.5), (4 * BAR, 0.5), (6 * BAR, 0.25)]
    );
    assert_ne!(put.master_lanes[0].id, doc.master_lanes[0].id);
    assert_eq!(put.master_lanes[0].id, LaneId(doc.next_id));
    assert_eq!(put.next_id, doc.next_id + 1);
    let back = remove_time(&put, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        lane_points(&back.master_lanes[0]),
        lane_points(&doc.master_lanes[0])
    );
    let twice = duplicate_range(&doc, 0, 4 * BAR).unwrap();
    let end = doc.master_lanes[0].value_at(4 * BAR - 1).unwrap();
    assert_eq!(
        lane_points(&twice.master_lanes[0]),
        [
            (0, 0.75),
            (4 * BAR - 1, end),
            (4 * BAR, 0.75),
            (8 * BAR, 0.25)
        ]
    );
    let gone = remove_time(&doc, BAR, 2 * BAR).unwrap();
    assert_eq!(
        lane_points(&gone.master_lanes[0])[2..],
        [(BAR, 0.5), (3 * BAR, 0.25)]
    );
    // Empty bars after its last point leave it as it is.
    let put = insert_time(&doc, 4 * BAR, 2).unwrap();
    assert!(Arc::ptr_eq(&put.master_lanes[0], &doc.master_lanes[0]));
}

// ---- clips, markers, the loop ----

/// Track 1 with a clip over bars 1 to 3 (a note held over the head of bar 2, one in bar 2)
/// and one over bar 4; markers at bars 0, 2 and 4.
fn clips_song() -> (Doc, ClipId, ClipId) {
    let mut doc = Doc::new_default();
    let a = record(
        &mut doc,
        T1,
        BAR,
        3 * BAR,
        vec![note(BAR - Q, 2 * Q, 60), note(BAR + 2 * Q, Q, 64)],
    );
    let b = record(&mut doc, T1, 4 * BAR, 5 * BAR, vec![note(0, Q, 67)]);
    mark(&mut doc, 0, "イントロ");
    mark(&mut doc, 2 * BAR, "A メロ");
    mark(&mut doc, 4 * BAR, "サビ");
    (doc, a, b)
}

fn ons(doc: &Doc, track: TrackId) -> Vec<(u64, bool, u8)> {
    use crate::playback::PlayKind;
    track_events(doc, track)
        .into_iter()
        .filter_map(|e| match e.kind {
            PlayKind::NoteOn { key, .. } => Some((e.tick, true, key)),
            PlayKind::NoteOff { key } => Some((e.tick, false, key)),
            _ => None,
        })
        .collect()
}

#[test]
fn empty_bars_cut_a_clip_over_the_cut_and_move_its_later_half() {
    let (doc, a, b) = clips_song();
    let new = insert_time(&doc, 2 * BAR, 2).unwrap();
    let half = new.tracks[0].clips[1].id;
    assert_eq!(
        spans(&new, T1),
        [
            (a.0, BAR, 2 * BAR),
            (half.0, 4 * BAR, 5 * BAR),
            (b.0, 6 * BAR, 7 * BAR)
        ]
    );
    assert!(doc.clip(half).is_none());
    // The note over the cut ends there; the one after it sounds two bars later.
    assert_eq!(
        ons(&new, T1),
        [
            (2 * BAR - Q, true, 60),
            (2 * BAR, false, 60),
            (4 * BAR + 2 * Q, true, 64),
            (4 * BAR + 3 * Q, false, 64),
            (6 * BAR, true, 67),
            (6 * BAR + Q, false, 67),
        ]
    );
    // The take of the later half is the one the clip had; no note of it changed.
    let (_, old) = doc.clip(a).unwrap();
    let (_, right) = new.clip(half).unwrap();
    assert!(Arc::ptr_eq(&right.keys()[0].take, &old.keys()[0].take));
    assert_eq!(right.keys()[0].offset, -(BAR as i64));
    assert_eq!(
        marks(&new),
        [(0, "イントロ"), (4 * BAR, "A メロ"), (6 * BAR, "サビ")]
    );
    check(&new);
    // The clip of bar 4 moved as it is: the same id, the same take.
    assert!(Arc::ptr_eq(
        &new.clip(b).unwrap().1.keys()[0].take,
        &doc.clip(b).unwrap().1.keys()[0].take
    ));
    // Taking the bars out again leaves the clip in two halves that sound as the clip did,
    // but for the note that was cut.
    let back = remove_time(&new, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        spans(&back, T1),
        [
            (a.0, BAR, 2 * BAR),
            (half.0, 2 * BAR, 3 * BAR),
            (b.0, 4 * BAR, 5 * BAR)
        ]
    );
    assert_eq!(marks(&back), marks(&doc));
    assert_eq!(ons(&back, T1)[2..], ons(&doc, T1)[2..]);
}

#[test]
fn bars_taken_out_take_their_clips_and_markers_and_pull_up_what_follows() {
    let (doc, a, b) = clips_song();
    let new = remove_time(&doc, 2 * BAR, 4 * BAR).unwrap();
    assert_eq!(
        spans(&new, T1),
        [(a.0, BAR, 2 * BAR), (b.0, 2 * BAR, 3 * BAR)]
    );
    assert_eq!(marks(&new), [(0, "イントロ"), (2 * BAR, "サビ")]);
    assert_eq!(
        ons(&new, T1),
        [
            (2 * BAR - Q, true, 60),
            (2 * BAR, false, 60),
            (2 * BAR, true, 67),
            (2 * BAR + Q, false, 67),
        ]
    );
    check(&new);
    // The first bars: the clip is cut at bar 2, and its later half comes to the head.
    let new = remove_time(&doc, 0, 2 * BAR).unwrap();
    let half = new.tracks[0].clips[0].id;
    assert_eq!(spans(&new, T1), [(half.0, 0, BAR), (b.0, 2 * BAR, 3 * BAR)]);
    assert_eq!(marks(&new), [(0, "A メロ"), (2 * BAR, "サビ")]);
    assert_eq!(ons(&new, T1)[..2], [(2 * Q, true, 64), (3 * Q, false, 64)]);
}

#[test]
fn a_copy_has_new_clips_on_the_same_takes_and_a_marker_of_the_same_name() {
    let (doc, a, b) = clips_song();
    let new = duplicate_range(&doc, 2 * BAR, 4 * BAR).unwrap();
    let ids: Vec<ClipId> = new.tracks[0].clips.iter().map(|c| c.id).collect();
    let (half, copy) = (ids[1], ids[2]);
    assert_eq!(
        spans(&new, T1),
        [
            (a.0, BAR, 2 * BAR),
            (half.0, 2 * BAR, 3 * BAR),
            (copy.0, 4 * BAR, 5 * BAR),
            (b.0, 6 * BAR, 7 * BAR)
        ]
    );
    assert!(doc.clip(half).is_none() && doc.clip(copy).is_none() && half != copy);
    let take = |id: ClipId| new.clip(id).unwrap().1.keys()[0].clone();
    assert!(Arc::ptr_eq(&take(half).take, &take(copy).take));
    assert_eq!(take(half).offset, take(copy).offset);
    assert_eq!(
        marks(&new),
        [
            (0, "イントロ"),
            (2 * BAR, "A メロ"),
            (4 * BAR, "A メロ"),
            (6 * BAR, "サビ")
        ]
    );
    let made = new.markers[2].id;
    assert!(doc.marker(made).is_none());
    assert_eq!(new.markers[1].id, doc.markers[1].id);
    assert_eq!(new.markers[3].id, doc.markers[2].id);
    check(&new);
    IdBook::new(&doc).saw(&new, Some(&doc)).unwrap();
    // Without the copy the song is the one after the cut alone.
    let back = remove_time(&new, 4 * BAR, 6 * BAR).unwrap();
    assert_eq!(
        spans(&back, T1),
        [
            (a.0, BAR, 2 * BAR),
            (half.0, 2 * BAR, 3 * BAR),
            (b.0, 4 * BAR, 5 * BAR)
        ]
    );
    assert_eq!(back.markers, doc.markers);
}

fn vocal_track(doc: &mut Doc, start: u64, end: u64, shape: AudioShape) -> ClipId {
    let source = Arc::new(AudioSource {
        id: AudioId(doc.next_id),
        uid: [7; 16],
        file: "歌 テイク 1.wav".to_owned(),
        frames: 960_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    });
    let take = ClipTake {
        take: Arc::new(AudioTake {
            id: TakeId(doc.next_id + 1),
            source: source.clone(),
            first_frame: 0,
            frames: 960_000,
            sync: Some(SourceTempo {
                tempo: doc.timeline.tempo,
                head_ramp: false,
                points: Vec::new(),
            }),
        }),
        offset: 0,
    };
    let id = ClipId(doc.next_id + 2);
    doc.next_id += 3;
    let clip = Clip {
        id,
        name: "歌 テイク 1".to_owned(),
        start,
        end,
        takes: ClipTakes::Vocal {
            takes: vec![take],
            shape,
        },
        active: 0,
        comp: Vec::new(),
    };
    doc.tracks.push(Arc::new(Track {
        id: T3,
        name: crate::vocal_track_name(crate::Language::Ja).to_owned(),
        kind: TrackKind::Vocal {
            input: VocalInput::Mono1,
        },
        clips: vec![Arc::new(clip)],
        automation: Vec::new(),
        ..Track::clone(&doc.tracks[0])
    }));
    doc.audio = Arc::new(vec![source]);
    id
}

#[test]
fn an_audio_clip_is_cut_as_s_cuts_it_and_moved_with_its_takes_as_they_are() {
    let mut doc = Doc::new_default();
    let shape = AudioShape {
        fade_in: Q,
        fade_out: 2 * Q,
        ..AudioShape::RECORDED
    };
    let c = vocal_track(&mut doc, BAR, 5 * BAR, shape);
    let new = insert_time(&doc, 2 * BAR, 4).unwrap();
    let half = new.tracks[1].clips[1].id;
    assert_eq!(
        spans(&new, T3),
        [(c.0, BAR, 2 * BAR), (half.0, 6 * BAR, 9 * BAR)]
    );
    let (_, old) = doc.clip(c).unwrap();
    let (_, left) = new.clip(c).unwrap();
    let (_, right) = new.clip(half).unwrap();
    // The same audio at the same place of the take: what plays in each half is what played.
    assert!(Arc::ptr_eq(
        &left.takes.vocal()[0].take,
        &old.takes.vocal()[0].take
    ));
    assert!(Arc::ptr_eq(
        &right.takes.vocal()[0].take,
        &old.takes.vocal()[0].take
    ));
    assert_eq!(left.takes.vocal()[0].offset, 0);
    assert_eq!(right.takes.vocal()[0].offset, -(BAR as i64));
    let fades = |c: &Clip| c.takes.shape().map(|s| (s.fade_in, s.fade_out)).unwrap();
    assert_eq!(fades(left), (Q, 0));
    assert_eq!(fades(right), (0, 2 * Q));
    // A clip that is only moved keeps its shape, its offsets and its takes.
    let moved = insert_time(&doc, BAR, 1).unwrap();
    let (_, m) = moved.clip(c).unwrap();
    assert_eq!((m.start, m.end), (2 * BAR, 6 * BAR));
    assert_eq!(m.takes, old.takes);
    // A copy plays the same takes.
    let twice = duplicate_range(&doc, BAR, 5 * BAR).unwrap();
    let copy = &twice.tracks[1].clips[1];
    assert_eq!((copy.start, copy.end), (5 * BAR, 9 * BAR));
    assert_ne!(copy.id, c);
    assert_eq!(copy.takes, old.takes);
    assert_eq!(twice.audio, doc.audio);
}

#[test]
fn the_loop_moves_with_what_it_is_on() {
    let range = |a: u64, b: u64| TickRange::new(a * BAR, b * BAR);
    let with_loop = |a: u64, b: u64| {
        let mut doc = Doc::new_default();
        doc.loop_range = range(a, b);
        doc
    };
    // Empty bars at bar 4.
    for ((a, b), want) in [
        ((1, 3), range(1, 3)),
        ((2, 4), range(2, 4)),
        ((3, 6), range(3, 8)),
        ((4, 6), range(6, 8)),
        ((5, 6), range(7, 8)),
    ] {
        let new = insert_time(&with_loop(a, b), 4 * BAR, 2).unwrap();
        assert_eq!(new.loop_range, want, "{a} {b}");
        let back = remove_time(&new, 4 * BAR, 6 * BAR).unwrap();
        assert_eq!(back.loop_range, range(a, b), "{a} {b}");
    }
    // Bars 4 and 5 go.
    for ((a, b), want) in [
        ((1, 4), range(1, 4)),
        ((1, 5), range(1, 4)),
        ((1, 8), range(1, 6)),
        ((4, 6), None),
        ((4, 5), None),
        ((5, 6), None),
        ((5, 8), range(4, 6)),
        ((4, 8), range(4, 6)),
        ((6, 8), range(4, 6)),
        ((7, 8), range(5, 6)),
    ] {
        let new = remove_time(&with_loop(a, b), 4 * BAR, 6 * BAR).unwrap();
        assert_eq!(new.loop_range, want, "{a} {b}");
    }
    // A copy of bars 2 and 3 at bar 4: the loop on the section stays on the section.
    for ((a, b), want) in [
        ((2, 4), range(2, 4)),
        ((1, 3), range(1, 3)),
        ((3, 5), range(3, 7)),
        ((4, 6), range(6, 8)),
    ] {
        let new = duplicate_range(&with_loop(a, b), 2 * BAR, 4 * BAR).unwrap();
        assert_eq!(new.loop_range, want, "{a} {b}");
        let back = remove_time(&new, 4 * BAR, 6 * BAR).unwrap();
        assert_eq!(back.loop_range, range(a, b), "{a} {b}");
    }
}

// ---- sections and steps ----

#[test]
fn a_section_is_from_its_marker_to_the_next_later_one_or_to_the_songs_end() {
    let (mut doc, _, _) = clips_song();
    let ids: Vec<MarkerId> = doc.markers.iter().map(|m| m.id).collect();
    let usable = |from: u64, to: u64, bars| Section::Usable {
        from: from * BAR,
        to: to * BAR,
        bars,
    };
    assert_eq!(section_of(&doc, ids[0]), Some(usable(0, 2, 2)));
    assert_eq!(section_of(&doc, ids[1]), Some(usable(2, 4, 2)));
    // The last clip ends at bar 5.
    assert_eq!(section_of(&doc, ids[2]), Some(usable(4, 5, 1)));
    assert_eq!(section_of(&doc, MarkerId(999)), None);
    let twin = mark(&mut doc, 2 * BAR, "もう1つ");
    assert_eq!(section_of(&doc, twin), Some(usable(2, 4, 2)));
    let off = mark(&mut doc, 3 * BAR + Q, "拍");
    assert_eq!(section_of(&doc, ids[1]), Some(Section::NotOnBars));
    assert_eq!(section_of(&doc, off), Some(Section::NotOnBars));
    let end = mark(&mut doc, 5 * BAR, "おわり");
    assert_eq!(section_of(&doc, end), Some(Section::Empty));
    let past = mark(&mut doc, 9 * BAR, "先");
    assert_eq!(section_of(&doc, end), Some(usable(5, 9, 4)));
    assert_eq!(section_of(&doc, past), Some(Section::Empty));
    for (edit, want) in [
        (
            duplicate_section(&doc, MarkerId(999)),
            EditError::NoSuchPoint,
        ),
        (delete_section(&doc, MarkerId(999)), EditError::NoSuchPoint),
        (insert_space(&doc, MarkerId(999), 4), EditError::NoSuchPoint),
        (duplicate_section(&doc, off), EditError::Unchanged),
        (delete_section(&doc, ids[1]), EditError::Unchanged),
        (insert_space(&doc, off, 4), EditError::Unchanged),
        (insert_space(&doc, past, 4), EditError::Unchanged),
        (delete_section(&doc, past), EditError::Unchanged),
        (insert_space(&doc, ids[0], 0), EditError::Unchanged),
    ] {
        assert_eq!(edit, Err(want));
    }
}

#[test]
fn each_is_one_step_that_undo_takes_back_whole() {
    let mut doc = rich(Tempos::Ramps, true);
    let ids: Vec<MarkerId> = doc.markers.iter().map(|m| m.id).collect();
    let mut h = History::new();
    let mut book = IdBook::new(&doc);
    let mut seen = vec![doc.clone()];
    type Make<'a> = &'a dyn Fn(&Doc) -> Result<Edit, EditError>;
    let steps: [(Make, StepKind); 4] = [
        (
            &|d| duplicate_section(d, ids[1]),
            StepKind::DuplicateSection {
                marker: ids[1],
                bars: 2,
            },
        ),
        (
            &|d| insert_space(d, ids[2], 3),
            StepKind::InsertSpace {
                marker: ids[2],
                bars: 3,
            },
        ),
        (
            &|d| delete_section(d, ids[0]),
            StepKind::DeleteSection {
                marker: ids[0],
                bars: 2,
            },
        ),
        (
            &|d| delete_section(d, ids[3]),
            StepKind::DeleteSection {
                marker: ids[3],
                bars: 2,
            },
        ),
    ];
    for (make, step) in &steps {
        let e = make(&doc).unwrap();
        assert_eq!(&e.step, step);
        let before = doc.clone();
        h.apply(&mut doc, e);
        check(&doc);
        book.saw(&doc, Some(&before)).unwrap();
        seen.push(doc.clone());
    }
    assert_eq!(h.undo_count(), 4);
    for want in seen.iter().rev().skip(1) {
        h.undo(&mut doc).unwrap();
        assert_eq!(held(&doc), held(want));
    }
    for want in seen.iter().skip(1) {
        h.redo(&mut doc).unwrap();
        assert_eq!(held(&doc), held(want));
        book.saw(&doc, None).unwrap();
    }
}

#[test]
fn a_song_that_would_get_too_long_is_refused() {
    let mut doc = Doc::new_default();
    let far = (MAX_TICK / BAR - 2) * BAR;
    mark(&mut doc, far, "遠く");
    mark(&mut doc, BAR, "A");
    assert_eq!(
        insert_time(&doc, BAR, 1).map(|d| d.markers[1].tick),
        Ok(far + BAR)
    );
    assert_eq!(insert_time(&doc, BAR, 4), Err(EditError::OffTheSong));
    assert_eq!(
        duplicate_range(&doc, 0, 4 * BAR),
        Err(EditError::OffTheSong)
    );
    assert_eq!(insert_time(&doc, far, 4), Err(EditError::OffTheSong));
    type Put = Box<dyn Fn(&mut Doc)>;
    let far_things: [(&str, Put); 5] = [
        (
            "clip",
            Box::new(move |d| {
                record(d, T1, far, far + BAR, vec![note(0, Q, 60)]);
            }),
        ),
        (
            "lane",
            Box::new(move |d| volume(d, false, &[(far + BAR, 0.5)])),
        ),
        (
            "tempo",
            Box::new(move |d| {
                let e = crate::timeline::add_tempo_point(d, far + BAR, tempo(9_000));
                *d = e.unwrap().doc;
            }),
        ),
        (
            "loop",
            Box::new(move |d| d.loop_range = TickRange::new(BAR, far + BAR)),
        ),
        (
            "master",
            Box::new(move |d| {
                let l = lane(d, AutoTarget::Volume, false, &[(far + BAR, 0.5)]);
                d.master_lanes = vec![l];
            }),
        ),
    ];
    for (what, put) in far_things {
        let mut doc = Doc::new_default();
        put(&mut doc);
        assert_eq!(insert_time(&doc, 0, 1).map(|_| ()), Ok(()), "{what}");
        assert_eq!(
            insert_time(&doc, 0, 2),
            Err(EditError::OffTheSong),
            "{what}"
        );
        assert_eq!(
            duplicate_range(&doc, 0, 2 * BAR),
            Err(EditError::OffTheSong),
            "{what}"
        );
    }
    // Bar numbers past what a meter change can hold.
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (1, 16), &[(u32::MAX - 1, 4, 4)]);
    assert_eq!(
        insert_time(&doc, 0, 1).map(|d| meters(&d)[1].0),
        Ok(u32::MAX)
    );
    assert_eq!(insert_time(&doc, 0, 2), Err(EditError::OffTheSong));
}

// ---- every list at once ----

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tempos {
    Flat,
    Jumps,
    LongRamp,
    Ramps,
    OffBars,
    TickBefore,
}

const ALL_TEMPOS: [Tempos; 6] = [
    Tempos::Flat,
    Tempos::Jumps,
    Tempos::LongRamp,
    Tempos::Ramps,
    Tempos::OffBars,
    Tempos::TickBefore,
];

/// 4/4, 3/4 from bar 4, 4/4 from bar 8; markers at bars 0, 2, 4, 6, 8 and 10; clips on two
/// keys tracks and a vocal track up to bar 12; lanes on tracks and the master; a loop over
/// bars 2 and 3. `crossing`: also clips over the heads of bars 2, 4, 6 and 8.
fn rich(tempos: Tempos, crossing: bool) -> Doc {
    let mut doc = Doc::new_default();
    with_meters(&mut doc, (4, 4), &[(4, 3, 4), (8, 4, 4)]);
    let b = |n: u64| TempoMap::new(&doc.timeline).tick_of_bar(n);
    let b: Vec<u64> = (0..=14).map(b).collect();
    let (head, head_ramp, points) = match tempos {
        Tempos::Flat => (12_000, false, vec![]),
        Tempos::Jumps => (
            12_000,
            false,
            vec![point(b[2], 14_000, false), point(b[5], 9_000, false)],
        ),
        Tempos::LongRamp => (9_001, true, vec![point(b[12], 17_777, false)]),
        Tempos::Ramps => (
            10_000,
            false,
            vec![
                point(b[2], 12_000, true),
                point(b[4], 15_000, false),
                point(b[6], 13_000, true),
                point(b[8], 10_000, true),
                point(b[9], 11_000, false),
            ],
        ),
        Tempos::OffBars => (
            11_000,
            true,
            vec![
                point(b[1] + Q, 13_000, true),
                point(b[3] + Q, 9_000, false),
                point(b[6] + Q, 10_000, true),
                point(b[10] + Q, 16_000, false),
            ],
        ),
        Tempos::TickBefore => (
            10_000,
            false,
            vec![point(b[4] - 1, 10_500, true), point(b[7], 14_000, false)],
        ),
    };
    doc.timeline = Arc::new(Timeline {
        tempo: tempo(head),
        head_ramp,
        tempo_points: points,
        ..Timeline::clone(&doc.timeline)
    });
    doc.tracks.push(Arc::new(Track {
        id: T2,
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::SINE,
        ..Track::clone(&doc.tracks[0])
    }));
    for (from, to) in [(0, 2), (2, 3), (4, 6), (8, 10), (10, 12)] {
        let len = b[to] - b[from];
        let notes = vec![note(0, Q, 60), note(len - Q, Q, 62)];
        record(&mut doc, T1, b[from], b[to], notes);
    }
    if crossing {
        // Notes held over the heads of bars 2 and 4.
        let notes = vec![
            note(0, Q, 48),
            note(b[2] - b[1] - Q, 2 * Q, 50),
            note(b[4] - b[1] - 1, Q, 52),
        ];
        record(&mut doc, T2, b[1], b[5], notes);
        record(&mut doc, T2, b[5], b[9], vec![note(Q, b[8] - b[5], 55)]);
    } else {
        record(&mut doc, T2, b[6], b[8], vec![note(Q, Q, 55)]);
    }
    let shape = AudioShape {
        fade_in: Q,
        fade_out: Q,
        ..AudioShape::RECORDED
    };
    match crossing {
        true => vocal_track(&mut doc, b[3], b[9], shape),
        false => vocal_track(&mut doc, b[2], b[4], shape),
    };
    let lanes = vec![
        lane(
            &mut doc,
            AutoTarget::Volume,
            false,
            &[
                (b[1], 0.2),
                (b[3], 0.6),
                (b[5], 0.4),
                (b[6] - 1, 0.4),
                (b[6], 0.8),
                (b[9], 0.5),
            ],
        ),
        lane(
            &mut doc,
            AutoTarget::Mute,
            true,
            &[(b[2], 1.0), (b[5], 0.0), (b[8], 1.0), (b[11], 0.0)],
        ),
    ];
    track_mut(&mut doc, T1).automation = lanes;
    let lanes = vec![
        lane(&mut doc, AutoTarget::Pan, false, &[(b[7], 0.3)]),
        lane(&mut doc, AutoTarget::Volume, false, &[]),
    ];
    track_mut(&mut doc, T2).automation = lanes;
    let l = lane(
        &mut doc,
        AutoTarget::Volume,
        false,
        &[(0, 0.8), (b[4], 0.7), (b[10], 0.75), (b[12], 0.0)],
    );
    doc.master_lanes = vec![l];
    for (i, name) in ["イントロ", "A メロ", "B メロ", "サビ", "間奏", "おわり"]
        .into_iter()
        .enumerate()
    {
        mark(&mut doc, b[2 * i], name);
    }
    doc.loop_range = TickRange::new(b[2], b[4]);
    check(&doc);
    doc
}

/// The cuts of the sections of `doc`: `(from, to, bars)`.
fn cuts(doc: &Doc) -> Vec<(u64, u64, u32)> {
    (doc.markers.iter())
        .map(|m| match section_of(doc, m.id).unwrap() {
            Section::Usable { from, to, bars } => (from, to, bars),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn lanes(doc: &Doc) -> Vec<&Arc<Lane>> {
    (doc.tracks.iter())
        .flat_map(|t| &t.automation)
        .chain(&doc.master_lanes)
        .collect()
}

/// A lane has a new id when its points changed, and only then.
fn lane_ids_follow(old: &Doc, new: &Doc, what: &str) {
    for (a, b) in lanes(old).into_iter().zip(lanes(new)) {
        assert_eq!(
            a.id == b.id,
            lane_points(a) == lane_points(b),
            "{what}: lane id"
        );
    }
}

/// Checks that `new` is `old` before `keep`, then `gap` ticks where nothing moves, then
/// `old` from `resume` on: the time of each tick before the seam, the time between ticks
/// after it, the values of every lane, and the notes that sound.
fn is_spliced(old: &Doc, new: &Doc, keep: u64, gap: u64, resume: u64, what: &str) {
    check(new);
    let (was, now) = (TempoMap::new(&old.timeline), TempoMap::new(&new.timeline));
    let end = was.tick_of_bar(14);
    let to_new = |tick: u64| tick - resume + keep + gap;
    // A tempo is kept in hundredths: a point made inside a ramp moves the times inside
    // that ramp by a little.
    const SECONDS: f64 = 2e-3;
    let mut tick = 0;
    while tick <= end {
        if tick <= keep {
            let (a, b) = (was.seconds_at(tick as f64), now.seconds_at(tick as f64));
            assert!((a - b).abs() < SECONDS, "{what}: time of {tick}: {a} {b}");
            if tick < keep {
                assert_eq!(was.meter_at(tick), now.meter_at(tick), "{what}: {tick}");
            }
        }
        if tick >= resume {
            let a = was.seconds_at(tick as f64) - was.seconds_at(resume as f64);
            let b = now.seconds_at(to_new(tick) as f64) - now.seconds_at(to_new(resume) as f64);
            assert!((a - b).abs() < SECONDS, "{what}: time from {tick}: {a} {b}");
            assert_eq!(
                was.meter_at(tick).meter,
                now.meter_at(to_new(tick)).meter,
                "{what}: {tick}"
            );
            let (a, b) = (was.tempo_at(tick as f64), now.tempo_at(to_new(tick) as f64));
            assert!((a - b).abs() < 0.011, "{what}: tempo at {tick}: {a} {b}");
        }
        tick += Q / 2;
    }
    // In the empty bars the tempo stays what it came to.
    if gap > 0 && keep > 0 {
        let held = was.tempo_at(keep as f64 - 1e-6);
        for tick in [keep, keep + gap / 2, keep + gap - 1] {
            let t = now.tempo_at(tick as f64);
            assert!(
                (t - held).abs() < 0.011,
                "{what}: tempo in the gap {t} {held}"
            );
        }
    }
    for (a, b) in lanes(old).into_iter().zip(lanes(new)) {
        assert_eq!((a.target, a.step, a.resting), (b.target, b.step, b.resting));
        assert_eq!(a.points.is_empty(), b.points.is_empty());
        let mut tick = 0;
        while tick <= end && !a.points.is_empty() {
            let near = |x: Option<f32>, y: Option<f32>, at: u64| {
                let (x, y) = (x.unwrap(), y.unwrap());
                assert!(
                    (x - y).abs() < 1e-5,
                    "{what}: lane {:?} at {at}: {x} {y}",
                    a.target
                );
            };
            if tick < keep {
                near(a.value_at(tick), b.value_at(tick), tick);
            }
            if tick >= resume {
                near(a.value_at(tick), b.value_at(to_new(tick)), tick);
            }
            tick += Q / 2;
        }
        if gap > 0 && keep > 0 && !a.points.is_empty() {
            let held = a.value_at(keep - 1);
            let held = if a.step { held } else { a.value_at(keep) };
            for tick in [keep, keep + gap / 2, keep + gap - 1] {
                let v = b.value_at(tick).unwrap();
                assert!((v - held.unwrap()).abs() < 1e-5, "{what}: lane in the gap");
            }
        }
    }
    // The notes: those on before `keep` (ended there at the latest), and those on from
    // `resume`, as far from each other as they were.
    for t in &old.tracks {
        let before: Vec<_> = ons(old, t.id)
            .into_iter()
            .filter(|&(_, on, _)| on)
            .collect();
        let after: Vec<_> = ons(new, t.id)
            .into_iter()
            .filter(|&(_, on, _)| on)
            .collect();
        let mut want: Vec<_> = (before.iter().copied())
            .filter(|&(tick, _, _)| tick < keep)
            .collect();
        want.extend(
            (before.iter())
                .filter(|&&(tick, _, _)| tick >= resume)
                .map(|&(tick, on, key)| (to_new(tick), on, key)),
        );
        assert_eq!(after, want, "{what}: notes of track {}", t.id.0);
        let audio = |doc: &Doc| -> Vec<(u64, u64, i64, TakeId)> {
            (doc.track(t.id).unwrap().clips.iter())
                .flat_map(|c| {
                    (c.takes.vocal().iter())
                        .map(|ct| (c.start, c.end, c.zero(ct), ct.take.id))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        // Audio: the same take plays over every tick it played over, at the same place
        // of the take.
        let plays = |doc: &Doc, tick: u64| {
            audio(doc)
                .into_iter()
                .find(|&(start, end, _, _)| (start..end).contains(&tick))
                .map(|(_, _, zero, take)| (tick as i64 - zero, take))
        };
        let mut tick = 0;
        while tick <= end {
            if tick < keep {
                assert_eq!(
                    plays(old, tick),
                    plays(new, tick),
                    "{what}: audio at {tick}"
                );
            }
            if tick >= resume {
                assert_eq!(plays(old, tick), plays(new, to_new(tick)), "{what}: audio");
            }
            if gap > 0 && (keep..keep + gap).contains(&tick) {
                assert_eq!(plays(new, tick), None, "{what}: audio in the gap");
            }
            tick += Q / 2;
        }
    }
    if gap > 0 {
        for t in &new.tracks {
            assert!(
                (t.clips.iter()).all(|c| c.end <= keep || c.start >= keep + gap),
                "{what}: a clip in the gap"
            );
        }
        assert!(
            new.markers
                .iter()
                .all(|m| !(keep..keep + gap).contains(&m.tick))
        );
    }
}

#[test]
fn every_section_of_every_song_splices_and_comes_back() {
    for tempos in ALL_TEMPOS {
        for crossing in [false, true] {
            let doc = rich(tempos, crossing);
            for (i, (from, to, bars)) in cuts(&doc).into_iter().enumerate() {
                let what = format!("{tempos:?}, crossing {crossing}, section {i}");
                let len = to - from;

                // Empty bars, and the same bars out again.
                for n in [1u32, bars, 7] {
                    let put = insert_time(&doc, from, n).unwrap();
                    let gap = u64::from(n) * (len / u64::from(bars));
                    is_spliced(&doc, &put, from, gap, from, &format!("{what}: {n} bars in"));
                    lane_ids_follow(&doc, &put, &what);
                    assert_eq!(marks(&put).len(), marks(&doc).len());
                    let back = remove_time(&put, from, from + gap).unwrap();
                    check(&back);
                    if crossing {
                        // The clips cut at `from` stay cut; all else is as it was.
                        assert_eq!(back.timeline, doc.timeline, "{what}");
                        assert_eq!(back.markers, doc.markers, "{what}");
                        assert_eq!(back.loop_range, doc.loop_range, "{what}");
                        for (a, b) in lanes(&doc).into_iter().zip(lanes(&back)) {
                            assert_eq!(lane_points(a), lane_points(b), "{what}");
                        }
                        is_spliced(
                            &doc,
                            &back,
                            from,
                            0,
                            from,
                            &format!("{what}: {n} bars back"),
                        );
                    } else {
                        assert_eq!(sans_ids(&back), sans_ids(&doc), "{what}: {n} bars back");
                    }
                }

                // A copy, and the copy out again.
                let twice = duplicate_range(&doc, from, to).unwrap();
                is_spliced(&doc, &twice, to, 0, from, &format!("{what}: copy"));
                lane_ids_follow(&doc, &twice, &what);
                IdBook::new(&doc).saw(&twice, Some(&doc)).unwrap();
                let back = remove_time(&twice, to, to + len).unwrap();
                check(&back);
                if crossing {
                    assert_eq!(back.timeline, doc.timeline, "{what}");
                    assert_eq!(back.markers, doc.markers, "{what}");
                    assert_eq!(back.loop_range, doc.loop_range, "{what}");
                    for (a, b) in lanes(&doc).into_iter().zip(lanes(&back)) {
                        assert_eq!(lane_points(a), lane_points(b), "{what}");
                    }
                } else {
                    assert_eq!(sans_ids(&back), sans_ids(&doc), "{what}: copy back");
                }

                // The section out.
                let gone = remove_time(&doc, from, to).unwrap();
                is_spliced(&doc, &gone, from, 0, to, &format!("{what}: out"));
                lane_ids_follow(&doc, &gone, &what);
                assert_eq!(marks(&gone).len(), marks(&doc).len() - 1);
            }
        }
    }
}
