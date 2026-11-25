use std::collections::HashMap;

use super::*;
use crate::effects::{EffectKind, compressor};
use crate::history::History;
use crate::id_book::held;
use crate::instrument::PluginRef;
use crate::mixer::{self, Missing, StripId};
use crate::{BuiltinInstrument, TICKS_PER_QUARTER, edit};

const T1: TrackId = TrackId(1);
const S1: StripId = StripId::Track(T1);
const Q: u64 = TICKS_PER_QUARTER;

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
            home: T1.into(),
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
    let knob = AutoTarget::Instrument { param: 3 };
    assert!(
        add_lane(&doc, T1, knob, false).is_ok(),
        "the piano's 「ゆれ」"
    );
    let instrument = AutoTarget::Instrument { param: 4 };
    assert_eq!(
        add_lane(&doc, T1, instrument, false).unwrap_err(),
        EditError::NoSuchInsert,
        "the piano has four knobs, and is no plugin"
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
    let (e, _, _) = add_point(&empty, id, Q, None, 0.8).unwrap();
    assert_eq!(ticks_values(lane(&e.doc, AutoTarget::Volume)), [(Q, 0.8)]);
    let (e, _, _) = add_point(&empty, id, Q, Some(7.0), 0.8).unwrap();
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
            home: T1.into(),
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
            home: T1.into(),
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
    let (e, _) = write_press(&e.doc, T1, AutoTarget::Mute, 2 * Q, 1.0).unwrap();
    assert_eq!(ticks_values(lane(&e.doc, AutoTarget::Mute)), [(2 * Q, 1.0)]);
    assert_eq!(
        write_press(&e.doc, T1, AutoTarget::Mute, 2 * Q, 1.0).unwrap_err(),
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
            home: T1.into(),
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
fn a_missing_instrument_takes_its_lanes_and_the_knobs_along() {
    let synth = InstrumentRef::Plugin(plugin("lv2:synth"));
    let mut doc = edit::set_instrument(&Doc::new_default(), T1, synth)
        .unwrap()
        .doc;
    // A parameter id that names a knob of the sine wave too, and one that does not.
    for target in [
        AutoTarget::Instrument { param: 2 },
        AutoTarget::Instrument { param: 9 },
        AutoTarget::Volume,
    ] {
        doc = add_lane(&doc, T1, target, false).unwrap().0.doc;
    }
    let knobs = crate::SoundKnobs::default().with(crate::SoundKnob::Brightness, 10.0);
    Arc::make_mut(&mut doc.tracks[0]).knobs = knobs.unwrap();
    let e = mixer::remove_missing(&doc, Missing::Plugin("lv2:synth")).unwrap();
    let t = &e.doc.tracks[0];
    assert_eq!(t.instrument, InstrumentRef::SINE);
    let targets: Vec<_> = t.automation.iter().map(|l| l.target).collect();
    assert_eq!(targets, [AutoTarget::Volume]);
    assert!(t.knobs.is_default());
    e.doc.check_lanes().unwrap();
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

#[test]
fn a_vocal_track_has_no_lanes_of_an_instrument() {
    let (e, vocal) = edit::add_vocal_track(&Doc::new_default(), None, crate::Language::Ja).unwrap();
    let doc = e.doc;
    for param in [0, 3, 9] {
        let target = AutoTarget::Instrument { param };
        assert!(!doc.track(vocal).unwrap().has_target(target));
        assert_eq!(
            add_lane(&doc, vocal, target, false).unwrap_err(),
            EditError::NoSuchInsert
        );
    }
    let (e, _) = add_lane(&doc, vocal, AutoTarget::Volume, false).unwrap();
    let mut doc = e.doc;
    let lane = Lane {
        id: LaneId(doc.next_id),
        target: AutoTarget::Instrument { param: 0 },
        ..lane_with(vec![p(0, 0.5)], false)
    };
    doc.next_id += 1;
    let ti = doc.tracks.iter().position(|t| t.id == vocal).unwrap();
    Arc::make_mut(&mut doc.tracks[ti])
        .automation
        .push(Arc::new(lane));
    assert!(doc.check_lanes().unwrap_err().contains("has no target"));
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
        let tick = rng.below(20) * (Q / 4);
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
                    let d_tick = rng.below(9) as i64 * (Q / 4) as i64 - Q as i64;
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

// ---- the master's lane and the fade-out (round 12, M5) ----

const BAR: u64 = 4 * Q;
const M: LaneHome = LaneHome::Master;

fn master_lane(doc: &Doc) -> &Lane {
    doc.lane_of(M, AutoTarget::Volume).unwrap()
}

/// A new song with a clip on track 1 over `[start, end)`.
fn song_with_clip(start: u64, end: u64) -> Doc {
    with_clip(&Doc::new_default(), start, end)
}

/// `doc` with one more clip on track 1 over `[start, end)`.
fn with_clip(doc: &Doc, start: u64, end: u64) -> Doc {
    let take = crate::NewTake {
        origin: start,
        length: end - start,
        notes: Vec::new(),
        controls: Vec::new(),
    };
    edit::add_recording(doc, T1, vec![take], 0).unwrap().0.doc
}

#[test]
fn the_master_has_a_lane_of_its_volume_only() {
    let doc = Doc::new_default();
    for target in [AutoTarget::Pan, AutoTarget::Mute, AutoTarget::Send(Bus::A)] {
        assert_eq!(
            add_lane(&doc, M, target, false).map(|_| ()),
            Err(EditError::NoSuchInsert)
        );
    }
    let (e, id) = add_lane(&doc, M, AutoTarget::Volume, false).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddLane {
            home: M,
            target: AutoTarget::Volume
        }
    );
    assert_eq!(e.doc.lane(id).map(|(home, _)| home), Some(M));
    assert!(e.doc.tracks[0].automation.is_empty());
    e.doc.check_lanes().unwrap();
    assert_eq!(
        add_lane(&e.doc, M, AutoTarget::Volume, false).map(|_| ()),
        Err(EditError::Unchanged)
    );
}

#[test]
fn every_step_of_a_lane_works_on_the_masters() {
    let mut doc = Doc::new_default();
    let mut history = History::new();
    let start = held(&doc);
    let (e, id) = add_lane(&doc, M, AutoTarget::Volume, false).unwrap();
    history.apply(&mut doc, e);
    let (e, id, at) = add_point(&doc, id, Q, Some(0.5), 0.8).unwrap();
    assert_eq!(at, 0);
    history.apply(&mut doc, e);
    let (e, id, _) = add_point(&doc, id, 2 * Q, None, 0.8).unwrap();
    history.apply(&mut doc, e);
    let (e, id, places) = move_points(&doc, id, &[1], Q as i64, 0.25).unwrap();
    assert_eq!(
        e.step,
        StepKind::MoveAutoPoints {
            home: M,
            target: AutoTarget::Volume,
            count: 1
        }
    );
    assert_eq!(places, [1]);
    history.apply(&mut doc, e);
    assert_eq!(ticks_values(master_lane(&doc)), [(Q, 0.5), (3 * Q, 0.75)]);
    let (e, _) = write(
        &doc,
        M,
        AutoTarget::Volume,
        &[vec![p(5 * Q, 0.1), p(6 * Q, 0.2)]],
        0.8,
    )
    .unwrap();
    history.apply(&mut doc, e);
    assert_eq!(master_lane(&doc).points.len(), 6);
    let e = rest_lane(&doc, M, AutoTarget::Volume).unwrap();
    history.apply(&mut doc, e);
    assert!(master_lane(&doc).resting);
    let e = restore_lane(&doc, M, AutoTarget::Volume).unwrap();
    history.apply(&mut doc, e);
    assert!(!master_lane(&doc).resting);
    let (e, gone) = delete_points(&doc, master_lane(&doc).id, &[0]).unwrap();
    history.apply(&mut doc, e);
    assert_eq!(master_lane(&doc).id, gone);
    assert_ne!(gone, id);
    doc.check_lanes().unwrap();
    // The points as they were are not named any more.
    assert_eq!(
        add_point(&doc, id, Q, None, 0.8).map(|_| ()),
        Err(EditError::NoSuchPoint)
    );
    let e = delete_lane(&doc, M, AutoTarget::Volume).unwrap();
    history.apply(&mut doc, e);
    assert!(doc.master_lanes.is_empty());
    while history.undo(&mut doc).is_some() {}
    assert_eq!(held(&doc), start);
}

#[test]
fn the_lanes_of_the_master_are_checked_as_a_tracks_are() {
    let (e, id) = add_lane(&Doc::new_default(), M, AutoTarget::Volume, false).unwrap();
    let doc = e.doc;
    let with = |change: &dyn Fn(&mut Lane)| {
        let mut doc = doc.clone();
        let mut l = Lane::clone(&doc.master_lanes[0]);
        change(&mut l);
        doc.master_lanes[0] = Arc::new(l);
        doc
    };
    with(&|_| ()).check_lanes().unwrap();
    assert!(with(&|l| l.target = AutoTarget::Pan).check_lanes().is_err());
    assert!(with(&|l| l.id = LaneId(u32::MAX)).check_lanes().is_err());
    let backwards = with(&|l| l.points = vec![p(Q, 0.5), p(Q, 0.6)].into());
    assert!(backwards.check_lanes().is_err());
    let high = with(&|l| l.points = vec![p(Q, 1.5)].into());
    assert!(high.check_lanes().is_err());
    let mut twice = doc.clone();
    twice.master_lanes.push(doc.master_lanes[0].clone());
    assert!(twice.check_lanes().is_err());
    // An id a track's lane has too.
    let (e, track_lane) = add_lane(&doc, T1, AutoTarget::Volume, false).unwrap();
    assert_ne!(track_lane, id);
    let mut same = e.doc;
    Arc::make_mut(&mut Arc::make_mut(&mut same.tracks[0]).automation[0]).id = id;
    assert!(same.check_lanes().is_err());
}

#[test]
fn a_fade_out_ends_at_the_songs_end_and_starts_bars_before() {
    // The last clip ends on the head of a bar: the fade starts on one too.
    let doc = song_with_clip(BAR, 7 * BAR);
    assert_eq!(crate::sections::last_clip_end(&doc), 7 * BAR);
    let e = add_fade_out(&doc, 4).unwrap();
    let fader = doc.master.gain.position();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(3 * BAR, fader), (7 * BAR, 0.0)]
    );
    // The last clip ends inside bar 7 (from 1): the fade ends there, where 「曲ぜんぶ」
    // ends, and not at the head of bar 8; it is as long as the four bars before bar 7.
    let doc = song_with_clip(BAR, 6 * BAR + Q);
    assert_eq!(crate::sections::last_clip_end(&doc), 6 * BAR + Q);
    let map = TempoMap::new(&doc.timeline);
    assert_eq!(crate::sections::song_end(&doc, &map), 7 * BAR);
    let e = add_fade_out(&doc, 4).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddFadeOut {
            range: TickRange::new(2 * BAR + Q, 6 * BAR + Q).unwrap()
        }
    );
    let l = master_lane(&e.doc);
    assert_eq!(ticks_values(l), [(2 * BAR + Q, fader), (6 * BAR + Q, 0.0)]);
    assert!(!l.step && !l.resting);
    assert_eq!(
        l.value_at(0),
        Some(fader),
        "before the fade: as the fader was"
    );
    assert_eq!(
        l.value_at(4 * BAR + Q),
        Some(fader / 2.0),
        "a straight line"
    );
    e.doc.check_lanes().unwrap();
    // One step: undo takes the lane away again.
    let mut history = History::new();
    let mut now = doc.clone();
    history.apply(&mut now, e);
    history.undo(&mut now).unwrap();
    assert_eq!(held(&now), held(&doc));
    assert!(history.undo(&mut now).is_none());
}

#[test]
fn a_fade_out_longer_than_the_song_starts_at_its_head() {
    let doc = song_with_clip(0, 2 * BAR);
    let e = add_fade_out(&doc, 16).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(0, doc.master.gain.position()), (2 * BAR, 0.0)]
    );
    let whole = add_fade_out(&doc, 2).unwrap();
    assert_eq!(ticks_values(master_lane(&whole.doc))[0].0, 0);
    // Two bars and a quarter: two bars back from the end is a quarter in; three is the head.
    let doc = song_with_clip(0, 2 * BAR + Q);
    let ticks = |bars| ticks_values(master_lane(&add_fade_out(&doc, bars).unwrap().doc));
    assert_eq!(ticks(2)[0].0, Q);
    assert_eq!(ticks(3)[0].0, 0);
    assert_eq!(ticks(3)[1].0, 2 * BAR + Q);
}

#[test]
fn a_fade_out_counts_bars_of_the_meters_they_are_in() {
    let doc = song_with_clip(0, 4 * BAR);
    // From bar 3 (from 1) on the bars are 3/4.
    let three = crate::Meter::new(3, 4).unwrap();
    let doc = crate::timeline::add_meter_change(&doc, 2, three)
        .unwrap()
        .doc;
    // Two bars of 4/4, then bars of three quarters: the clip ends 8 quarters after the
    // change, 2 quarters into a bar. The four bars before that bar are two of 3/4 and two
    // of 4/4, 14 quarters, and the fade is as long.
    let e = add_fade_out(&doc, 4).unwrap();
    let ticks: Vec<u64> = (master_lane(&e.doc).points.iter())
        .map(|p| p.tick)
        .collect();
    assert_eq!(ticks, [2 * Q, 4 * BAR]);
    // On a head of a bar: the bars before it, each of its own meter.
    let doc = song_with_clip(0, 2 * BAR + 6 * Q);
    let three = crate::Meter::new(3, 4).unwrap();
    let doc = crate::timeline::add_meter_change(&doc, 2, three)
        .unwrap()
        .doc;
    let e = add_fade_out(&doc, 3).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc))[0].0,
        BAR,
        "two bars of 3/4 and one of 4/4 back"
    );
}

#[test]
fn a_fade_out_without_clips_or_bars_is_none() {
    let doc = Doc::new_default();
    assert_eq!(add_fade_out(&doc, 4).map(|_| ()), Err(EditError::NoTakes));
    let doc = song_with_clip(0, 2 * BAR);
    assert_eq!(add_fade_out(&doc, 0).map(|_| ()), Err(EditError::Unchanged));
    let once = add_fade_out(&doc, 1).unwrap().doc;
    assert_eq!(
        add_fade_out(&once, 1).map(|_| ()),
        Err(EditError::Unchanged)
    );
}

#[test]
fn a_fade_out_starts_from_the_line_and_takes_the_points_after_its_start() {
    let doc = song_with_clip(0, 8 * BAR);
    let (e, id) = add_lane(&doc, M, AutoTarget::Volume, false).unwrap();
    let put = [p(0, 0.4), p(2 * BAR, 0.4), p(6 * BAR, 0.8), p(7 * BAR, 0.2)];
    let doc = put_points(&e.doc, M, AutoTarget::Volume, &put)
        .unwrap()
        .0
        .doc;
    let e = add_fade_out(&doc, 4).unwrap();
    let l = master_lane(&e.doc);
    assert_eq!(
        ticks_values(l),
        [(0, 0.4), (2 * BAR, 0.4), (4 * BAR, 0.6), (8 * BAR, 0.0)],
        "the line as it was up to the start, then the fade"
    );
    assert_ne!(l.id, id);
    assert_ne!(l.id, master_lane(&doc).id);
    e.doc.check_lanes().unwrap();
    // A point on the start itself is taken by the fade's.
    let again = add_fade_out(&e.doc, 6).unwrap();
    assert_eq!(
        ticks_values(master_lane(&again.doc)),
        [(0, 0.4), (2 * BAR, 0.4), (8 * BAR, 0.0)]
    );
}

/// A fade-out added again starts from the value from before the fade that is there.
#[test]
fn a_fade_out_again_is_shorter_or_longer_from_the_value_before_the_old_one() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let first = add_fade_out(&doc, 4).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&first)),
        [(4 * BAR, fader), (8 * BAR, 0.0)]
    );
    // Shorter: from the fader's value, not from the old line; the old head is gone.
    let shorter = add_fade_out(&first, 2).unwrap();
    assert_eq!(
        shorter.step,
        StepKind::AddFadeOut {
            range: TickRange::new(6 * BAR, 8 * BAR).unwrap()
        }
    );
    assert_eq!(
        ticks_values(master_lane(&shorter.doc)),
        [(6 * BAR, fader), (8 * BAR, 0.0)]
    );
    assert_eq!(master_lane(&shorter.doc).value_at(5 * BAR), Some(fader));
    shorter.doc.check_lanes().unwrap();
    // Longer.
    let longer = add_fade_out(&first, 6).unwrap();
    assert_eq!(
        ticks_values(master_lane(&longer.doc)),
        [(2 * BAR, fader), (8 * BAR, 0.0)]
    );
    // Shorter and shorter, then longer than all: always two points.
    let third = add_fade_out(&shorter.doc, 1).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&third)),
        [(7 * BAR, fader), (8 * BAR, 0.0)]
    );
    let back = add_fade_out(&third, 4).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&back)),
        ticks_values(master_lane(&first))
    );
    // Each is one step, and undo gives the very lane back.
    let mut history = History::new();
    let mut now = first.clone();
    history.apply(&mut now, shorter);
    let longer = add_fade_out(&now, 6).unwrap();
    history.apply(&mut now, longer);
    assert_eq!(
        ticks_values(master_lane(&now)),
        [(2 * BAR, fader), (8 * BAR, 0.0)]
    );
    history.undo(&mut now).unwrap();
    assert_eq!(
        ticks_values(master_lane(&now)),
        [(6 * BAR, fader), (8 * BAR, 0.0)]
    );
    history.undo(&mut now).unwrap();
    assert_eq!(held(&now), held(&first));
    assert!(Arc::ptr_eq(&now.master_lanes[0], &first.master_lanes[0]));
    history.redo(&mut now).unwrap();
    history.redo(&mut now).unwrap();
    assert_eq!(
        ticks_values(master_lane(&now)),
        [(2 * BAR, fader), (8 * BAR, 0.0)]
    );
}

/// The line before the new start is never touched: the old fade's head stays when the
/// line comes to it from another value.
#[test]
fn a_fade_out_again_keeps_the_line_before_its_start() {
    let doc = song_with_clip(0, 8 * BAR);
    let doc = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let put = [p(0, 0.5), p(8 * BAR, 1.0)];
    let doc = put_points(&doc, M, AutoTarget::Volume, &put).unwrap().0.doc;
    // The lane rises: the fade starts at what it is at bar 5.
    let first = add_fade_out(&doc, 4).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&first)),
        [(0, 0.5), (4 * BAR, 0.75), (8 * BAR, 0.0)]
    );
    // Shorter: the rise up to bar 5 stays, the value it reached is held up to the start.
    let shorter = add_fade_out(&first, 2).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&shorter)),
        [(0, 0.5), (4 * BAR, 0.75), (6 * BAR, 0.75), (8 * BAR, 0.0)]
    );
    // Shorter again: the head of the fade that was there came in flat, so it goes.
    let third = add_fade_out(&shorter, 1).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&third)),
        [(0, 0.5), (4 * BAR, 0.75), (7 * BAR, 0.75), (8 * BAR, 0.0)]
    );
    // Longer: the start is on the line before the old fade and takes its value there.
    let longer = add_fade_out(&third, 6).unwrap().doc;
    assert_eq!(
        ticks_values(master_lane(&longer)),
        [(0, 0.5), (2 * BAR, 0.625), (8 * BAR, 0.0)]
    );
}

#[test]
fn a_fade_out_again_over_points_drawn_by_hand() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let first = add_fade_out(&doc, 4).unwrap().doc;
    // Points drawn before the fade, and one drawn on it.
    let drawn = [
        p(BAR, 0.3),
        p(2 * BAR, 0.6),
        p(4 * BAR, fader),
        p(6 * BAR, 0.7),
        p(8 * BAR, 0.0),
    ];
    let doc = put_points(&first, M, AutoTarget::Volume, &drawn)
        .unwrap()
        .0
        .doc;
    // From bar 6: the points before stay, the drawn one inside goes with the old end. The
    // lane's last line (from the drawn point down) is the fade that is there, and the new
    // start is not on it: it takes the line's value.
    let e = add_fade_out(&doc, 3).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [
            (BAR, 0.3),
            (2 * BAR, 0.6),
            (4 * BAR, fader),
            (5 * BAR, (fader + 0.7) / 2.0),
            (8 * BAR, 0.0)
        ]
    );
    // From bar 8, on the last line: from the drawn point's value, and that point stays
    // (the line comes to it from another value).
    let e = add_fade_out(&doc, 1).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [
            (BAR, 0.3),
            (2 * BAR, 0.6),
            (4 * BAR, fader),
            (6 * BAR, 0.7),
            (7 * BAR, 0.7),
            (8 * BAR, 0.0)
        ]
    );
    let mut history = History::new();
    let mut now = doc.clone();
    history.apply(&mut now, e);
    history.undo(&mut now).unwrap();
    assert_eq!(held(&now), held(&doc));
    assert_eq!(
        ticks_values(master_lane(&now)),
        ticks_values(master_lane(&doc))
    );
    // A line drawn by hand down to the bottom long before the end is the fade that is
    // there, as one the menu made: the new fade takes its place, from the value before it.
    let bare = song_with_clip(0, 8 * BAR);
    let bare = add_lane(&bare, M, AutoTarget::Volume, false).unwrap().0.doc;
    let down = [p(0, 0.9), p(2 * BAR, 0.0)];
    let doc = put_points(&bare, M, AutoTarget::Volume, &down)
        .unwrap()
        .0
        .doc;
    let e = add_fade_out(&doc, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(6 * BAR, 0.9), (8 * BAR, 0.0)]
    );
    // A last point that is not at the bottom is no fade.
    let up = [p(4 * BAR, 0.25), p(8 * BAR, 0.75)];
    let doc = put_points(&bare, M, AutoTarget::Volume, &up).unwrap().0.doc;
    let e = add_fade_out(&doc, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(4 * BAR, 0.25), (6 * BAR, 0.5), (8 * BAR, 0.0)]
    );
}

/// A fade made when the song was longer or ended on a later place: its end is from the new
/// start on and goes.
#[test]
fn a_fade_out_again_after_the_songs_end_moved_back() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let old = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let put = [p(6 * BAR, fader), p(9 * BAR, 0.0)];
    let old = put_points(&old, M, AutoTarget::Volume, &put).unwrap().0.doc;
    let e = add_fade_out(&old, 1).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(7 * BAR, fader), (8 * BAR, 0.0)]
    );
}

/// Clips were added after a fade-out: the old fade lies in the middle of the song, wholly
/// before the new start, and the new one takes its place.
#[test]
fn a_fade_out_again_after_the_song_grew_takes_the_place_of_the_old_one() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let first = add_fade_out(&doc, 4).unwrap().doc;
    let grown = with_clip(&first, 12 * BAR, 16 * BAR);
    assert_eq!(
        ticks_values(master_lane(&grown)),
        [(4 * BAR, fader), (8 * BAR, 0.0)]
    );
    let e = add_fade_out(&grown, 2).unwrap();
    assert_eq!(
        e.step,
        StepKind::AddFadeOut {
            range: TickRange::new(14 * BAR, 16 * BAR).unwrap()
        }
    );
    let l = master_lane(&e.doc);
    assert_eq!(ticks_values(l), [(14 * BAR, fader), (16 * BAR, 0.0)]);
    for tick in [0, 4 * BAR, 6 * BAR, 8 * BAR, 12 * BAR, 14 * BAR] {
        assert_eq!(l.value_at(tick), Some(fader), "as before the old fade");
    }
    assert_eq!(l.value_at(15 * BAR), Some(fader / 2.0));
    assert_eq!(l.value_at(16 * BAR), Some(0.0));
    e.doc.check_lanes().unwrap();
    // The same again is nothing.
    assert_eq!(
        add_fade_out(&e.doc, 2).map(|_| ()),
        Err(EditError::Unchanged)
    );
    // One step: undo gives the very lane back, redo the new fade.
    let mut history = History::new();
    let mut now = grown.clone();
    history.apply(&mut now, e);
    history.undo(&mut now).unwrap();
    assert_eq!(held(&now), held(&grown));
    assert!(Arc::ptr_eq(&now.master_lanes[0], &grown.master_lanes[0]));
    history.redo(&mut now).unwrap();
    assert_eq!(
        ticks_values(master_lane(&now)),
        [(14 * BAR, fader), (16 * BAR, 0.0)]
    );
    assert!(history.redo(&mut now).is_none());

    // The lane comes into the old fade's head from another value: the head stays, and its
    // value is held over the old fade's place up to the new start.
    let rising = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let put = [p(0, 0.5), p(8 * BAR, 1.0)];
    let rising = put_points(&rising, M, AutoTarget::Volume, &put)
        .unwrap()
        .0
        .doc;
    let first = add_fade_out(&rising, 4).unwrap().doc;
    let grown = with_clip(&first, 8 * BAR, 16 * BAR);
    let e = add_fade_out(&grown, 4).unwrap();
    let l = master_lane(&e.doc);
    assert_eq!(
        ticks_values(l),
        [(0, 0.5), (4 * BAR, 0.75), (12 * BAR, 0.75), (16 * BAR, 0.0)]
    );
    assert_eq!(
        l.value_at(2 * BAR),
        Some(0.625),
        "before the head: untouched"
    );
    assert_eq!(l.value_at(6 * BAR), Some(0.75));
    assert_eq!(l.value_at(8 * BAR), Some(0.75));
}

/// Where the old fade's end lies decides nothing: before the new start, on it, between it
/// and the new end. Only the head being before the new start does.
#[test]
fn a_fade_out_again_wherever_the_old_one_ends() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let first = add_fade_out(&doc, 4).unwrap().doc;
    // The song is one bar longer: the old end (bar 9) is inside a new fade of two bars.
    let grown = with_clip(&first, 8 * BAR, 9 * BAR);
    let e = add_fade_out(&grown, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(7 * BAR, fader), (9 * BAR, 0.0)]
    );
    // The old end on the new start.
    let e = add_fade_out(&grown, 1).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(8 * BAR, fader), (9 * BAR, 0.0)]
    );
    // The whole old fade from the new start on: both points go as any point there, and
    // the start takes the lane's value, the one before the old head.
    let e = add_fade_out(&grown, 6).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(3 * BAR, fader), (9 * BAR, 0.0)]
    );
    // The old head on the new start: the same fade but for its end.
    let e = add_fade_out(&grown, 5).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [(4 * BAR, fader), (9 * BAR, 0.0)]
    );
}

/// Only the lane's last line is a fade that is there.
#[test]
fn a_line_to_the_bottom_with_points_after_it_is_no_fade() {
    let doc = song_with_clip(0, 16 * BAR);
    let doc = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let with = |points: &[AutoPoint]| {
        put_points(&doc, M, AutoTarget::Volume, points)
            .unwrap()
            .0
            .doc
    };
    // Down to the bottom and up again: all of it stays, the fade starts where the lane is.
    let dip = with(&[p(2 * BAR, 0.8), p(4 * BAR, 0.0), p(6 * BAR, 0.6)]);
    let e = add_fade_out(&dip, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [
            (2 * BAR, 0.8),
            (4 * BAR, 0.0),
            (6 * BAR, 0.6),
            (14 * BAR, 0.6),
            (16 * BAR, 0.0)
        ]
    );
    // Down, up, and down again: the last line is the fade, the dip before it stays.
    let twice = with(&[
        p(2 * BAR, 0.8),
        p(4 * BAR, 0.0),
        p(6 * BAR, 0.6),
        p(8 * BAR, 0.0),
    ]);
    let e = add_fade_out(&twice, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [
            (2 * BAR, 0.8),
            (4 * BAR, 0.0),
            (6 * BAR, 0.6),
            (14 * BAR, 0.6),
            (16 * BAR, 0.0)
        ]
    );
    // Down and then held at the bottom: the last line lies at the bottom, so there is no
    // value before it to come back to; the line down before it is not the last and stays.
    let held_down = with(&[p(2 * BAR, 0.8), p(4 * BAR, 0.0), p(6 * BAR, 0.0)]);
    let e = add_fade_out(&held_down, 2).unwrap();
    assert_eq!(
        ticks_values(master_lane(&e.doc)),
        [
            (2 * BAR, 0.8),
            (4 * BAR, 0.0),
            (14 * BAR, 0.0),
            (16 * BAR, 0.0)
        ]
    );
}

#[test]
fn a_fade_out_on_a_resting_lane_takes_the_old_fades_end_and_starts_from_the_fader() {
    let doc = song_with_clip(0, 8 * BAR);
    let fader = doc.master.gain.position();
    let doc = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let put = [p(BAR, 0.3), p(2 * BAR, 0.9), p(4 * BAR, 0.0)];
    let doc = put_points(&doc, M, AutoTarget::Volume, &put).unwrap().0.doc;
    let doc = rest_lane(&doc, M, AutoTarget::Volume).unwrap().doc;
    let e = add_fade_out(&doc, 2).unwrap();
    let l = master_lane(&e.doc);
    assert!(!l.resting);
    assert_eq!(
        ticks_values(l),
        [(BAR, 0.3), (2 * BAR, 0.9), (6 * BAR, fader), (8 * BAR, 0.0)]
    );
}

#[test]
fn a_fade_out_wakes_a_resting_lane_and_starts_from_the_fader() {
    let doc = song_with_clip(0, 8 * BAR);
    let doc = add_lane(&doc, M, AutoTarget::Volume, false).unwrap().0.doc;
    let put = [p(BAR, 0.3), p(6 * BAR, 0.9)];
    let doc = put_points(&doc, M, AutoTarget::Volume, &put).unwrap().0.doc;
    let doc = rest_lane(&doc, M, AutoTarget::Volume).unwrap().doc;
    let fader = doc.master.gain.position();
    let e = add_fade_out(&doc, 4).unwrap();
    let l = master_lane(&e.doc);
    assert!(!l.resting);
    assert_eq!(
        ticks_values(l),
        [(BAR, 0.3), (4 * BAR, fader), (8 * BAR, 0.0)]
    );
    // The same fade on a lane that only rests: the rest goes, and that is the step.
    let rested = rest_lane(&e.doc, M, AutoTarget::Volume).unwrap().doc;
    let mut history = History::new();
    let mut now = rested.clone();
    let woke = add_fade_out(&rested, 4).unwrap();
    assert_eq!(master_lane(&woke.doc).id, master_lane(&rested).id);
    history.apply(&mut now, woke);
    assert!(!master_lane(&now).resting);
    history.undo(&mut now).unwrap();
    assert_eq!(held(&now), held(&rested));
    // A lane with no points is as no lane: from the fader.
    let empty = add_lane(&song_with_clip(0, 8 * BAR), M, AutoTarget::Volume, false);
    let e = add_fade_out(&empty.unwrap().0.doc, 2).unwrap();
    assert_eq!(ticks_values(master_lane(&e.doc))[0], (6 * BAR, fader));
}

#[test]
fn lowering_all_tracks_leaves_the_masters_lane() {
    let doc = song_with_clip(0, 4 * BAR);
    let doc = add_fade_out(&doc, 2).unwrap().doc;
    let lowered = mixer::lower_all_tracks(&doc, 3.0).unwrap().doc;
    assert!(Arc::ptr_eq(&lowered.master_lanes[0], &doc.master_lanes[0]));
    assert_eq!(lowered.master, doc.master);
}
