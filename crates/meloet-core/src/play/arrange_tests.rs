use meloet_protocol::{ClipEdge, Command};

use super::tests::{cmd, ready, record_once, report, started};
use super::*;
use crate::state::{Effect, Model};

const BAR: u64 = 1920;

fn clips(m: &Model) -> Vec<ClipView> {
    m.view().song.tracks[0].clips.clone()
}

fn sequences(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Engine(EngineOp::Sequence { track: 0, .. })))
        .count()
}

/// One clip over the first bar, recorded with a note at 0.
fn one_clip() -> (Model, ClipView) {
    let mut m = ready();
    record_once(&mut m, &[(60, 0.0, 100.0)], BAR as f64);
    let c = clips(&m)[0].clone();
    (m, c)
}

#[test]
fn a_move_is_one_step_and_selects_the_clip() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(m, Command::SelectClip { clip: None });
    let steps = m.view().song.undo_count;
    let effects = cmd(
        m,
        Command::MoveClip {
            clip: c.id,
            ticks: 2 * BAR as i64,
            track: 1,
            copy: false,
        },
    );
    assert_eq!(sequences(&effects), 1);
    let moved = clips(m)[0].clone();
    assert_eq!((moved.id, moved.start), (c.id, c.start + 2 * BAR));
    assert_eq!(moved.take_id, c.take_id);
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert_eq!(m.view().arrange.selected_clip, Some(c.id));
    assert_eq!(m.view().arrange.piano_roll_clip, Some(c.id));
    // Nowhere to go: no step.
    cmd(
        m,
        Command::MoveClip {
            clip: c.id,
            ticks: 0,
            track: 1,
            copy: false,
        },
    );
    cmd(
        m,
        Command::MoveClip {
            clip: c.id,
            ticks: 0,
            track: 7,
            copy: false,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 1);
    cmd(m, Command::Undo);
    assert_eq!(clips(m)[0].start, c.start);
}

#[test]
fn a_copy_and_a_duplicate_are_selected_and_shown() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(
        m,
        Command::MoveClip {
            clip: c.id,
            ticks: 4 * BAR as i64,
            track: 1,
            copy: true,
        },
    );
    let all = clips(m);
    assert_eq!(all.len(), 2);
    let copy = all[1].clone();
    assert_ne!(copy.id, c.id);
    assert_eq!(copy.take_id, c.take_id, "the take is shared");
    assert_eq!(m.view().arrange.selected_clip, Some(copy.id));
    assert_eq!(m.view().arrange.piano_roll_clip, Some(copy.id));
    cmd(m, Command::DuplicateClip { clip: c.id });
    let all = clips(m);
    assert_eq!(all.len(), 3);
    assert_eq!(all[1].start, c.end);
    assert_eq!(m.view().arrange.selected_clip, Some(all[1].id));
    // The first duplicate is right after it now: no room.
    let steps = m.view().song.undo_count;
    cmd(m, Command::DuplicateClip { clip: c.id });
    assert_eq!(m.view().song.undo_count, steps);
    // Undo takes the copy away, and the piano roll with it.
    cmd(m, Command::Undo);
    assert_eq!(clips(m).len(), 2);
    assert_eq!(m.view().arrange.piano_roll_clip, None);
}

#[test]
fn a_trim_keeps_the_notes_in_place_and_tells_the_screen_the_offset() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    let effects = cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::Start,
            ticks: 480,
        },
    );
    assert_eq!(sequences(&effects), 1);
    let t = clips(m)[0].clone();
    assert_eq!((t.start, t.end), (c.start + 480, c.end));
    assert_eq!(t.take_offset, c.take_offset - 480);
    assert_eq!(t.take_id, c.take_id, "the take does not change");
    assert_eq!(
        t.start as i64 + t.take_offset,
        c.start as i64 + c.take_offset
    );
    // The note at the head no longer sounds.
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Sequence { track: 0, events }) if events.is_empty()
    )));
    cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::Start,
            ticks: -480,
        },
    );
    assert_eq!(clips(m)[0], c);
}

#[test]
fn nothing_changes_clips_while_recording() {
    let (mut m, c) = one_clip();
    let m = &mut m;
    cmd(m, Command::Locate { tick: 4 * BAR });
    cmd(m, Command::Record);
    started(m);
    let before = m.view().song;
    for command in [
        Command::MoveClip {
            clip: c.id,
            ticks: BAR as i64,
            track: 1,
            copy: false,
        },
        Command::MoveClip {
            clip: c.id,
            ticks: 8 * BAR as i64,
            track: 1,
            copy: true,
        },
        Command::DuplicateClip { clip: c.id },
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::End,
            ticks: -10,
        },
    ] {
        cmd(m, command);
    }
    assert_eq!(m.view().song, before);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 4.0 * BAR as f64,
            lap: 0,
        },
    );
    // Playing: taken, and the engine hears the track again.
    cmd(m, Command::PlayStop);
    let effects = cmd(
        m,
        Command::TrimClip {
            clip: c.id,
            edge: ClipEdge::End,
            ticks: -10,
        },
    );
    assert_eq!(sequences(&effects), 1);
    assert_eq!(clips(m)[0].end, c.end - 10);
}
