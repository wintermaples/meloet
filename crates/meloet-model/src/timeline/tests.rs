use super::*;
use crate::TrackId;
use crate::edit::{self, NewTake};
use crate::history::History;
use crate::id_book::{IdBook, held};
use crate::song::Note;
use crate::tempo_map::TempoMap;
use crate::time::TICKS_PER_QUARTER;

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
/// A bar of 3/4.
const BAR34: u64 = 3 * Q;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn meter(num: u8, den: u8) -> Meter {
    Meter::new(num, den).unwrap()
}

fn points(doc: &Doc) -> Vec<(u64, u32, bool)> {
    (doc.timeline.tempo_points.iter())
        .map(|p| (p.tick, p.tempo.centi(), p.ramp))
        .collect()
}

fn changes(doc: &Doc) -> Vec<(u32, u8, u8)> {
    (doc.timeline.meter_changes.iter())
        .map(|c| (c.bar, c.meter.num(), c.meter.den()))
        .collect()
}

fn markers(doc: &Doc) -> Vec<(u32, u64, &str)> {
    (doc.markers.iter())
        .map(|m| (m.id.0, m.tick, m.name.as_str()))
        .collect()
}

fn take(origin: u64, length: u64) -> NewTake {
    NewTake {
        origin,
        length,
        notes: vec![Note {
            from: None,
            start: 0,
            length: 1,
            key: 60,
            velocity: 100,
        }],
        controls: Vec::new(),
    }
}

#[test]
fn tempo_points_are_added_set_moved_and_deleted() {
    let doc = Doc::new_default();
    let e = add_tempo_point(&doc, 4 * BAR, tempo(10_400)).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddTempoPoint {
            tick: 4 * BAR,
            tempo: tempo(10_400)
        }
    );
    let doc = e.doc;
    let doc = add_tempo_point(&doc, 8 * BAR, tempo(12_000)).unwrap().doc;
    assert_eq!(
        points(&doc),
        [(4 * BAR, 10_400, false), (8 * BAR, 12_000, false)]
    );
    // On a point, and on the head: that one takes the tempo.
    let e = add_tempo_point(&doc, 4 * BAR, tempo(10_000)).unwrap();
    assert_eq!(
        e.step,
        StepKind::SetTempoPoint {
            tick: 4 * BAR,
            from: tempo(10_400),
            to: tempo(10_000)
        }
    );
    assert_eq!(points(&e.doc).len(), 2);
    let e = add_tempo_point(&doc, 0, tempo(9_000)).unwrap();
    assert_eq!(
        e.step,
        StepKind::Tempo {
            from: tempo(9_600),
            to: tempo(9_000)
        }
    );
    assert_eq!(e.doc.timeline.tempo, tempo(9_000));
    assert_eq!(
        add_tempo_point(&doc, 4 * BAR, tempo(10_400)),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        set_tempo_at(&doc, 5 * BAR, tempo(10_400)),
        Err(EditError::NoSuchPoint)
    );
    assert_eq!(
        add_tempo_point(&doc, MAX_TICK + 1, tempo(10_400)),
        Err(EditError::OffTheSong)
    );

    // A ramp, and a point put into it: the line goes on.
    let doc = set_tempo_ramp(&doc, 4 * BAR, true).unwrap().doc;
    assert_eq!(
        set_tempo_ramp(&doc, 4 * BAR, true),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        set_tempo_ramp(&doc, 5 * BAR, true),
        Err(EditError::NoSuchPoint)
    );
    let inside = add_tempo_point(&doc, 6 * BAR, tempo(11_200)).unwrap().doc;
    assert_eq!(
        points(&inside),
        [
            (4 * BAR, 10_400, true),
            (6 * BAR, 11_200, true),
            (8 * BAR, 12_000, false)
        ]
    );
    let map = TempoMap::new(&inside.timeline);
    let was = TempoMap::new(&doc.timeline);
    for tick in [4 * BAR, 5 * BAR, 6 * BAR, 7 * BAR, 8 * BAR, 9 * BAR] {
        let (now, then) = (map.seconds_at(tick as f64), was.seconds_at(tick as f64));
        assert!((now - then).abs() < 1e-9, "{tick}: {now} {then}");
    }
    // After the last point there is no line to keep.
    let after = add_tempo_point(&inside, 9 * BAR, tempo(9_000)).unwrap().doc;
    assert_eq!(points(&after)[3], (9 * BAR, 9_000, false));
    let head = set_tempo_ramp(&doc, 0, true).unwrap();
    assert!(head.doc.timeline.head_ramp);
    assert_eq!(head.step, StepKind::SetTempoRamp { tick: 0, on: true });
    let first = add_tempo_point(&head.doc, BAR, tempo(9_800)).unwrap().doc;
    assert_eq!(points(&first)[0], (BAR, 9_800, true));

    // A move keeps the ramp, takes the place of a point it lands on, and passes others.
    let e = move_tempo_point(&inside, 4 * BAR, 6 * BAR, tempo(10_500)).unwrap();
    assert_eq!(
        e.step,
        StepKind::MoveTempoPoint {
            from: 4 * BAR,
            to: 6 * BAR,
            tempo: tempo(10_500)
        }
    );
    assert_eq!(
        points(&e.doc),
        [(6 * BAR, 10_500, true), (8 * BAR, 12_000, false)]
    );
    let passed = move_tempo_point(&inside, 4 * BAR, 10 * BAR, tempo(10_400)).unwrap();
    assert_eq!(
        points(&passed.doc),
        [
            (6 * BAR, 11_200, true),
            (8 * BAR, 12_000, false),
            (10 * BAR, 10_400, true)
        ]
    );
    // In place: only the value. The head: only the value.
    let e = move_tempo_point(&inside, 6 * BAR, 6 * BAR, tempo(11_000)).unwrap();
    assert!(matches!(e.step, StepKind::SetTempoPoint { .. }));
    let e = move_tempo_point(&inside, 0, 6 * BAR, tempo(11_000)).unwrap();
    assert!(matches!(e.step, StepKind::Tempo { .. }));
    assert_eq!(points(&e.doc), points(&inside));
    assert_eq!(
        move_tempo_point(&inside, 6 * BAR, 6 * BAR, tempo(11_200)),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        move_tempo_point(&inside, 6 * BAR, 0, tempo(11_200)),
        Err(EditError::OffTheSong)
    );
    assert_eq!(
        move_tempo_point(&inside, 5 * BAR, 7 * BAR, tempo(11_200)),
        Err(EditError::NoSuchPoint)
    );

    let e = delete_tempo_point(&inside, 6 * BAR).unwrap();
    assert_eq!(e.step, StepKind::DeleteTempoPoint { tick: 6 * BAR });
    assert_eq!(points(&e.doc), points(&doc));
    assert_eq!(delete_tempo_point(&inside, 0), Err(EditError::NoSuchPoint));
    assert_eq!(
        delete_tempo_point(&inside, 5 * BAR),
        Err(EditError::NoSuchPoint)
    );
    // The tracks are shared.
    assert!(Arc::ptr_eq(&doc.tracks[0], &e.doc.tracks[0]));
}

#[test]
fn deleting_the_last_tempo_point_ends_the_ramp_into_it() {
    let doc = Doc::new_default();
    let doc = add_tempo_point(&doc, 4 * BAR, tempo(10_400)).unwrap().doc;
    let doc = add_tempo_point(&doc, 8 * BAR, tempo(12_000)).unwrap().doc;
    let doc = set_tempo_ramp(&doc, 0, true).unwrap().doc;
    let ramping = set_tempo_ramp(&doc, 4 * BAR, true).unwrap().doc;

    // The point before the deleted last one has nothing to ramp to.
    let e = delete_tempo_point(&ramping, 8 * BAR).unwrap();
    assert_eq!(e.step, StepKind::DeleteTempoPoint { tick: 8 * BAR });
    assert_eq!(points(&e.doc), [(4 * BAR, 10_400, false)]);
    assert!(e.doc.timeline.head_ramp);
    // So a point added after it later is not ramped into.
    let later = add_tempo_point(&e.doc, 12 * BAR, tempo(9_000)).unwrap().doc;
    assert_eq!(
        points(&later),
        [(4 * BAR, 10_400, false), (12 * BAR, 9_000, false)]
    );

    // A point that is not the last: the ramp before it goes on to the next point.
    let e = delete_tempo_point(&ramping, 4 * BAR).unwrap();
    assert_eq!(points(&e.doc), [(8 * BAR, 12_000, false)]);
    assert!(e.doc.timeline.head_ramp);

    // The only point: the head is the last.
    let one = delete_tempo_point(&e.doc, 8 * BAR).unwrap().doc;
    assert!(points(&one).is_empty());
    assert!(!one.timeline.head_ramp);

    // One step: undo brings the point and the ramp back.
    let mut history = History::new();
    let mut now = ramping.clone();
    let e = delete_tempo_point(&ramping, 8 * BAR).unwrap();
    history.apply(&mut now, e);
    assert!(!now.timeline.tempo_points[0].ramp);
    history.undo(&mut now);
    assert_eq!(now.timeline, ramping.timeline);
}

#[test]
fn meter_changes_are_added_set_moved_and_deleted() {
    let doc = Doc::new_default();
    let e = add_meter_change(&doc, 8, meter(3, 4)).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddMeterChange {
            bar: 8,
            meter: meter(3, 4)
        }
    );
    let doc = add_meter_change(&e.doc, 4, meter(6, 8)).unwrap().doc;
    assert_eq!(changes(&doc), [(4, 6, 8), (8, 3, 4)]);
    let e = add_meter_change(&doc, 8, meter(5, 4)).unwrap();
    assert_eq!(
        e.step,
        StepKind::SetMeterChange {
            bar: 8,
            from: meter(3, 4),
            to: meter(5, 4)
        }
    );
    let e = add_meter_change(&doc, 0, meter(5, 4)).unwrap();
    assert_eq!(
        e.step,
        StepKind::Meter {
            from: Meter::FOUR_FOUR,
            to: meter(5, 4)
        }
    );
    assert_eq!(changes(&e.doc), changes(&doc));
    assert_eq!(
        set_meter_at(&doc, 8, meter(3, 4)),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        set_meter_at(&doc, 7, meter(3, 4)),
        Err(EditError::NoSuchPoint)
    );

    let e = move_meter_change(&doc, 4, 8).unwrap();
    assert_eq!(e.step, StepKind::MoveMeterChange { from: 4, to: 8 });
    assert_eq!(changes(&e.doc), [(8, 6, 8)]);
    let e = move_meter_change(&doc, 4, 12).unwrap();
    assert_eq!(changes(&e.doc), [(8, 3, 4), (12, 6, 8)]);
    assert_eq!(move_meter_change(&doc, 4, 4), Err(EditError::Unchanged));
    assert_eq!(move_meter_change(&doc, 4, 0), Err(EditError::OffTheSong));
    assert_eq!(move_meter_change(&doc, 0, 3), Err(EditError::NoSuchPoint));
    assert_eq!(move_meter_change(&doc, 5, 6), Err(EditError::NoSuchPoint));

    let e = delete_meter_change(&doc, 4).unwrap();
    assert_eq!(e.step, StepKind::DeleteMeterChange { bar: 4 });
    assert_eq!(changes(&e.doc), [(8, 3, 4)]);
    assert_eq!(delete_meter_change(&doc, 0), Err(EditError::NoSuchPoint));
}

#[test]
fn markers_keep_the_order_of_tick_then_id() {
    let doc = Doc::new_default();
    let (e, a) = add_marker(&doc, 4 * BAR, "  サビ ").unwrap();
    assert_eq!(e.step, StepKind::AddMarker { marker: a });
    assert_eq!(e.doc.next_id, doc.next_id + 1);
    let (e, b) = add_marker(&e.doc, BAR, "A メロ").unwrap();
    let (e, c) = add_marker(&e.doc, BAR, "マーカー 3").unwrap();
    let doc = e.doc;
    assert_eq!(
        markers(&doc),
        [
            (b.0, BAR, "A メロ"),
            (c.0, BAR, "マーカー 3"),
            (a.0, 4 * BAR, "サビ")
        ]
    );
    assert_eq!(add_marker(&doc, 0, "   "), Err(EditError::Unchanged));
    assert_eq!(
        add_marker(&doc, MAX_TICK + 1, "x"),
        Err(EditError::OffTheSong)
    );

    let e = rename_marker(&doc, c, " B メロ　").unwrap();
    assert_eq!(e.step, StepKind::RenameMarker { marker: c });
    assert_eq!(e.doc.marker(c).unwrap().name, "B メロ");
    assert_eq!(e.doc.next_id, doc.next_id);
    assert_eq!(rename_marker(&doc, c, ""), Err(EditError::Unchanged));
    assert_eq!(
        rename_marker(&doc, c, " マーカー 3 "),
        Err(EditError::Unchanged)
    );
    assert_eq!(
        rename_marker(&doc, MarkerId(99), "x"),
        Err(EditError::NoSuchPoint)
    );

    let e = move_marker(&doc, b, 4 * BAR).unwrap();
    assert_eq!(e.step, StepKind::MoveMarker { marker: b });
    assert_eq!(
        markers(&e.doc).iter().map(|m| m.0).collect::<Vec<_>>(),
        [c.0, a.0, b.0]
    );
    assert_eq!(move_marker(&doc, b, BAR), Err(EditError::Unchanged));

    let e = delete_marker(&doc, b).unwrap();
    assert_eq!(e.step, StepKind::DeleteMarker { marker: b });
    assert!(e.doc.marker(b).is_none());
    assert_eq!(e.doc.markers.len(), 2);
    assert_eq!(delete_marker(&e.doc, b), Err(EditError::NoSuchPoint));
}

#[test]
fn the_loop_range_is_a_step_and_the_song_ends_at_a_bar() {
    let doc = Doc::new_default();
    let range = TickRange::new(BAR, 3 * BAR).unwrap();
    let e = set_loop(&doc, range).unwrap();
    assert_eq!(e.step, StepKind::SetLoop { range });
    assert_eq!(e.doc.loop_range, Some(range));
    assert_eq!(set_loop(&e.doc, range), Err(EditError::Unchanged));

    // No clips: 4 bars from the marker, as long as the bars are there.
    assert_eq!(loop_end_from(&doc, 2 * BAR, 4), 6 * BAR);
    assert_eq!(loop_end_from(&doc, 2 * BAR + Q, 4), 6 * BAR + Q);
    let three = add_meter_change(&doc, 3, meter(3, 4)).unwrap().doc;
    assert_eq!(loop_end_from(&three, 2 * BAR, 4), 3 * BAR + 3 * BAR34);
    // The last clip's end, up to the head of a bar of the meter there.
    let (e, _) = edit::add_recording(&three, TrackId(1), vec![take(0, 3 * BAR + 100)], 0).unwrap();
    assert_eq!(loop_end_from(&e.doc, BAR, 4), 3 * BAR + BAR34);
    assert_eq!(loop_end_from(&e.doc, 0, 4), 3 * BAR + BAR34);
    // The marker at the end or after it: 4 bars again.
    assert_eq!(
        loop_end_from(&e.doc, 3 * BAR + BAR34, 4),
        3 * BAR + 5 * BAR34
    );
    let (whole, _) = edit::add_recording(&doc, TrackId(1), vec![take(0, 2 * BAR)], 0).unwrap();
    assert_eq!(loop_end_from(&whole.doc, 0, 4), 2 * BAR);
}

/// What must not move when a meter change comes, goes or moves: every tick of the song.
#[derive(Debug, PartialEq)]
struct Ticks {
    /// Start, end and the starts of the notes.
    clips: Vec<(u64, u64, Vec<u64>)>,
    markers: Vec<u64>,
    tempo_points: Vec<u64>,
    loop_range: Option<TickRange>,
}

fn ticks(doc: &Doc) -> Ticks {
    let clips = (doc.tracks.iter())
        .flat_map(|t| &t.clips)
        .map(|c| {
            let notes = c.active_take().take.notes.iter().map(|n| n.start).collect();
            (c.start, c.end, notes)
        })
        .collect();
    Ticks {
        clips,
        markers: doc.markers.iter().map(|m| m.tick).collect(),
        tempo_points: doc.timeline.tempo_points.iter().map(|p| p.tick).collect(),
        loop_range: doc.loop_range,
    }
}

#[test]
fn meter_changes_before_them_leave_every_tick_and_every_bar_number() {
    let doc = Doc::new_default();
    let (e, _) = edit::add_recording(&doc, TrackId(1), vec![take(8 * BAR + 77, BAR)], 0).unwrap();
    let (e, _) = add_marker(&e.doc, 9 * BAR, "サビ").unwrap();
    let e = add_tempo_point(&e.doc, 10 * BAR + Q, tempo(12_000)).unwrap();
    let e = set_loop(&e.doc, TickRange::new(8 * BAR, 12 * BAR).unwrap()).unwrap();
    let doc = add_meter_change(&e.doc, 20, meter(3, 4)).unwrap().doc;
    let was = ticks(&doc);
    let later_bar = |d: &Doc| d.timeline.meter_changes.last().map(|c| c.bar);

    let added = add_meter_change(&doc, 2, meter(3, 2)).unwrap().doc;
    assert_eq!(ticks(&added), was);
    assert_eq!(later_bar(&added), Some(20));
    // The later flag is still at the head of bar 20, which is another tick now.
    let (old, new) = (TempoMap::new(&doc.timeline), TempoMap::new(&added.timeline));
    assert_eq!(old.tick_of_bar(20), 20 * BAR);
    assert_eq!(new.tick_of_bar(20), 2 * BAR + 18 * (6 * Q));
    assert_eq!(new.meter_at(new.tick_of_bar(20)).meter, meter(3, 4));
    // The tempo point is in the middle of a beat now, and stays there.
    assert_eq!(old.nearest_beat(10 * BAR + Q), 10 * BAR + Q);
    assert_eq!(new.nearest_beat(10 * BAR + Q), 10 * BAR + 2 * Q);

    let moved = move_meter_change(&added, 2, 5).unwrap().doc;
    assert_eq!(ticks(&moved), was);
    assert_eq!(later_bar(&moved), Some(20));
    let set = set_meter_at(&moved, 5, meter(1, 16)).unwrap().doc;
    assert_eq!(ticks(&set), was);
    let head = set_meter(&set, meter(6, 8)).unwrap().doc;
    assert_eq!(ticks(&head), was);
    let deleted = delete_meter_change(&head, 5).unwrap().doc;
    assert_eq!(ticks(&deleted), was);
    assert_eq!(later_bar(&deleted), Some(20));
}

/// xorshift64*, so the test needs nothing outside std.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const KINDS: usize = 14;

fn random_edit(rng: &mut Rng, doc: &Doc) -> Option<(usize, Edit)> {
    let t = &doc.timeline;
    let tick = rng.below(40) * (Q / 2);
    let bar = rng.below(24) as u32;
    let some_tempo = tempo(3_000 + rng.below(27_001) as u32);
    let some_meter = meter(1 + rng.below(12) as u8, Meter::DENS[rng.below(4) as usize]);
    let point = |rng: &mut Rng| match t.tempo_points.len() as u64 {
        0 => 0,
        // Now and then the head, or a place with no point.
        n => match rng.below(n + 2) {
            i if i < n => t.tempo_points[i as usize].tick,
            i => (i - n) * 7,
        },
    };
    let change = |rng: &mut Rng| match t.meter_changes.len() as u64 {
        0 => 0,
        n => match rng.below(n + 2) {
            i if i < n => t.meter_changes[i as usize].bar,
            i => (i - n) as u32 * 31,
        },
    };
    let marker = |rng: &mut Rng| match doc.markers.len() as u64 {
        0 => MarkerId(0),
        n => doc.markers[rng.below(n) as usize].id,
    };
    let kind = rng.below(KINDS as u64) as usize;
    let edit = match kind {
        0 => add_tempo_point(doc, tick, some_tempo),
        1 => move_tempo_point(doc, point(rng), tick, some_tempo),
        2 => set_tempo_at(doc, point(rng), some_tempo),
        3 => set_tempo_ramp(doc, point(rng), rng.below(2) == 0),
        4 => delete_tempo_point(doc, point(rng)),
        5 => add_meter_change(doc, bar, some_meter),
        6 => move_meter_change(doc, change(rng), bar),
        7 => set_meter_at(doc, change(rng), some_meter),
        8 => delete_meter_change(doc, change(rng)),
        9 => add_marker(doc, tick, &format!("マーカー {}", doc.markers.len() + 1)).map(|(e, _)| e),
        10 => rename_marker(doc, marker(rng), &format!("名前 {}", rng.below(5))),
        11 => move_marker(doc, marker(rng), tick),
        12 => delete_marker(doc, marker(rng)),
        _ => set_loop(doc, TickRange::new(tick, tick + 1 + rng.below(8) * Q)?),
    };
    edit.ok().map(|e| (kind, e))
}

fn well_formed(doc: &Doc, ctx: &str) {
    assert_eq!(doc.timeline.check(), Ok(()), "{ctx}");
    assert!(
        (doc.timeline.tempo_points.iter()).all(|p| p.tick <= MAX_TICK),
        "{ctx}"
    );
    assert!(
        (doc.markers.windows(2)).all(|w| (w[0].tick, w[0].id) < (w[1].tick, w[1].id)),
        "{ctx}: markers out of order"
    );
    assert!(doc.markers.iter().all(|m| !m.name.is_empty()), "{ctx}");
}

#[test]
fn random_walks_keep_the_timeline_well_formed_and_come_back() {
    let mut applied = [0usize; KINDS];
    for seed in 1..=30u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut h = History::new();
        let mut doc = Doc::new_default();
        let (e, _) = edit::add_recording(&doc, TrackId(1), vec![take(0, BAR)], 0).unwrap();
        doc = e.doc;
        let mut book = IdBook::new(&doc);
        let mut line = vec![doc.clone()];
        let mut at = 0usize;
        for round in 0..600 {
            let ctx = format!("seed {seed}, round {round}");
            match rng.below(10) {
                0..=4 => {
                    if let Some((kind, e)) = random_edit(&mut rng, &doc) {
                        applied[kind] += 1;
                        let before = doc.clone();
                        h.apply(&mut doc, e);
                        book.saw(&doc, Some(&before))
                            .unwrap_or_else(|e| panic!("{ctx}: {e}"));
                        // The clip, its take and its notes are the very same.
                        assert!(Arc::ptr_eq(&doc.tracks[0], &before.tracks[0]), "{ctx}");
                        line.truncate(at + 1);
                        line.push(doc.clone());
                        at += 1;
                    }
                }
                5..=7 => {
                    assert_eq!(h.undo(&mut doc).is_some(), at > 0, "{ctx}");
                    at = at.saturating_sub(1);
                }
                _ => {
                    assert_eq!(h.redo(&mut doc).is_some(), at + 1 < line.len(), "{ctx}");
                    at = (at + 1).min(line.len() - 1);
                }
            }
            well_formed(&doc, &ctx);
            book.saw(&doc, None)
                .unwrap_or_else(|e| panic!("{ctx}: {e}"));
            assert_eq!(held(&doc), held(&line[at]), "{ctx}");
        }
        while h.undo(&mut doc).is_some() {}
        assert_eq!(held(&doc), held(&line[0]));
        while h.redo(&mut doc).is_some() {}
        assert_eq!(held(&doc), held(line.last().unwrap()));
    }
    assert!(applied.iter().all(|&n| n > 50), "{applied:?}");
}

#[test]
fn a_clip_written_or_grown_after_a_meter_change_takes_the_bars_there() {
    use crate::notes::{WriteAt, write_note};
    // Two bars of 4/4, then 3/4: bars start at 2 BAR, 2 BAR + 3 Q, 2 BAR + 6 Q, …
    let base = add_meter_change(&Doc::new_default(), 2, meter(3, 4))
        .unwrap()
        .doc;
    let note = |start| Note {
        from: None,
        start,
        length: 100,
        key: 60,
        velocity: 100,
    };
    let range = |doc: &Doc, i: usize| {
        let c = &doc.tracks[0].clips[i];
        (c.start, c.end)
    };
    // No clip there: from the head of the note's bar, 4 bars of the meter there.
    let doc = write_note(
        &base,
        WriteAt::Track(TrackId(1)),
        note(2 * BAR + BAR34 + 60),
    )
    .unwrap()
    .edit
    .doc;
    assert_eq!(range(&doc, 0), (2 * BAR + BAR34, 2 * BAR + 5 * BAR34));
    // Before the change the bars are 4/4, and the 4 bars cross it.
    let other = write_note(&base, WriteAt::Track(TrackId(1)), note(BAR + 5))
        .unwrap()
        .edit
        .doc;
    assert_eq!(range(&other, 0), (BAR, 2 * BAR + 3 * BAR34));
    // A note written before the clip grows it back to the head of the bar there.
    let clip = doc.tracks[0].clips[0].id;
    let grown = write_note(&doc, WriteAt::Clip(clip), note(2 * BAR + BAR34 - 100))
        .unwrap()
        .edit
        .doc;
    assert_eq!(range(&grown, 0), (2 * BAR, 2 * BAR + 5 * BAR34));
    let grown = write_note(&doc, WriteAt::Clip(clip), note(2 * BAR + 5 * BAR34 + 10))
        .unwrap()
        .edit
        .doc;
    assert_eq!(range(&grown, 0), (2 * BAR + BAR34, 2 * BAR + 6 * BAR34));
}
