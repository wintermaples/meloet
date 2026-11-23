//! The master's lane, of its volume (round 12, M5), and 「フェードアウトをつける」.

use meloet_model::{LaneHome as Home, StepKind, StripId};
use meloet_protocol::{
    AutoTarget, BarSpan, Command, FADE_OUT_BARS_MAX, IndexRange, LaneHome, LanePoints, LaneRef,
    StatusMessage, StepName, StepPlace, StepWhat, StripRef, TransportState,
};

use super::{BAR, Q, VOLUME, last_step, play_at, steps};
use crate::play::EngineOp;
use crate::play::tests::{cmd, hear, ready, record_once, status};
use crate::songfile;
use crate::state::{Effect, Model};

const MASTER: LaneHome = LaneHome::Master;
const MODEL_VOLUME: meloet_model::AutoTarget = meloet_model::AutoTarget::Volume;

fn master_ref() -> LaneRef {
    LaneRef {
        home: MASTER,
        target: VOLUME,
    }
}

fn gain(db: f32, commit: bool) -> Command {
    Command::SetGain {
        strip: StripRef::Master,
        db: Some(db),
        commit,
    }
}

fn lane(m: &Model) -> meloet_protocol::LaneView {
    let lanes = m.view().song.master.lanes;
    assert_eq!(lanes.len(), 1, "the master has one lane");
    lanes[0].clone()
}

fn points(m: &Model) -> Vec<(u64, f32)> {
    let bytes = m.play().lane(lane(m).lane_id).expect("the points");
    let lane = LanePoints::decode(&bytes).unwrap();
    lane.points.iter().map(|p| (p.tick, p.value)).collect()
}

fn master_gain(m: &Model) -> Option<f32> {
    m.view().song.master.gain_db
}

/// The lane of the master each time the engine is told of it: its points, none when it
/// plays nothing.
fn sent(effects: &[Effect]) -> Vec<Vec<(u64, f32)>> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::MasterAutomation { lanes }) => {
                assert!(lanes.len() <= 1 && lanes.iter().all(|l| l.target == MODEL_VOLUME));
                Some(lanes.first().map_or(Vec::new(), |l| l.points.clone()))
            }
            _ => None,
        })
        .collect()
}

fn holds(effects: &[Effect]) -> Vec<bool> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::HoldMasterLane { on }) => Some(*on),
            _ => None,
        })
        .collect()
}

/// A lane on the master that goes from 0.5 at the head to 0.5 at bar 9, flat.
fn with_lane(m: &mut Model) {
    cmd(
        m,
        Command::AddLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    for tick in [0, 8 * BAR] {
        let lane = lane(m).lane_id;
        cmd(
            m,
            Command::AddAutoPoint {
                lane,
                tick,
                value: Some(0.5),
            },
        );
    }
}

/// A song with one clip over bars 1 to 4.
fn with_clip() -> Model {
    let mut m = ready();
    record_once(&mut m, &[(60, 0.0, Q as f64)], (4 * BAR) as f64);
    m.set_now(200, None);
    assert_eq!(m.view().song.tracks[0].clips[0].end, 4 * BAR);
    m
}

#[test]
fn the_master_takes_a_lane_of_its_volume_and_no_other() {
    let m = &mut ready();
    let before = steps(m).len();
    for target in [AutoTarget::Pan, AutoTarget::Mute] {
        let home = MASTER;
        cmd(m, Command::AddLane { home, target });
    }
    assert!(m.view().song.master.lanes.is_empty());
    assert_eq!(steps(m).len(), before);

    let effects = cmd(
        m,
        Command::AddLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    assert!(sent(&effects).is_empty(), "no point: nothing moves");
    assert_eq!(
        last_step(m),
        StepKind::AddLane {
            home: Home::Master,
            target: MODEL_VOLUME
        }
    );
    let made = lane(m);
    assert_eq!(made.name, meloet_protocol::LaneName::Volume);
    assert!(!made.step && !made.resting && made.points == 0);
    assert!(m.view().song.tracks.iter().all(|t| t.lanes.is_empty()));
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::AddLane);
    assert_eq!(row.place, Some(StepPlace::Master));
    assert_eq!(
        row.what,
        StepWhat::Lane {
            target: meloet_protocol::LaneName::Volume,
            points: None
        }
    );
    // A second one unfolds the lane, which is no step.
    cmd(
        m,
        Command::FoldLane {
            home: MASTER,
            target: VOLUME,
            folded: true,
        },
    );
    assert_eq!(m.view().automation.folded, [master_ref()]);
    let count = steps(m).len();
    cmd(
        m,
        Command::AddLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    assert!(m.view().automation.folded.is_empty());
    assert_eq!(steps(m).len(), count);
    cmd(m, Command::Undo);
    assert!(m.view().song.master.lanes.is_empty());
}

#[test]
fn points_of_the_masters_lane_are_steps_the_engine_hears_and_undo_brings_the_selection_back() {
    let m = &mut ready();
    cmd(
        m,
        Command::AddLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    let first = lane(m).lane_id;
    let effects = cmd(
        m,
        Command::AddAutoPoint {
            lane: first,
            tick: 0,
            value: Some(0.5),
        },
    );
    assert_eq!(sent(&effects), [vec![(0, 0.5)]]);
    let second = lane(m).lane_id;
    assert_ne!(second, first, "new points, a new id");
    // The points as they were are not named any more.
    let count = steps(m).len();
    cmd(
        m,
        Command::AddAutoPoint {
            lane: first,
            tick: BAR,
            value: Some(0.1),
        },
    );
    assert_eq!(steps(m).len(), count);
    let effects = cmd(
        m,
        Command::AddAutoPoint {
            lane: second,
            tick: BAR,
            value: Some(0.25),
        },
    );
    assert_eq!(sent(&effects), [vec![(0, 0.5), (BAR, 0.25)]]);
    let selected = |m: &Model| m.view().automation.selected;
    assert_eq!(
        selected(m).map(|s| (s.lane_id, s.points)),
        Some((lane(m).lane_id, vec![IndexRange { start: 1, end: 2 }]))
    );
    let before_move = selected(m);
    cmd(
        m,
        Command::MoveAutoPoints {
            lane: lane(m).lane_id,
            points: vec![1],
            d_tick: BAR as i64,
            d_value: 0.25,
        },
    );
    assert_eq!(points(m), [(0, 0.5), (2 * BAR, 0.5)]);
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::MoveAutoPoints);
    assert_eq!(row.place, Some(StepPlace::Master));
    // A step that leaves the points tells the engine nothing of them.
    let effects = cmd(m, Command::SetTempo { bpm: 90.0 });
    assert!(sent(&effects).is_empty());
    cmd(m, Command::Undo);
    let effects = cmd(m, Command::Undo);
    assert_eq!(sent(&effects), [vec![(0, 0.5), (BAR, 0.25)]]);
    assert_eq!(selected(m), before_move);
    cmd(m, Command::Redo);
    cmd(
        m,
        Command::DeleteAutoPoints {
            lane: lane(m).lane_id,
            points: vec![0, 1],
        },
    );
    assert!(points(m).is_empty());
    assert_eq!(selected(m), None);
}

#[test]
fn a_deleted_lane_leaves_the_masters_fader_where_the_line_was() {
    let m = &mut ready();
    with_lane(m);
    cmd(
        m,
        Command::MoveAutoPoints {
            lane: lane(m).lane_id,
            points: vec![0, 1],
            d_tick: 0,
            d_value: 0.1,
        },
    );
    let effects = cmd(
        m,
        Command::DeleteLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    assert_eq!(sent(&effects), [vec![]]);
    assert_eq!(master_gain(m), Some(-6.0));
    assert!(m.view().song.master.lanes.is_empty());
    cmd(m, Command::Undo);
    assert_eq!(master_gain(m), Some(0.0));
    assert_eq!(lane(m).points, 2);
}

#[test]
fn a_hand_on_the_masters_fader_rests_its_lane_with_the_value_in_one_step() {
    let m = &mut ready();
    with_lane(m);
    let before = steps(m).len();
    let effects = cmd(m, gain(-2.0, false));
    assert_eq!(holds(&effects), [true]);
    for db in [-4.0, -6.0] {
        assert!(holds(&cmd(m, gain(db, false))).is_empty());
    }
    assert_eq!(steps(m).len(), before);
    let effects = cmd(m, gain(-6.0, true));
    assert_eq!(holds(&effects), [false]);
    assert_eq!(
        sent(&effects),
        [vec![]],
        "at rest the engine plays it no more"
    );
    assert_eq!(steps(m).len(), before + 1);
    assert_eq!(
        last_step(m),
        StepKind::SetGain {
            strip: StripId::Master
        }
    );
    assert!(lane(m).resting);
    assert_eq!(master_gain(m), Some(-6.0));
    assert_eq!(status(m), Some(StatusMessage::MasterAutomationRested));
    // At rest, a hand is a step as before, and the lane stays at rest.
    cmd(m, gain(-3.0, false));
    cmd(m, gain(-3.0, true));
    assert_eq!(steps(m).len(), before + 2);
    assert!(lane(m).resting);
    // 「● 書く」 is not for a lane at rest.
    cmd(
        m,
        Command::SetLaneWrite {
            home: MASTER,
            target: VOLUME,
            on: true,
        },
    );
    assert!(m.view().automation.armed.is_empty());
    // 「もどす」 is a step.
    let effects = cmd(
        m,
        Command::RestoreLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    assert_eq!(sent(&effects), [vec![(0, 0.5), (8 * BAR, 0.5)]]);
    assert!(!lane(m).resting);
    assert_eq!(status(m), Some(StatusMessage::MasterAutomationRestored));
    assert_eq!(
        last_step(m),
        StepKind::RestoreLane {
            home: Home::Master,
            target: MODEL_VOLUME
        }
    );
    // Undo takes back the restore, the value, then the value and the rest together.
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert!(lane(m).resting);
    cmd(m, Command::Undo);
    assert!(!lane(m).resting);
    assert_eq!(master_gain(m), Some(0.0));
    // The tracks' faders have nothing to do with it.
    assert!(m.view().song.tracks.iter().all(|t| t.lanes.is_empty()));
}

#[test]
fn esc_and_a_typed_value_on_the_masters_fader() {
    let m = &mut ready();
    with_lane(m);
    let before = steps(m).len();
    cmd(m, gain(-6.0, false));
    let effects = cmd(m, Command::CancelMixPreview);
    assert_eq!(holds(&effects), [false]);
    assert_eq!(steps(m).len(), before);
    assert!(!lane(m).resting);
    // Typed while writing: the lane rests.
    cmd(
        m,
        Command::SetLaneWrite {
            home: MASTER,
            target: VOLUME,
            on: true,
        },
    );
    assert_eq!(m.view().automation.armed, [master_ref()]);
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, (2 * Q) as f64);
    cmd(m, gain(-6.0, true));
    assert!(lane(m).resting);
    assert_eq!(master_gain(m), Some(-6.0));
    assert!(m.view().automation.armed.is_empty());
}

#[test]
fn writing_on_the_masters_fader_is_one_step_where_it_was_heard() {
    let m = &mut ready();
    with_lane(m);
    cmd(
        m,
        Command::SetLaneWrite {
            home: MASTER,
            target: VOLUME,
            on: true,
        },
    );
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, (2 * Q) as f64);
    assert_eq!(status(m), Some(StatusMessage::MasterAutomationWriting));
    let before = steps(m).len();
    cmd(m, gain(-6.0, false));
    let view = m.view().automation;
    assert_eq!(view.writing.len(), 1);
    assert_eq!(view.writing[0].lane, master_ref());
    assert_eq!(view.writing[0].from_tick, 2 * Q);
    play_at(m, 2_000, (2 * Q + Q / 2) as f64);
    cmd(m, gain(0.0, false));
    play_at(m, 3_000, (3 * Q) as f64);
    cmd(m, gain(0.0, true));
    assert_eq!(steps(m).len(), before + 1);
    assert_eq!(
        last_step(m),
        StepKind::WriteAutomation {
            home: Home::Master,
            target: MODEL_VOLUME
        }
    );
    assert_eq!(master_gain(m), Some(0.0), "the hand's value stays");
    let got = points(m);
    let want = [
        (0, 0.5),
        (2 * Q - 1, 0.5),
        (2 * Q, 0.6),
        (2 * Q + Q / 2, 0.8),
        (3 * Q, 0.8),
        (3 * Q + 1, 0.5),
        (8 * BAR, 0.5),
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(g.0 == w.0 && (g.1 - w.1).abs() < 1e-6, "{got:?}");
    }
    assert!(!lane(m).resting);
    // Stopping turns 「● 書く」 off.
    cmd(m, Command::PlayStop);
    hear(m, 4_000, (3 * Q) as f64, TransportState::Stopped, 0);
    assert!(m.view().automation.armed.is_empty());
    cmd(m, Command::Undo);
    assert_eq!(points(m), [(0, 0.5), (8 * BAR, 0.5)]);
}

#[test]
fn the_mixer_says_once_that_the_masters_fader_moves() {
    let m = &mut ready();
    assert_eq!(m.play().automation_moving(), None);
    with_lane(m);
    assert_eq!(
        m.play().automation_moving(),
        Some(StatusMessage::AutomationMoving {
            tracks: Vec::new(),
            master: true
        })
    );
    cmd(m, gain(-6.0, true));
    assert_eq!(m.play().automation_moving(), None, "at rest");
}

#[test]
fn a_fade_out_is_one_step_at_the_songs_end() {
    let m = &mut with_clip();
    let before = steps(m).len();
    let fader = meloet_model::Gain::UNITY.position();
    let effects = cmd(m, Command::AddFadeOut { bars: 2 });
    assert_eq!(steps(m).len(), before + 1);
    assert_eq!(points(m), [(2 * BAR, fader), (4 * BAR, 0.0)]);
    assert_eq!(sent(&effects), [vec![(2 * BAR, fader), (4 * BAR, 0.0)]]);
    assert_eq!(status(m), None);
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::AddFadeOut);
    assert_eq!(row.place, Some(StepPlace::Master));
    assert_eq!(
        row.what,
        StepWhat::Bars {
            span: BarSpan { from: 3, to: 5 }
        }
    );
    // The same again is no step.
    cmd(m, Command::AddFadeOut { bars: 2 });
    assert_eq!(steps(m).len(), before + 1);
    let effects = cmd(m, Command::AddFadeOut { bars: 1 });
    // A shorter one starts from the fader's value, not on the line of the fade that is there.
    assert_eq!(points(m), [(3 * BAR, fader), (4 * BAR, 0.0)]);
    assert_eq!(sent(&effects), [vec![(3 * BAR, fader), (4 * BAR, 0.0)]]);
    cmd(
        m,
        Command::AddFadeOut {
            bars: FADE_OUT_BARS_MAX,
        },
    );
    assert_eq!(points(m), [(0, fader), (4 * BAR, 0.0)]);
    assert_eq!(steps(m).len(), before + 3);
    // Only 1 to 16 bars.
    for bars in [0, FADE_OUT_BARS_MAX + 1, u32::MAX] {
        cmd(m, Command::AddFadeOut { bars });
        assert_eq!(steps(m).len(), before + 3);
    }
    // Undo, three times, takes the lane away again; each brings back the lane as it was.
    let effects = cmd(m, Command::Undo);
    assert_eq!(sent(&effects), [vec![(3 * BAR, fader), (4 * BAR, 0.0)]]);
    assert_eq!(points(m), [(3 * BAR, fader), (4 * BAR, 0.0)]);
    cmd(m, Command::Undo);
    assert_eq!(points(m), [(2 * BAR, fader), (4 * BAR, 0.0)]);
    let effects = cmd(m, Command::Undo);
    assert_eq!(sent(&effects), [vec![]]);
    assert!(m.view().song.master.lanes.is_empty());
    let effects = cmd(m, Command::Redo);
    assert_eq!(sent(&effects), [vec![(2 * BAR, fader), (4 * BAR, 0.0)]]);
    cmd(m, Command::Redo);
    assert_eq!(points(m), [(3 * BAR, fader), (4 * BAR, 0.0)]);
    cmd(m, Command::Redo);
    assert_eq!(points(m), [(0, fader), (4 * BAR, 0.0)]);
}

/// The fade ends where the export's 「曲ぜんぶ」 ends: at the last clip's end, also when
/// that is inside a bar.
#[test]
fn a_fade_out_ends_where_the_whole_song_is_exported_to() {
    let exported = |m: &mut Model| {
        cmd(m, Command::OpenExport);
        let span = m.view().export.dialog.expect("the dialog").span;
        cmd(m, Command::CloseExport);
        span.end
    };
    let m = &mut with_clip();
    cmd(m, Command::AddFadeOut { bars: 2 });
    assert_eq!(points(m).last(), Some(&(4 * BAR, 0.0)));
    assert_eq!(exported(m), 4 * BAR);

    let m = &mut with_clip();
    let clip = m.view().song.tracks[0].clips[0].id;
    cmd(
        m,
        Command::TrimClip {
            clip,
            edge: meloet_protocol::ClipEdge::End,
            ticks: -((3 * Q) as i64),
        },
    );
    let end = m.view().song.tracks[0].clips[0].end;
    assert_eq!(end, 3 * BAR + Q, "inside bar 4");
    let fader = meloet_model::Gain::UNITY.position();
    cmd(m, Command::AddFadeOut { bars: 2 });
    assert_eq!(points(m), [(BAR + Q, fader), (end, 0.0)]);
    assert_eq!(exported(m), end);
    // Longer than the song: from its head.
    cmd(m, Command::AddFadeOut { bars: 4 });
    assert_eq!(points(m), [(0, fader), (end, 0.0)]);
}

#[test]
fn a_fade_out_again_over_points_drawn_by_hand_is_one_step() {
    let m = &mut with_clip();
    let fader = meloet_model::Gain::UNITY.position();
    cmd(m, Command::AddFadeOut { bars: 2 });
    for (tick, value) in [(BAR, 0.5), (2 * BAR + 2 * Q, 0.7)] {
        let lane = lane(m).lane_id;
        cmd(
            m,
            Command::AddAutoPoint {
                lane,
                tick,
                value: Some(value),
            },
        );
    }
    let half = 2 * BAR + 2 * Q;
    let drawn = [(BAR, 0.5), (2 * BAR, fader), (half, 0.7), (4 * BAR, 0.0)];
    assert_eq!(points(m), drawn);
    let before = steps(m).len();
    // A longer one: the point before its start stays, the ones from it on go.
    let effects = cmd(m, Command::AddFadeOut { bars: 3 });
    assert_eq!(points(m), [(BAR, 0.5), (4 * BAR, 0.0)]);
    assert_eq!(sent(&effects), [vec![(BAR, 0.5), (4 * BAR, 0.0)]]);
    assert_eq!(steps(m).len(), before + 1);
    cmd(m, Command::Undo);
    assert_eq!(points(m), drawn);
    cmd(m, Command::Redo);
    assert_eq!(points(m), [(BAR, 0.5), (4 * BAR, 0.0)]);
    cmd(m, Command::Undo);
    // A shorter one, on the lane's last line: from the drawn point's value; the points
    // before the start stay.
    let shorter = [
        (BAR, 0.5),
        (2 * BAR, fader),
        (half, 0.7),
        (3 * BAR, 0.7),
        (4 * BAR, 0.0),
    ];
    let effects = cmd(m, Command::AddFadeOut { bars: 1 });
    assert_eq!(points(m), shorter);
    assert_eq!(sent(&effects), [shorter.to_vec()]);
    assert_eq!(steps(m).len(), before + 1);
    cmd(m, Command::Undo);
    assert_eq!(points(m), drawn);
    cmd(m, Command::Redo);
    assert_eq!(points(m), shorter);
}

#[test]
fn a_fade_out_needs_a_clip_and_a_song_that_is_not_recording() {
    let m = &mut ready();
    let before = steps(m).len();
    let effects = cmd(m, Command::AddFadeOut { bars: 4 });
    assert_eq!(status(m), Some(StatusMessage::FadeOutNoClips));
    assert!(sent(&effects).is_empty());
    assert_eq!(steps(m).len(), before);
    assert!(m.view().song.master.lanes.is_empty());

    let m = &mut with_clip();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    crate::play::tests::started(m);
    let before = steps(m).len();
    cmd(m, Command::AddFadeOut { bars: 4 });
    assert_eq!(steps(m).len(), before);
    assert!(m.view().song.master.lanes.is_empty());
}

#[test]
fn a_fade_out_starts_where_the_master_sounds_and_wakes_a_lane_at_rest() {
    let m = &mut with_clip();
    // The fader by hand at −6 dB: the fade starts there.
    cmd(m, gain(-6.0, true));
    cmd(m, Command::AddFadeOut { bars: 1 });
    assert_eq!(points(m), [(3 * BAR, 0.6), (4 * BAR, 0.0)]);
    assert_eq!(
        master_gain(m),
        Some(-6.0),
        "the fader stays where the hand put it"
    );
    // The hand rests the lane; the next fade starts from the fader and wakes it.
    cmd(m, gain(0.0, false));
    cmd(m, gain(0.0, true));
    assert!(lane(m).resting);
    let effects = cmd(m, Command::AddFadeOut { bars: 2 });
    assert!(!lane(m).resting);
    assert_eq!(points(m), [(2 * BAR, 0.8), (4 * BAR, 0.0)]);
    assert_eq!(sent(&effects), [vec![(2 * BAR, 0.8), (4 * BAR, 0.0)]]);
    // One step: undo brings the rest and the old points back together.
    cmd(m, Command::Undo);
    assert!(lane(m).resting);
    assert_eq!(points(m), [(3 * BAR, 0.6), (4 * BAR, 0.0)]);
    // A lane that moves, with the fade on it: from the value before that fade.
    cmd(m, Command::Redo);
    cmd(m, Command::AddFadeOut { bars: 1 });
    assert_eq!(points(m), [(3 * BAR, 0.8), (4 * BAR, 0.0)]);
}

#[test]
fn empty_bars_put_into_the_song_move_the_fade_with_its_end() {
    let m = &mut with_clip();
    cmd(m, Command::AddFadeOut { bars: 2 });
    cmd(
        m,
        Command::AddMarker {
            name: "サビ".into(),
            tick: Some(BAR),
        },
    );
    let marker = m.view().timeline.renaming.unwrap();
    cmd(m, Command::CloseMarkerName);
    let before = lane(m).lane_id;
    let effects = cmd(m, Command::InsertSpace { marker, bars: 4 });
    let fader = meloet_model::Gain::UNITY.position();
    assert_eq!(points(m), [(6 * BAR, fader), (8 * BAR, 0.0)]);
    assert_eq!(sent(&effects), [vec![(6 * BAR, fader), (8 * BAR, 0.0)]]);
    assert_ne!(lane(m).lane_id, before);
    cmd(m, Command::Undo);
    assert_eq!(points(m), [(2 * BAR, fader), (4 * BAR, 0.0)]);
    assert_eq!(lane(m).lane_id, before);
    m.play().song().doc.check_lanes().unwrap();
}

#[test]
fn the_song_keeps_the_masters_lane_and_its_fold_through_the_file() {
    let m = &mut with_clip();
    cmd(m, Command::AddFadeOut { bars: 2 });
    cmd(
        m,
        Command::FoldLane {
            home: MASTER,
            target: VOLUME,
            folded: true,
        },
    );
    let song = m.play().song().clone();
    assert!(song.session.master_lane_folded);
    let bytes = songfile::encode(&song);
    let back = songfile::decode(&bytes).unwrap();
    let held = |s: &meloet_model::Song| {
        (s.doc.master_lanes.iter())
            .map(|l| (l.target, l.step, l.resting, l.points.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(held(&back), held(&song));
    assert!(back.session.master_lane_folded);
    assert!(back.session.folded_lanes.is_empty());
    back.doc.check_lanes().unwrap();
    // A fold that names a master's lane the song does not have is skipped.
    cmd(
        m,
        Command::DeleteLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    assert!(m.view().automation.folded.is_empty());
    let mut song = m.play().song().clone();
    song.session.master_lane_folded = true;
    let back = songfile::decode(&songfile::encode(&song)).unwrap();
    assert!(!back.session.master_lane_folded);
    // Undo brings the lane back unfolded: the fold is no part of a step.
    cmd(m, Command::Undo);
    assert!(m.view().automation.folded.is_empty());
}

#[test]
fn a_new_stream_is_told_the_masters_lane_and_the_hand_on_it() {
    let m = &mut ready();
    with_lane(m);
    cmd(m, gain(-6.0, false));
    let mut effects = Vec::new();
    m.play_mut().stream_opened(&mut effects);
    assert_eq!(sent(&effects), [vec![(0, 0.5), (8 * BAR, 0.5)]]);
    assert_eq!(holds(&effects), [true]);
    // The engine is the same one from stream to stream: a lane that went while none ran
    // is taken from it by the next.
    cmd(m, Command::CancelMixPreview);
    cmd(
        m,
        Command::DeleteLane {
            home: MASTER,
            target: VOLUME,
        },
    );
    let mut effects = Vec::new();
    m.play_mut().stream_opened(&mut effects);
    assert_eq!(sent(&effects), [vec![]]);
    assert!(holds(&effects).is_empty());
}
