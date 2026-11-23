use meloet_protocol::{Command, IndexRange, StatusMessage, TakeNotes};

use crate::play::tests::{cmd, ready, record_once, status};
use crate::state::Model;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;

fn shown_take(m: &Model) -> u32 {
    m.view().song.tracks[0].clips[0].take_id
}

fn notes(m: &Model) -> TakeNotes {
    TakeNotes::decode(&m.take_notes(shown_take(m)).unwrap()).unwrap()
}

/// Three notes a quarter apart, and the first two selected.
fn three_notes() -> Model {
    let mut m = ready();
    record_once(
        &mut m,
        &[
            (60, 10.0, 100.0),
            (64, (Q + 10) as f64, (Q + 100) as f64),
            (67, (2 * Q + 10) as f64, (2 * Q + 100) as f64),
        ],
        (5 * Q / 2) as f64,
    );
    let take = shown_take(&m);
    cmd(
        &mut m,
        Command::SelectNotes {
            take,
            notes: vec![0, 1],
            mode: meloet_protocol::SelectMode::Replace,
        },
    );
    m
}

#[test]
fn each_tool_is_one_step_keeps_the_selection_and_undoes() {
    let m = &mut three_notes();
    let before = notes(m);
    let take = shown_take(m);
    let steps = m.view().song.undo_count;
    let tools = [
        Command::ScaleVelocity {
            take,
            notes: vec![0, 1],
            percent: 50,
        },
        Command::RampVelocity {
            take,
            notes: vec![0, 1],
            from: 20,
            to: 120,
        },
        Command::RandomVelocity {
            take,
            notes: vec![0, 1],
            amount: 32,
            seed: 7,
        },
        Command::Legato {
            take,
            notes: vec![0, 1],
        },
    ];
    // Undo brings the take back, so each command is of the shown take.
    for tool in tools {
        cmd(m, tool.clone());
        assert_eq!(m.view().song.undo_count, steps + 1, "{tool:?}");
        assert_ne!(shown_take(m), take, "a new take");
        assert_eq!(
            m.view().piano_roll.selected_notes,
            [IndexRange { start: 0, end: 2 }],
            "{tool:?}"
        );
        assert_ne!(notes(m).notes, before.notes, "{tool:?}");
        // The third note is not picked.
        assert_eq!(notes(m).notes[2], before.notes[2]);
        cmd(m, Command::Undo);
        assert_eq!(shown_take(m), take);
        assert_eq!(notes(m).notes, before.notes, "{tool:?}");
        assert_eq!(
            m.view().piano_roll.selected_notes,
            [IndexRange { start: 0, end: 2 }]
        );
    }
}

#[test]
fn what_the_status_line_says() {
    let m = &mut three_notes();
    let take = shown_take(m);
    cmd(
        m,
        Command::ScaleVelocity {
            take,
            notes: vec![1, 0, 1],
            percent: 1000,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::VelocityScaled {
            count: 2,
            percent: 300
        })
    );
    let after = notes(m);
    assert!(after.notes[..2].iter().all(|n| n.velocity == 127));
    let take = shown_take(m);
    let steps = m.view().song.undo_count;
    // Nothing changes (all at 127): said, but no step.
    cmd(
        m,
        Command::ScaleVelocity {
            take,
            notes: vec![0, 1],
            percent: 200,
        },
    );
    assert_eq!(m.view().song.undo_count, steps);
    assert_eq!(
        status(m),
        Some(StatusMessage::VelocityScaled {
            count: 2,
            percent: 200
        })
    );
    cmd(
        m,
        Command::RampVelocity {
            take,
            notes: vec![0, 1, 2],
            from: 0,
            to: 200,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::VelocityRamped {
            count: 3,
            from: 1,
            to: 127
        })
    );
    let take = shown_take(m);
    cmd(
        m,
        Command::RandomVelocity {
            take,
            notes: vec![0],
            amount: 0,
            seed: 3,
        },
    );
    assert!(matches!(
        status(m),
        Some(StatusMessage::VelocityRandomized {
            count: 1,
            amount: 1
        })
    ));
    let take = shown_take(m);
    cmd(
        m,
        Command::Legato {
            take,
            notes: vec![0, 1, 2],
        },
    );
    assert_eq!(status(m), Some(StatusMessage::NotesLegato { count: 3 }));
    let n = notes(m).notes;
    assert_eq!(n[0].start + n[0].length, n[1].start);
    assert_eq!(n[1].start + n[1].length, n[2].start);
}

#[test]
fn an_old_take_or_no_notes_does_nothing() {
    let m = &mut three_notes();
    let take = shown_take(m);
    let steps = m.view().song.undo_count;
    cmd(
        m,
        Command::Legato {
            take: take + 1000,
            notes: vec![0, 1],
        },
    );
    cmd(
        m,
        Command::ScaleVelocity {
            take,
            notes: vec![],
            percent: 50,
        },
    );
    cmd(
        m,
        Command::RampVelocity {
            take,
            notes: vec![9],
            from: 1,
            to: 2,
        },
    );
    assert_eq!(m.view().song.undo_count, steps);
    assert!(!matches!(
        status(m),
        Some(
            StatusMessage::NotesLegato { .. }
                | StatusMessage::VelocityScaled { .. }
                | StatusMessage::VelocityRamped { .. }
        )
    ));
}
