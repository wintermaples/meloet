//! A batch of the AI link: one step, or nothing. And what "no change" is.

use std::time::Duration;

use meloet_model::{Doc, Note, TrackId};
use meloet_protocol::{Command, SendBus, StripRef, TimelineRef};

use crate::ai::{AiAnswer, AiAsk, AiDone, AiPlace, AiRefusal};
use crate::settings::Settings;
use crate::state::{Input, Model};

fn ask(m: &mut Model, ask: AiAsk) -> AiAnswer {
    m.handle(Input::Ai(ask));
    m.take_ai_answer().expect("an answer")
}

fn done(m: &mut Model, a: AiAsk) -> AiDone {
    match ask(m, a.clone()) {
        AiAnswer::Done(done) => *done,
        other => panic!("{a:?}: {other:?}"),
    }
}

fn refused(m: &mut Model, a: AiAsk) -> AiRefusal {
    match ask(m, a.clone()) {
        AiAnswer::Refused(refusal) => refusal,
        other => panic!("{a:?}: {other:?}"),
    }
}

fn in_batch(batch: u64, ask: AiAsk) -> AiAsk {
    AiAsk::InBatch {
        batch,
        ask: Box::new(ask),
    }
}

fn screen(batch: u64, command: Command) -> AiAsk {
    in_batch(batch, AiAsk::Screen(command))
}

fn note(start: u64, key: u8) -> Note {
    Note {
        start,
        length: 480,
        key,
        velocity: 90,
        from: None,
    }
}

/// The song, but for the ids it has given out: those never come back.
fn doc(m: &Model) -> Doc {
    Doc {
        next_id: 0,
        ..m.ai_song().song.doc
    }
}

/// Steps done, steps that can be redone.
fn counts(m: &Model) -> (u32, u32) {
    let history = m.view().history;
    (history.current, history.redo)
}

fn track(m: &Model) -> u32 {
    doc(m).tracks[0].id.0
}

fn gain(track: u32, db: f32) -> Command {
    Command::SetGain {
        strip: StripRef::Track { id: track },
        db: Some(db),
        commit: true,
    }
}

fn rename(track: u32, name: &str) -> Command {
    Command::RenameTrack {
        track,
        name: name.into(),
    }
}

/// The owner's case and a clip with notes, in the batch 7. Returns the clip.
fn mix_and_write(m: &mut Model) -> u32 {
    let t = track(m);
    done(m, AiAsk::Begin { batch: 7 });
    done(m, screen(7, rename(t, "ベース")));
    done(m, screen(7, Command::ToggleMute { track: t }));
    done(m, screen(7, gain(t, -6.0)));
    let pan = Command::SetPan {
        strip: StripRef::Track { id: t },
        pan: -30,
        commit: true,
    };
    done(m, screen(7, pan));
    let send = Command::SetSend {
        track: t,
        bus: SendBus::A,
        db: Some(-12.0),
        commit: true,
    };
    done(m, screen(7, send));
    let made = done(m, in_batch(7, AiAsk::CreateClip { track: 1, tick: 0 }));
    let clip = made.clip.expect("the clip made");
    let wrote = done(
        m,
        in_batch(
            7,
            AiAsk::WriteNotes {
                at: AiPlace::Clip(clip),
                notes: vec![note(0, 60), note(480, 64)],
            },
        ),
    );
    assert_eq!(wrote.clip, Some(clip));
    clip
}

#[test]
fn a_batch_is_one_step_and_undo_and_redo_go_over_all_of_it() {
    let m = &mut Model::new(Settings::default());
    done(m, AiAsk::SetTempo { tick: 0, bpm: 90.0 });
    let before = doc(m);
    let clip = mix_and_write(m);
    assert_eq!(
        counts(m),
        (8, 0),
        "steps as usual until the batch is closed"
    );
    let after = doc(m);
    let newest = m.view().history.newest;

    let closed = done(m, AiAsk::Commit { batch: 7 });
    assert_eq!(closed.undo_count, 2);
    assert_eq!(counts(m), (2, 0));
    assert_eq!(m.view().history.total, 2);
    assert_eq!(doc(m), after, "closing changes nothing of the song");
    let row = closed.step.expect("the batch's row");
    assert_eq!(row.index, 2);
    assert_eq!(m.play.history_rows(1, 9).len(), 2);
    assert_eq!(m.play.history_rows(2, 2)[0], row);
    // The screen lets go of the row it had at that place.
    assert_ne!(m.view().history.newest, newest);
    assert_eq!(row.step, m.view().history.newest);
    let take = after.clip(meloet_model::ClipId(clip)).unwrap().1;
    let take = take.active_take_id();

    for _ in 0..2 {
        let undone = done(m, AiAsk::Undo);
        assert_eq!(undone.undo_count, 1);
        assert_eq!(counts(m), (1, 1));
        assert_eq!(doc(m), before);
        m.handle(Input::Command(Command::Redo));
        assert_eq!(counts(m), (2, 0));
        assert_eq!(doc(m), after);
        let now = doc(m);
        let shown = now.clip(meloet_model::ClipId(clip)).unwrap().1;
        assert_eq!(shown.active_take_id(), take, "the same take, with its id");
    }
    // The batch is closed: writes are taken again, and the step before it is its own.
    done(m, AiAsk::Undo);
    done(m, AiAsk::Undo);
    assert_eq!(counts(m), (0, 2));
}

#[test]
fn a_batch_taken_back_leaves_no_trace() {
    let m = &mut Model::new(Settings::default());
    done(m, AiAsk::SetTempo { tick: 0, bpm: 90.0 });
    done(m, AiAsk::SetTempo { tick: 0, bpm: 95.0 });
    let name = doc(m).name;
    m.play.saved_as(name);
    assert!(!m.ai_song().unsaved);
    let before = doc(m);
    let rows = m.play.history_rows(1, 9);
    let unsaved = m.ai_song().unsaved;

    mix_and_write(m);
    assert!(m.ai_song().unsaved);
    let t = track(m);
    assert_eq!(
        refused(m, screen(7, rename(t + 100, "x"))),
        AiRefusal::NoSuchTrack(t + 100)
    );
    assert_eq!(
        counts(m),
        (9, 0),
        "the failed ask took nothing back by itself"
    );
    let back = done(m, AiAsk::Rollback { batch: 7 });
    assert_eq!(back.undo_count, 2);
    assert_eq!(counts(m), (2, 0));
    assert_eq!(doc(m), before);
    assert_eq!(m.play.history_rows(1, 9), rows);
    assert_eq!(m.ai_song().unsaved, unsaved);
    // Nothing of the batch is there to redo.
    m.handle(Input::Command(Command::Redo));
    assert_eq!(doc(m), before);
    // The batch is closed.
    assert_eq!(refused(m, screen(7, rename(t, "x"))), AiRefusal::BatchLost);
    done(m, AiAsk::Screen(rename(t, "x")));
}

#[test]
fn the_step_that_could_be_redone_can_be_redone_after_a_batch_taken_back() {
    let m = &mut Model::new(Settings::default());
    done(m, AiAsk::SetTempo { tick: 0, bpm: 90.0 });
    done(m, AiAsk::SetTempo { tick: 0, bpm: 95.0 });
    let end = doc(m);
    done(m, AiAsk::Undo);
    let before = doc(m);
    let rows = m.play.history_rows(1, 9);
    assert_eq!(counts(m), (1, 1));

    mix_and_write(m);
    assert_eq!(counts(m), (8, 0));
    done(m, AiAsk::Rollback { batch: 7 });
    assert_eq!(counts(m), (1, 1));
    assert_eq!(doc(m), before);
    assert_eq!(m.play.history_rows(1, 9), rows);
    m.handle(Input::Command(Command::Redo));
    assert_eq!(doc(m), end);
    assert_eq!(counts(m), (2, 0));

    // More than one step to redo: they are let go, as a step of the batch lets them go.
    done(m, AiAsk::Undo);
    done(m, AiAsk::Undo);
    let start = doc(m);
    assert_eq!(counts(m), (0, 2));
    mix_and_write(m);
    done(m, AiAsk::Rollback { batch: 7 });
    assert_eq!(counts(m), (0, 0));
    assert_eq!(doc(m), start);
}

#[test]
fn an_empty_batch_and_a_batch_of_one_step() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    done(m, AiAsk::Begin { batch: 1 });
    let closed = done(m, AiAsk::Commit { batch: 1 });
    assert_eq!((closed.step, closed.undo_count), (None, 0));

    done(m, AiAsk::Begin { batch: 2 });
    done(m, screen(2, gain(t, -3.0)));
    // The value it has: no failure, and no step.
    assert_eq!(refused(m, screen(2, gain(t, -3.0))), AiRefusal::Unchanged);
    let newest = m.view().history.newest;
    let closed = done(m, AiAsk::Commit { batch: 2 });
    assert_eq!(closed.undo_count, 1);
    assert_eq!(closed.step.map(|row| row.step), Some(newest));

    done(m, AiAsk::Begin { batch: 3 });
    assert_eq!(refused(m, screen(3, gain(t, -3.0))), AiRefusal::Unchanged);
    done(m, AiAsk::Rollback { batch: 3 });
    assert_eq!(counts(m), (1, 0));
}

#[test]
fn only_the_batch_writes_while_it_is_open() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    done(m, AiAsk::Begin { batch: 1 });
    assert_eq!(refused(m, AiAsk::Begin { batch: 2 }), AiRefusal::BatchOpen);
    assert_eq!(
        refused(m, AiAsk::Screen(gain(t, -3.0))),
        AiRefusal::BatchOpen
    );
    assert_eq!(refused(m, AiAsk::Undo), AiRefusal::BatchOpen);
    assert_eq!(
        refused(m, screen(2, gain(t, -3.0))),
        AiRefusal::BatchLost,
        "another batch's number"
    );
    assert_eq!(refused(m, AiAsk::Commit { batch: 2 }), AiRefusal::BatchLost);
    assert_eq!(
        refused(m, AiAsk::Rollback { batch: 2 }),
        AiRefusal::BatchLost
    );
    for not in [
        AiAsk::Undo,
        AiAsk::Save,
        AiAsk::NewSong,
        AiAsk::Begin { batch: 1 },
        AiAsk::Commit { batch: 1 },
        AiAsk::Open {
            path: "/nowhere/a.meloet".into(),
        },
    ] {
        assert_eq!(refused(m, in_batch(1, not)), AiRefusal::NotInBatch);
    }
    assert_eq!(counts(m), (0, 0));
    done(m, screen(1, gain(t, -3.0)));
    done(m, AiAsk::Commit { batch: 1 });
    assert_eq!(
        refused(m, AiAsk::Commit { batch: 1 }),
        AiRefusal::BatchLost,
        "closed"
    );
    done(m, AiAsk::Undo);
}

#[test]
fn a_change_by_someone_else_ends_the_batch_where_it_is() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    // A step of the person after the batch's last ask.
    done(m, AiAsk::Begin { batch: 1 });
    done(m, screen(1, gain(t, -3.0)));
    done(m, screen(1, rename(t, "a")));
    m.handle(Input::Command(Command::ToggleMute { track: t }));
    assert_eq!(counts(m), (3, 0));
    let now = doc(m);
    assert_eq!(refused(m, screen(1, rename(t, "b"))), AiRefusal::BatchLost);
    assert_eq!(
        refused(m, AiAsk::Rollback { batch: 1 }),
        AiRefusal::BatchLost
    );
    assert_eq!(doc(m), now, "the person's step is not taken back");
    assert_eq!(counts(m), (3, 0));
    done(m, AiAsk::Screen(rename(t, "c")));

    // Closed over the person's step: the steps stay as they are, and `step` says so.
    done(m, AiAsk::Begin { batch: 2 });
    done(m, screen(2, gain(t, -4.0)));
    done(m, screen(2, rename(t, "d")));
    m.handle(Input::Command(Command::ToggleMute { track: t }));
    let closed = done(m, AiAsk::Commit { batch: 2 });
    assert_eq!(closed.step, None);
    assert_eq!(counts(m), (7, 0));

    // The person's undo of a step of the batch.
    done(m, AiAsk::Begin { batch: 3 });
    done(m, screen(3, gain(t, -5.0)));
    m.handle(Input::Command(Command::Undo));
    assert_eq!(refused(m, screen(3, rename(t, "e"))), AiRefusal::BatchLost);
    assert_eq!(
        refused(m, AiAsk::Rollback { batch: 3 }),
        AiRefusal::BatchLost
    );
    assert_eq!(counts(m), (7, 1));

    // Undone and done again by the person: as if nothing came between.
    done(m, AiAsk::Begin { batch: 4 });
    done(m, screen(4, gain(t, -7.0)));
    done(m, screen(4, rename(t, "f")));
    m.handle(Input::Command(Command::Undo));
    m.handle(Input::Command(Command::Redo));
    done(m, screen(4, rename(t, "g")));
    done(m, AiAsk::Commit { batch: 4 });
    assert_eq!(counts(m), (8, 0));

    // Another song in its place.
    done(m, AiAsk::Begin { batch: 5 });
    done(m, screen(5, gain(t, -8.0)));
    m.replace_song(meloet_model::Song::new_default(), false, &mut Vec::new());
    let t = track(m);
    let new = doc(m);
    assert_eq!(refused(m, screen(5, rename(t, "h"))), AiRefusal::BatchLost);
    assert_eq!(
        refused(m, AiAsk::Rollback { batch: 5 }),
        AiRefusal::BatchLost
    );
    assert_eq!(counts(m), (0, 0));
    assert_eq!(doc(m), new);
}

#[test]
fn undoing_a_batch_brings_back_what_was_selected_before_it() {
    let m = &mut Model::new(Settings::default());
    let a = done(m, AiAsk::CreateClip { track: 1, tick: 0 })
        .clip
        .unwrap();
    m.handle(Input::Command(Command::SelectClip { clip: Some(a) }));
    assert_eq!(m.view().arrange.selected_clips, [a]);

    done(m, AiAsk::Begin { batch: 1 });
    done(m, screen(1, gain(track(m), -3.0)));
    done(m, in_batch(1, AiAsk::DeleteClips { clips: vec![a] }));
    done(
        m,
        in_batch(
            1,
            AiAsk::AddMarker {
                tick: 0,
                name: "A".into(),
            },
        ),
    );
    assert!(m.view().arrange.selected_clips.is_empty());
    done(m, AiAsk::Commit { batch: 1 });
    assert!(m.view().arrange.selected_clips.is_empty());

    m.handle(Input::Command(Command::Undo));
    assert_eq!(m.view().arrange.selected_clips, [a]);
    assert_eq!(m.view().arrange.selected_clip, Some(a));
    m.handle(Input::Command(Command::Redo));
    assert!(m.view().arrange.selected_clips.is_empty());
    m.handle(Input::Command(Command::Undo));
    assert_eq!(m.view().arrange.selected_clips, [a]);

    // Taken back: selected as before it, too.
    done(m, AiAsk::Begin { batch: 2 });
    done(m, in_batch(2, AiAsk::DeleteClips { clips: vec![a] }));
    assert!(m.view().arrange.selected_clips.is_empty());
    done(m, AiAsk::Rollback { batch: 2 });
    assert_eq!(m.view().arrange.selected_clips, [a]);
}

#[test]
fn solo_and_the_loop_switch_go_back_with_the_batch() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    let session = |m: &Model| {
        let s = m.ai_song().song.session;
        (s.loop_on, s.soloed.contains(&TrackId(t)))
    };
    assert_eq!(session(m), (false, false));
    done(m, AiAsk::Begin { batch: 1 });
    let solo = done(m, screen(1, Command::ToggleSolo { track: t }));
    assert_eq!(solo.step, None);
    let looped = Command::SetLoop {
        from_bar: 1,
        to_bar: 3,
    };
    assert!(done(m, screen(1, looped)).step.is_some());
    assert_eq!(session(m), (true, true));
    done(m, AiAsk::Rollback { batch: 1 });
    assert_eq!(session(m), (false, false));
    assert_eq!(doc(m).loop_range, None);

    // With no step at all.
    done(m, AiAsk::Begin { batch: 2 });
    done(m, screen(2, Command::ToggleSolo { track: t }));
    done(m, AiAsk::Rollback { batch: 2 });
    assert_eq!(session(m), (false, false));
    // Kept by a batch that is closed.
    done(m, AiAsk::Begin { batch: 3 });
    done(m, screen(3, Command::ToggleSolo { track: t }));
    let closed = done(m, AiAsk::Commit { batch: 3 });
    assert_eq!(closed.step, None);
    assert_eq!(session(m), (false, true));
}

#[test]
fn a_batch_left_open_is_closed_and_taken_back_by_the_next_ask() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    let before = doc(m);
    done(m, AiAsk::Begin { batch: 1 });
    done(m, screen(1, gain(t, -3.0)));
    done(m, screen(1, rename(t, "a")));
    assert_eq!(
        refused(m, AiAsk::Screen(rename(t, "b"))),
        AiRefusal::BatchOpen
    );
    let batch = m.ai.batch.as_mut().unwrap();
    batch.asked = batch.asked.checked_sub(Duration::from_secs(11)).unwrap();
    let wrote = done(m, AiAsk::Screen(rename(t, "b")));
    assert_eq!(wrote.undo_count, 1);
    done(m, AiAsk::Undo);
    assert_eq!(doc(m), before);
    assert_eq!(refused(m, AiAsk::Commit { batch: 1 }), AiRefusal::BatchLost);
}

#[test]
fn no_batch_begins_and_none_is_taken_back_while_recording() {
    let m = &mut crate::state::tests::playing();
    let t = track(m);
    done(m, AiAsk::Begin { batch: 1 });
    done(m, screen(1, gain(t, -3.0)));
    m.handle(Input::Command(Command::Record));
    let busy = m.ai_song().busy.expect("counting in or recording");
    assert_eq!(refused(m, screen(1, rename(t, "a"))), AiRefusal::Busy(busy));
    assert_eq!(
        refused(m, AiAsk::Rollback { batch: 1 }),
        AiRefusal::Busy(busy)
    );
    assert_eq!(m.ai_song().undo_count, 1, "the step stays");
    assert_eq!(refused(m, AiAsk::Begin { batch: 2 }), AiRefusal::Busy(busy));
}

#[test]
fn the_song_as_it_is_already_is_no_failure_and_a_wrong_name_is_one() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    let name = doc(m).tracks[0].name.clone();
    let screen = |m: &mut Model, command: Command| refused(m, AiAsk::Screen(command));
    let marker = |id: u32| TimelineRef::Marker { id };

    done(m, AiAsk::Screen(gain(t, -3.0)));
    for same in [
        gain(t, -3.0),
        rename(t, &name),
        Command::MoveTrack { track: t, to: 0 },
        Command::SetPan {
            strip: StripRef::Track { id: t },
            pan: 0,
            commit: true,
        },
        Command::SetMeterChange {
            bar: 0,
            num: 4,
            den: 4,
        },
        Command::SetTempoRamp { tick: 0, on: false },
    ] {
        assert_eq!(screen(m, same.clone()), AiRefusal::Unchanged, "{same:?}");
    }
    assert_eq!(m.ai_song().undo_count, 1);

    let none = t + 100;
    assert_eq!(screen(m, gain(none, -3.0)), AiRefusal::NoSuchTrack(none));
    assert_eq!(screen(m, rename(none, "x")), AiRefusal::NoSuchTrack(none));
    assert_eq!(
        screen(m, Command::ToggleMute { track: none }),
        AiRefusal::NoSuchTrack(none)
    );
    assert_eq!(
        screen(m, Command::ToggleSolo { track: none }),
        AiRefusal::NoSuchTrack(none)
    );
    assert_eq!(
        screen(m, Command::DeleteTrack { track: none }),
        AiRefusal::NoSuchTrack(none)
    );
    let split = Command::SplitClipAt { clip: 999, tick: 0 };
    assert_eq!(screen(m, split), AiRefusal::NoSuchClip(999));
    let join = Command::JoinClips {
        clips: vec![998, 999],
    };
    assert_eq!(screen(m, join), AiRefusal::NoSuchClip(998));
    for wrong in [
        Command::DeleteTimeline {
            target: marker(999),
        },
        Command::RenameMarker {
            id: 999,
            name: "x".into(),
        },
        Command::MoveMarker { id: 999, tick: 0 },
        Command::DeleteTimeline {
            target: TimelineRef::TempoPoint { tick: 1920 },
        },
        Command::DeleteTimeline {
            target: TimelineRef::MeterChange { bar: 5 },
        },
        // Not an intent of the AI link.
        Command::SaveSong,
        Command::DeleteSelected,
    ] {
        let refusal = screen(m, wrong.clone());
        assert!(
            matches!(refusal, AiRefusal::Cannot(_)),
            "{wrong:?}: {refusal:?}"
        );
    }

    // A clip cut on its edge, and clips that cannot be joined.
    let a = done(m, AiAsk::CreateClip { track: 1, tick: 0 })
        .clip
        .unwrap();
    let split = Command::SplitClipAt { clip: a, tick: 0 };
    assert!(matches!(screen(m, split), AiRefusal::Cannot(_)));
    let b = done(
        m,
        AiAsk::CreateClip {
            track: 1,
            tick: 1920 * 4 * 8,
        },
    );
    let far = done(
        m,
        AiAsk::CreateClip {
            track: 1,
            tick: 1920 * 4 * 16,
        },
    );
    let join = Command::JoinClips {
        clips: vec![a, far.clip.unwrap()],
    };
    assert!(matches!(screen(m, join), AiRefusal::Cannot(_)));
    // Onto another clip.
    let onto = Command::MoveClips {
        clips: vec![a],
        ticks: 1920 * 4 * 8,
        tracks: 0,
        copy: false,
    };
    assert_eq!(screen(m, onto), AiRefusal::Overlap);
    assert!(b.clip.is_some());
    assert_eq!(m.ai_song().undo_count, 4);
}

#[test]
fn the_line_said_before_is_not_taken_for_the_answer() {
    let m = &mut Model::new(Settings::default());
    let t = track(m);
    done(m, AiAsk::Screen(gain(t, -3.0)));
    m.handle(Input::Command(Command::Undo));
    let said = m.view().status;
    assert!(said.message.is_some(), "the line says what was undone");
    done(m, AiAsk::Screen(gain(t, -3.0)));
    assert_eq!(
        refused(m, AiAsk::Screen(gain(t, -3.0))),
        AiRefusal::Unchanged
    );
    assert_eq!(m.view().status, said, "and it stays on the line");
}

#[test]
fn the_loop_switched_on_by_its_range_is_a_change_with_no_step() {
    let m = &mut Model::new(Settings::default());
    let range = Command::SetLoop {
        from_bar: 1,
        to_bar: 3,
    };
    assert!(done(m, AiAsk::Screen(range.clone())).step.is_some());
    done(m, AiAsk::Screen(Command::ToggleLoop));
    assert!(!m.ai_song().song.session.loop_on);
    let on = done(m, AiAsk::Screen(range.clone()));
    assert_eq!((on.step, on.undo_count), (None, 1));
    assert!(m.ai_song().song.session.loop_on);
    assert_eq!(refused(m, AiAsk::Screen(range)), AiRefusal::Unchanged);
}
