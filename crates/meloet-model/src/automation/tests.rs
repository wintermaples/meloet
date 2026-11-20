use std::collections::HashMap;

use super::*;
use crate::effects::{EffectKind, compressor};
use crate::history::History;
use crate::id_book::held;
use crate::instrument::PluginRef;
use crate::mixer::{self, Missing, StripId};
use crate::{BuiltinInstrument, edit};

const T1: TrackId = TrackId(1);
const S1: StripId = StripId::Track(T1);

fn p(tick: u64, value: f32) -> AutoPoint {
    AutoPoint { tick, value }
}

fn lane_with(points: Vec<AutoPoint>, step: bool) -> Lane {
    Lane {
        id: LaneId(1),
        target: AutoTarget::Volume,
        step,
        points: points.into(),
        resting: false,
    }
}

fn plugin(key: &str) -> Arc<PluginRef> {
    Arc::new(PluginRef::new(key, key, "", ""))
}

/// A new song whose track 1 has a compressor in slot 0 and a plugin in slot 1.
fn song_with_inserts() -> Doc {
    let doc = Doc::new_default();
    let doc = mixer::add_effect(&doc, S1, 0, EffectKind::Compressor)
        .unwrap()
        .doc;
    mixer::add_plugin_effect(&doc, S1, 1, plugin("vst3:fx"))
        .unwrap()
        .doc
}

fn insert_id(doc: &Doc, slot: usize) -> InsertId {
    doc.insert(S1, slot).unwrap().id
}

fn lane(doc: &Doc, target: AutoTarget) -> &Lane {
    doc.track(T1).unwrap().lane(target).unwrap()
}

fn ticks_values(l: &Lane) -> Vec<(u64, f32)> {
    l.points.iter().map(|p| (p.tick, p.value)).collect()
}

#[test]
fn the_line_between_points() {
    let l = lane_with(vec![p(100, 0.2), p(200, 0.6), p(300, 0.6)], false);
    assert_eq!(l.value_at(0), Some(0.2), "before the first: the first");
    assert_eq!(l.value_at(100), Some(0.2));
    assert!((l.value_at(150).unwrap() - 0.4).abs() < 1e-6);
    assert_eq!(l.value_at(200), Some(0.6));
    assert_eq!(l.value_at(1000), Some(0.6), "after the last: the last");
    let s = lane_with(l.points.to_vec(), true);
    assert_eq!(
        s.value_at(199),
        Some(0.2),
        "a step stays at the point before"
    );
    assert_eq!(s.value_at(200), Some(0.6));
    assert_eq!(lane_with(Vec::new(), false).value_at(5), None);
    // A jump: two points a tick apart.
    let j = lane_with(vec![p(0, 0.0), p(99, 0.0), p(100, 1.0)], false);
    assert_eq!(j.value_at(99), Some(0.0));
    assert_eq!(j.value_at(100), Some(1.0));
}

#[test]
fn a_lane_needs_its_target() {
    let doc = song_with_inserts();
    let comp = insert_id(&doc, 0);
    let fx = insert_id(&doc, 1);
    let (e, id) = add_lane(&doc, T1, AutoTarget::Volume, false).unwrap();
    assert!(id.0 >= doc.next_id && e.doc.next_id > id.0);
    assert_eq!(
        e.step,
        StepKind::AddLane {
            track: T1,
            target: AutoTarget::Volume
        }
    );
    assert_eq!(
        add_lane(&e.doc, T1, AutoTarget::Volume, false).unwrap_err(),
        EditError::Unchanged
    );
    let comp_param = AutoTarget::Insert {
        insert: comp,
        param: compressor::THRESHOLD,
    };
    assert!(add_lane(&doc, T1, comp_param, false).is_ok());
    let no_param = AutoTarget::Insert {
        insert: comp,
        param: 99,
    };
    assert_eq!(
        add_lane(&doc, T1, no_param, false).unwrap_err(),
        EditError::NoSuchInsert
    );
    let any_plugin_param = AutoTarget::Insert {
        insert: fx,
        param: 12345,
    };
    assert!(add_lane(&doc, T1, any_plugin_param, false).is_ok());
    let gone = AutoTarget::Insert {
        insert: InsertId(9999),
        param: 0,
    };
    assert_eq!(
        add_lane(&doc, T1, gone, false).unwrap_err(),
        EditError::NoSuchInsert
    );
    let instrument = AutoTarget::Instrument { param: 3 };
    assert_eq!(
        add_lane(&doc, T1, instrument, false).unwrap_err(),
        EditError::NoSuchInsert,
        "the piano is no plugin"
    );
    assert!(add_lane(&doc, TrackId(7), AutoTarget::Pan, false).is_err());
}

fn with_volume(points: Vec<AutoPoint>) -> Doc {
    let (e, _) = add_lane(&Doc::new_default(), T1, AutoTarget::Volume, false).unwrap();
    let mut doc = e.doc;
    let t = Arc::make_mut(&mut doc.tracks[0]);
    Arc::make_mut(&mut t.automation[0]).points = points.into();
    doc
}

#[test]
fn points_are_added_on_the_line() {
    let doc = with_volume(vec![p(0, 0.0), p(1000, 1.0)]);
    let id = lane(&doc, AutoTarget::Volume).id;
    let (e, new_id, at) = add_point(&doc, id, 250, None, 0.8).unwrap();
    assert_ne!(new_id, id, "the points changed: a new id");
    assert_eq!(at, 1);
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [(0, 0.0), (250, 0.25), (1000, 1.0)]
    );
    // On a point: it takes the value.
    let (e2, _, at) = add_point(&e.doc, new_id, 250, Some(0.9), 0.8).unwrap();
    assert_eq!(at, 1);
    assert_eq!(lane(&e2.doc, AutoTarget::Volume).points[1].value, 0.9);
    assert_eq!(
        add_point(&e2.doc, new_id, 0, None, 0.8).unwrap_err(),
        EditError::NoSuchPoint,
        "an old id does nothing"
    );
    // An empty lane: the hand's value.
    let empty = with_volume(Vec::new());
    let id = lane(&empty, AutoTarget::Volume).id;
    let (e, _, _) = add_point(&empty, id, 480, None, 0.8).unwrap();
    assert_eq!(ticks_values(lane(&e.doc, AutoTarget::Volume)), [(480, 0.8)]);
    let (e, _, _) = add_point(&empty, id, 480, Some(7.0), 0.8).unwrap();
    assert_eq!(lane(&e.doc, AutoTarget::Volume).points[0].value, 1.0);
}

#[test]
fn a_drag_of_points_stops_together() {
    let doc = with_volume(vec![p(100, 0.2), p(200, 0.9), p(300, 0.5), p(400, 0.1)]);
    let id = lane(&doc, AutoTarget::Volume).id;
    // Left past the start and up past 1: the group stops at tick 0 and 1.0.
    let (e, _, at) = move_points(&doc, id, &[1, 0], -500, 0.5).unwrap();
    assert_eq!(at, [0, 1]);
    let got = ticks_values(lane(&e.doc, AutoTarget::Volume));
    assert_eq!(got[0].0, 0);
    assert_eq!(got[1].0, 100);
    assert!((got[0].1 - 0.3).abs() < 1e-6 && (got[1].1 - 1.0).abs() < 1e-6);
    // Onto another point: that one is gone, and the moved one can pass it.
    let (e, _, at) = move_points(&doc, id, &[0], 200, 0.0).unwrap();
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [(200, 0.9), (300, 0.2), (400, 0.1)]
    );
    assert_eq!(at, [1]);
    assert_eq!(
        e.step,
        StepKind::MoveAutoPoints {
            track: T1,
            target: AutoTarget::Volume,
            count: 1
        }
    );
    assert_eq!(
        move_points(&doc, id, &[0], 0, 0.0).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        move_points(&doc, id, &[9], 5, 0.0).unwrap_err(),
        EditError::NoSuchPoint
    );
    let (e, _) = delete_points(&doc, id, &[3, 1]).unwrap();
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [(100, 0.2), (300, 0.5)]
    );
}

#[test]
fn writing_puts_the_line_back_around_what_was_written() {
    let doc = with_volume(vec![p(0, 0.5), p(2000, 0.5)]);
    let pass = vec![p(1000, 0.1), p(1100, 0.2), p(1200, 0.3), p(1300, 0.9)];
    let (e, _) = write(&doc, T1, AutoTarget::Volume, &[pass], 0.8).unwrap();
    assert_eq!(
        e.step,
        StepKind::WriteAutomation {
            track: T1,
            target: AutoTarget::Volume
        }
    );
    // The straight run from 0.1 to 0.3 is thinned to its ends.
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [
            (0, 0.5),
            (999, 0.5),
            (1000, 0.1),
            (1200, 0.3),
            (1300, 0.9),
            (1301, 0.5),
            (2000, 0.5)
        ]
    );
    // An empty lane goes back to where the hand put the value.
    let empty = with_volume(Vec::new());
    let (e, _) = write(&empty, T1, AutoTarget::Volume, &[vec![p(10, 0.2)]], 0.8).unwrap();
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [(9, 0.8), (10, 0.2), (11, 0.8)]
    );
}

#[test]
fn a_later_pass_of_a_loop_wins() {
    let doc = with_volume(Vec::new());
    let first = vec![p(100, 0.1), p(200, 0.1), p(300, 0.1)];
    let second = vec![p(150, 0.6), p(250, 0.6)];
    let (e, _) = write(&doc, T1, AutoTarget::Volume, &[first, second], 0.8).unwrap();
    assert_eq!(
        ticks_values(lane(&e.doc, AutoTarget::Volume)),
        [
            (99, 0.8),
            (100, 0.1),
            (149, 0.1),
            (150, 0.6),
            (250, 0.6),
            (251, 0.1),
            (300, 0.1),
            (301, 0.8)
        ]
    );
}

#[test]
fn thinning_stays_within_the_tolerance() {
    let mut rng = Rng(7);
    let mut points = Vec::new();
    let mut v = 0.5f32;
    for i in 0..2000u64 {
        v = (v + (rng.below(200) as f32 - 100.0) / 50000.0).clamp(0.0, 1.0);
        points.push(p(i * 7, v));
    }
    let kept = thin(&points, false, THIN_TOLERANCE);
    assert!(kept.len() < points.len() / 2, "{} kept", kept.len());
    assert_eq!(kept.first(), points.first());
    assert_eq!(kept.last(), points.last());
    for q in &points {
        let line = line_at(&kept, false, q.tick).unwrap();
        assert!((line - q.value).abs() <= THIN_TOLERANCE + 1e-6, "{q:?}");
    }
    let steps = [p(0, 0.0), p(5, 0.0), p(6, 1.0), p(9, 1.0), p(12, 0.0)];
    assert_eq!(
        thin(&steps, true, THIN_TOLERANCE),
        [p(0, 0.0), p(6, 1.0), p(12, 0.0)]
    );
}

#[test]
fn a_press_writes_one_point() {
    let (e, _) = add_lane(&Doc::new_default(), T1, AutoTarget::Mute, true).unwrap();
    let (e, _) = write_press(&e.doc, T1, AutoTarget::Mute, 960, 1.0).unwrap();
    assert_eq!(ticks_values(lane(&e.doc, AutoTarget::Mute)), [(960, 1.0)]);
    assert_eq!(
        write_press(&e.doc, T1, AutoTarget::Mute, 960, 1.0).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn resting_keeps_the_points_and_their_id() {
    let doc = with_volume(vec![p(0, 0.5)]);
    let id = lane(&doc, AutoTarget::Volume).id;
    let e = rest_lane(&doc, T1, AutoTarget::Volume).unwrap();
    assert!(lane(&e.doc, AutoTarget::Volume).resting);
    assert_eq!(lane(&e.doc, AutoTarget::Volume).id, id);
    assert_eq!(
        rest_lane(&e.doc, T1, AutoTarget::Volume).unwrap_err(),
        EditError::Unchanged
    );
    let back = restore_lane(&e.doc, T1, AutoTarget::Volume).unwrap();
    assert!(!lane(&back.doc, AutoTarget::Volume).resting);
    assert_eq!(
        back.step,
        StepKind::RestoreLane {
            track: T1,
            target: AutoTarget::Volume
        }
    );
}

#[test]
fn lanes_go_with_what_they_move() {
    let doc = song_with_inserts();
    let (comp, fx) = (insert_id(&doc, 0), insert_id(&doc, 1));
    let comp_lane = AutoTarget::Insert {
        insert: comp,
        param: compressor::THRESHOLD,
    };
    let fx_lane = AutoTarget::Insert {
        insert: fx,
        param: 1,
    };
    let mut d = doc;
    for target in [AutoTarget::Volume, comp_lane, fx_lane] {
        d = add_lane(&d, T1, target, false).unwrap().0.doc;
    }
    // Moving the slots keeps every lane.
    let moved = mixer::move_insert(&d, S1, 0, 3).unwrap().doc;
    assert_eq!(moved.tracks[0].automation.len(), 3);
    let removed = mixer::remove_effect(&d, S1, 0).unwrap().doc;
    let targets: Vec<_> = removed.tracks[0]
        .automation
        .iter()
        .map(|l| l.target)
        .collect();
    assert_eq!(targets, [AutoTarget::Volume, fx_lane]);
    let replaced = mixer::replace_effect(&d, S1, 0, EffectKind::Compressor);
    assert_eq!(
        replaced.unwrap_err(),
        EditError::Unchanged,
        "the same kind is no change"
    );
    let replaced = mixer::replace_effect(&d, S1, 0, EffectKind::Eq4)
        .unwrap()
        .doc;
    assert_eq!(replaced.tracks[0].automation.len(), 2);
    let missing = mixer::remove_missing(&d, Missing::Plugin("vst3:fx"))
        .unwrap()
        .doc;
    let targets: Vec<_> = missing.tracks[0]
        .automation
        .iter()
        .map(|l| l.target)
        .collect();
    assert_eq!(targets, [AutoTarget::Volume, comp_lane]);
    for doc in [&moved, &removed, &replaced, &missing] {
        doc.check_lanes().unwrap();
    }
}

#[test]
fn the_instrument_takes_its_lanes_along() {
    let synth = InstrumentRef::Plugin(plugin("vst3:synth"));
    let doc = edit::set_instrument(&Doc::new_default(), T1, synth.clone())
        .unwrap()
        .doc;
    let target = AutoTarget::Instrument { param: 7 };
    let doc = add_lane(&doc, T1, target, false).unwrap().0.doc;
    // The same plugin with another state keeps them.
    let again = InstrumentRef::Plugin(Arc::new(
        plugin("vst3:synth").with_state(Arc::from(&[1u8][..])),
    ));
    let same = edit::set_instrument(&doc, T1, again).unwrap().doc;
    assert_eq!(same.tracks[0].automation.len(), 1);
    let piano = InstrumentRef::Builtin(BuiltinInstrument::GrandPiano);
    let other = edit::set_instrument(&doc, T1, piano).unwrap().doc;
    assert!(other.tracks[0].automation.is_empty());
    let e = edit::remove_instrument(&doc, T1).unwrap();
    assert_eq!(e.step, StepKind::RemoveInstrument { track: T1 });
    assert_eq!(e.doc.tracks[0].instrument, InstrumentRef::SINE);
    assert!(e.doc.tracks[0].automation.is_empty());
    assert_eq!(
        edit::remove_instrument(&e.doc, T1).unwrap_err(),
        EditError::Unchanged
    );
}

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

fn some_target(rng: &mut Rng, doc: &Doc) -> AutoTarget {
    let t = &doc.tracks[0];
    let inserts: Vec<_> = t.channel.inserts.iter().flatten().collect();
    match rng.below(6) {
        0 => AutoTarget::Volume,
        1 => AutoTarget::Pan,
        2 => AutoTarget::Send(Bus::B),
        3 => AutoTarget::Mute,
        4 => AutoTarget::Instrument {
            param: rng.below(3) as u32,
        },
        _ => match inserts.get(rng.below(4) as usize) {
            Some(i) => AutoTarget::Insert {
                insert: i.id,
                param: rng.below(3) as u32,
            },
            None => AutoTarget::Pan,
        },
    }
}

/// Lanes, points, writes, rests, and the slots and instrument under them, done and undone at
/// random: every lane has its target, and no lane id comes back with other points.
#[test]
fn random_steps_keep_every_lane_with_its_target() {
    let mut rng = Rng(0x5eed_1a7e);
    let mut doc = Doc::new_default();
    let mut history = History::new();
    let mut seen: HashMap<LaneId, crate::ChunkedVec<AutoPoint>> = HashMap::new();
    let kinds = [EffectKind::Compressor, EffectKind::Eq4, EffectKind::Delay];
    for round in 0..3000 {
        let target = some_target(&mut rng, &doc);
        let lanes: Vec<Arc<Lane>> = doc.tracks[0].automation.clone();
        let some_lane = lanes.get(rng.below(lanes.len() as u64 + 1) as usize);
        let tick = rng.below(20) * 120;
        let value = rng.below(101) as f32 / 100.0;
        let slot = rng.below(4) as usize;
        let edit = match rng.below(16) {
            0 | 1 => add_lane(&doc, T1, target, rng.below(2) == 0).map(|(e, _)| e),
            2 => delete_lane(&doc, T1, target),
            3 | 4 => match some_lane {
                Some(l) => add_point(&doc, l.id, tick, Some(value), 0.8).map(|(e, ..)| e),
                None => continue,
            },
            5 => match some_lane {
                Some(l) => {
                    let picked = [rng.below(4) as usize, rng.below(4) as usize];
                    let d_tick = rng.below(9) as i64 * 120 - 480;
                    move_points(&doc, l.id, &picked, d_tick, value - 0.5).map(|(e, ..)| e)
                }
                None => continue,
            },
            6 => match some_lane {
                Some(l) => delete_points(&doc, l.id, &[rng.below(4) as usize]).map(|(e, _)| e),
                None => continue,
            },
            7 => match some_lane {
                Some(l) => {
                    let pass = (0..rng.below(30))
                        .map(|i| p(tick + i * 10, (value + i as f32 / 50.0).min(1.0)))
                        .collect();
                    write(&doc, T1, l.target, &[pass], 0.8).map(|(e, _)| e)
                }
                None => continue,
            },
            8 => rest_lane(&doc, T1, target),
            9 => restore_lane(&doc, T1, target),
            10 => mixer::add_effect(&doc, S1, slot, kinds[rng.below(3) as usize]),
            11 => mixer::add_plugin_effect(&doc, S1, slot, plugin("vst3:fx")),
            12 => mixer::remove_effect(&doc, S1, slot),
            13 => mixer::replace_effect(&doc, S1, slot, kinds[rng.below(3) as usize]),
            14 => mixer::move_insert(&doc, S1, slot, rng.below(4) as usize),
            _ => match rng.below(3) {
                0 => edit::set_instrument(&doc, T1, InstrumentRef::Plugin(plugin("vst3:s"))),
                1 => edit::remove_instrument(&doc, T1),
                _ => edit::set_instrument(&doc, T1, InstrumentRef::SINE),
            },
        };
        match edit {
            Ok(e) => history.apply(&mut doc, e),
            Err(_) => match rng.below(4) {
                0 => drop(history.undo(&mut doc)),
                1 => drop(history.redo(&mut doc)),
                _ => continue,
            },
        }
        doc.check_lanes()
            .unwrap_or_else(|e| panic!("round {round}: {e}"));
        for l in &doc.tracks[0].automation {
            let had = seen.entry(l.id).or_insert_with(|| l.points.clone());
            assert_eq!(
                *had, l.points,
                "round {round}: lane id {} came back changed",
                l.id.0
            );
        }
    }
    // Undoing everything brings back the song as it started.
    while history.undo(&mut doc).is_some() {}
    assert_eq!(held(&doc), held(&Doc::new_default()));
}
