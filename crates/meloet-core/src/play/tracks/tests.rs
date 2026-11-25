use meloet_protocol::{Command, StatusMessage, TrackRef, TransportState};

use crate::play::EngineOp;
use crate::play::mix::tests::three_tracks;
use crate::play::tests::{cmd, ready, status};
use crate::state::{Effect, Model};

/// (id, number, name, color) of every track.
fn tracks(m: &Model) -> Vec<(u32, u32, String, u8)> {
    m.view()
        .song
        .tracks
        .iter()
        .map(|t| (t.id, t.number, t.name.clone(), t.color))
        .collect()
}

fn ids(m: &Model) -> Vec<u32> {
    tracks(m).iter().map(|t| t.0).collect()
}

fn engine_ops(effects: &[Effect]) -> Vec<EngineOp> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

fn track_ref(number: u32) -> TrackRef {
    TrackRef {
        number,
        name: format!("トラック {number}"),
    }
}

#[test]
fn deleting_a_track_is_a_step_that_says_so_and_renumbers() {
    let m = &mut ready();
    three_tracks(m);
    let effects = cmd(m, Command::DeleteTrack { track: 2 });
    assert_eq!(
        status(m),
        Some(StatusMessage::TrackDeleted {
            track: track_ref(2)
        })
    );
    assert_eq!(
        tracks(m),
        [
            (1, 1, "ピアノ".to_owned(), 1),
            (3, 2, "トラック 3".to_owned(), 3)
        ],
        "the number follows the place, the color stays"
    );
    assert_eq!(
        engine_ops(&effects),
        [EngineOp::ClearSeat { seat: 1 }],
        "its seat is emptied; track 3 stays on its own"
    );
    assert_eq!(m.view().song.undo_count, 1);
    assert_eq!(m.view().song.play_target, Some(1));

    cmd(m, Command::Undo);
    assert_eq!(ids(m), [1, 2, 3]);
    cmd(m, Command::Redo);
    assert_eq!(ids(m), [1, 3]);
    // A track that is gone.
    cmd(m, Command::DeleteTrack { track: 2 });
    assert_eq!(m.view().song.undo_count, 1);
}

#[test]
fn deleting_the_selected_track_selects_the_one_below_or_above() {
    let m = &mut ready();
    three_tracks(m);
    let target = |m: &Model| m.view().song.play_target;
    cmd(
        m,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );
    cmd(m, Command::DeleteTrack { track: 2 });
    assert_eq!(target(m), Some(3), "the one below");
    cmd(m, Command::DeleteTrack { track: 3 });
    assert_eq!(target(m), Some(1), "it was the last: the one above");
    cmd(m, Command::DeleteTrack { track: 1 });
    assert_eq!(target(m), None);
    assert!(m.view().song.tracks.is_empty());
    // Undo brings a track back, and there is a selected track again.
    cmd(m, Command::Undo);
    assert_eq!((ids(m), target(m)), (vec![1], Some(1)));
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert_eq!((ids(m), target(m)), (vec![1, 2, 3], Some(1)));
}

#[test]
fn a_step_that_takes_the_selected_track_away_selects_the_one_at_its_place_or_the_last() {
    let m = &mut ready();
    three_tracks(m);
    let target = |m: &Model| m.view().song.play_target;
    cmd(m, Command::DuplicateTrack { track: 2 });
    let copy = ids(m)[2];
    assert_eq!(target(m), Some(copy));
    cmd(m, Command::Undo);
    assert_eq!(
        (ids(m), target(m)),
        (vec![1, 2, 3], Some(3)),
        "the one below"
    );
    cmd(m, Command::DuplicateTrack { track: 3 });
    assert_eq!(target(m), Some(ids(m)[3]));
    cmd(m, Command::Undo);
    assert_eq!(target(m), Some(3), "it was the last: the one above");
}

#[test]
fn renaming_keeps_the_old_name_for_an_empty_one() {
    let m = &mut ready();
    three_tracks(m);
    let rename = |m: &mut Model, name: &str| {
        cmd(
            m,
            Command::RenameTrack {
                track: 2,
                name: name.into(),
            },
        );
        (tracks(m)[1].2.clone(), m.view().song.undo_count)
    };
    assert_eq!(rename(m, " うら "), ("うら".to_owned(), 1));
    assert_eq!(rename(m, ""), ("うら".to_owned(), 1));
    assert_eq!(rename(m, "  \u{3000}"), ("うら".to_owned(), 1));
    assert_eq!(rename(m, "うら"), ("うら".to_owned(), 1));
    cmd(m, Command::Undo);
    assert_eq!(tracks(m)[1].2, "トラック 2");
}

#[test]
fn moving_renumbers_keeps_colors_and_the_selected_track() {
    let m = &mut ready();
    three_tracks(m);
    let effects = cmd(m, Command::MoveTrack { track: 1, to: 2 });
    assert_eq!(
        tracks(m)
            .iter()
            .map(|t| (t.0, t.1, t.3))
            .collect::<Vec<_>>(),
        [(2, 1, 2), (3, 2, 3), (1, 3, 1)]
    );
    assert_eq!(m.view().song.play_target, Some(1));
    assert_eq!(
        engine_ops(&effects),
        [],
        "every track keeps its seat: the engine is told nothing"
    );
    assert_eq!(m.view().song.undo_count, 1);
    // Past the end is the end; where it is already is no step.
    cmd(m, Command::MoveTrack { track: 2, to: 99 });
    assert_eq!(ids(m), [3, 1, 2]);
    cmd(m, Command::MoveTrack { track: 2, to: 99 });
    cmd(m, Command::MoveTrack { track: 3, to: 0 });
    assert_eq!(m.view().song.undo_count, 2);
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert_eq!(ids(m), [1, 2, 3]);
}

#[test]
fn a_color_is_one_of_five() {
    let m = &mut ready();
    three_tracks(m);
    let color = |m: &mut Model, color: u8| {
        cmd(m, Command::SetTrackColor { track: 1, color });
        (tracks(m)[0].3, m.view().song.undo_count)
    };
    assert_eq!(color(m, 4), (4, 1));
    assert_eq!(color(m, 4), (4, 1));
    assert_eq!(color(m, 0), (4, 1));
    assert_eq!(color(m, 6), (4, 1));
    cmd(m, Command::Undo);
    assert_eq!(tracks(m)[0].3, 1);
}

#[test]
fn the_arm_button_only_selects_and_says_where_r_records() {
    let m = &mut ready();
    three_tracks(m);
    let arm = |m: &mut Model, track: u32| {
        cmd(
            m,
            Command::SelectTrack {
                track,
                from_arm: true,
            },
        );
        (m.view().song.play_target, status(m))
    };
    let moved = |number| {
        Some(StatusMessage::ArmMoved {
            track: track_ref(number),
        })
    };
    assert_eq!(arm(m, 2), (Some(2), moved(2)));
    // The ● of the selected track changes nothing, and says nothing again.
    let (target, said) = arm(m, 2);
    assert_eq!(target, Some(2));
    assert_ne!(said, moved(2));
    assert_eq!(arm(m, 9).0, Some(2), "a track that is not there");
    // A plain click selects without the sentence.
    cmd(
        m,
        Command::SelectTrack {
            track: 3,
            from_arm: false,
        },
    );
    assert_eq!(m.view().song.play_target, Some(3));
    assert_ne!(status(m), moved(3));
    // R records to the selected track.
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.recording.map(|r| r.track), Some(3));
}

#[test]
fn nothing_changes_the_tracks_while_counting_in() {
    let m = &mut ready();
    three_tracks(m);
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    let before = tracks(m);
    for command in [
        Command::DeleteTrack { track: 2 },
        Command::RenameTrack {
            track: 2,
            name: "うら".into(),
        },
        Command::MoveTrack { track: 2, to: 0 },
        Command::SetTrackColor { track: 2, color: 5 },
        Command::SelectTrack {
            track: 2,
            from_arm: true,
        },
    ] {
        cmd(m, command);
    }
    assert_eq!(tracks(m), before);
    assert_eq!(m.view().song.undo_count, 0);
    assert_eq!(m.view().song.play_target, Some(1));
    assert_eq!(m.view().transport.recording.map(|r| r.track), Some(1));
}
