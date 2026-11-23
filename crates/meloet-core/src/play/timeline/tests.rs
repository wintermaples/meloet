use std::collections::HashSet;

use meloet_model::Doc;
use meloet_protocol::{
    BarSpan, Command, MarkerView, MeterChangeView, TempoPointView, TickSpan, TimelineRef,
    TimelineView, TransportState,
};

use super::super::tests::{cmd, hear, ready, record_once, started, status};
use super::super::{EngineOp, Heard, StepKind};
use super::*;
use crate::songfile;
use crate::state::Model;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn tl(m: &Model) -> TimelineView {
    m.view().timeline
}

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn points(m: &Model) -> Vec<(u64, f64, bool)> {
    (tl(m).tempo_points.iter())
        .map(|p| (p.tick, p.tempo_bpm, p.ramp))
        .collect()
}

fn changes(m: &Model) -> Vec<(u32, u8, u8)> {
    (tl(m).meter_changes.iter())
        .map(|c| (c.bar, c.meter_num, c.meter_den))
        .collect()
}

fn markers(m: &Model) -> Vec<(u64, String)> {
    (tl(m).markers.iter())
        .map(|k| (k.tick, k.name.clone()))
        .collect()
}

fn point(tick: u64) -> Option<TimelineRef> {
    Some(TimelineRef::TempoPoint { tick })
}

fn flag(bar: u32) -> Option<TimelineRef> {
    Some(TimelineRef::MeterChange { bar })
}

fn marker(id: u32) -> Option<TimelineRef> {
    Some(TimelineRef::Marker { id })
}

fn loops(effects: &[Effect]) -> Vec<(u64, u64, bool)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::Loop { start, end, on }) => Some((*start, *end, *on)),
            _ => None,
        })
        .collect()
}

/// `record_once`, and the engine heard as stopped afterwards: the play line is where
/// `Locate` puts it.
fn recorded(m: &mut Model, stop: u64) {
    record_once(m, &[(60, 0.0, 100.0)], stop as f64);
    m.set_now(200, None);
}

/// K at `tick`, the name field left open. Returns the marker's id.
fn add_marker_at(m: &mut Model, tick: u64, name: &str) -> u32 {
    cmd(m, Command::Locate { tick });
    cmd(
        m,
        Command::AddMarker {
            name: name.into(),
            tick: None,
        },
    );
    tl(m).renaming.unwrap()
}

#[test]
fn a_tempo_point_snaps_is_selected_and_is_one_step() {
    let m = &mut ready();
    // 4 bars and a little under half a beat: the beat before. Half a beat: the later one.
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 4 * BAR + Q / 2 - 1,
            bpm: 104.0,
        },
    );
    assert_eq!(points(m), [(4 * BAR, 104.0, false)]);
    assert_eq!(tl(m).selected, point(4 * BAR));
    assert_eq!(steps(m), 1);
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoPointSelected {
            bar: 5,
            beat: 1,
            tempo_bpm: 104.0
        })
    );
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 8 * BAR - Q / 2,
            bpm: 120.004,
        },
    );
    assert_eq!(points(m)[1], (8 * BAR, 120.0, false));
    assert_eq!(tl(m).selected, point(8 * BAR));
    // On a point: it takes the value. The same value: no step, selected all the same.
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 4 * BAR,
            bpm: 100.0,
        },
    );
    assert_eq!(points(m)[0], (4 * BAR, 100.0, false));
    assert_eq!(steps(m), 3);
    cmd(m, Command::SelectTimeline { target: None });
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 4 * BAR,
            bpm: 100.0,
        },
    );
    assert_eq!((steps(m), tl(m).selected), (3, point(4 * BAR)));
    // Tick 0 is the head's tempo.
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 100,
            bpm: 90.0,
        },
    );
    assert_eq!((tl(m).tempo_bpm, points(m).len()), (90.0, 2));
    assert_eq!(tl(m).selected, point(0));
    assert_eq!(m.play().steps().len(), 4);
    assert!(matches!(m.play().steps()[3], StepKind::Tempo { .. }));
    // A tempo out of range does nothing.
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 12 * BAR,
            bpm: 301.0,
        },
    );
    assert_eq!((steps(m), points(m).len()), (4, 2));

    // Undo and redo bring the selection back with the song.
    cmd(m, Command::Undo);
    assert_eq!((tl(m).tempo_bpm, tl(m).selected), (96.0, point(4 * BAR)));
    cmd(m, Command::Undo);
    assert_eq!(points(m)[0], (4 * BAR, 104.0, false));
    assert_eq!(tl(m).selected, point(8 * BAR));
    cmd(m, Command::Undo);
    assert_eq!((points(m).len(), tl(m).selected), (1, point(4 * BAR)));
    cmd(m, Command::Undo);
    assert_eq!((points(m).len(), tl(m).selected), (0, None));
    cmd(m, Command::Redo);
    assert_eq!(tl(m).selected, point(4 * BAR));
    // Commands for a point that undo took away do nothing.
    for command in [
        Command::MoveTempoPoint {
            tick: 8 * BAR,
            to_tick: 9 * BAR,
            bpm: 130.0,
        },
        Command::SetTempoRamp {
            tick: 8 * BAR,
            on: true,
        },
        Command::DeleteTimeline {
            target: TimelineRef::TempoPoint { tick: 8 * BAR },
        },
        Command::SelectTimeline {
            target: point(8 * BAR),
        },
    ] {
        cmd(m, command);
        assert_eq!((steps(m), m.view().song.redo_count), (1, 3));
        assert_eq!(tl(m).selected, point(4 * BAR));
    }
}

#[test]
fn a_tempo_point_moves_takes_a_place_ramps_and_goes() {
    let m = &mut ready();
    for (tick, bpm) in [(4 * BAR, 104.0), (8 * BAR, 120.0), (12 * BAR, 90.0)] {
        cmd(m, Command::AddTempoPoint { tick, bpm });
    }
    cmd(
        m,
        Command::SetTempoRamp {
            tick: 4 * BAR,
            on: true,
        },
    );
    assert_eq!(steps(m), 4);
    assert_eq!(tl(m).selected, point(12 * BAR), "a ramp selects nothing");
    cmd(
        m,
        Command::SelectTimeline {
            target: point(4 * BAR),
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoRampSelected {
            bar: 5,
            beat: 1,
            tempo_bpm: 104.0,
            to_bar: 9,
            to_beat: 1,
            to_tempo_bpm: 120.0
        })
    );
    // The head, with and without a ramp.
    cmd(m, Command::SelectTimeline { target: point(0) });
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoPointSelected {
            bar: 1,
            beat: 1,
            tempo_bpm: 96.0
        })
    );
    cmd(m, Command::SetTempoRamp { tick: 0, on: true });
    assert!(tl(m).head_ramp);
    cmd(m, Command::SelectTimeline { target: point(0) });
    assert!(matches!(
        status(m),
        Some(StatusMessage::TempoRampSelected {
            bar: 1,
            to_bar: 5,
            ..
        })
    ));
    assert_eq!(steps(m), 5);

    // A drag: place and value in one step, the ramp stays, the point stays selected.
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 4 * BAR,
            to_tick: 5 * BAR + Q + 20,
            bpm: 110.0,
        },
    );
    assert_eq!(points(m)[0], (5 * BAR + Q, 110.0, true));
    assert_eq!((steps(m), tl(m).selected), (6, point(5 * BAR + Q)));
    assert!(matches!(
        status(m),
        Some(StatusMessage::TempoRampSelected {
            bar: 6,
            beat: 2,
            ..
        })
    ));
    // Onto another point: that one goes. Past another point: it is passed.
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 5 * BAR + Q,
            to_tick: 8 * BAR + 10,
            bpm: 110.0,
        },
    );
    assert_eq!(points(m), [(8 * BAR, 110.0, true), (12 * BAR, 90.0, false)]);
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 8 * BAR,
            to_tick: 14 * BAR,
            bpm: 110.0,
        },
    );
    assert_eq!(
        points(m),
        [(12 * BAR, 90.0, false), (14 * BAR, 110.0, true)]
    );
    // The last point's ramp does nothing, and is not told of.
    assert!(matches!(
        status(m),
        Some(StatusMessage::TempoPointSelected { bar: 15, .. })
    ));
    // Toward the head: the first beat after it. The head only takes the value.
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 12 * BAR,
            to_tick: 3,
            bpm: 90.0,
        },
    );
    assert_eq!(points(m)[0], (Q, 90.0, false));
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 0,
            to_tick: 7 * BAR,
            bpm: 100.0,
        },
    );
    assert_eq!((tl(m).tempo_bpm, points(m).len()), (100.0, 2));
    assert_eq!(tl(m).selected, point(0));
    let before = steps(m);
    // Nothing changes: no step.
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: Q,
            to_tick: Q + 20,
            bpm: 90.0,
        },
    );
    assert_eq!((steps(m), tl(m).selected), (before, point(Q)));

    // Delete; the head does not go.
    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::TempoPoint { tick: Q },
        },
    );
    assert_eq!(points(m), [(14 * BAR, 110.0, true)]);
    assert_eq!((steps(m), tl(m).selected), (before + 1, None));
    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::TempoPoint { tick: 0 },
        },
    );
    assert_eq!(steps(m), before + 1);
    cmd(m, Command::Undo);
    assert_eq!((points(m).len(), tl(m).selected), (2, point(Q)));
}

#[test]
fn a_value_typed_on_a_point_between_beats_leaves_it_there() {
    let m = &mut ready();
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 10 * BAR + Q,
            bpm: 120.0,
        },
    );
    // Half notes from bar 2 on: the point is in the middle of a beat now.
    cmd(
        m,
        Command::AddMeterChange {
            bar: 2,
            num: 3,
            den: 2,
        },
    );
    cmd(
        m,
        Command::MoveTempoPoint {
            tick: 10 * BAR + Q,
            to_tick: 10 * BAR + Q,
            bpm: 126.0,
        },
    );
    assert_eq!(points(m), [(10 * BAR + Q, 126.0, false)]);
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoPointSelected {
            bar: 8,
            beat: 2,
            tempo_bpm: 126.0
        }),
        "2 bars of 4/4, then bars of 3 half notes: half a beat into beat 2 of bar 8"
    );
}

#[test]
fn a_meter_change_is_added_moved_set_and_deleted() {
    let m = &mut ready();
    cmd(
        m,
        Command::AddMeterChange {
            bar: 8,
            num: 3,
            den: 4,
        },
    );
    assert_eq!(changes(m), [(8, 3, 4)]);
    assert_eq!((steps(m), tl(m).selected), (1, flag(8)));
    assert_eq!(
        status(m),
        Some(StatusMessage::MeterChangeSelected {
            bar: 9,
            meter_num: 3,
            meter_den: 4
        })
    );
    // Values that are no meter do nothing.
    for (num, den) in [(0, 4), (13, 4), (4, 3)] {
        cmd(m, Command::AddMeterChange { bar: 4, num, den });
        cmd(m, Command::SetMeterChange { bar: 8, num, den });
    }
    assert_eq!((steps(m), changes(m).len()), (1, 1));
    cmd(
        m,
        Command::AddMeterChange {
            bar: 4,
            num: 6,
            den: 8,
        },
    );
    // Bar 0 is the head's meter.
    cmd(
        m,
        Command::AddMeterChange {
            bar: 0,
            num: 2,
            den: 2,
        },
    );
    assert_eq!((tl(m).meter_num, tl(m).meter_den), (2, 2));
    assert_eq!(tl(m).selected, flag(0));
    assert_eq!(changes(m), [(4, 6, 8), (8, 3, 4)]);
    assert_eq!(steps(m), 3);

    cmd(
        m,
        Command::SetMeterChange {
            bar: 4,
            num: 7,
            den: 8,
        },
    );
    assert_eq!(changes(m)[0], (4, 7, 8));
    assert_eq!((steps(m), tl(m).selected), (4, flag(4)));
    // Onto another flag: that one goes. Bar 0 is no place for a flag, and does not move.
    cmd(m, Command::MoveMeterChange { bar: 4, to_bar: 8 });
    assert_eq!(changes(m), [(8, 7, 8)]);
    assert_eq!((steps(m), tl(m).selected), (5, flag(8)));
    cmd(m, Command::MoveMeterChange { bar: 8, to_bar: 0 });
    assert_eq!(changes(m), [(1, 7, 8)]);
    cmd(m, Command::MoveMeterChange { bar: 0, to_bar: 3 });
    cmd(m, Command::MoveMeterChange { bar: 5, to_bar: 3 });
    assert_eq!((steps(m), changes(m)), (6, vec![(1, 7, 8)]));

    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::MeterChange { bar: 1 },
        },
    );
    assert_eq!((steps(m), changes(m).len(), tl(m).selected), (7, 0, None));
    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::MeterChange { bar: 0 },
        },
    );
    assert_eq!(steps(m), 7);
    cmd(m, Command::Undo);
    assert_eq!((changes(m), tl(m).selected), (vec![(1, 7, 8)], flag(1)));
    cmd(m, Command::Undo);
    assert_eq!((changes(m), tl(m).selected), (vec![(8, 7, 8)], flag(8)));
    cmd(m, Command::Redo);
    assert_eq!((changes(m), tl(m).selected), (vec![(1, 7, 8)], flag(1)));
}

#[test]
fn the_transport_changes_the_point_in_force_and_says_which() {
    let m = &mut ready();
    // With no changes in the song: the head, and nothing is said of it.
    cmd(m, Command::SetTempo { bpm: 100.0 });
    cmd(m, Command::SetMeter { num: 3, den: 4 });
    assert_eq!((tl(m).tempo_bpm, tl(m).meter_num), (100.0, 3));
    assert!(!matches!(
        status(m),
        Some(StatusMessage::TempoPointSet { .. } | StatusMessage::MeterChangeSet { .. })
    ));
    cmd(m, Command::SetMeter { num: 4, den: 4 });

    for (tick, bpm) in [(4 * BAR, 104.0), (8 * BAR, 120.0)] {
        cmd(m, Command::AddTempoPoint { tick, bpm });
    }
    cmd(
        m,
        Command::SetTempoRamp {
            tick: 4 * BAR,
            on: true,
        },
    );
    cmd(
        m,
        Command::AddMeterChange {
            bar: 8,
            num: 3,
            den: 4,
        },
    );
    cmd(m, Command::SelectTimeline { target: None });
    let before = steps(m);
    // Before the first point: the head.
    cmd(m, Command::Locate { tick: BAR });
    cmd(m, Command::SetTempo { bpm: 90.0 });
    assert_eq!(tl(m).tempo_bpm, 90.0);
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoPointSet {
            bar: 1,
            beat: 1,
            tempo_bpm: 90.0
        })
    );
    cmd(m, Command::SetMeter { num: 5, den: 4 });
    assert_eq!(
        status(m),
        Some(StatusMessage::MeterChangeSet {
            bar: 1,
            meter_num: 5,
            meter_den: 4
        })
    );
    cmd(m, Command::SetMeter { num: 4, den: 4 });
    // Inside the ramp: the point it starts from.
    cmd(m, Command::Locate { tick: 6 * BAR + 77 });
    cmd(m, Command::SetTempo { bpm: 108.0 });
    assert_eq!(points(m)[0], (4 * BAR, 108.0, true));
    assert_eq!(
        status(m),
        Some(StatusMessage::TempoPointSet {
            bar: 5,
            beat: 1,
            tempo_bpm: 108.0
        })
    );
    // At the flag and after it: the flag.
    cmd(m, Command::Locate { tick: 8 * BAR });
    cmd(m, Command::SetMeter { num: 6, den: 8 });
    assert_eq!(changes(m), [(8, 6, 8)]);
    assert_eq!(
        status(m),
        Some(StatusMessage::MeterChangeSet {
            bar: 9,
            meter_num: 6,
            meter_den: 8
        })
    );
    cmd(m, Command::SetTempo { bpm: 130.0 });
    assert_eq!(points(m)[1], (8 * BAR, 130.0, false));
    assert_eq!(steps(m), before + 6);
    assert_eq!(tl(m).selected, None, "the transport selects nothing");
    // The same value: no step, and nothing is said.
    cmd(m, Command::SetTempo { bpm: 130.0 });
    assert_eq!(steps(m), before + 6);
    assert!(!matches!(
        status(m),
        Some(StatusMessage::TempoPointSet { .. })
    ));
}

#[test]
fn a_marker_at_a_tick_snaps_there_and_not_at_the_play_line() {
    let m = &mut ready();
    cmd(m, Command::Locate { tick: 6 * BAR });
    cmd(
        m,
        Command::AddMarker {
            name: "マーカー 1".into(),
            tick: Some(2 * BAR + Q / 2 + 10),
        },
    );
    let id = tl(m).markers[0].id;
    assert_eq!(markers(m), [(2 * BAR + Q, "マーカー 1".to_owned())]);
    assert_eq!((tl(m).selected, tl(m).renaming), (marker(id), Some(id)));
    assert_eq!(
        status(m),
        Some(StatusMessage::MarkerAdded {
            name: "マーカー 1".into()
        })
    );
    assert_eq!(steps(m), 1);
    cmd(
        m,
        Command::AddMarker {
            name: "マーカー 2".into(),
            tick: Some(u64::MAX),
        },
    );
    assert_eq!(steps(m), 2);
    assert!(markers(m)[1].0 <= meloet_model::MAX_TICK);
    cmd(m, Command::Undo);
    assert_eq!(markers(m).len(), 1);
}

#[test]
fn a_marker_and_the_name_typed_into_its_field_are_one_step() {
    let m = &mut ready();
    cmd(
        m,
        Command::Locate {
            tick: 2 * BAR + Q / 2 + 10,
        },
    );
    cmd(
        m,
        Command::AddMarker {
            name: "マーカー 1".into(),
            tick: None,
        },
    );
    let id = tl(m).markers[0].id;
    assert_eq!(markers(m), [(2 * BAR + Q, "マーカー 1".to_owned())]);
    assert_eq!((tl(m).selected, tl(m).renaming), (marker(id), Some(id)));
    assert_eq!(
        status(m),
        Some(StatusMessage::MarkerAdded {
            name: "マーカー 1".into()
        })
    );
    assert_eq!(steps(m), 1);
    cmd(
        m,
        Command::RenameMarker {
            id,
            name: "  サビ ".into(),
        },
    );
    assert_eq!(markers(m)[0].1, "サビ");
    assert_eq!((steps(m), tl(m).renaming), (1, None));
    assert_eq!(
        m.play().steps(),
        [StepKind::AddMarker {
            marker: MarkerId(id)
        }]
    );
    // Undo takes the marker away whole; redo brings it back with the name typed.
    cmd(m, Command::Undo);
    assert_eq!((markers(m).len(), tl(m).selected), (0, None));
    cmd(m, Command::Redo);
    assert_eq!(markers(m)[0].1, "サビ");
    assert_eq!((tl(m).selected, tl(m).renaming), (marker(id), None));

    // Afterwards a name is its own step.
    cmd(m, Command::OpenMarkerName { id });
    assert_eq!(tl(m).renaming, Some(id));
    cmd(
        m,
        Command::RenameMarker {
            id,
            name: "A メロ".into(),
        },
    );
    assert_eq!((steps(m), tl(m).renaming), (2, None));
    // Empty, spaces only, the same: the old name, no step, and the field closes.
    for name in ["", " 　 ", "A メロ", " A メロ "] {
        cmd(m, Command::OpenMarkerName { id });
        cmd(
            m,
            Command::RenameMarker {
                id,
                name: name.into(),
            },
        );
        assert_eq!((steps(m), tl(m).renaming), (2, None), "{name:?}");
        assert_eq!(markers(m)[0].1, "A メロ");
    }
    // A marker that is not there.
    cmd(m, Command::OpenMarkerName { id: id + 50 });
    assert_eq!(tl(m).renaming, None);
    cmd(
        m,
        Command::RenameMarker {
            id: id + 50,
            name: "x".into(),
        },
    );
    assert_eq!(steps(m), 2);
}

#[test]
fn what_ends_the_one_step_of_a_new_marker() {
    // Esc: the name it was added with stays, and a name typed later is another step.
    let m = &mut ready();
    let id = add_marker_at(m, 0, "マーカー 1");
    cmd(m, Command::CloseMarkerName);
    assert_eq!(
        (tl(m).renaming, markers(m)[0].1.as_str()),
        (None, "マーカー 1")
    );
    cmd(m, Command::OpenMarkerName { id });
    cmd(
        m,
        Command::RenameMarker {
            id,
            name: "サビ".into(),
        },
    );
    assert_eq!(steps(m), 2);

    // Another step in between.
    let m = &mut ready();
    let id = add_marker_at(m, 0, "マーカー 1");
    cmd(m, Command::SetTempo { bpm: 100.0 });
    assert_eq!(tl(m).renaming, Some(id), "the field stays open");
    cmd(
        m,
        Command::RenameMarker {
            id,
            name: "サビ".into(),
        },
    );
    assert_eq!(steps(m), 3);
    cmd(m, Command::Undo);
    assert_eq!(markers(m)[0].1, "マーカー 1");

    // Undo while the field is open: the marker goes and the field with it.
    let m = &mut ready();
    let id = add_marker_at(m, 0, "マーカー 1");
    cmd(m, Command::Undo);
    assert_eq!((tl(m).renaming, markers(m).len()), (None, 0));
    cmd(m, Command::Redo);
    assert_eq!(tl(m).renaming, None);
    cmd(
        m,
        Command::RenameMarker {
            id,
            name: "サビ".into(),
        },
    );
    assert_eq!(steps(m), 2);

    // The marker deleted while its field is open.
    let m = &mut ready();
    let id = add_marker_at(m, 0, "マーカー 1");
    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::Marker { id },
        },
    );
    assert_eq!((tl(m).renaming, tl(m).selected, steps(m)), (None, None, 2));
    cmd(m, Command::Undo);
    assert_eq!((tl(m).renaming, tl(m).selected), (None, marker(id)));

    // A second marker: the first one's field is not the open one any more.
    let m = &mut ready();
    let first = add_marker_at(m, 0, "マーカー 1");
    let second = add_marker_at(m, BAR, "マーカー 2");
    assert_ne!(first, second);
    cmd(
        m,
        Command::RenameMarker {
            id: first,
            name: "イントロ".into(),
        },
    );
    assert_eq!((steps(m), tl(m).renaming), (3, Some(second)));
    cmd(
        m,
        Command::RenameMarker {
            id: second,
            name: "サビ".into(),
        },
    );
    assert_eq!(steps(m), 4, "the first name was a step in between");

    // Another song.
    let mut p = Play::new(false, 0);
    let effects = &mut Vec::new();
    p.add_marker("マーカー 1", None, effects);
    p.set_timeline_open(true);
    let view = p.timeline_view();
    assert!(view.open && view.renaming.is_some() && view.selected.is_some());
    p.replace_song(meloet_model::Song::new_default(), false, effects);
    let view = p.timeline_view();
    assert_eq!(
        (view.renaming, view.selected, view.open, view.markers.len()),
        (None, None, false, 0)
    );
}

#[test]
fn markers_move_go_and_step() {
    let m = &mut ready();
    let a = add_marker_at(m, 4 * BAR, "A");
    let b = add_marker_at(m, 8 * BAR, "B");
    let c = add_marker_at(m, 8 * BAR, "C");
    cmd(m, Command::CloseMarkerName);
    assert_eq!(
        tl(m).markers,
        [
            MarkerView {
                id: a,
                tick: 4 * BAR,
                name: "A".into()
            },
            MarkerView {
                id: b,
                tick: 8 * BAR,
                name: "B".into()
            },
            MarkerView {
                id: c,
                tick: 8 * BAR,
                name: "C".into()
            },
        ]
    );
    assert!(a < b && b < c);
    let before = steps(m);

    // A press: selected, and the play line goes there. No step.
    cmd(m, Command::GoToMarker { id: a });
    assert_eq!(tl(m).selected, marker(a));
    assert_eq!(m.view().transport.position, 4 * BAR);
    // Alt → and Alt ←: the next one later, the last one earlier, selected as one pressed.
    cmd(m, Command::StepMarker { forward: true });
    assert_eq!(
        (m.view().transport.position, tl(m).selected),
        (8 * BAR, marker(b))
    );
    cmd(m, Command::StepMarker { forward: true });
    assert_eq!(m.view().transport.position, 8 * BAR, "none after it");
    cmd(m, Command::StepMarker { forward: false });
    assert_eq!(
        (m.view().transport.position, tl(m).selected),
        (4 * BAR, marker(a))
    );
    cmd(m, Command::StepMarker { forward: false });
    assert_eq!(m.view().transport.position, 4 * BAR, "none before it");
    cmd(m, Command::Locate { tick: 5 * BAR });
    cmd(m, Command::StepMarker { forward: false });
    assert_eq!(m.view().transport.position, 4 * BAR);
    assert_eq!(steps(m), before);
    cmd(m, Command::GoToMarker { id: c + 9 });
    assert_eq!(tl(m).selected, marker(a));

    // A drag: snapped, selected, one step; the order follows.
    cmd(
        m,
        Command::MoveMarker {
            id: c,
            tick: 2 * BAR + 100,
        },
    );
    assert_eq!(tl(m).markers[0].id, c);
    assert_eq!(tl(m).markers[0].tick, 2 * BAR);
    assert_eq!((steps(m), tl(m).selected), (before + 1, marker(c)));
    cmd(
        m,
        Command::MoveMarker {
            id: c,
            tick: 2 * BAR + 30,
        },
    );
    assert_eq!(steps(m), before + 1);
    cmd(m, Command::Undo);
    assert_eq!(tl(m).markers[2].id, c);
    assert_eq!(tl(m).selected, marker(a));

    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::Marker { id: a },
        },
    );
    assert_eq!((tl(m).markers.len(), tl(m).selected), (2, None));
    cmd(m, Command::MoveMarker { id: a, tick: 0 });
    cmd(m, Command::LoopFromMarker { id: a });
    assert_eq!(steps(m), before + 1);
    assert!(!m.view().song.loop_on);
}

#[test]
fn the_marker_at_the_ear_while_playing() {
    let m = &mut ready();
    cmd(m, Command::PlayStop);
    hear(
        m,
        1,
        (3 * BAR + Q + Q / 2 - 20) as f64,
        TransportState::Playing,
        0,
    );
    m.set_now(
        2,
        Some(Heard {
            tick: (3 * BAR + Q + Q / 2 + 10) as f64,
            state: TransportState::Playing,
            lap: 0,
        }),
    );
    cmd(
        m,
        Command::AddMarker {
            name: "ここ".into(),
            tick: None,
        },
    );
    assert_eq!(markers(m), [(3 * BAR + 2 * Q, "ここ".to_owned())]);
}

#[test]
fn the_loop_range_is_a_step_and_on_and_off_are_not() {
    let m = &mut ready();
    cmd(m, Command::Locate { tick: 2 * BAR + 5 });
    let made = cmd(m, Command::ToggleLoop);
    assert_eq!(loops(&made), [(2 * BAR, 6 * BAR, true)]);
    assert_eq!(steps(m), 1);
    cmd(m, Command::ToggleLoop);
    cmd(m, Command::ToggleLoop);
    assert_eq!(steps(m), 1);
    let set = cmd(
        m,
        Command::SetLoop {
            from_bar: 9,
            to_bar: 5,
        },
    );
    assert_eq!(loops(&set), [(4 * BAR, 8 * BAR, true)]);
    assert_eq!(steps(m), 2);
    // The same range again: no step, and the loop comes on.
    cmd(m, Command::ToggleLoop);
    let again = cmd(
        m,
        Command::SetLoop {
            from_bar: 5,
            to_bar: 9,
        },
    );
    assert_eq!(loops(&again), [(4 * BAR, 8 * BAR, true)]);
    assert_eq!(steps(m), 2);
    // Undo brings the range back, tells the engine, and leaves the loop on.
    let undone = cmd(m, Command::Undo);
    assert_eq!(loops(&undone), [(2 * BAR, 6 * BAR, true)]);
    assert_eq!(
        m.view().song.loop_range,
        Some(TickSpan {
            start: 2 * BAR,
            end: 6 * BAR
        })
    );
    let undone = cmd(m, Command::Undo);
    assert_eq!(loops(&undone), [(0, 0, false)]);
    assert_eq!(m.view().song.loop_range, None);
    assert!(!m.view().song.loop_on, "no range, nothing to go round");
    cmd(m, Command::Redo);
    cmd(m, Command::Redo);
    assert_eq!(
        m.play().steps()[1],
        StepKind::SetLoop {
            range: TickRange::new(4 * BAR, 8 * BAR).unwrap()
        }
    );
}

#[test]
fn bars_follow_the_meter_where_they_are() {
    let m = &mut ready();
    // Two bars of 4/4, then 3/4: bar 3 starts at 2 * BAR, bar 4 at 2 * BAR + 3 * Q.
    cmd(
        m,
        Command::AddMeterChange {
            bar: 2,
            num: 3,
            den: 4,
        },
    );
    cmd(
        m,
        Command::SetLoop {
            from_bar: 2,
            to_bar: 5,
        },
    );
    assert_eq!(
        m.view().song.loop_range,
        Some(TickSpan {
            start: BAR,
            end: 2 * BAR + 2 * 3 * Q
        })
    );
    cmd(m, Command::SelectTimeline { target: None });
    assert!(matches!(
        status(m),
        Some(StatusMessage::Summary {
            loop_bars: Some(BarSpan { from: 2, to: 5 }),
            ..
        })
    ));
    cmd(m, Command::Undo);
    cmd(
        m,
        Command::Locate {
            tick: 2 * BAR + 3 * Q + 60,
        },
    );
    cmd(m, Command::ToggleLoop);
    assert_eq!(
        m.view().song.loop_range,
        Some(TickSpan {
            start: 2 * BAR + 3 * Q,
            end: 2 * BAR + 5 * 3 * Q
        })
    );
    // The count-in counts the beats of the bar the recording starts in.
    cmd(m, Command::ToggleLoop);
    assert_eq!(m.view().transport.count_in_beats, 3);
    cmd(m, Command::Locate { tick: 100 });
    assert_eq!(m.view().transport.count_in_beats, 4);
}

#[test]
fn a_paste_lands_on_the_bar_of_the_meter_there() {
    let m = &mut ready();
    recorded(m, BAR);
    let clip = m.view().song.tracks[0].clips[0].id;
    cmd(
        m,
        Command::AddMeterChange {
            bar: 2,
            num: 3,
            den: 4,
        },
    );
    cmd(m, Command::CopyClips { clips: vec![clip] });
    cmd(
        m,
        Command::Locate {
            tick: 2 * BAR + 3 * Q + 60,
        },
    );
    cmd(m, Command::PasteClips { to: None });
    let clips = m.view().song.tracks[0].clips.clone();
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[1].start, 2 * BAR + 3 * Q);
}

#[test]
fn loop_from_a_marker_to_the_next_or_to_the_song_end() {
    let m = &mut ready();
    recorded(m, 5 * BAR + 100);
    let a = add_marker_at(m, 2 * BAR, "A");
    let b = add_marker_at(m, 4 * BAR, "B");
    let b2 = add_marker_at(m, 4 * BAR, "B2");
    let late = add_marker_at(m, 9 * BAR + Q, "おわり");
    cmd(m, Command::CloseMarkerName);
    let before = steps(m);
    let o = cmd(m, Command::LoopFromMarker { id: a });
    assert_eq!(loops(&o), [(2 * BAR, 4 * BAR, true)]);
    assert_eq!(steps(m), before + 1);
    assert!(m.view().song.loop_on);
    assert_eq!(tl(m).selected, marker(late), "selects nothing");
    // The same range: on again, with no step.
    cmd(m, Command::ToggleLoop);
    let o = cmd(m, Command::LoopFromMarker { id: a });
    assert_eq!(loops(&o), [(2 * BAR, 4 * BAR, true)]);
    assert_eq!(steps(m), before + 1);
    // Two markers at one tick: to the next later one.
    let o = cmd(m, Command::LoopFromMarker { id: b });
    assert_eq!(loops(&o), [(4 * BAR, 9 * BAR + Q, true)]);
    let o = cmd(m, Command::LoopFromMarker { id: b2 });
    assert_eq!(loops(&o), [(4 * BAR, 9 * BAR + Q, true)]);
    assert_eq!(steps(m), before + 2);
    // The last marker, after every clip: 4 bars from it.
    let o = cmd(m, Command::LoopFromMarker { id: late });
    assert_eq!(loops(&o), [(9 * BAR + Q, 13 * BAR + Q, true)]);
    // The last marker with a clip after it: to the clip's end, up to the head of a bar.
    cmd(
        m,
        Command::DeleteTimeline {
            target: TimelineRef::Marker { id: late },
        },
    );
    let o = cmd(m, Command::LoopFromMarker { id: b });
    assert_eq!(loops(&o), [(4 * BAR, 6 * BAR, true)]);
    // Undo takes the range back and leaves the loop on.
    let o = cmd(m, Command::Undo);
    assert_eq!(loops(&o), [(9 * BAR + Q, 13 * BAR + Q, true)]);
}

#[test]
fn nothing_of_the_timeline_changes_while_recording() {
    let m = &mut ready();
    let id = add_marker_at(m, BAR, "A");
    cmd(m, Command::CloseMarkerName);
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 4 * BAR,
            bpm: 104.0,
        },
    );
    cmd(
        m,
        Command::AddMeterChange {
            bar: 4,
            num: 3,
            den: 4,
        },
    );
    cmd(m, Command::SelectTimeline { target: None });
    let before = (tl(m), steps(m), m.view().song.loop_range);
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    for round in 0..2 {
        for command in [
            Command::SetTempo { bpm: 100.0 },
            Command::SetMeter { num: 3, den: 4 },
            Command::AddTempoPoint {
                tick: 8 * BAR,
                bpm: 99.0,
            },
            Command::MoveTempoPoint {
                tick: 4 * BAR,
                to_tick: 5 * BAR,
                bpm: 99.0,
            },
            Command::SetTempoRamp {
                tick: 4 * BAR,
                on: true,
            },
            Command::AddMeterChange {
                bar: 6,
                num: 5,
                den: 4,
            },
            Command::MoveMeterChange { bar: 4, to_bar: 5 },
            Command::SetMeterChange {
                bar: 4,
                num: 5,
                den: 4,
            },
            Command::DeleteTimeline {
                target: TimelineRef::TempoPoint { tick: 4 * BAR },
            },
            Command::DeleteTimeline {
                target: TimelineRef::Marker { id },
            },
            Command::AddMarker {
                name: "B".into(),
                tick: None,
            },
            Command::OpenMarkerName { id },
            Command::RenameMarker {
                id,
                name: "C".into(),
            },
            Command::MoveMarker { id, tick: 3 * BAR },
            Command::StepMarker { forward: true },
            Command::LoopFromMarker { id },
            Command::SetLoop {
                from_bar: 1,
                to_bar: 3,
            },
            Command::ToggleLoop,
        ] {
            cmd(m, command.clone());
            assert_eq!(
                (tl(m), steps(m), m.view().song.loop_range),
                before,
                "{command:?}"
            );
            assert_eq!(m.view().transport.position, BAR, "{command:?}");
        }
        if round == 0 {
            started(m);
            hear(m, 1, BAR as f64, TransportState::Recording, 0);
            assert_eq!(m.view().transport.state, TransportState::Recording);
        }
    }
    // Selecting and opening the row change nothing of the song, and work.
    cmd(m, Command::SelectTimeline { target: marker(id) });
    cmd(m, Command::SetTimelineOpen { open: true });
    assert_eq!((tl(m).selected, tl(m).open), (marker(id), true));
}

#[test]
fn the_section_as_the_screen_gets_it() {
    let m = &mut ready();
    cmd(m, Command::SetTempoRamp { tick: 0, on: true });
    cmd(
        m,
        Command::AddTempoPoint {
            tick: 4 * BAR,
            bpm: 104.5,
        },
    );
    cmd(
        m,
        Command::AddMeterChange {
            bar: 8,
            num: 6,
            den: 8,
        },
    );
    let id = add_marker_at(m, BAR, "A");
    cmd(m, Command::SetTimelineOpen { open: true });
    assert_eq!(
        tl(m),
        TimelineView {
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            head_ramp: true,
            tempo_points: vec![TempoPointView {
                tick: 4 * BAR,
                tempo_bpm: 104.5,
                ramp: false
            }],
            meter_changes: vec![MeterChangeView {
                bar: 8,
                meter_num: 6,
                meter_den: 8
            }],
            markers: vec![MarkerView {
                id,
                tick: BAR,
                name: "A".into()
            }],
            selected: marker(id),
            renaming: Some(id),
            open: true,
        }
    );
    cmd(m, Command::SetTimelineOpen { open: false });
    assert!(!tl(m).open);
    assert_eq!(steps(m), 4);
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

fn held(doc: &Doc) -> Doc {
    Doc {
        next_id: 0,
        ..doc.clone()
    }
}

fn random_command(rng: &mut Rng, view: &TimelineView) -> Command {
    let tick = rng.below(24 * 4) * Q + rng.below(3) * 111;
    let bar = rng.below(20) as u32;
    let bpm = 30.0 + rng.below(27_001) as f64 / 100.0;
    let (num, den) = (
        1 + rng.below(12) as u8,
        [2, 4, 8, 16][rng.below(4) as usize],
    );
    // Enter in the name field of the marker just made, often enough to be seen.
    if let Some(id) = view.renaming
        && rng.below(3) == 0
    {
        let name = ["サビ", "A メロ"][rng.below(2) as usize].into();
        return Command::RenameMarker { id, name };
    }
    // Now and then a place with nothing at it, as a command sent just before an undo has.
    let stale = rng.below(6) == 0;
    let a_point = match view.tempo_points.len() as u64 {
        n if n > 0 && !stale => view.tempo_points[rng.below(n) as usize].tick,
        _ => rng.below(3) * 7,
    };
    let a_flag = match view.meter_changes.len() as u64 {
        n if n > 0 && !stale => view.meter_changes[rng.below(n) as usize].bar,
        _ => rng.below(3) as u32 * 31,
    };
    let a_marker = match view.markers.len() as u64 {
        n if n > 0 && !stale => view.markers[rng.below(n) as usize].id,
        _ => rng.below(1000) as u32,
    };
    let target = match rng.below(3) {
        0 => TimelineRef::TempoPoint { tick: a_point },
        1 => TimelineRef::MeterChange { bar: a_flag },
        _ => TimelineRef::Marker { id: a_marker },
    };
    match rng.below(21) {
        0 => Command::AddTempoPoint { tick, bpm },
        1 => Command::MoveTempoPoint {
            tick: a_point,
            to_tick: if rng.below(4) == 0 { a_point } else { tick },
            bpm,
        },
        2 => Command::SetTempoRamp {
            tick: a_point,
            on: rng.below(2) == 0,
        },
        3 => Command::AddMeterChange { bar, num, den },
        4 => Command::MoveMeterChange {
            bar: a_flag,
            to_bar: bar,
        },
        5 => Command::SetMeterChange {
            bar: a_flag,
            num,
            den,
        },
        6 => Command::SelectTimeline {
            target: (rng.below(5) > 0).then_some(target),
        },
        7 => Command::DeleteTimeline { target },
        8 | 9 => Command::AddMarker {
            name: format!("マーカー {}", view.markers.len() + 1),
            tick: None,
        },
        10 => Command::OpenMarkerName { id: a_marker },
        11 => Command::RenameMarker {
            id: a_marker,
            name: ["サビ", "A メロ", "", " 間奏 "][rng.below(4) as usize].into(),
        },
        12 => Command::CloseMarkerName,
        13 => Command::MoveMarker { id: a_marker, tick },
        14 => Command::GoToMarker { id: a_marker },
        15 => Command::StepMarker {
            forward: rng.below(2) == 0,
        },
        16 => Command::LoopFromMarker { id: a_marker },
        17 => Command::SetLoop {
            from_bar: 1 + bar,
            to_bar: 1 + rng.below(20) as u32,
        },
        18 => Command::ToggleLoop,
        19 => Command::SetTempo { bpm },
        _ => Command::Locate { tick },
    }
}

fn well_formed(m: &Model, ctx: &str) {
    let doc = &m.play().song().doc;
    assert_eq!(doc.timeline.check(), Ok(()), "{ctx}");
    assert!(
        (doc.markers.windows(2)).all(|w| (w[0].tick, w[0].id) < (w[1].tick, w[1].id)),
        "{ctx}: markers out of order"
    );
    let view = tl(m);
    if let Some(at) = view.selected {
        assert!(has(doc, at), "{ctx}: {at:?} is selected and not there");
    }
    if let Some(id) = view.renaming {
        assert!(doc.marker(MarkerId(id)).is_some(), "{ctx}: renaming {id}");
    }
}

#[test]
fn random_commands_undo_and_redo_come_back_to_the_same_songs_and_selections() {
    let mut made = 0;
    let mut amended = 0;
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let m = &mut ready();
        recorded(m, 3 * BAR);
        let base = steps(m) as usize;
        let clip_ids: HashSet<u32> = {
            let doc = &m.play().song().doc;
            let c = &doc.tracks[0].clips[0];
            [c.id.0, c.active_take().take.id.0].into()
        };
        // Every Doc of the history, with what was selected before the step that made it
        // and after it.
        let first = m.play().song().doc.clone();
        let mut line = vec![(first, None, None)];
        let mut at = 0usize;
        let mut given: HashSet<u32> = HashSet::new();
        let mut gone: HashSet<u32> = HashSet::new();
        for round in 0..500 {
            let ctx = format!("seed {seed}, round {round}");
            let view = tl(m);
            match rng.below(10) {
                0..=6 => {
                    let command = random_command(&mut rng, &view);
                    let before = m.play().song().doc.clone();
                    cmd(m, command.clone());
                    let doc = m.play().song().doc.clone();
                    let now = steps(m) as usize - base;
                    let ctx = format!("{ctx}, {command:?}");
                    if now == at + 1 {
                        made += 1;
                        line.truncate(at + 1);
                        line.push((doc.clone(), view.selected, tl(m).selected));
                        at += 1;
                        // A new marker has an id never given out, to a marker or a clip.
                        for k in doc.markers.iter() {
                            if before.marker(k.id).is_none() {
                                assert!(!clip_ids.contains(&k.id.0), "{ctx}");
                                assert!(given.insert(k.id.0), "{ctx}: id given twice");
                                assert!(!gone.contains(&k.id.0), "{ctx}");
                            }
                        }
                    } else {
                        assert_eq!(now, at, "{ctx}");
                        if held(&doc) != held(&before) {
                            // The name typed into the field of the marker just made.
                            assert!(matches!(command, Command::RenameMarker { .. }), "{ctx}");
                            assert_eq!(m.view().song.redo_count, 0, "{ctx}");
                            amended += 1;
                            line[at].0 = doc.clone();
                        }
                    }
                    for k in before.markers.iter() {
                        if doc.marker(k.id).is_none() {
                            gone.insert(k.id.0);
                        }
                    }
                }
                // Not past the recording the walk starts from.
                7..=8 if at > 0 => {
                    cmd(m, Command::Undo);
                    let before_step = line[at].1;
                    at -= 1;
                    assert_eq!(tl(m).selected, before_step, "{ctx}: undo");
                }
                7..=8 => {}
                _ => {
                    cmd(m, Command::Redo);
                    if at + 1 < line.len() {
                        at += 1;
                        assert_eq!(tl(m).selected, line[at].2, "{ctx}: redo");
                    }
                }
            }
            well_formed(m, &ctx);
            assert_eq!(steps(m) as usize - base, at, "{ctx}");
            assert_eq!(held(&m.play().song().doc), held(&line[at].0), "{ctx}");
            assert!(
                m.play().song().doc.next_id >= line.iter().map(|l| l.0.next_id).max().unwrap(),
                "{ctx}: next_id went back"
            );
            if round % 50 == 0 {
                let song = m.play().song();
                let back = songfile::decode(&songfile::encode(song)).unwrap();
                assert_eq!(&back, song, "{ctx}: saved and opened");
            }
        }
    }
    assert!(made > 1000 && amended > 5, "{made} {amended}");
}
