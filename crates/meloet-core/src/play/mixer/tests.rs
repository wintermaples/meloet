use crate::seen::*;
use meloet_model::effects::{EffectKind as ModelEffect, reverb};
use meloet_model::{Bus, Gain, Pan, StripId, TrackId};
use meloet_protocol::{
    Band, BarSpan, Command, EffectKind, EffectName, EffectRef, InsertRef, NamedInsert, ParamEnd,
    ParamRef, SendBus, ShelfMode, SongFile, StatusMessage, StripRef, TransportState,
};

use super::Preview;
use crate::play::mix::tests::three_tracks;
use crate::play::tests::{cmd, ready, status};
use crate::state::Model;

const T1: StripRef = StripRef::Track { id: 1 };
const A: StripRef = StripRef::Return { bus: SendBus::A };
const B: StripRef = StripRef::Return { bus: SendBus::B };

fn at(strip: StripRef, slot: u8) -> InsertRef {
    InsertRef { strip, slot }
}

fn gain(strip: StripRef, db: Option<f32>, commit: bool) -> Command {
    Command::SetGain { strip, db, commit }
}

fn decay(value: f32, commit: bool) -> Command {
    Command::SetParam {
        insert: at(A, 0),
        effect: EffectKind::Reverb,
        param: reverb::DECAY,
        value,
        commit,
    }
}

fn previews(m: &Model) -> Vec<Preview> {
    m.play().mix_previews().to_vec()
}

fn undo_count(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn params_in_front(m: &Model) -> bool {
    m.view().workspace.layout.right_front == Some(Band::Params)
}

#[test]
fn a_drag_is_heard_as_a_preview_and_is_one_step_when_it_ends() {
    let m = &mut ready();
    let before = m.view();
    for db in [-1.0, -2.5, -4.0] {
        let effects = cmd(m, gain(T1, Some(db), false));
        assert_eq!(effects.len(), 1, "only the engine hears it: {effects:?}");
    }
    assert_eq!(
        previews(m),
        [Preview::Gain(
            StripId::Track(TrackId(1)),
            Gain::from_db(-4.0)
        )],
        "one value for the control, the last"
    );
    assert_eq!(m.view().song, before.song, "the song is as it was");
    assert_eq!(undo_count(m), 0);

    cmd(m, gain(T1, Some(-4.8), true));
    assert!(previews(m).is_empty());
    let song = m.view().song;
    assert_eq!(song.tracks[0].gain_db, Some(-4.8));
    assert_eq!((song.undo_count, song.file), (1, SongFile::Changed));

    // Letting go where the drag began is no step.
    cmd(m, gain(T1, Some(0.0), false));
    cmd(m, gain(T1, Some(-4.8), true));
    assert!(previews(m).is_empty());
    assert_eq!(undo_count(m), 1);

    cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks[0].gain_db, Some(0.0));
    cmd(m, Command::Redo);
    assert_eq!(m.view().song.tracks[0].gain_db, Some(-4.8));
}

#[test]
fn esc_and_a_change_of_the_song_end_the_previews() {
    let m = &mut ready();
    cmd(m, gain(T1, Some(-9.0), false));
    cmd(
        m,
        Command::SetPan {
            strip: A,
            pan: 40,
            commit: false,
        },
    );
    cmd(m, decay(5.0, false));
    assert_eq!(previews(m).len(), 3);
    cmd(m, Command::CancelMixPreview);
    assert!(previews(m).is_empty());
    assert_eq!(undo_count(m), 0);
    assert_eq!(m.view().song.tracks[0].gain_db, Some(0.0));

    // A step under a drag, an undo, and another song.
    cmd(m, gain(T1, Some(-9.0), false));
    cmd(m, Command::ToggleMute { track: 1 });
    assert!(previews(m).is_empty());
    cmd(m, gain(T1, Some(-9.0), false));
    cmd(m, Command::Undo);
    assert!(previews(m).is_empty());
    cmd(m, gain(T1, Some(-9.0), false));
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(A, 0)),
        },
    );
    three_tracks(m);
    assert!(previews(m).is_empty());
    assert_eq!(m.view().params.selected, None);
    // A step ends every preview, also the one of another control.
    cmd(m, gain(T1, Some(-9.0), false));
    cmd(m, gain(A, Some(-3.0), false));
    cmd(m, gain(T1, Some(-9.0), true));
    assert!(
        previews(m).is_empty(),
        "the step ended the other preview too"
    );
}

#[test]
fn values_are_brought_into_range_and_strips_that_are_not_there_do_nothing() {
    let m = &mut ready();
    cmd(m, gain(T1, Some(40.0), true));
    assert_eq!(m.view().song.tracks[0].gain_db, Some(6.0));
    cmd(m, gain(T1, Some(-61.0), true));
    assert_eq!(m.view().song.tracks[0].gain_db, None, "below −60 is −∞");
    cmd(m, gain(T1, None, true));
    assert_eq!(undo_count(m), 2, "−∞ again is no step");
    cmd(m, gain(StripRef::Master, None, true));
    assert_eq!(m.view().song.master.gain_db, None);
    cmd(m, gain(B, Some(-0.3), true));
    assert_eq!(m.view().song.returns[1].gain_db, Some(-0.3));

    let pan = |strip, pan, commit| Command::SetPan { strip, pan, commit };
    cmd(m, pan(T1, -20, true));
    cmd(m, pan(A, 900, true));
    let song = m.view().song;
    assert_eq!((song.tracks[0].pan, song.returns[0].pan), (-20, 100));
    cmd(m, pan(StripRef::Master, 10, false));
    assert_eq!(previews(m), [Preview::Pan(StripId::Master, Pan::new(10))]);
    cmd(m, pan(StripRef::Master, 10, true));
    assert_eq!(m.view().song.master.pan, 10, "the master has a pan too");
    let steps = undo_count(m);

    let send = |track, bus, db, commit| Command::SetSend {
        track,
        bus,
        db,
        commit,
    };
    cmd(m, send(1, SendBus::B, Some(-18.0), false));
    assert_eq!(
        previews(m),
        [Preview::Send(TrackId(1), Bus::B, Gain::from_db(-18.0))]
    );
    cmd(m, send(1, SendBus::B, Some(-18.0), true));
    cmd(m, send(1, SendBus::A, None, true));
    assert_eq!(m.view().song.tracks[0].sends, [None, Some(-18.0)]);
    assert_eq!(undo_count(m), steps + 2);

    let gone = StripRef::Track { id: 9 };
    for commit in [false, true] {
        cmd(m, gain(gone, Some(-3.0), commit));
        cmd(m, pan(gone, 5, commit));
        cmd(m, send(9, SendBus::A, Some(-3.0), commit));
    }
    assert!(previews(m).is_empty());
    assert_eq!(undo_count(m), steps + 2);
}

#[test]
fn a_parameter_names_its_effect_and_is_clamped() {
    let m = &mut ready();
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(A, 0)),
        },
    );
    cmd(m, decay(99.0, false));
    assert_eq!(
        previews(m),
        [Preview::Param {
            strip: StripId::Return(Bus::A),
            slot: 0,
            kind: ModelEffect::Reverb,
            param: reverb::DECAY,
            value: 10.0
        }]
    );
    let values = |m: &Model| m.view().params.selected.builtin().values;
    assert_eq!(values(m)[0], 2.4, "a preview is not in the state");
    cmd(m, decay(4.0, true));
    assert!(previews(m).is_empty());
    assert_eq!(values(m)[0], 4.0);
    assert_eq!(undo_count(m), 1);
    cmd(m, Command::Undo);
    assert_eq!(values(m)[0], 2.4);

    // Another effect than the one in the slot, an empty slot, a parameter it has not.
    let stale = |effect, slot, param| Command::SetParam {
        insert: at(A, slot),
        effect,
        param,
        value: 1.0,
        commit: true,
    };
    cmd(m, stale(EffectKind::Delay, 0, 0));
    cmd(m, stale(EffectKind::Reverb, 1, 0));
    cmd(m, stale(EffectKind::Reverb, 0, 99));
    cmd(m, stale(EffectKind::Reverb, 9, 0));
    assert_eq!(values(m)[0], 2.4);
    assert_eq!(m.view().song.redo_count, 1, "none of them was a step");
}

#[test]
fn an_insert_is_selected_and_the_panel_opens_in_front() {
    let m = &mut ready();
    assert_eq!(m.view().params.selected, None);
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(T1, 0)),
        },
    );
    assert_eq!(m.view().params.selected, None, "an empty slot");
    assert!(!params_in_front(m));

    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(A, 0)),
        },
    );
    let selected = m.view().params.selected.builtin();
    assert_eq!(selected.insert, at(A, 0));
    assert_eq!(selected.effect, EffectKind::Reverb);
    assert_eq!(
        selected.values,
        [2.4, 60.0, 20.0, 6000.0, 120.0, 100.0, 100.0]
    );
    assert_eq!(selected.row, None);
    assert!(params_in_front(m));
    assert_eq!(undo_count(m), 0, "selecting is no step");

    // A row, and the status line for it; the loop goes in front (mock F4-Mixer).
    cmd(m, Command::SelectParam { param: Some(0) });
    assert_eq!(m.view().params.selected.builtin().row, Some(0));
    let said = |loop_bars| {
        Some(StatusMessage::ParamSelected {
            loop_bars,
            insert: at(A, 0),
            effect: EffectKind::Reverb,
            param: 0,
        })
    };
    assert_eq!(status(m), said(None));
    cmd(
        m,
        Command::SetLoop {
            from_bar: 5,
            to_bar: 9,
        },
    );
    assert_eq!(status(m), said(Some(BarSpan { from: 5, to: 9 })));
    cmd(m, Command::PlayStop);
    assert_eq!(m.view().transport.state, TransportState::Playing);
    assert_eq!(status(m), said(Some(BarSpan { from: 5, to: 9 })));
    cmd(m, Command::PlayStop);
    // Not while the panel is closed.
    cmd(m, Command::ToggleBand { band: Band::Params });
    assert_ne!(status(m), said(Some(BarSpan { from: 5, to: 9 })));
    assert!(m.view().params.selected.is_some(), "it stays selected");
    cmd(m, Command::ToggleBand { band: Band::Params });

    cmd(m, Command::SelectParam { param: Some(7) });
    assert_eq!(m.view().params.selected.builtin().row, None, "no such row");
    cmd(m, Command::SelectParam { param: Some(6) });
    // Another insert: no row. The master's limiter.
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(StripRef::Master, 0)),
        },
    );
    let selected = m.view().params.selected.builtin();
    assert_eq!((selected.effect, selected.row), (EffectKind::Limiter, None));
    assert_eq!(selected.values, [-0.3, 0.0, 100.0]);
    cmd(m, Command::SelectInsert { insert: None });
    assert_eq!(m.view().params.selected, None);
}

#[test]
fn an_effect_goes_in_from_the_shelf_and_comes_out() {
    let m = &mut ready();
    let slot = ShelfMode::InsertEffect { strip: T1, slot: 1 };
    // Not into a slot that holds an effect, nor into one that is not there.
    for mode in [
        ShelfMode::InsertEffect { strip: A, slot: 0 },
        ShelfMode::InsertEffect { strip: T1, slot: 4 },
        ShelfMode::InsertEffect {
            strip: StripRef::Track { id: 9 },
            slot: 0,
        },
    ] {
        cmd(m, Command::OpenShelf { mode });
        assert!(m.view().workspace.layout.right_open.is_empty());
    }
    cmd(m, Command::OpenShelf { mode: slot });
    let shelf = m.view().shelf;
    assert_eq!((shelf.mode, shelf.selected_effect()), (slot, None));
    assert_eq!(m.view().workspace.layout.right_front, Some(Band::Shelf));
    cmd(m, Command::ShelfConfirm);
    assert_eq!(undo_count(m), 0, "no row chosen");
    let choose = |effect| Command::ShelfSelectEffect { effect };
    cmd(m, choose(Some(EffectKind::Eq4)));
    cmd(m, choose(Some(EffectKind::Compressor)));
    assert_eq!(
        m.view().shelf.selected_effect(),
        Some(EffectKind::Compressor)
    );
    assert_eq!(undo_count(m), 0, "nothing is in the song before Enter");
    assert!(m.view().transport.can_record);

    cmd(m, Command::ShelfConfirm);
    let state = m.view();
    assert_eq!(
        state.song.tracks[0].inserts(),
        [None, Some(EffectKind::Compressor), None, None]
    );
    assert_eq!(state.song.undo_count, 1);
    assert_eq!(
        (state.shelf.mode, state.shelf.selected_effect()),
        (ShelfMode::NewTrack, None)
    );
    let selected = state.params.selected.builtin();
    assert_eq!(selected.insert, at(T1, 1));
    assert_eq!(selected.values, [-18.0, 4.0, 10.0, 100.0, 0.0]);
    assert!(params_in_front(m), "the new effect is shown at once");
    assert!(state.workspace.layout.right_open.contains(&Band::Shelf));

    // Undo takes it out and nothing is selected; redo does not select it again.
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.tracks[0].inserts(), [None; 4]);
    assert_eq!(m.view().params.selected, None);
    cmd(m, Command::Redo);
    assert_eq!(m.view().params.selected, None);

    // The shelf waiting for a slot that an undo or redo fills goes back to the instruments.
    cmd(m, Command::Undo);
    cmd(m, Command::OpenShelf { mode: slot });
    cmd(m, choose(Some(EffectKind::Chorus)));
    cmd(m, Command::Redo);
    let shelf = m.view().shelf;
    assert_eq!(
        (shelf.mode, shelf.selected_effect()),
        (ShelfMode::NewTrack, None)
    );
    // A row of the effects is not chosen while the shelf lists instruments.
    cmd(m, choose(Some(EffectKind::Chorus)));
    assert_eq!(m.view().shelf.selected_effect(), None);

    // Taking it out is a step; the selected one is no longer selected.
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(T1, 1)),
        },
    );
    cmd(m, Command::RemoveEffect { insert: at(T1, 1) });
    assert_eq!(m.view().song.tracks[0].inserts(), [None; 4]);
    assert_eq!(m.view().params.selected, None);
    let steps = undo_count(m);
    cmd(m, Command::RemoveEffect { insert: at(T1, 1) });
    assert_eq!(undo_count(m), steps, "an empty slot");
    cmd(m, Command::Undo);
    assert_eq!(
        m.view().song.tracks[0].inserts()[1],
        Some(EffectKind::Compressor)
    );
}

#[test]
fn a_deleted_track_takes_its_selected_insert_along() {
    let m = &mut ready();
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::InsertEffect { strip: T1, slot: 0 },
        },
    );
    cmd(
        m,
        Command::ShelfSelectEffect {
            effect: Some(EffectKind::Delay),
        },
    );
    cmd(m, Command::ShelfConfirm);
    cmd(m, Command::SelectParam { param: Some(3) });
    assert!(m.view().params.selected.is_some());
    cmd(m, Command::DeleteTrack { track: 1 });
    assert_eq!(m.view().params.selected, None);
    cmd(m, Command::Undo);
    assert_eq!(
        m.view().params.selected,
        None,
        "undo does not select it again"
    );
    assert_eq!(
        m.view().song.tracks[0].inserts()[0],
        Some(EffectKind::Delay)
    );
}

#[test]
fn the_mute_of_a_return_is_a_step() {
    let m = &mut ready();
    cmd(m, Command::ToggleReturnMute { bus: SendBus::B });
    let song = m.view().song;
    assert_eq!(
        (song.returns[0].muted, song.returns[1].muted),
        (false, true)
    );
    assert_eq!(song.undo_count, 1);
    cmd(m, Command::Undo);
    assert!(!m.view().song.returns[1].muted);
    cmd(m, Command::Redo);
    assert!(m.view().song.returns[1].muted);
}

#[test]
fn the_mixer_works_while_recording() {
    let m = &mut ready();
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(A, 0)),
        },
    );
    // A drag that began before the recording goes on and ends as a step.
    cmd(m, gain(T1, Some(-6.0), false));
    cmd(m, Command::SetCountIn { bars: 1 });
    cmd(m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    cmd(m, gain(T1, Some(-9.0), false));
    assert_eq!(
        previews(m),
        [Preview::Gain(
            StripId::Track(TrackId(1)),
            Gain::from_db(-9.0)
        )]
    );
    cmd(m, gain(T1, Some(-9.0), true));
    assert!(previews(m).is_empty());
    assert_eq!(m.view().song.tracks[0].gain_db, Some(-9.0));

    cmd(m, decay(5.0, true));
    cmd(m, Command::ToggleReturnMute { bus: SendBus::A });
    cmd(m, Command::RemoveEffect { insert: at(B, 0) });
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::InsertEffect { strip: T1, slot: 0 },
        },
    );
    cmd(
        m,
        Command::ShelfSelectEffect {
            effect: Some(EffectKind::Chorus),
        },
    );
    cmd(m, Command::ShelfConfirm);
    let state = m.view();
    assert_eq!(state.transport.state, TransportState::CountIn);
    assert_eq!(state.song.undo_count, 5);
    assert!(state.song.returns[0].muted);
    assert_eq!(state.song.returns[1].inserts()[0], None);
    assert_eq!(state.song.tracks[0].inserts()[0], Some(EffectKind::Chorus));
    assert_eq!(state.params.selected.builtin().effect, EffectKind::Chorus);

    // Instruments are not tried while recording, and nothing is undone.
    let shelf = |mode| Command::OpenShelf { mode };
    assert!(cmd(m, shelf(ShelfMode::NewTrack)).is_empty());
    cmd(m, Command::Undo);
    assert_eq!(m.view().song.undo_count, 5);
    // Looking is fine: another insert can be selected.
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(A, 0)),
        },
    );
    assert_eq!(
        m.view().params.selected.builtin().effect,
        EffectKind::Reverb
    );
}

fn builtin(effect: EffectKind) -> EffectName {
    EffectName::Builtin { effect }
}

fn passed_by(m: &Model, strip: StripRef) -> [bool; 4] {
    let song = m.view().song;
    let slots = match strip {
        StripRef::Track { id } => song.tracks.into_iter().find(|t| t.id == id).unwrap().slots,
        StripRef::Return { bus: SendBus::A } => song.returns[0].slots.clone(),
        StripRef::Return { bus: SendBus::B } => song.returns[1].slots.clone(),
        StripRef::Master => song.master.slots,
    };
    slots.map(|slot| slot.is_some_and(|i| i.bypassed))
}

/// Puts `effect` into an empty slot from the shelf.
fn put(m: &mut Model, strip: StripRef, slot: u8, effect: EffectKind) {
    let mode = ShelfMode::InsertEffect { strip, slot };
    cmd(m, Command::OpenShelf { mode });
    let effect = Some(effect);
    cmd(m, Command::ShelfSelectEffect { effect });
    cmd(m, Command::ShelfConfirm);
}

fn selected_at(m: &Model) -> (InsertRef, EffectKind, Option<u32>) {
    let selected = m.view().params.selected.builtin();
    (selected.insert, selected.effect, selected.row)
}

#[test]
fn passing_an_insert_by_is_a_step_that_is_said_and_keeps_the_values() {
    let m = &mut ready();
    let reverb = at(A, 0);
    let said = |bypassed| {
        Some(StatusMessage::EffectBypassed {
            insert: reverb,
            effect: builtin(EffectKind::Reverb),
            bypassed,
        })
    };
    cmd(m, Command::ToggleBypass { insert: reverb });
    assert_eq!(passed_by(m, A), [true, false, false, false]);
    assert_eq!(status(m), said(true));
    let song = m.view().song;
    assert_eq!((song.undo_count, song.file), (1, SongFile::Changed));

    // The parameters still move while it is passed by.
    cmd(m, decay(4.0, true));
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(reverb),
        },
    );
    assert_eq!(m.view().params.selected.builtin().values[0], 4.0);

    cmd(m, Command::ToggleBypass { insert: reverb });
    assert_eq!(passed_by(m, A), [false; 4]);
    assert_eq!(status(m), said(false));
    assert_eq!(undo_count(m), 3);
    cmd(m, Command::Undo);
    assert_eq!(passed_by(m, A), [true, false, false, false]);
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert_eq!(passed_by(m, A), [false; 4]);
    cmd(m, Command::Redo);
    assert_eq!(passed_by(m, A), [true, false, false, false]);

    // An empty slot, and a strip that is not there: nothing.
    let steps = undo_count(m);
    for insert in [
        at(T1, 0),
        at(A, 1),
        at(StripRef::Track { id: 9 }, 0),
        at(A, 7),
    ] {
        cmd(m, Command::ToggleBypass { insert });
        assert_eq!(undo_count(m), steps);
        assert!(!matches!(
            status(m),
            Some(StatusMessage::EffectBypassed { .. })
        ));
    }

    // While recording too.
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    crate::play::tests::started(m);
    cmd(
        m,
        Command::ToggleBypass {
            insert: at(StripRef::Master, 0),
        },
    );
    assert_eq!(m.view().transport.state, TransportState::Recording);
    assert_eq!(passed_by(m, StripRef::Master), [true, false, false, false]);
}

#[test]
fn an_insert_moves_inside_its_strip_and_what_names_it_goes_along() {
    let m = &mut ready();
    put(m, T1, 0, EffectKind::Eq4);
    assert_eq!(
        status(m),
        Some(StatusMessage::EffectInserted {
            insert: at(T1, 0),
            effect: builtin(EffectKind::Eq4),
        })
    );
    put(m, T1, 1, EffectKind::Compressor);
    cmd(m, Command::ToggleBypass { insert: at(T1, 0) });
    cmd(
        m,
        Command::SelectInsert {
            insert: Some(at(T1, 0)),
        },
    );
    cmd(m, Command::SelectParam { param: Some(2) });
    let steps = undo_count(m);

    // To the last slot: the ones between move up by one, the empty one too.
    let moved = Command::MoveInsert {
        insert: at(T1, 0),
        to: 3,
    };
    cmd(m, moved.clone());
    use EffectKind::{Compressor, Eq4};
    let inserts = |m: &Model| m.view().song.tracks[0].inserts();
    assert_eq!(inserts(m), [Some(Compressor), None, None, Some(Eq4)]);
    assert_eq!(passed_by(m, T1), [false, false, false, true]);
    assert_eq!(
        status(m),
        Some(StatusMessage::InsertsReordered {
            strip: T1,
            order: vec![builtin(Compressor), builtin(Eq4)],
        })
    );
    assert_eq!(undo_count(m), steps + 1);
    assert_eq!(
        selected_at(m),
        (at(T1, 3), Eq4, Some(2)),
        "it stays selected"
    );

    // Undo and redo take the selection along.
    cmd(m, Command::Undo);
    assert_eq!(inserts(m), [Some(Eq4), Some(Compressor), None, None]);
    assert_eq!(selected_at(m), (at(T1, 0), Eq4, Some(2)));
    cmd(m, Command::Redo);
    assert_eq!(selected_at(m), (at(T1, 3), Eq4, Some(2)));

    // Past the end is the end, where it is already; an empty slot does not move.
    for (slot, to) in [(3, 9), (3, 3), (1, 0), (2, 3), (7, 0)] {
        let insert = at(T1, slot);
        cmd(m, Command::MoveInsert { insert, to });
        assert_eq!(undo_count(m), steps + 1, "{slot} to {to}");
    }
    let nowhere = at(StripRef::Track { id: 9 }, 0);
    cmd(
        m,
        Command::MoveInsert {
            insert: nowhere,
            to: 1,
        },
    );
    assert_eq!(undo_count(m), steps + 1);

    // Another one moves past the selected one.
    let insert = at(T1, 0);
    cmd(m, Command::MoveInsert { insert, to: 9 });
    assert_eq!(inserts(m), [None, None, Some(Eq4), Some(Compressor)]);
    assert_eq!(selected_at(m), (at(T1, 2), Eq4, Some(2)));
    assert_eq!(passed_by(m, T1), [false, false, true, false]);

    // The shelf, open for a slot of the strip, stays with that slot.
    let replace = |slot| ShelfMode::ReplaceEffect { strip: T1, slot };
    cmd(m, Command::OpenShelf { mode: replace(2) });
    let insert = at(T1, 2);
    cmd(m, Command::MoveInsert { insert, to: 0 });
    assert_eq!(m.view().shelf.mode, replace(0));
    let empty = |slot| ShelfMode::InsertEffect { strip: T1, slot };
    cmd(m, Command::OpenShelf { mode: empty(1) });
    let insert = at(T1, 3);
    cmd(m, Command::MoveInsert { insert, to: 1 });
    assert_eq!(inserts(m), [Some(Eq4), Some(Compressor), None, None]);
    assert_eq!(m.view().shelf.mode, empty(2));
    cmd(m, Command::Undo);
    assert_eq!(m.view().shelf.mode, empty(1));

    // The returns and the master move theirs the same way.
    put(m, A, 2, EffectKind::Chorus);
    let insert = at(A, 2);
    cmd(m, Command::MoveInsert { insert, to: 0 });
    assert_eq!(
        m.view().song.returns[0].inserts(),
        [
            Some(EffectKind::Chorus),
            Some(EffectKind::Reverb),
            None,
            None
        ]
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::InsertsReordered {
            strip: A,
            order: vec![builtin(EffectKind::Chorus), builtin(EffectKind::Reverb)],
        })
    );
}

#[test]
fn an_effect_is_replaced_from_the_shelf_and_undo_brings_the_old_one_back_as_it_was() {
    let m = &mut ready();
    let reverb = at(A, 0);
    cmd(m, decay(4.0, true));
    cmd(m, Command::ToggleBypass { insert: reverb });
    let steps = undo_count(m);

    // Not for an empty slot, nor for one that is not there.
    for mode in [
        ShelfMode::ReplaceEffect { strip: A, slot: 1 },
        ShelfMode::ReplaceEffect { strip: T1, slot: 0 },
        ShelfMode::ReplaceEffect { strip: A, slot: 4 },
        ShelfMode::ReplaceEffect {
            strip: StripRef::Track { id: 9 },
            slot: 0,
        },
    ] {
        cmd(m, Command::OpenShelf { mode });
        assert!(m.view().workspace.layout.right_open.is_empty());
    }
    let mode = ShelfMode::ReplaceEffect { strip: A, slot: 0 };
    cmd(m, Command::OpenShelf { mode });
    let shelf = m.view().shelf;
    assert_eq!(m.view().workspace.layout.right_front, Some(Band::Shelf));
    assert_eq!((shelf.mode, shelf.chosen_effect), (mode, None));
    let current = EffectRef::Builtin {
        effect: EffectKind::Reverb,
    };
    assert_eq!(shelf.current_effect, Some(current));
    assert!(m.view().transport.can_record, "nothing is tried");

    // No row, and the row of what it holds already: no step, and the shelf waits.
    let choose = |effect| Command::ShelfSelectEffect { effect };
    cmd(m, Command::ShelfConfirm);
    cmd(m, choose(Some(EffectKind::Reverb)));
    cmd(m, Command::ShelfConfirm);
    assert_eq!(undo_count(m), steps);
    assert_eq!(m.view().shelf.mode, mode);

    cmd(m, choose(Some(EffectKind::Delay)));
    assert_eq!(undo_count(m), steps, "nothing is in the song before Enter");
    cmd(m, Command::ShelfConfirm);
    assert_eq!(
        status(m),
        Some(StatusMessage::InsertReplaced {
            insert: reverb,
            effect: builtin(EffectKind::Delay),
        })
    );
    let state = m.view();
    assert_eq!(state.song.undo_count, steps + 1);
    assert_eq!(state.song.returns[0].inserts()[0], Some(EffectKind::Delay));
    assert_eq!(passed_by(m, A), [false; 4], "the new one is heard");
    let shelf = state.shelf;
    assert_eq!(
        (shelf.mode, shelf.chosen_effect, shelf.current_effect),
        (ShelfMode::NewTrack, None, None)
    );
    // As a new one on a return, selected and shown.
    let selected = state.params.selected.builtin();
    assert_eq!(
        (selected.insert, selected.effect),
        (reverb, EffectKind::Delay)
    );
    assert_eq!(selected.values[7], 100.0);
    assert!(params_in_front(m));

    // Undo: the reverb with its decay, passed by as it was.
    cmd(m, Command::Undo);
    let selected = m.view().params.selected.builtin();
    assert_eq!(
        (selected.effect, selected.values[0]),
        (EffectKind::Reverb, 4.0)
    );
    assert_eq!(passed_by(m, A), [true, false, false, false]);
    cmd(m, Command::Redo);
    assert_eq!(
        m.view().song.returns[0].inserts()[0],
        Some(EffectKind::Delay)
    );

    // The shelf waiting for a slot that is emptied goes back to the instruments.
    cmd(m, Command::OpenShelf { mode });
    cmd(m, choose(Some(EffectKind::Chorus)));
    cmd(m, Command::RemoveEffect { insert: reverb });
    assert_eq!(
        status(m),
        Some(StatusMessage::EffectRemoved {
            insert: reverb,
            effect: builtin(EffectKind::Delay),
        })
    );
    let shelf = m.view().shelf;
    assert_eq!(
        (shelf.mode, shelf.chosen_effect),
        (ShelfMode::NewTrack, None)
    );

    // While recording too.
    cmd(m, Command::Undo);
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    crate::play::tests::started(m);
    cmd(m, Command::OpenShelf { mode });
    cmd(m, choose(Some(EffectKind::Chorus)));
    cmd(m, Command::ShelfConfirm);
    assert_eq!(
        m.view().song.returns[0].inserts()[0],
        Some(EffectKind::Chorus)
    );
    assert_eq!(m.view().transport.state, TransportState::Recording);
}

#[test]
fn a_typed_value_is_read_brought_into_range_or_turned_down() {
    let m = &mut ready();
    let of = |effect, param| ParamRef::Builtin {
        insert: at(A, 0),
        effect,
        param,
    };
    let typed = |m: &mut Model, text: &str| {
        let param = of(EffectKind::Reverb, reverb::DECAY);
        let text = text.to_owned();
        cmd(m, Command::TypeParam { param, text });
    };
    let decay_now = |m: &Model| {
        let reverb = m.play().song().doc.returns[0].channel.inserts[0].clone();
        reverb.unwrap().value(reverb::DECAY).unwrap()
    };
    let spoke = |m: &Model| {
        matches!(
            status(m),
            Some(StatusMessage::ParamClamped { .. } | StatusMessage::ParamRejected { .. })
        )
    };
    typed(m, "4");
    assert_eq!((decay_now(m), undo_count(m)), (4.0, 1));
    assert!(!spoke(m));
    typed(m, "1.5 s");
    assert_eq!((decay_now(m), undo_count(m)), (1.5, 2));
    typed(m, "800 ms");
    assert_eq!(decay_now(m), 0.8);
    typed(m, "0.8");
    assert_eq!(undo_count(m), 3, "the value it has is no step");

    // Past the end: the end, and the status line says so.
    let clamped = |value| {
        Some(StatusMessage::ParamClamped {
            to: ParamEnd::Builtin {
                effect: EffectKind::Reverb,
                param: reverb::DECAY,
                value,
            },
        })
    };
    typed(m, "99");
    assert_eq!((decay_now(m), undo_count(m)), (10.0, 4));
    assert_eq!(status(m), clamped(10.0));
    typed(m, "25 s");
    assert_eq!(undo_count(m), 4);
    assert_eq!(
        status(m),
        clamped(10.0),
        "said again, though nothing changed"
    );
    typed(m, "−3");
    assert_eq!(decay_now(m), 0.2);
    assert_eq!(status(m), clamped(0.2));

    // No number: nothing changes, and the status line names what stays.
    for text in ["", "long", "s"] {
        typed(m, text);
        assert_eq!((decay_now(m), undo_count(m)), (0.2, 5), "{text:?}");
        assert_eq!(
            status(m),
            Some(StatusMessage::ParamRejected {
                param: of(EffectKind::Reverb, reverb::DECAY),
                kept: "0.2".into(),
            })
        );
    }

    // A value typed while the control is previewed ends the preview.
    cmd(m, decay(7.0, false));
    typed(m, "3");
    assert!(previews(m).is_empty());
    assert_eq!(decay_now(m), 3.0);

    // Another effect than the slot holds, a parameter it has not, an empty slot: dropped.
    let steps = undo_count(m);
    for param in [
        of(EffectKind::Delay, 1),
        of(EffectKind::Reverb, 7),
        ParamRef::Builtin {
            insert: at(T1, 0),
            effect: EffectKind::Reverb,
            param: 0,
        },
    ] {
        let text = "5".to_owned();
        cmd(m, Command::TypeParam { param, text });
        assert_eq!(undo_count(m), steps);
        assert!(!spoke(m));
    }
    // A step and a switch read their number too.
    cmd(m, Command::RemoveEffect { insert: at(B, 0) });
    cmd(m, Command::Undo);
    for (param, text, value) in [(0, "3", 3.0), (0, "9", 4.0), (2, "0", 0.0)] {
        let param = ParamRef::Builtin {
            insert: at(B, 0),
            effect: EffectKind::Delay,
            param,
        };
        let text = text.to_owned();
        cmd(
            m,
            Command::TypeParam {
                param: param.clone(),
                text,
            },
        );
        cmd(
            m,
            Command::SelectInsert {
                insert: Some(at(B, 0)),
            },
        );
        let ParamRef::Builtin { param, .. } = param else {
            unreachable!()
        };
        assert_eq!(
            m.view().params.selected.builtin().values[param as usize],
            value
        );
    }
}

#[test]
fn opening_the_mixer_says_once_what_is_passed_by() {
    let m = &mut ready();
    let mixer = Command::ToggleBand { band: Band::Mixer };
    let open = |m: &Model| {
        let lower = m.view().workspace.layout.lower;
        lower.iter().any(|b| b.band == Band::Mixer)
    };
    assert!(!open(m));
    cmd(m, mixer.clone());
    assert!(open(m));
    assert!(!matches!(
        status(m),
        Some(StatusMessage::MixerPassesBy { .. })
    ));
    cmd(m, mixer.clone());
    put(m, T1, 2, EffectKind::Chorus);
    for insert in [at(StripRef::Master, 0), at(T1, 2), at(B, 0)] {
        cmd(m, Command::ToggleBypass { insert });
    }
    let named = |insert, effect| NamedInsert {
        insert,
        effect: builtin(effect),
    };
    let said = Some(StatusMessage::MixerPassesBy {
        bypassed: vec![
            named(at(T1, 2), EffectKind::Chorus),
            named(at(B, 0), EffectKind::Delay),
            named(at(StripRef::Master, 0), EffectKind::Limiter),
        ],
        stopped: Vec::new(),
    });
    cmd(m, mixer.clone());
    assert_eq!(status(m), said, "tracks, returns, master");
    // Once: the next command puts it away, and folding the mixer does not say it.
    cmd(
        m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    assert_ne!(status(m), said);
    cmd(m, mixer.clone());
    assert_ne!(status(m), said);
    cmd(m, mixer);
    assert_eq!(status(m), said);
}
