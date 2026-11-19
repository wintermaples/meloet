use meloet_model::effects::{EffectKind as ModelEffect, delay, limiter, reverb};
use meloet_model::{Bus, Gain, Insert, Pan, Song, StripId, TrackId};
use meloet_protocol::{
    Command, EffectKind, InsertRef, InstrumentKind, SendBus, ShelfMode, SongAnswer, StripRef,
    TransportState,
};

use crate::instruments;
use crate::play::mix::tests::three_tracks;
use crate::play::seats::tests::FakeEngine;
use crate::play::tests::{cmd, ready, record_once, started};
use crate::play::{AUDITION_SEAT, EngineOp, StripSeat};
use crate::songfile;
use crate::state::{Effect, Input, Model};

const T1: StripRef = StripRef::Track { id: 1 };
const T2: StripRef = StripRef::Track { id: 2 };
const A: StripRef = StripRef::Return { bus: SendBus::A };
const MASTER: StripRef = StripRef::Master;
const S1: StripId = StripId::Track(TrackId(1));
const S2: StripId = StripId::Track(TrackId(2));
const SA: StripId = StripId::Return(Bus::A);
const SB: StripId = StripId::Return(Bus::B);

fn rig() -> (Model, FakeEngine) {
    instruments::tests::EVERY_SOUND.set(true);
    let mut m = ready();
    let mut engine = FakeEngine::new(&mut m);
    engine.take(&three_tracks(&mut m));
    engine.check(&m, "three tracks");
    (m, engine)
}

/// The ops one command sends, after the engine took them and was compared with the song.
fn go(m: &mut Model, engine: &mut FakeEngine, command: Command) -> Vec<EngineOp> {
    let ctx = format!("{command:?}");
    let effects = cmd(m, command);
    engine.take(&effects);
    engine.check(m, &ctx);
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn gain(strip: StripRef, db: f32, commit: bool) -> Command {
    let db = Some(db);
    Command::SetGain { strip, db, commit }
}

fn at(strip: StripRef, slot: u8) -> InsertRef {
    InsertRef { strip, slot }
}

fn add(m: &mut Model, e: &mut FakeEngine, strip: StripRef, slot: u8, effect: EffectKind) {
    let mode = ShelfMode::InsertEffect { strip, slot };
    go(m, e, Command::OpenShelf { mode });
    let effect = Some(effect);
    go(m, e, Command::ShelfSelectEffect { effect });
}

/// Opens `song` as if read from a file, leaving what is not saved behind.
fn open(m: &mut Model, song: Song) -> Vec<Effect> {
    let mut effects = cmd(m, Command::OpenSong);
    if let Some(dialog) = m.view().workspace.song_dialog {
        effects.extend(cmd(
            m,
            Command::AnswerSongDialog {
                dialog: dialog.id,
                answer: SongAnswer::DontSave,
            },
        ));
    }
    let path = std::path::PathBuf::from("/nowhere/曲/曲.meloet");
    effects.extend(m.handle(Input::SongFilePicked(Some(path.clone()))));
    effects.extend(m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    }));
    effects
}

fn db(db: f64) -> Gain {
    Gain::from_db(db)
}

#[test]
fn the_engine_starts_with_the_mixer_of_the_new_song() {
    let m = &mut Model::new(Default::default());
    let ops = m.engine_seats();
    let insert = |strip, kind| EngineOp::Insert {
        strip,
        slot: 0,
        insert: Some(Insert::new(kind)),
    };
    assert_eq!(
        ops[2..],
        [
            EngineOp::Sends {
                seat: 0,
                sends: [db(-15.0), Gain::SILENT]
            },
            insert(StripSeat::Return(Bus::A), ModelEffect::Reverb),
            insert(StripSeat::Return(Bus::B), ModelEffect::Delay),
            insert(StripSeat::Master, ModelEffect::Limiter),
        ]
    );
    let limiter = Insert::new(ModelEffect::Limiter);
    assert_eq!(limiter.value(limiter::CEILING), Some(-0.3));
    // After that, an input that changes nothing in the mixer sends nothing.
    assert!(m.handle(Input::Tick).is_empty());
}

#[test]
fn a_drag_is_heard_at_every_move_and_only_on_its_strip() {
    let (m, e) = &mut rig();
    let strip = StripSeat::Track(0);
    for (value, commit) in [(-3.0, false), (-6.0, false), (-6.0, true)] {
        let ops = go(m, e, gain(T1, value, commit));
        let heard = [EngineOp::Strip {
            strip,
            gain: db(f64::from(value)),
            pan: Pan::CENTER,
        }];
        // The end of the drag is a step, and the engine has the value already.
        assert_eq!(ops, if commit { &heard[..0] } else { &heard[..] });
    }
    assert_eq!(m.view().song.undo_count, 1);

    // Esc: the sound is the song's again.
    go(m, e, gain(A, -12.0, false));
    go(m, e, gain(MASTER, 3.0, false));
    let ops = go(m, e, Command::CancelMixPreview);
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert_eq!(e.strip(m, SA).gain, Gain::UNITY);

    // Pan and sends, on a track the song plays while it is muted too.
    go(m, e, Command::ToggleMute { track: 2 });
    let pan = |pan, commit| Command::SetPan {
        strip: T2,
        pan,
        commit,
    };
    go(m, e, pan(-40, false));
    assert_eq!(e.strip(m, S2).pan, Pan::new(-40));
    go(m, e, pan(-50, true));
    let send = |db, commit| Command::SetSend {
        track: 2,
        bus: SendBus::B,
        db,
        commit,
    };
    let ops = go(m, e, send(Some(-20.0), false));
    assert_eq!(
        ops,
        [EngineOp::Sends {
            seat: 1,
            sends: [Gain::SILENT, db(-20.0)]
        }]
    );
    go(m, e, send(None, true));
    assert_eq!(e.strip(m, S2).sends, [Gain::SILENT; 2]);
    go(m, e, pan(100, false));
    go(m, e, Command::Undo);
    assert_eq!(e.strip(m, S2).pan, Pan::CENTER, "undo ends the preview too");
}

#[test]
fn an_effect_is_made_only_when_its_slot_gets_another_kind() {
    let (m, e) = &mut rig();
    let made = e.effects_made;
    let reverb = e.strip(m, SA).inserts[0].clone().unwrap();
    assert_eq!(reverb.1.kind(), Some(ModelEffect::Reverb));

    // Parameters go to the effect that is there.
    let decay = |value, commit| Command::SetParam {
        insert: at(A, 0),
        effect: EffectKind::Reverb,
        param: reverb::DECAY,
        value,
        commit,
    };
    let ops = go(m, e, decay(4.0, false));
    assert_eq!(
        ops,
        [EngineOp::Param {
            strip: StripSeat::Return(Bus::A),
            slot: 0,
            param: reverb::DECAY,
            value: 4.0
        }]
    );
    assert!(go(m, e, decay(4.0, true)).is_empty());
    go(m, e, decay(99.0, false));
    assert_eq!(
        e.strip(m, SA).inserts[0].as_ref().unwrap().1.value(0),
        Some(10.0)
    );
    go(m, e, Command::CancelMixPreview);
    go(m, e, gain(A, -3.0, true));
    go(m, e, Command::ToggleReturnMute { bus: SendBus::A });
    assert_eq!(
        e.strip(m, SA).gain,
        Gain::SILENT,
        "a muted return is silent"
    );
    go(m, e, Command::ToggleReturnMute { bus: SendBus::A });
    assert_eq!(e.strip(m, SA).gain, db(-3.0));
    for _ in 0..4 {
        go(m, e, Command::Undo);
    }
    assert_eq!(e.effects_made, made);
    assert_eq!(e.strip(m, SA).inserts[0].as_ref().unwrap().0, reverb.0);

    // An effect goes in, with the mix a reverb starts with on a track; the others stay.
    add(m, e, T1, 2, EffectKind::Reverb);
    let ops = go(m, e, Command::ShelfConfirm);
    let mut on_a_track = Insert::new(ModelEffect::Reverb);
    on_a_track.set(reverb::MIX, 30.0);
    assert_eq!(
        ops,
        [EngineOp::Insert {
            strip: StripSeat::Track(0),
            slot: 2,
            insert: Some(on_a_track)
        }]
    );
    add(m, e, MASTER, 1, EffectKind::Delay);
    go(m, e, Command::ShelfConfirm);
    let on_master = e.strip(m, StripId::Master).inserts[1].clone().unwrap().1;
    assert_eq!(on_master.value(delay::MIX), Some(30.0));
    add(m, e, A, 1, EffectKind::Delay);
    go(m, e, Command::ShelfConfirm);
    let on_return = e.strip(m, SA).inserts[1].clone().unwrap().1;
    assert_eq!(on_return, Insert::new(ModelEffect::Delay));
    assert_eq!(e.effects_made, made + 3);

    // Out, back with undo (a new one), out again with redo.
    let ops = go(m, e, Command::RemoveEffect { insert: at(T1, 2) });
    assert_eq!(
        ops,
        [EngineOp::Insert {
            strip: StripSeat::Track(0),
            slot: 2,
            insert: None
        }]
    );
    go(m, e, Command::Undo);
    assert!(e.strip(m, S1).inserts[2].is_some());
    go(m, e, Command::Redo);
    assert!(e.strip(m, S1).inserts[2].is_none());
    assert_eq!(e.effects_made, made + 6);
    assert_eq!(e.strip(m, SA).inserts[0].as_ref().unwrap().0, reverb.0);
}

#[test]
fn moving_tracks_sends_nothing_and_a_deleted_track_comes_back_with_its_strip() {
    let (m, e) = &mut rig();
    add(m, e, T2, 0, EffectKind::Eq4);
    go(m, e, Command::ShelfConfirm);
    go(m, e, gain(T2, -5.0, true));
    let eq = e.strip(m, S2).inserts[0].clone().unwrap();
    let made = e.effects_made;

    assert!(go(m, e, Command::MoveTrack { track: 2, to: 0 }).is_empty());
    assert!(go(m, e, Command::MoveTrack { track: 3, to: 1 }).is_empty());
    go(m, e, Command::DeleteTrack { track: 1 });
    assert_eq!(e.strip(m, S2).inserts[0], Some(eq.clone()), "others stay");
    assert_eq!(e.effects_made, made);

    let ops = go(m, e, Command::DeleteTrack { track: 2 });
    assert!(ops.contains(&EngineOp::ClearSeat { seat: 1 }), "{ops:?}");
    go(m, e, Command::Undo);
    let back = e.strip(m, S2);
    assert_eq!(back.gain, db(-5.0));
    assert_eq!(back.inserts[0].as_ref().unwrap().1, eq.1);
    assert_ne!(back.inserts[0].as_ref().unwrap().0, eq.0, "made anew");
}

#[test]
fn a_fader_does_not_send_the_notes_again() {
    let (m, e) = &mut rig();
    e.take(&record_once(m, &[(60, 0.0, 100.0)], 400.0));
    e.check(m, "recorded");
    e.take(&cmd(m, Command::PlayStop));
    for command in [
        gain(T1, -3.0, true),
        Command::ToggleReturnMute { bus: SendBus::B },
        Command::RemoveEffect { insert: at(A, 0) },
        Command::Undo,
    ] {
        let ops = go(m, e, command);
        assert!(
            !ops.iter().any(|op| matches!(op, EngineOp::Sequence { .. })),
            "{ops:?}"
        );
        assert!(!ops.is_empty());
    }
}

#[test]
fn an_auditioned_instrument_sends_as_its_track_will() {
    let (m, e) = &mut rig();
    let select = |kind| Command::ShelfSelect {
        instrument: Some(kind),
    };
    let mode = ShelfMode::NewTrack;
    go(m, e, Command::OpenShelf { mode });
    let ops = go(m, e, select(InstrumentKind::StudioKit));
    assert!(ops.contains(&EngineOp::Sends {
        seat: AUDITION_SEAT,
        sends: [db(-24.0), Gain::SILENT]
    }));
    go(m, e, select(InstrumentKind::FingerBass));
    assert_eq!(e.audition_strip().sends, [Gain::SILENT; 2]);
    go(m, e, select(InstrumentKind::WarmPad));

    // Enter: the new seat has its sends before the instrument moves onto it.
    let ops = go(m, e, Command::ShelfConfirm);
    let sends = ops.iter().position(|op| {
        *op == EngineOp::Sends {
            seat: 3,
            sends: [db(-15.0), Gain::SILENT],
        }
    });
    let moved = ops.iter().position(|op| {
        *op == EngineOp::MoveInstrument {
            from: AUDITION_SEAT,
            to: 3,
        }
    });
    assert!(sends.is_some() && sends < moved, "{ops:?}");
    assert_eq!(e.audition_strip().sends, [Gain::SILENT; 2]);

    // Esc on a row: the audition seat is emptied, and its strip with it.
    go(m, e, select(InstrumentKind::Organ));
    let ops = go(m, e, Command::ShelfCancel);
    assert_eq!(ops.len(), 2, "the target and the seat: {ops:?}");
    go(m, e, select(InstrumentKind::Organ));
    assert_eq!(e.audition_strip().sends[0], db(-15.0));
}

#[test]
fn another_song_sends_what_differs_and_keeps_the_effects_that_stay() {
    let (m, e) = &mut rig();
    add(m, e, T1, 0, EffectKind::Compressor);
    go(m, e, Command::ShelfConfirm);
    go(m, e, gain(MASTER, -2.0, true));
    let limiter = e.strip(m, StripId::Master).inserts[0].clone().unwrap();
    let delay = e.strip(m, SB).inserts[0].clone().unwrap();

    // The same song from its file, with one value changed.
    let mut song = songfile::decode(&songfile::encode(m.play().song())).unwrap();
    let mut master = (*song.doc.master).clone();
    master.inserts[0]
        .as_mut()
        .unwrap()
        .set(limiter::CEILING, -1.0);
    song.doc.master = master.into();
    e.take(&open(m, song));
    e.check(m, "read");
    let now = e.strip(m, StripId::Master).inserts[0].clone().unwrap();
    assert_eq!(now.0, limiter.0, "the same limiter");
    assert_eq!(now.1.value(limiter::CEILING), Some(-1.0));
    assert_eq!(e.strip(m, SB).inserts[0], Some(delay));
    assert_eq!(e.strip(m, StripId::Master).gain, db(-2.0));

    // A new song: the mixer of a new song.
    e.take(&open(m, Song::new_default()));
    e.check(m, "new");
    assert!(e.strip(m, S1).inserts[0].is_none());
    assert_eq!(e.strip(m, StripId::Master).gain, Gain::UNITY);
}

#[test]
fn the_engine_hears_the_mixer_while_recording() {
    let (m, e) = &mut rig();
    cmd(m, Command::SetCountIn { bars: 0 });
    e.take(&cmd(m, Command::Record));
    e.take(&started(m));
    assert_eq!(m.view().transport.state, TransportState::Recording);
    assert_eq!(go(m, e, gain(T1, -4.0, false)).len(), 1);
    assert!(go(m, e, gain(T1, -4.0, true)).is_empty());
    assert_eq!(
        go(m, e, Command::ToggleReturnMute { bus: SendBus::A }).len(),
        1
    );
    assert_eq!(
        go(m, e, Command::RemoveEffect { insert: at(A, 0) }).len(),
        1
    );
    let ops = go(m, e, Command::ToggleMute { track: 2 });
    assert_eq!(ops, [EngineOp::SilentTracks { tracks: vec![1] }]);
    assert_eq!(m.view().transport.state, TransportState::Recording);
    assert_eq!(m.view().song.undo_count, 4);
}

#[test]
fn passing_by_and_moving_tell_the_engine_and_make_no_effect() {
    let (m, e) = &mut rig();
    add(m, e, T1, 0, EffectKind::Eq4);
    go(m, e, Command::ShelfConfirm);
    add(m, e, T1, 1, EffectKind::Compressor);
    go(m, e, Command::ShelfConfirm);
    let eq = e.strip(m, S1).inserts[0].clone().unwrap();
    let made = e.effects_made;
    let strip = StripSeat::Track(0);
    let bypass = |slot, bypassed| EngineOp::Bypass {
        strip,
        slot,
        bypassed,
    };
    let moved = |from, to| EngineOp::MoveInsert { strip, from, to };

    let ops = go(m, e, Command::ToggleBypass { insert: at(T1, 0) });
    assert_eq!(ops, [bypass(0, true)]);
    let insert = at(T1, 0);
    let ops = go(m, e, Command::MoveInsert { insert, to: 2 });
    assert_eq!(ops, [moved(0, 2)], "the mark goes along in the engine");
    assert_eq!(e.strip(m, S1).inserts[2], Some(eq.clone()), "the same one");
    assert_eq!(e.strip(m, S1).bypassed, [false, false, true, false]);

    // A parameter of the moved one goes to where it is now.
    let low = Command::SetParam {
        insert: at(T1, 2),
        effect: EffectKind::Eq4,
        param: 1,
        value: 6.0,
        commit: true,
    };
    let ops = go(m, e, low);
    assert_eq!(
        ops,
        [EngineOp::Param {
            strip,
            slot: 2,
            param: 1,
            value: 6.0
        }]
    );

    assert_eq!(go(m, e, Command::Undo).len(), 1);
    assert_eq!(go(m, e, Command::Undo), [moved(2, 0)]);
    assert_eq!(go(m, e, Command::Undo), [bypass(0, false)]);
    assert_eq!(e.strip(m, S1).inserts[0], Some(eq.clone()));
    assert_eq!(go(m, e, Command::Redo), [bypass(0, true)]);
    assert_eq!(go(m, e, Command::Redo), [moved(0, 2)]);
    assert_eq!(e.effects_made, made, "nothing made, nothing put away");

    // Replaced: a new effect, heard; undo makes the old one anew, passed by as it was.
    let mode = ShelfMode::ReplaceEffect { strip: T1, slot: 2 };
    go(m, e, Command::OpenShelf { mode });
    let effect = Some(EffectKind::Reverb);
    go(m, e, Command::ShelfSelectEffect { effect });
    let ops = go(m, e, Command::ShelfConfirm);
    let mut reverb = Insert::new(ModelEffect::Reverb);
    reverb.set(reverb::MIX, 30.0);
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert!(ops.contains(&bypass(2, false)));
    assert!(ops.contains(&EngineOp::Insert {
        strip,
        slot: 2,
        insert: Some(reverb)
    }));
    let ops = go(m, e, Command::Undo);
    assert_eq!(ops.len(), 2, "{ops:?}");
    assert!(ops.contains(&bypass(2, true)));
    assert_eq!(e.effects_made, made + 2);
    let back = e.strip(m, S1).inserts[2].clone().unwrap();
    assert_eq!(back.1.kind(), Some(ModelEffect::Eq4));
    assert_ne!(back.0, eq.0, "made anew");

    // While a control is dragged, a move ends the preview and takes the effect as it is.
    go(m, e, gain(T1, -3.0, false));
    let insert = at(T1, 0);
    let ops = go(m, e, Command::MoveInsert { insert, to: 3 });
    assert!(ops.contains(&moved(0, 3)), "{ops:?}");
    assert_eq!(e.strip(m, S1).gain, Gain::UNITY);
}

#[test]
fn what_is_passed_by_and_the_order_come_back_from_the_file_and_for_a_new_engine() {
    let (m, e) = &mut rig();
    add(m, e, T2, 0, EffectKind::Eq4);
    go(m, e, Command::ShelfConfirm);
    add(m, e, T2, 1, EffectKind::Chorus);
    go(m, e, Command::ShelfConfirm);
    go(m, e, Command::ToggleBypass { insert: at(T2, 0) });
    let insert = at(T2, 0);
    go(m, e, Command::MoveInsert { insert, to: 3 });
    go(m, e, Command::ToggleBypass { insert: at(A, 0) });
    let made = e.effects_made;
    let before = m.view().song;

    // Saved and read: the same song, and the engine keeps what it has.
    let song = songfile::decode(&songfile::encode(m.play().song())).unwrap();
    let passed = |song: &Song, strip, slot| song.doc.insert(strip, slot).unwrap().bypassed;
    assert!(passed(&song, S2, 3) && passed(&song, SA, 0) && !passed(&song, S2, 0));
    assert_eq!(
        song.doc.insert(S2, 0).unwrap().kind(),
        Some(ModelEffect::Chorus)
    );
    e.take(&open(m, song));
    e.check(m, "read");
    assert_eq!(e.effects_made, made);
    let after = m.view().song;
    assert_eq!(after.tracks[1].slots, before.tracks[1].slots);
    assert_eq!(after.returns, before.returns);
    assert_eq!(e.strip(m, S2).bypassed, [false, false, false, true]);

    // An engine made anew is told the marks with the effects.
    let ops = m.engine_seats();
    for (strip, slot) in [(StripSeat::Track(1), 3), (StripSeat::Return(Bus::A), 0)] {
        let mark = EngineOp::Bypass {
            strip,
            slot,
            bypassed: true,
        };
        assert!(ops.contains(&mark), "{ops:?}");
    }
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op, EngineOp::Bypass { .. }))
            .count(),
        2
    );
    let mut book = crate::mixbook::MixBook::default();
    for op in &ops {
        book.note(op);
    }
    book.note(&EngineOp::MoveInsert {
        strip: StripSeat::Track(1),
        from: 3,
        to: 1,
    });
    let again = book.ops();
    assert!(again.contains(&EngineOp::Bypass {
        strip: StripSeat::Track(1),
        slot: 1,
        bypassed: true
    }));
    assert!(again.iter().any(|op| matches!(
        op,
        EngineOp::Insert { strip: StripSeat::Track(1), slot: 1, insert: Some(i) }
            if i.kind() == Some(ModelEffect::Eq4)
    )));
    assert!(!again.iter().any(|op| matches!(
        op,
        EngineOp::Insert {
            strip: StripSeat::Track(1),
            slot: 3,
            ..
        }
    )));

    // A new song has nothing passed by.
    e.take(&open(m, Song::new_default()));
    e.check(m, "new");
    assert_eq!(e.strip(m, SA).bypassed, [false; 4]);
}
