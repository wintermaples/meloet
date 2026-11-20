use crate::seen::*;
use meloet_devices::{ClockSettings, DevicesPicture};
use meloet_protocol::{
    Band, Command, InstrumentKind, ShelfMode, StatusMessage, TrackRef, TransportState,
};

use crate::instruments;
use crate::play::mix::tests::three_tracks;
use crate::play::seats::AUDITION_SEAT;
use crate::play::seats::tests::FakeEngine;
use crate::play::tests::{cmd, ready, status};
use crate::play::{EngineOp, Play};
use crate::state::{Effect, Input, Model};

use InstrumentKind::{ElectricPiano, GrandPiano, Organ, Sine, SynthBass};

/// Three tracks (グランドピアノ, サイン波, サイン波), every sound in the shelf, and the engine.
fn rig() -> (Model, FakeEngine) {
    instruments::tests::EVERY_SOUND.set(true);
    let mut m = ready();
    let mut engine = FakeEngine::new(&mut m);
    engine.take(&three_tracks(&mut m));
    (m, engine)
}

fn go(m: &mut Model, engine: &mut FakeEngine, command: Command) {
    let ctx = format!("{command:?}");
    engine.take(&cmd(m, command));
    engine.check(m, &ctx);
}

fn select(instrument: InstrumentKind) -> Command {
    Command::ShelfSelect {
        instrument: Some(instrument),
    }
}

fn open(mode: ShelfMode) -> Command {
    Command::OpenShelf { mode }
}

fn shelf_open(m: &Model) -> bool {
    m.view().workspace.layout.right_open.contains(&Band::Shelf)
}

fn rows(m: &Model) -> Vec<(u32, String, u8, InstrumentKind)> {
    m.view()
        .song
        .tracks
        .iter()
        .map(|t| (t.id, t.name.clone(), t.color, t.instrument()))
        .collect()
}

#[test]
fn a_new_track_is_auditioned_on_its_own_seat_then_added_below_the_selected_one() {
    let (m, e) = &mut rig();
    go(m, e, open(ShelfMode::NewTrack));
    assert!(shelf_open(m));
    assert_eq!(m.view().workspace.layout.right_front, Some(Band::Shelf));
    assert_eq!(m.view().shelf.selected(), None);
    assert!(m.view().transport.can_record);

    go(m, e, select(SynthBass));
    assert_eq!(m.view().shelf.selected(), Some(SynthBass));
    assert_eq!(e.target(), Some(AUDITION_SEAT), "the keys play the row");
    assert_eq!(e.sounding(AUDITION_SEAT), Some(SynthBass));
    assert_eq!(
        status(m),
        Some(StatusMessage::AuditionNewTrack {
            instrument: SynthBass
        })
    );
    assert_eq!(m.view().song.tracks.len(), 3, "not in the song yet");
    assert_eq!(m.view().song.undo_count, 0);

    // Nothing is recorded while a row is auditioned.
    assert!(!m.view().transport.can_record);
    go(m, e, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::Stopped);
    assert_eq!(m.view().transport.recording, None);

    // Another row takes its place.
    go(m, e, select(Organ));
    assert_eq!(e.sounding(AUDITION_SEAT), Some(Organ));

    go(
        m,
        e,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );
    assert_eq!(e.target(), Some(AUDITION_SEAT), "still the row");
    let effects = cmd(m, Command::ShelfConfirm);
    assert!(
        effects.contains(&Effect::Engine(EngineOp::MoveInstrument {
            from: AUDITION_SEAT,
            to: 3
        })),
        "the instrument that was played is the track's: none is made anew"
    );
    e.take(&effects);
    e.check(m, "confirmed");
    assert_eq!(
        rows(m),
        [
            (1, "ピアノ".to_owned(), 1, GrandPiano),
            (2, "トラック 2".to_owned(), 2, Sine),
            (4, "オルガン".to_owned(), 3, Organ),
            (3, "トラック 3".to_owned(), 3, Sine),
        ],
        "right below the selected track, with a new id"
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::TrackAdded {
            track: TrackRef {
                number: 3,
                name: "オルガン".into()
            }
        })
    );
    let view = m.view();
    assert_eq!(view.song.play_target, Some(4), "the new track is selected");
    assert_eq!(view.song.undo_count, 1, "one step");
    assert_eq!(view.shelf.selected(), None);
    assert!(shelf_open(m), "the shelf stays open");
    assert!(view.transport.can_record);
    assert_eq!(e.target(), Some(3));

    // Enter with no row does nothing.
    go(m, e, Command::ShelfConfirm);
    assert_eq!(m.view().song.undo_count, 1);

    go(m, e, Command::Undo);
    assert_eq!(m.view().song.tracks.len(), 3);
    assert!(m.view().song.play_target.is_some());
    go(m, e, Command::Redo);
    assert_eq!(rows(m)[2], (4, "オルガン".to_owned(), 3, Organ));
    // The next one never gets the id again.
    go(m, e, Command::Undo);
    go(m, e, select(Sine));
    go(m, e, Command::ShelfConfirm);
    assert!(rows(m).iter().any(|r| r.0 == 5 && r.1 == "シンセ"));
}

#[test]
fn a_song_with_no_tracks_gets_its_first_track_from_the_shelf() {
    let (m, e) = &mut rig();
    for track in [1, 2, 3] {
        go(m, e, Command::DeleteTrack { track });
    }
    go(m, e, open(ShelfMode::NewTrack));
    go(m, e, select(GrandPiano));
    assert_eq!(e.target(), Some(AUDITION_SEAT));
    go(m, e, Command::ShelfConfirm);
    assert_eq!(rows(m), [(4, "ピアノ".to_owned(), 1, GrandPiano)]);
    assert_eq!(m.view().song.play_target, Some(4));
    assert!(m.view().transport.can_record);
}

#[test]
fn an_audition_ends_with_cancel_esc_closing_the_shelf_and_a_row_that_is_not_there() {
    let (m, e) = &mut rig();
    let audition = |m: &mut Model, e: &mut FakeEngine| {
        go(m, e, open(ShelfMode::NewTrack));
        go(m, e, select(Organ));
        assert_eq!(e.target(), Some(AUDITION_SEAT));
    };
    let ended = |m: &Model, e: &FakeEngine| {
        assert_eq!(m.view().shelf.selected(), None);
        assert_eq!(
            e.target(),
            Some(0),
            "the keys are on the selected track again"
        );
        assert_eq!(e.sounding(AUDITION_SEAT), None);
        assert!(m.view().transport.can_record);
        assert_eq!(m.view().song.undo_count, 0);
    };

    audition(m, e);
    go(m, e, Command::ShelfCancel);
    ended(m, e);
    assert!(shelf_open(m));

    // Esc ends the audition first, and closes the shelf the next time.
    audition(m, e);
    go(m, e, Command::Back);
    ended(m, e);
    assert!(shelf_open(m));
    go(m, e, Command::Back);
    assert!(!shelf_open(m));

    audition(m, e);
    go(m, e, Command::ToggleBand { band: Band::Shelf });
    assert!(!shelf_open(m));
    ended(m, e);

    audition(m, e);
    go(m, e, Command::ShelfSelect { instrument: None });
    ended(m, e);

    // A sound that is not on this PC is no row: without a sound pack, only the sine wave
    // (spec 3, 85).
    instruments::tests::EVERY_SOUND.set(false);
    assert_eq!(m.view().shelf.available, [Sine]);
    assert!(m.view().library.pack_missing);
    go(m, e, select(Sine));
    assert_eq!(m.view().shelf.selected(), Some(Sine));
    go(m, e, select(Organ));
    ended(m, e);
    instruments::tests::EVERY_SOUND.set(true);

    // Opening the shelf again, in whatever mode, puts the row away.
    audition(m, e);
    go(m, e, open(ShelfMode::NewTrack));
    ended(m, e);

    // A closed shelf takes no row.
    go(m, e, Command::ToggleBand { band: Band::Shelf });
    go(m, e, select(Organ));
    ended(m, e);
}

#[test]
fn another_song_and_another_output_end_the_audition() {
    let (m, e) = &mut rig();
    go(m, e, open(ShelfMode::Replace { track: 2 }));
    go(m, e, select(Organ));
    assert_eq!(e.sounding(1), Some(Organ));
    e.take(&three_tracks(m));
    e.check(m, "another song");
    let shelf = m.view().shelf;
    assert_eq!((shelf.mode, shelf.selected()), (ShelfMode::NewTrack, None));
    assert!(shelf_open(m), "the shelf itself stays");

    // The output goes away: the stream closes, and the row with it.
    go(m, e, select(Organ));
    assert_eq!(e.target(), Some(AUDITION_SEAT));
    let effects = m.handle(Input::Devices(DevicesPicture {
        outputs: vec![],
        inputs: vec![],
        clock: ClockSettings::default(),
    }));
    assert!(effects.contains(&Effect::CloseOutput));
    e.take(&effects);
    e.check(m, "the output is gone");
    assert_eq!(m.view().shelf.selected(), None);
    assert_eq!(e.target(), Some(0));

    // Without a stream a row can still be picked and a track added.
    go(m, e, select(SynthBass));
    go(m, e, Command::ShelfConfirm);
    assert_eq!(m.view().song.tracks.len(), 4);
}

#[test]
fn the_shelf_does_not_open_or_audition_while_counting_in() {
    let (m, e) = &mut rig();
    go(m, e, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    go(m, e, open(ShelfMode::NewTrack));
    assert!(!shelf_open(m));
    go(m, e, Command::ToggleBand { band: Band::Shelf });
    go(m, e, select(Organ));
    assert_eq!(m.view().shelf.selected(), None);
    go(m, e, Command::ShelfConfirm);
    assert_eq!(m.view().song.tracks.len(), 3);
}

#[test]
fn replacing_tries_the_row_on_the_track_and_puts_its_own_instrument_back_on_cancel() {
    let (m, e) = &mut rig();
    let own = e.owns(m);
    go(m, e, open(ShelfMode::Replace { track: 9 }));
    assert!(!shelf_open(m), "a track that is not there");

    go(m, e, open(ShelfMode::Replace { track: 2 }));
    let shelf = m.view().shelf;
    assert_eq!(shelf.mode, ShelfMode::Replace { track: 2 });
    assert_eq!((shelf.current(), shelf.selected()), (Some(Sine), None));
    assert_eq!(e.target(), Some(0), "nothing is tried yet");

    go(m, e, select(ElectricPiano));
    assert_eq!(
        e.sounding(1),
        Some(ElectricPiano),
        "the track plays with it"
    );
    assert_eq!(e.target(), Some(1), "and so do the keys");
    assert_eq!(rows(m)[1].3, Sine, "the song is as it was");
    assert_eq!(m.view().song.undo_count, 0);
    assert!(!m.view().transport.can_record);
    assert_eq!(
        status(m),
        Some(StatusMessage::AuditionReplace {
            instrument: ElectricPiano,
            track: TrackRef {
                number: 2,
                name: "トラック 2".into()
            },
            current: Sine,
        })
    );
    go(m, e, select(Organ));
    assert_eq!(e.sounding(1), Some(Organ));
    assert_eq!(e.owns(m), own, "its own instrument waits, the same one");

    go(m, e, Command::Back);
    assert_eq!(e.sounding(1), Some(Sine));
    assert_eq!(e.owns(m), own, "the one it had, not a new one");
    assert_eq!(e.target(), Some(0));
    let shelf = m.view().shelf;
    assert_eq!(shelf.mode, ShelfMode::Replace { track: 2 });
    assert_eq!(shelf.selected(), None);
    assert!(m.view().transport.can_record);

    // The instrument it has already: no step, and its own one stays.
    go(m, e, select(Sine));
    go(m, e, Command::ShelfConfirm);
    assert_eq!(m.view().song.undo_count, 0);
    assert_eq!(m.view().shelf.selected(), None);
    assert_eq!(e.owns(m), own);
}

#[test]
fn replacing_is_one_step_that_keeps_the_name_and_undo_brings_the_instrument_back() {
    let (m, e) = &mut rig();
    go(m, e, open(ShelfMode::Replace { track: 1 }));
    go(m, e, select(ElectricPiano));
    let effects = cmd(m, Command::ShelfConfirm);
    assert_eq!(
        effects
            .iter()
            .filter(|x| matches!(x, Effect::Engine(_)))
            .collect::<Vec<_>>(),
        [&Effect::Engine(EngineOp::EndTrial {
            seat: 0,
            keep: true
        }),],
        "the instrument that was tried stays; none is made anew"
    );
    e.take(&effects);
    e.check(m, "replaced");
    assert_eq!(rows(m)[0], (1, "ピアノ".to_owned(), 1, ElectricPiano));
    assert_eq!(
        status(m),
        Some(StatusMessage::InstrumentReplaced {
            lanes_removed: 0,
            track: TrackRef {
                number: 1,
                name: "ピアノ".into()
            },
            instrument: ElectricPiano,
        })
    );
    let view = m.view();
    assert_eq!(view.song.undo_count, 1);
    assert_eq!(view.shelf.mode, ShelfMode::Replace { track: 1 });
    assert_eq!(
        (view.shelf.current(), view.shelf.selected()),
        (Some(ElectricPiano), None)
    );
    let other = e.owns(m);

    go(m, e, Command::Undo);
    assert_eq!(rows(m)[0].3, GrandPiano);
    assert_eq!(e.sounding(0), Some(GrandPiano));
    assert_eq!(m.view().shelf.current(), Some(GrandPiano));
    let now = e.owns(m);
    assert_eq!(
        now[&meloet_model::TrackId(2)],
        other[&meloet_model::TrackId(2)]
    );
    go(m, e, Command::Redo);
    assert_eq!(e.sounding(0), Some(ElectricPiano));

    // Undo while the track is tried with a third instrument: the trial ends, and the track
    // has what the song says.
    go(m, e, select(Organ));
    go(m, e, Command::Undo);
    assert_eq!(m.view().shelf.selected(), None);
    assert_eq!(e.sounding(0), Some(GrandPiano));

    // The track goes while it is tried: the shelf is for a new track again.
    go(m, e, select(Organ));
    go(m, e, Command::DeleteTrack { track: 1 });
    let shelf = m.view().shelf;
    assert_eq!((shelf.mode, shelf.selected()), (ShelfMode::NewTrack, None));
    assert_eq!(shelf.current(), None);
}

#[test]
fn the_engine_at_the_start_has_the_song_and_the_keys_on_its_first_track() {
    let mut p = Play::new(true, 1);
    assert_eq!(
        p.engine_seats()[..2],
        [
            EngineOp::Instrument {
                seat: 0,
                instrument: meloet_model::InstrumentRef::GRAND_PIANO
            },
            EngineOp::PlayTarget { seat: Some(0) }
        ]
    );
}
