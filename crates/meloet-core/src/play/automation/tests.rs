use meloet_model::automation::LaneId;
use meloet_model::{Gain, StepKind, TrackId};
use meloet_protocol::{
    AutoTarget, Command, IndexRange, LanePoints, LaneRef, StatusMessage, StripRef, TrackRef,
    TransportState,
};

use super::place;
use crate::play::tests::{cmd, hear, ready, status};
use crate::songfile;
use crate::state::Model;

const T1: StripRef = StripRef::Track { id: 1 };
const BAR: u64 = 1920;
const VOLUME: AutoTarget = AutoTarget::Volume;

fn volume_ref() -> LaneRef {
    LaneRef {
        track: 1,
        target: VOLUME,
    }
}

fn gain(db: f32, commit: bool) -> Command {
    Command::SetGain {
        strip: T1,
        db: Some(db),
        commit,
    }
}

fn steps(m: &Model) -> Vec<StepKind> {
    m.play().steps()
}

fn last_step(m: &Model) -> StepKind {
    *steps(m).last().expect("a step")
}

fn lane_id(m: &Model, target: AutoTarget) -> u32 {
    let view = m.view();
    let lanes = &view.song.tracks[0].lanes;
    lanes
        .iter()
        .find(|l| l.target == target)
        .expect("the lane")
        .lane_id
}

fn points(m: &Model, target: AutoTarget) -> Vec<(u64, f32)> {
    let bytes = m.play().lane(lane_id(m, target)).expect("the points");
    let lane = LanePoints::decode(&bytes).unwrap();
    lane.points.iter().map(|p| (p.tick, p.value)).collect()
}

fn resting(m: &Model, target: AutoTarget) -> bool {
    let view = m.view();
    let lanes = &view.song.tracks[0].lanes;
    lanes.iter().find(|l| l.target == target).unwrap().resting
}

fn track_gain(m: &Model) -> Option<f32> {
    m.view().song.tracks[0].gain_db
}

/// A volume lane on track 1 that goes from 0.5 at the head to 0.5 at bar 9, flat.
fn with_volume_lane(m: &mut Model) {
    cmd(
        m,
        Command::AddLane {
            track: 1,
            target: VOLUME,
        },
    );
    for tick in [0, 8 * BAR] {
        let lane = lane_id(m, VOLUME);
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

fn play_at(m: &mut Model, t_us: u64, tick: f64) {
    hear(m, t_us, tick, TransportState::Playing, 0);
}

fn piano() -> TrackRef {
    TrackRef {
        number: 1,
        name: "ピアノ".into(),
    }
}

#[test]
fn the_knob_places_match_the_screen() {
    // ui/src/components/mixer/scale.ts: +6 is 1, 0 dB 0.8, −60 is 0, −∞ is 0.
    assert_eq!(place::of_gain(Gain::from_db(6.0)), 1.0);
    assert!((place::of_gain(Gain::UNITY) - 0.8).abs() < 1e-6);
    assert!((place::of_gain(Gain::from_db(-6.0)) - 0.6).abs() < 1e-6);
    assert_eq!(place::of_gain(Gain::from_db(-60.0)), 0.0);
    assert_eq!(place::of_gain(Gain::SILENT), 0.0);
    assert_eq!(place::gain_at(0.0), Gain::SILENT);
    assert_eq!(place::gain_at(0.8), Gain::UNITY);
    assert_eq!(place::gain_at(0.6), Gain::from_db(-6.0));
    use meloet_model::effects::{EffectKind, compressor};
    let ratio = place::of_param(EffectKind::Compressor, compressor::RATIO, 4.0);
    let back = place::param_at(EffectKind::Compressor, compressor::RATIO, ratio).unwrap();
    assert!((back - 4.0).abs() < 1e-3);
}

#[test]
fn lanes_and_points_are_steps_and_the_selection_comes_back_with_undo() {
    let m = &mut ready();
    let before = steps(m).len();
    cmd(
        m,
        Command::AddLane {
            track: 1,
            target: VOLUME,
        },
    );
    assert_eq!(steps(m).len(), before + 1);
    let view = m.view();
    let lane = &view.song.tracks[0].lanes[0];
    assert_eq!((lane.points, lane.step, lane.resting), (0, false, false));
    let first_id = lane.lane_id;
    // A lane that is there already is unfolded, which is no step.
    cmd(
        m,
        Command::FoldLane {
            track: 1,
            target: VOLUME,
            folded: true,
        },
    );
    assert_eq!(m.view().automation.folded, [volume_ref()]);
    cmd(
        m,
        Command::AddLane {
            track: 1,
            target: VOLUME,
        },
    );
    assert!(m.view().automation.folded.is_empty());
    assert_eq!(steps(m).len(), before + 1);

    // A point on the grid (1/16 = 120 ticks), the line's value without one: the hand's.
    cmd(
        m,
        Command::AddAutoPoint {
            lane: first_id,
            tick: 130,
            value: None,
        },
    );
    assert_eq!(points(m, VOLUME), [(120, 0.8)]);
    let second_id = lane_id(m, VOLUME);
    assert_ne!(second_id, first_id);
    let selected = m.view().automation.selected.unwrap();
    assert_eq!(selected.lane_id, second_id);
    assert_eq!(selected.points, [IndexRange { start: 0, end: 1 }]);
    // An old id does nothing.
    cmd(
        m,
        Command::AddAutoPoint {
            lane: first_id,
            tick: 960,
            value: Some(0.1),
        },
    );
    assert_eq!(points(m, VOLUME).len(), 1);
    assert_eq!(m.play().lane(first_id), None);

    cmd(
        m,
        Command::AddAutoPoint {
            lane: second_id,
            tick: 960,
            value: Some(0.2),
        },
    );
    let third_id = lane_id(m, VOLUME);
    cmd(
        m,
        Command::MoveAutoPoints {
            lane: third_id,
            points: vec![1],
            d_tick: 240,
            d_value: 0.1,
        },
    );
    let got = points(m, VOLUME);
    assert_eq!(got[1].0, 1200);
    assert!((got[1].1 - 0.3).abs() < 1e-6);
    assert_eq!(
        last_step(m),
        StepKind::MoveAutoPoints {
            track: TrackId(1),
            target: meloet_model::AutoTarget::Volume,
            count: 1
        }
    );
    let moved = m.view().automation.selected.unwrap();
    assert_eq!(moved.points, [IndexRange { start: 1, end: 2 }]);

    // Undo: the points, their id and the selection as they were.
    cmd(m, Command::Undo);
    assert_eq!(lane_id(m, VOLUME), third_id);
    let back = m.view().automation.selected.unwrap();
    assert_eq!(
        (back.lane_id, back.points),
        (third_id, vec![IndexRange { start: 1, end: 2 }])
    );
    cmd(m, Command::Redo);
    let id = lane_id(m, VOLUME);
    cmd(
        m,
        Command::SelectAutoPoints {
            lane: id,
            points: vec![0, 1, 9],
        },
    );
    assert_eq!(
        m.view().automation.selected.unwrap().points,
        [IndexRange { start: 0, end: 2 }]
    );
    cmd(
        m,
        Command::DeleteAutoPoints {
            lane: id,
            points: vec![0],
        },
    );
    assert_eq!(points(m, VOLUME).len(), 1);
    assert_eq!(m.view().automation.selected, None);
    cmd(
        m,
        Command::DeleteLane {
            track: 1,
            target: VOLUME,
        },
    );
    assert!(m.view().song.tracks[0].lanes.is_empty());
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks[0].lanes.len(), 1);
}

#[test]
fn a_deleted_lane_leaves_the_value_where_the_line_was() {
    let m = &mut ready();
    with_volume_lane(m);
    let lane = lane_id(m, VOLUME);
    // From 0.6 (−6 dB) at the head.
    cmd(
        m,
        Command::MoveAutoPoints {
            lane,
            points: vec![0],
            d_tick: 0,
            d_value: 0.1,
        },
    );
    cmd(
        m,
        Command::DeleteLane {
            track: 1,
            target: VOLUME,
        },
    );
    assert_eq!(track_gain(m), Some(-6.0));
}

#[test]
fn a_hand_on_a_moving_fader_rests_its_lane_with_the_value_in_one_step() {
    let m = &mut ready();
    with_volume_lane(m);
    let before = steps(m).len();
    for db in [-2.0, -4.0, -6.0] {
        cmd(m, gain(db, false));
    }
    assert_eq!(steps(m).len(), before);
    cmd(m, gain(-6.0, true));
    assert_eq!(steps(m).len(), before + 1);
    assert_eq!(
        last_step(m),
        StepKind::SetGain {
            strip: meloet_model::StripId::Track(TrackId(1))
        }
    );
    assert!(resting(m, VOLUME));
    assert_eq!(track_gain(m), Some(-6.0));
    assert!(matches!(
        status(m),
        Some(StatusMessage::AutomationRested { track, target: meloet_protocol::LaneName::Volume })
            if track == piano()
    ));
    // At rest, a hand is a step as before, and the lane stays at rest.
    cmd(m, gain(-3.0, false));
    cmd(m, gain(-3.0, true));
    assert_eq!(steps(m).len(), before + 2);
    assert!(resting(m, VOLUME));
    // 「もどす」 is a step.
    let effects = cmd(
        m,
        Command::RestoreLane {
            track: 1,
            target: VOLUME,
        },
    );
    drop(effects);
    assert!(!resting(m, VOLUME));
    assert!(matches!(
        status(m),
        Some(StatusMessage::AutomationRestored { .. })
    ));
    // Undo takes back the restore, the value, then the value and the rest together.
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert!(resting(m, VOLUME));
    cmd(m, Command::Undo);
    assert!(!resting(m, VOLUME));
    assert_eq!(track_gain(m), Some(0.0));
}

#[test]
fn esc_lets_the_lane_move_again_with_no_step() {
    let m = &mut ready();
    with_volume_lane(m);
    let before = steps(m).len();
    cmd(m, gain(-6.0, false));
    cmd(m, Command::CancelMixPreview);
    assert_eq!(steps(m).len(), before);
    assert!(!resting(m, VOLUME));
}

#[test]
fn a_typed_value_rests_the_lane_while_writing_too() {
    let m = &mut ready();
    with_volume_lane(m);
    cmd(
        m,
        Command::SetLaneWrite {
            track: 1,
            target: VOLUME,
            on: true,
        },
    );
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, 960.0);
    cmd(m, gain(-6.0, true));
    assert!(resting(m, VOLUME));
    assert_eq!(track_gain(m), Some(-6.0));
    assert!(
        m.view().automation.armed.is_empty(),
        "a lane at rest is not armed"
    );
}

#[test]
fn writing_puts_what_was_heard_where_it_was_heard_and_is_one_step() {
    let m = &mut ready();
    with_volume_lane(m);
    cmd(
        m,
        Command::SetLaneWrite {
            track: 1,
            target: VOLUME,
            on: true,
        },
    );
    assert_eq!(m.view().automation.armed, [volume_ref()]);
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, 960.0);
    assert!(matches!(
        status(m),
        Some(StatusMessage::AutomationWriting { .. })
    ));
    let before = steps(m).len();
    cmd(m, gain(-6.0, false));
    let view = m.view().automation;
    assert_eq!(view.writing.len(), 1);
    assert_eq!(view.writing[0].from_tick, 960);
    play_at(m, 2_000, 1080.0);
    cmd(m, gain(-6.0, false));
    play_at(m, 3_000, 1200.0);
    cmd(m, gain(0.0, false));
    play_at(m, 4_000, 1320.0);
    cmd(m, gain(0.0, true));
    assert_eq!(steps(m).len(), before + 1);
    assert_eq!(
        last_step(m),
        StepKind::WriteAutomation {
            track: TrackId(1),
            target: meloet_model::AutoTarget::Volume
        }
    );
    assert_eq!(track_gain(m), Some(0.0), "the hand's value stays");
    let got = points(m, VOLUME);
    let near = |a: f32, b: f32| (a - b).abs() < 1e-6;
    let want = [
        (0, 0.5),
        (959, 0.5),
        (960, 0.6),
        (1080, 0.6),
        (1200, 0.8),
        (1320, 0.8),
        (1321, 0.5),
        (8 * BAR, 0.5),
    ];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(g.0 == w.0 && near(g.1, w.1), "{got:?}");
    }
    assert!(m.view().automation.writing.is_empty());
    assert!(!resting(m, VOLUME));
    // Undo brings the lane back as it was.
    cmd(m, Command::Undo);
    assert_eq!(points(m, VOLUME), [(0, 0.5), (8 * BAR, 0.5)]);
}

#[test]
fn stopping_ends_the_writing_and_the_rest_of_the_drag_goes_nowhere() {
    let m = &mut ready();
    with_volume_lane(m);
    cmd(
        m,
        Command::SetLaneWrite {
            track: 1,
            target: VOLUME,
            on: true,
        },
    );
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, 960.0);
    let before = steps(m).len();
    cmd(m, gain(-6.0, false));
    play_at(m, 2_000, 1080.0);
    cmd(m, gain(-6.0, false));
    cmd(m, Command::PlayStop);
    assert_eq!(
        steps(m).len(),
        before + 1,
        "what was written until the stop"
    );
    assert!(m.view().automation.armed.is_empty());
    cmd(m, gain(-12.0, false));
    cmd(m, gain(-12.0, true));
    assert_eq!(steps(m).len(), before + 1, "the rest is not in the song");
    assert_eq!(track_gain(m), Some(0.0));
    assert!(!resting(m, VOLUME));
}

#[test]
fn a_loop_written_over_is_kept_pass_by_pass() {
    let m = &mut ready();
    with_volume_lane(m);
    cmd(
        m,
        Command::SetLoop {
            from_bar: 1,
            to_bar: 3,
        },
    );
    cmd(m, Command::ToggleLoop);
    cmd(
        m,
        Command::SetLaneWrite {
            track: 1,
            target: VOLUME,
            on: true,
        },
    );
    cmd(m, Command::PlayStop);
    let lap = |m: &mut Model, t: u64, tick: f64, lap: u32| {
        hear(m, t, tick, TransportState::Playing, lap);
    };
    lap(m, 1_000, 3000.0, 0);
    cmd(m, gain(-6.0, false));
    lap(m, 2_000, 3600.0, 0);
    cmd(m, gain(-6.0, false));
    lap(m, 3_000, 240.0, 1);
    cmd(m, gain(0.0, false));
    assert_eq!(
        m.view().automation.writing[0].from_tick,
        0,
        "from the loop's start"
    );
    lap(m, 4_000, 3300.0, 1);
    cmd(m, gain(0.0, true));
    let got = points(m, VOLUME);
    // The second pass ran over the first from 240 to 3300: the first is left at 3600 only.
    let ticks: Vec<u64> = got.iter().map(|p| p.0).collect();
    assert_eq!(
        ticks,
        [0, 239, 240, 3300, 3301, 3600, 3601, 8 * BAR],
        "{got:?}"
    );
}

#[test]
fn the_mute_writes_a_point_or_rests() {
    let m = &mut ready();
    let mute = AutoTarget::Mute;
    cmd(
        m,
        Command::AddLane {
            track: 1,
            target: mute,
        },
    );
    let lane = lane_id(m, mute);
    cmd(
        m,
        Command::AddAutoPoint {
            lane,
            tick: 0,
            value: Some(0.0),
        },
    );
    cmd(
        m,
        Command::SetLaneWrite {
            track: 1,
            target: mute,
            on: true,
        },
    );
    cmd(m, Command::PlayStop);
    play_at(m, 1_000, 1000.0);
    cmd(m, Command::ToggleMute { track: 1 });
    assert_eq!(points(m, mute), [(0, 0.0), (1000, 1.0)]);
    assert!(!m.view().song.tracks[0].muted, "the mute itself stays");
    cmd(m, Command::PlayStop);
    cmd(m, Command::ToggleMute { track: 1 });
    assert!(m.view().song.tracks[0].muted);
    assert!(resting(m, mute));
}

#[test]
fn a_song_keeps_its_lanes_and_folds_through_the_file() {
    let m = &mut ready();
    with_volume_lane(m);
    cmd(
        m,
        Command::AddLane {
            track: 1,
            target: AutoTarget::Pan,
        },
    );
    cmd(
        m,
        Command::FoldLane {
            track: 1,
            target: AutoTarget::Pan,
            folded: true,
        },
    );
    cmd(m, gain(-6.0, false));
    cmd(m, gain(-6.0, true));
    let song = m.play().song().clone();
    let bytes = songfile::encode(&song);
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains(r#""resting":true,"step":false,"target":{"kind":"volume"}"#),
        "{text}"
    );
    let back = songfile::decode(&bytes).unwrap();
    let lanes = |s: &meloet_model::Song| {
        (s.doc.tracks[0].automation.iter())
            .map(|l| (l.target, l.step, l.resting, l.points.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(lanes(&back), lanes(&song));
    assert_eq!(back.session.folded_lanes, song.session.folded_lanes);
    back.doc.check_lanes().unwrap();
    // The ids are given anew, above every other.
    assert!(
        back.doc.tracks[0]
            .automation
            .iter()
            .all(|l| l.id < LaneId(back.doc.next_id))
    );
}

mod random {
    use std::path::PathBuf;

    use meloet_model::effects::{EffectKind, compressor};
    use meloet_model::{Song, StripId, TrackId, mixer};
    use meloet_protocol::{AutoTarget, Command, SendBus, StripRef, TransportState};

    use crate::play::tests::{cmd, hear, ready};
    use crate::songfile;
    use crate::state::{Input, Model};

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

    /// A new song with a compressor in the first slot of track 1, opened.
    fn with_compressor(m: &mut Model) -> u32 {
        let mut song = Song::new_default();
        let strip = StripId::Track(TrackId(1));
        song.doc = mixer::add_effect(&song.doc, strip, 0, EffectKind::Compressor)
            .unwrap()
            .doc;
        let insert = song.doc.insert(strip, 0).unwrap().id.0;
        cmd(m, Command::OpenSong);
        let path = PathBuf::from("/nowhere/曲/曲.meloet");
        m.handle(Input::SongFilePicked(Some(path.clone())));
        m.handle(Input::SongRead {
            path,
            song: Box::new(song),
            from_previous: false,
        });
        insert
    }

    type Held = (meloet_model::AutoTarget, bool, bool, Vec<(u64, f32)>);

    fn lanes(song: &Song) -> Vec<Held> {
        (song.doc.tracks[0].automation.iter())
            .map(|l| {
                let points = l.points.iter().map(|p| (p.tick, p.value)).collect();
                (l.target, l.step, l.resting, points)
            })
            .collect()
    }

    /// Lanes, points, hands that rest and write, the transport, undo and redo, at random:
    /// the song keeps every lane with its target, and the file gives back what it had.
    #[test]
    fn lanes_hands_and_undo_at_random_survive_the_file() {
        let m = &mut ready();
        let insert = with_compressor(m);
        let mut rng = Rng(0xa070_1a7e);
        let targets = [
            AutoTarget::Volume,
            AutoTarget::Pan,
            AutoTarget::Mute,
            AutoTarget::Send { bus: SendBus::A },
            AutoTarget::Insert {
                insert,
                param: compressor::THRESHOLD,
            },
        ];
        let mut tick = 0.0;
        let mut t_us = 1_000;
        let mut playing = false;
        let mut seen = std::collections::HashMap::new();
        for round in 0..1500 {
            let target = targets[rng.below(targets.len() as u64) as usize];
            let view = m.view();
            let lanes_now = &view.song.tracks[0].lanes;
            let lane = lanes_now.get(rng.below(lanes_now.len() as u64 + 1) as usize);
            let lane_id = lane.map_or(1, |l| l.lane_id);
            let n = lane.map_or(1, |l| l.points.max(1) as u64);
            let value = rng.below(101) as f32 / 100.0;
            let command = match rng.below(20) {
                0 | 1 => Command::AddLane { track: 1, target },
                2 => Command::DeleteLane { track: 1, target },
                3..=5 => Command::AddAutoPoint {
                    lane: lane_id,
                    tick: rng.below(64) * 120,
                    value: Some(value),
                },
                6 => Command::MoveAutoPoints {
                    lane: lane_id,
                    points: vec![rng.below(n) as u32, rng.below(n) as u32],
                    d_tick: rng.below(9) as i64 * 120 - 480,
                    d_value: value - 0.5,
                },
                7 => Command::DeleteAutoPoints {
                    lane: lane_id,
                    points: vec![rng.below(n) as u32],
                },
                8 => Command::SetLaneWrite {
                    track: 1,
                    target,
                    on: rng.below(3) > 0,
                },
                9 => Command::RestoreLane { track: 1, target },
                10 => Command::FoldLane {
                    track: 1,
                    target,
                    folded: rng.below(2) == 0,
                },
                11..=13 => Command::SetGain {
                    strip: StripRef::Track { id: 1 },
                    db: Some(-(rng.below(30) as f32)),
                    commit: rng.below(4) == 0,
                },
                14 => Command::SetParam {
                    insert: meloet_protocol::InsertRef {
                        strip: StripRef::Track { id: 1 },
                        slot: 0,
                    },
                    effect: meloet_protocol::EffectKind::Compressor,
                    param: compressor::THRESHOLD,
                    value: -(rng.below(40) as f32),
                    commit: rng.below(3) == 0,
                },
                15 => Command::ToggleMute { track: 1 },
                16 => Command::CancelMixPreview,
                17 => {
                    playing = !playing;
                    Command::PlayStop
                }
                18 => Command::Undo,
                _ => Command::Redo,
            };
            let made = m.play().steps().len();
            cmd(m, command);
            if m.play().steps().len() > made {
                let kind = *m.play().steps().last().unwrap();
                *seen.entry(std::mem::discriminant(&kind)).or_insert(0) += 1;
            }
            if playing {
                tick += 37.0;
                t_us += 10_000;
                hear(m, t_us, tick, TransportState::Playing, 0);
            }
            (m.play().song().doc.check_lanes()).unwrap_or_else(|e| panic!("round {round}: {e}"));
        }
        if playing {
            cmd(m, Command::PlayStop);
        }
        use meloet_model::{AutoTarget as T, StepKind as K};
        let (t, v) = (TrackId(1), T::Volume);
        for kind in [
            K::WriteAutomation {
                track: t,
                target: v,
            },
            K::RestoreLane {
                track: t,
                target: v,
            },
            K::MoveAutoPoints {
                track: t,
                target: v,
                count: 1,
            },
            K::SetGain {
                strip: StripId::Track(t),
            },
        ] {
            let n = seen
                .get(&std::mem::discriminant(&kind))
                .copied()
                .unwrap_or(0);
            assert!(n > 3, "{kind:?}: {n} times");
        }
        let song = m.play().song().clone();
        assert!(!lanes(&song).is_empty(), "the test made lanes");
        let back = songfile::decode(&songfile::encode(&song)).unwrap();
        assert_eq!(lanes(&back), lanes(&song));
        assert_eq!(back.session.folded_lanes, song.session.folded_lanes);
        // Every step taken back: no lane is left.
        for _ in 0..3000 {
            cmd(m, Command::Undo);
        }
        assert!(m.play().song().doc.tracks[0].automation.is_empty());
    }
}

mod plugins {
    use meloet_model::{AutoTarget as Target, PluginPlace, StepKind, TrackId};
    use meloet_protocol::{AutoTarget, Command, LaneRef, LaneView, TransportState};

    use crate::play::Heard;
    use crate::play::windows::tests::{FRESH, LEVEL, MODE, Rig};
    use crate::state::PluginSaid;

    fn heard(tick: f64) -> Option<Heard> {
        Some(Heard {
            tick,
            state: TransportState::Playing,
            lap: 0,
        })
    }

    /// Surge XT on track 1, its panel showing, and a lane of its level with one point.
    fn surge_with_lane(r: &mut Rig) -> (u64, u32, meloet_protocol::PluginSlotRef) {
        let (surge, track, slot) = r.surge();
        r.cmd(Command::SelectInstrument { track });
        r.settle();
        let target = AutoTarget::Instrument { param: LEVEL };
        r.cmd(Command::AddLane { track, target });
        let lane = r.m.view().song.tracks[0].lanes[0].lane_id;
        r.cmd(Command::AddAutoPoint {
            lane,
            tick: 0,
            value: Some(0.3),
        });
        (surge, track, slot)
    }

    fn level_lane(r: &Rig) -> LaneView {
        r.m.view().song.tracks[0].lanes[0].clone()
    }

    #[test]
    fn a_lane_of_a_plugin_is_named_by_the_panel() {
        let mut r = Rig::new();
        let (_, track, _) = surge_with_lane(&mut r);
        let lane = level_lane(&r);
        assert!(!lane.step);
        assert_eq!(
            lane.name,
            meloet_protocol::LaneName::Plugin {
                plugin: "Surge XT".into(),
                param: Some("Level".into()),
            }
        );
        let target = AutoTarget::Instrument { param: MODE };
        r.cmd(Command::AddLane { track, target });
        assert!(
            r.m.view().song.tracks[0].lanes[1].step,
            "the mode comes in steps"
        );
    }

    #[test]
    fn a_hand_in_the_window_rests_the_lane_with_the_visits_step() {
        let mut r = Rig::new();
        let (surge, track, _) = surge_with_lane(&mut r);
        let place = PluginPlace::Instrument(TrackId(track));
        let steps = r.undo_count();
        r.said(surge, PluginSaid::WindowEntered);
        r.settle();
        r.m.play_mut()
            .plugin_touched(surge, LEVEL, true, &mut Vec::new());
        let held = LaneRef {
            track,
            target: AutoTarget::Instrument { param: LEVEL },
        };
        assert_eq!(r.m.view().automation.held, [held]);
        let turned = r.host.turn(surge, LEVEL, 0.9);
        r.go(turned);
        r.m.play_mut()
            .plugin_touched(surge, LEVEL, false, &mut Vec::new());
        r.m.play_mut().plugin_hands(surge, 1);
        r.said(surge, PluginSaid::WindowLeft);
        r.settle();
        assert_eq!(
            r.undo_count(),
            steps + 1,
            "one step for the window and the rest"
        );
        let last = r.m.play().steps().last().copied();
        assert_eq!(last, Some(StepKind::PluginWindow { place }));
        assert!(level_lane(&r).resting);
        assert!(r.m.view().automation.held.is_empty());
        r.cmd(Command::Undo);
        r.settle();
        assert!(!level_lane(&r).resting);
        assert_eq!(r.host.plugins[&surge], FRESH);
    }

    #[test]
    fn a_visit_with_no_hand_is_no_step_whatever_the_bytes_say() {
        let mut r = Rig::new();
        let (surge, ..) = surge_with_lane(&mut r);
        let steps = r.undo_count();
        r.said(surge, PluginSaid::WindowEntered);
        r.settle();
        // The lane moved the plugin while the person was in the window.
        let moved = r.host.turn(surge, LEVEL, 0.3);
        r.go(moved);
        r.m.play_mut().plugin_hands(surge, 0);
        r.said(surge, PluginSaid::WindowLeft);
        r.settle();
        assert_eq!(r.undo_count(), steps);
    }

    #[test]
    fn writing_from_the_window_is_its_own_step_at_once() {
        let mut r = Rig::new();
        let (surge, track, _) = surge_with_lane(&mut r);
        let target = AutoTarget::Instrument { param: LEVEL };
        r.cmd(Command::SetLaneWrite {
            track,
            target,
            on: true,
        });
        r.m.play_mut().state = TransportState::Playing;
        let steps = r.undo_count();
        r.said(surge, PluginSaid::WindowEntered);
        r.settle();
        let play = r.m.play_mut();
        play.plugin_touched(surge, LEVEL, true, &mut Vec::new());
        play.plugin_edited(surge, LEVEL, 0.8, heard(960.0));
        play.plugin_edited(surge, LEVEL, 0.2, heard(1440.0));
        let writing = r.m.view().automation.writing;
        assert_eq!(writing.len(), 1);
        assert_eq!(writing[0].from_tick, 960);
        r.m.play_mut()
            .plugin_touched(surge, LEVEL, false, &mut Vec::new());
        assert_eq!(r.undo_count(), steps + 1);
        let last = r.m.play().steps().last().copied();
        assert_eq!(
            last,
            Some(StepKind::WriteAutomation {
                track: TrackId(track),
                target: Target::Instrument { param: LEVEL }
            })
        );
        let lane = level_lane(&r);
        assert!(!lane.resting);
        assert_eq!(lane.points, 5, "0, 959, 960, 1440, 1441");
    }

    #[test]
    fn a_drag_in_the_panel_rests_the_lane_with_the_plugins_step() {
        let mut r = Rig::new();
        let (_, track, slot) = surge_with_lane(&mut r);
        let place = PluginPlace::Instrument(TrackId(track));
        let table = r.selected().table.expect("its table");
        let steps = r.undo_count();
        for (value, commit) in [(0.6, false), (0.7, false), (0.7, true)] {
            r.cmd(Command::SetPluginParam {
                slot,
                table,
                param: LEVEL,
                value,
                commit,
            });
        }
        r.settle();
        assert_eq!(r.undo_count(), steps + 1);
        let last = r.m.play().steps().last().copied();
        assert_eq!(
            last,
            Some(StepKind::SetPluginParam {
                place,
                param: LEVEL
            })
        );
        assert!(level_lane(&r).resting);
        r.cmd(Command::Undo);
        r.settle();
        assert!(!level_lane(&r).resting);
    }
}

mod file {
    use crate::songfile::{self, ReadError};

    fn song(automation: &str) -> Vec<u8> {
        let head = r#"{"format":"meloet-song","version":5,"app":"t","song":{"name":"x",
            "tempo":9600,"meter":[4,4],"nextId":10,"tracks":[{"id":1,"name":"a","color":1,
            "instrument":{"kind":"builtin","name":"sine"},"gainDb":0,"pan":0,
            "inserts":[{"id":5,"effect":"compressor","params":{}},null,null,null],
            "clips":[],"automation":"#;
        let tail = r#"}]},"takes":[],"session":{"playTarget":1,"loopOn":false,
            "foldedLanes":[{"track":1,"target":{"kind":"pan"}},
                           {"track":1,"target":{"kind":"bogus"}}]}}"#;
        format!("{head}{automation}{tail}").into_bytes()
    }

    #[test]
    fn what_is_not_understood_is_skipped_and_what_breaks_a_rule_is_broken() {
        let read = songfile::decode(&song(
            r#"[{"target":{"kind":"pan"},"step":false,"resting":true,"points":[[0,0.5],[960,1]]},
               {"target":{"kind":"insert","insert":5,"param":"ratio"},"step":true,
                "points":[[0,0.2]]},
               {"target":{"kind":"insert","insert":6,"param":"ratio"},"points":[]},
               {"target":{"kind":"later"},"points":[]},
               {"target":{"kind":"insert","insert":5,"param":"nothing"},"points":[]}]"#,
        ))
        .unwrap();
        let lanes = &read.doc.tracks[0].automation;
        assert_eq!(lanes.len(), 2);
        assert!(lanes[0].resting);
        assert_eq!(lanes[0].points.len(), 2);
        assert!(!lanes[1].step, "the table says the ratio is smooth");
        assert_eq!(read.session.folded_lanes.len(), 1);
        assert!(lanes.iter().all(|l| l.id.0 >= 10));
        read.doc.check_lanes().unwrap();
        for bad in [
            r#"[{"target":{"kind":"pan"},"points":[[960,0.5],[0,1]]}]"#,
            r#"[{"target":{"kind":"pan"},"points":[[0,0.5],[0,1]]}]"#,
            r#"[{"target":{"kind":"pan"},"points":[[0,1.5]]}]"#,
            r#"[{"target":{"kind":"pan"},"points":[]},{"target":{"kind":"pan"},"points":[]}]"#,
        ] {
            assert!(
                matches!(songfile::decode(&song(bad)), Err(ReadError::Broken)),
                "{bad}"
            );
        }
    }
}
