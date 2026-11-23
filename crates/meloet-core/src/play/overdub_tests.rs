use meloet_model::playback::PlayKind;
use meloet_protocol::{Command, StepWhat, TransportState};

use super::tests::{cmd, event, off, on, ready, report, started};
use super::*;
use crate::settings::Settings;
use crate::state::{Effect, Input, Model};

const BAR: u64 = 4 * meloet_model::TICKS_PER_QUARTER;
const SEAT: usize = 0;

fn ops(effects: &[Effect]) -> Vec<EngineOp> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

/// What the seat of the keys is sent of its events.
fn sent(effects: &[Effect]) -> Vec<(bool, Vec<u8>)> {
    let keys = |events: &[PlayEvent]| {
        (events.iter())
            .filter_map(|e| match e.kind {
                PlayKind::NoteOn { key, .. } => Some(key),
                _ => None,
            })
            .collect()
    };
    (ops(effects).into_iter())
        .filter_map(|op| match op {
            EngineOp::Sequence { track, events } if track == SEAT => Some((false, keys(&events))),
            EngineOp::GrowSequence { track, events } if track == SEAT => {
                Some((true, keys(&events)))
            }
            _ => None,
        })
        .collect()
}

/// What is heard now, then a tick of the core thread.
fn tick(m: &mut Model, t_us: u64, tick: f64, lap: u32) -> Vec<Effect> {
    m.set_now(
        t_us,
        Some(Heard {
            tick,
            state: TransportState::Recording,
            lap,
        }),
    );
    m.handle(Input::Tick)
}

fn lap_changed(m: &mut Model, lap: u32) {
    report(
        m,
        RecordReport::LapChanged {
            lap,
            frame: 0,
            file_frame: None,
        },
    );
}

fn stop(m: &mut Model, t_us: u64, tick: f64, lap: u32) {
    m.set_now(t_us, None);
    cmd(m, Command::PlayStop);
    report(m, RecordReport::Stopped { t_us, tick, lap });
}

/// A one-bar loop over bar 1, no count-in, and 「重ねる」 as given.
fn looping(overdub: bool) -> Model {
    let mut m = ready();
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(
        &mut m,
        Command::SetLoop {
            from_bar: 1,
            to_bar: 2,
        },
    );
    cmd(&mut m, Command::SetOverdub { on: overdub });
    m
}

fn the_clip(m: &Model) -> meloet_protocol::ClipView {
    let clips = &m.view().song.tracks[0].clips;
    assert_eq!(clips.len(), 1, "{clips:?}");
    clips[0].clone()
}

fn last_row_what(m: &Model) -> StepWhat {
    let n = m.view().song.undo_count;
    m.history_rows(n, n).pop().expect("a row").what
}

#[test]
fn each_pass_with_notes_is_a_step_and_sounds_from_the_next_pass() {
    let m = &mut looping(true);
    let ops_of_record = ops(&cmd(m, Command::Record));
    assert!(
        matches!(&ops_of_record[..], [EngineOp::Record { muted, .. }] if *muted == TrackSet::default()),
        "the track plays while it records: {ops_of_record:?}"
    );
    started(m);
    let steps = m.view().song.undo_count;
    let rec = m.view().transport.recording.unwrap();
    assert!(rec.overdub);
    assert_eq!((rec.clip, rec.take), (None, 1));

    // Pass 1: a kick on the first beat. Its notes go to the engine before the pass ends.
    tick(m, 1, 10.0, 0);
    event(m, on(36), 2, 0.0, 0);
    event(m, off(36), 3, 200.0, 0);
    assert!(sent(&tick(m, 4, BAR as f64 / 2.0, 0)).is_empty());
    assert_eq!(sent(&tick(m, 5, (BAR - 50) as f64, 0)), [(true, vec![36])]);
    assert!(sent(&tick(m, 6, (BAR - 20) as f64, 0)).is_empty(), "once");
    lap_changed(m, 1);
    assert_eq!(m.view().song.undo_count, steps, "still heard");
    let over = tick(m, 7, 10.0, 1);
    assert!(sent(&over).is_empty(), "the engine has it already");
    assert_eq!(m.view().song.undo_count, steps + 1);
    let clip = the_clip(m);
    assert_eq!(
        (clip.start, clip.end, clip.takes, clip.note_count),
        (0, BAR, 1, 1)
    );
    let rec = m.view().transport.recording.unwrap();
    assert_eq!((rec.clip, rec.take, rec.lap), (Some(clip.id), 1, 1));
    assert_eq!(last_row_what(m), StepWhat::Take { number: 1 });

    // Pass 2: a snare, let go only after the notes went ahead.
    event(m, on(38), 8, (BAR / 2) as f64, 1);
    assert_eq!(
        sent(&tick(m, 9, (BAR - 50) as f64, 1)),
        [(true, vec![36, 38])]
    );
    event(m, off(38), 10, (BAR - 10) as f64, 1);
    lap_changed(m, 2);
    let over = tick(m, 11, 10.0, 2);
    assert_eq!(
        sent(&over),
        [(true, vec![36, 38])],
        "sent again for the shorter note, keeping what sounds"
    );
    assert_eq!(m.view().song.undo_count, steps + 2);

    // A pass with no notes is no step.
    lap_changed(m, 3);
    tick(m, 12, 10.0, 3);
    assert_eq!(m.view().song.undo_count, steps + 2);

    // Pass 4: a hi-hat, then stop.
    event(m, on(42), 13, (BAR / 4) as f64, 3);
    event(m, off(42), 14, (BAR / 4 + 100) as f64, 3);
    stop(m, 15, (BAR / 2) as f64, 3);
    assert_eq!(m.view().song.undo_count, steps + 3);
    let clip = the_clip(m);
    assert_eq!((clip.takes, clip.take, clip.note_count), (1, 1, 3));
    assert!(m.view().transport.recording.is_none());

    // Undo takes away the last pass only.
    cmd(m, Command::Undo);
    let back = the_clip(m);
    assert_eq!((back.id, back.takes, back.note_count), (clip.id, 1, 2));
}

#[test]
fn a_key_held_over_the_end_of_a_pass_goes_into_that_pass() {
    let m = &mut looping(true);
    cmd(m, Command::Record);
    started(m);
    tick(m, 1, 10.0, 0);
    event(m, on(60), 2, (BAR - 100) as f64, 0);
    tick(m, 3, (BAR - 50) as f64, 0);
    lap_changed(m, 1);
    tick(m, 4, 10.0, 1);
    let clip = the_clip(m);
    assert_eq!(
        clip.note_count, 1,
        "in the step of the pass it was pressed in"
    );
    // Let go in the next pass: nothing more.
    event(m, off(60), 5, 30.0, 1);
    stop(m, 6, 40.0, 1);
    assert_eq!(the_clip(m).note_count, 1);
}

#[test]
fn without_a_loop_the_recording_adds_to_the_clip_as_one_step() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 1, 100.0, 0);
    event(m, off(60), 2, 300.0, 0);
    stop(m, 3, BAR as f64, 0);
    let first = the_clip(m);
    assert_eq!((first.start, first.end, first.takes), (0, BAR, 1));
    let steps = m.view().song.undo_count;

    cmd(m, Command::SetOverdub { on: true });
    cmd(m, Command::Locate { tick: 0 });
    let o = ops(&cmd(m, Command::Record));
    assert!(matches!(&o[..], [EngineOp::Record { muted, .. }] if *muted == TrackSet::default()));
    assert_eq!(m.view().transport.recording.unwrap().clip, Some(first.id));
    started(m);
    event(m, on(64), 4, 200.0, 0);
    event(m, off(64), 5, 400.0, 0);
    // Past the clip's end: it grows to the end of that bar.
    event(m, on(67), 6, (BAR + 100) as f64, 0);
    event(m, off(67), 7, (BAR + 300) as f64, 0);
    stop(m, 8, (BAR + 500) as f64, 0);
    let clip = the_clip(m);
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert_eq!(
        (clip.id, clip.start, clip.end, clip.takes, clip.note_count),
        (first.id, 0, 2 * BAR, 1, 3)
    );
    assert_ne!(clip.take_id, first.take_id, "a new take in the same place");
    assert_eq!(last_row_what(m), StepWhat::Take { number: 1 });
    cmd(m, Command::Undo);
    assert_eq!(the_clip(m), first);
}

#[test]
fn turning_it_on_or_off_while_recording_works_from_the_next_pass() {
    let m = &mut looping(false);
    cmd(m, Command::Record);
    started(m);
    let steps = m.view().song.undo_count;
    tick(m, 1, 10.0, 0);
    event(m, on(60), 2, 100.0, 0);
    event(m, off(60), 3, 200.0, 0);
    cmd(m, Command::SetOverdub { on: true });
    assert!(
        !m.view().transport.recording.unwrap().overdub,
        "not this pass"
    );
    assert!(sent(&tick(m, 4, (BAR - 50) as f64, 0)).is_empty());
    lap_changed(m, 1);
    let over = tick(m, 5, 10.0, 1);
    // The take of pass 1 is in, and the track is heard from now on.
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert!(ops(&over).contains(&EngineOp::RecordMuted {
        muted: TrackSet::default()
    }));
    let rec = m.view().transport.recording.unwrap();
    assert!(rec.overdub);
    let clip = the_clip(m);
    assert_eq!((clip.takes, clip.note_count), (1, 1));
    assert_eq!((rec.clip, rec.take), (Some(clip.id), 1));

    // Pass 2 adds to that take.
    event(m, on(62), 6, 300.0, 1);
    event(m, off(62), 7, 400.0, 1);
    cmd(m, Command::SetOverdub { on: false });
    lap_changed(m, 2);
    let over = tick(m, 8, 10.0, 2);
    assert_eq!(m.view().song.undo_count, steps + 2);
    let clip = the_clip(m);
    assert_eq!((clip.takes, clip.note_count), (1, 2));
    assert!(ops(&over).contains(&EngineOp::RecordMuted {
        muted: [SEAT].into_iter().collect()
    }));
    let rec = m.view().transport.recording.unwrap();
    assert_eq!((rec.overdub, rec.take), (false, 2));

    // Pass 3 is a take again.
    event(m, on(64), 9, 500.0, 2);
    event(m, off(64), 10, 600.0, 2);
    stop(m, 11, 700.0, 2);
    assert_eq!(m.view().song.undo_count, steps + 3);
    let clip = the_clip(m);
    assert_eq!((clip.takes, clip.take, clip.note_count), (2, 2, 1));
}

#[test]
fn the_switch_is_a_setting_and_no_step() {
    let m = &mut ready();
    assert!(!m.view().transport.overdub);
    let steps = m.view().song.undo_count;
    let effects = cmd(m, Command::SetOverdub { on: true });
    assert!(m.view().transport.overdub);
    assert_eq!(m.view().song.undo_count, steps);
    let saved: Vec<_> = (effects.iter())
        .filter_map(|e| match e {
            Effect::SaveSettings(s) => Some(s.overdub),
            _ => None,
        })
        .collect();
    assert_eq!(saved, [true]);
    assert!(
        !(cmd(m, Command::SetOverdub { on: true }).iter())
            .any(|e| matches!(e, Effect::SaveSettings(_))),
        "the same again saves nothing"
    );
    let again = Model::new(Settings {
        overdub: true,
        ..Settings::default()
    });
    assert!(again.view().transport.overdub);
}
