use meloet_model::{BuiltinInstrument as B, InstrumentRef, Song};
use meloet_protocol::{
    Band, Command, InstrumentKind, SampleView, ScaleMode, ShelfMode, StatusMessage,
};

use crate::play::EngineOp;
use crate::play::seats::AUDITION_SEAT;
use crate::play::seats::tests::FakeEngine;
use crate::play::tests::{cmd, ready, status};
use crate::settings::Settings;
use crate::state::{Effect, Input, Model};

use InstrumentKind::{
    ElectroKit, GrandPiano, Organ, Sine, StudioKit, SynthBass, SynthLead, WarmPad,
};

const PIANO: InstrumentRef = InstrumentRef::GRAND_PIANO;
const ORGAN: InstrumentRef = InstrumentRef::Builtin(B::Organ);

/// A new song (one track, グランドピアノ) with an output open, on a PC whose sound pack has
/// `installed`, and the engine. The samples of the piano are asked for and not read yet.
fn rig(installed: &[B]) -> (Model, FakeEngine) {
    let mut m = ready().with_sounds(installed.to_vec());
    let engine = FakeEngine::new(&mut m);
    engine.check(&m, "start");
    (m, engine)
}

fn go(m: &mut Model, engine: &mut FakeEngine, input: Input) -> Vec<Effect> {
    let ctx = format!("{input:?}");
    let effects = m.handle(input);
    engine.take(&effects);
    engine.check(m, &ctx);
    effects
}

fn command(m: &mut Model, engine: &mut FakeEngine, command: Command) -> Vec<Effect> {
    go(m, engine, Input::Command(command))
}

fn select(instrument: InstrumentKind) -> Command {
    Command::ShelfSelect {
        instrument: Some(instrument),
    }
}

fn open(mode: ShelfMode) -> Command {
    Command::OpenShelf { mode }
}

/// The effects about samples and the ops about instruments and the sample, in their order.
fn sound(effects: &[Effect]) -> Vec<Effect> {
    let wanted = |e: &&Effect| {
        matches!(
            e,
            Effect::LoadSound(_)
                | Effect::DropSound(_)
                | Effect::Engine(
                    EngineOp::Instrument { .. }
                        | EngineOp::TryInstrument { .. }
                        | EngineOp::EndTrial { .. }
                        | EngineOp::MoveInstrument { .. }
                        | EngineOp::ClearSeat { .. }
                        | EngineOp::PlaySample { .. }
                        | EngineOp::StopSample
                )
        )
    };
    effects.iter().filter(wanted).cloned().collect()
}

fn made(seat: usize, instrument: InstrumentRef) -> Effect {
    Effect::Engine(EngineOp::Instrument { seat, instrument })
}

fn tried(seat: usize, instrument: InstrumentRef) -> Effect {
    Effect::Engine(EngineOp::TryInstrument { seat, instrument })
}

#[test]
fn the_shelf_has_the_computed_instruments_and_the_ones_of_the_pack() {
    // Without the pack the shelf offers the sine wave alone (spec 3, 85).
    let (m, _) = &mut rig(&[]);
    assert_eq!(m.view().shelf.available, [Sine]);
    assert!(m.view().library.pack_missing);
    let (m, _) = &mut rig(&[B::StudioKit, B::GrandPiano]);
    assert_eq!(
        m.view().shelf.available,
        [
            GrandPiano, SynthBass, StudioKit, ElectroKit, WarmPad, SynthLead, Sine
        ],
        "in the order of the shelf"
    );
}

#[test]
fn the_sounds_of_the_song_are_read_at_the_start_and_its_seats_made_again_with_them() {
    let (m, e) = &mut rig(&[B::GrandPiano, B::Organ]);
    // `FakeEngine::new` handled the first input: the piano of the new song was asked for.
    let mut asked = Model::new(Settings::default()).with_sounds(vec![B::GrandPiano, B::Organ]);
    assert_eq!(
        sound(&asked.handle(Input::Tick)),
        [Effect::LoadSound(B::GrandPiano)]
    );
    assert_eq!(sound(&go(m, e, Input::Tick)), [], "asked for once");

    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::GrandPiano))),
        [made(0, PIANO)],
        "the silent one on the seat gives way to one that sounds"
    );
    assert_eq!(sound(&go(m, e, Input::SoundLoaded(B::GrandPiano))), []);
    assert_eq!(sound(&go(m, e, Input::Tick)), []);

    // The track gets the organ: its samples are read while it is tried, silent.
    command(m, e, open(ShelfMode::Replace { track: 1 }));
    assert_eq!(
        sound(&command(m, e, select(Organ))),
        [tried(0, ORGAN), Effect::LoadSound(B::Organ)]
    );
    let shelf = m.view().shelf;
    assert_eq!((shelf.selected, shelf.loading), (Some(Organ), Some(Organ)));
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::Organ))),
        [tried(0, ORGAN)]
    );
    assert_eq!(m.view().shelf.loading, None);
    // Entered: the piano is in no track and no row any more, and is let go of.
    assert_eq!(
        sound(&command(m, e, Command::ShelfConfirm)),
        [
            Effect::Engine(EngineOp::EndTrial {
                seat: 0,
                keep: true
            }),
            Effect::DropSound(B::GrandPiano),
        ]
    );
    // Undo brings the piano back: silent at first, read again, then sounding.
    assert_eq!(
        sound(&command(m, e, Command::Undo)),
        [
            made(0, PIANO),
            Effect::DropSound(B::Organ),
            Effect::LoadSound(B::GrandPiano),
        ]
    );
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::GrandPiano))),
        [made(0, PIANO)]
    );

    // Another song (a new one: the piano again) keeps what it shares with this one.
    assert_eq!(
        sound(&command(m, e, Command::NewSong)),
        [made(0, PIANO)],
        "made with the samples that are there; nothing is read or let go of"
    );
}

#[test]
fn a_row_whose_samples_are_not_read_says_so_and_another_row_puts_it_away() {
    let (m, e) = &mut rig(&[B::Organ, B::StudioKit]);
    let studio_kit = InstrumentRef::Builtin(B::StudioKit);
    command(m, e, open(ShelfMode::NewTrack));
    assert_eq!(
        sound(&command(m, e, select(Organ))),
        [made(AUDITION_SEAT, ORGAN), Effect::LoadSound(B::Organ)]
    );
    assert_eq!(m.view().shelf.loading, Some(Organ));
    assert_eq!(
        e.target(),
        Some(AUDITION_SEAT),
        "the keys wait there, silent"
    );
    assert!(!m.view().transport.can_record);

    // The next row before the organ is read: the organ stays wanted as the row before.
    assert_eq!(
        sound(&command(m, e, select(StudioKit))),
        [
            made(AUDITION_SEAT, studio_kit.clone()),
            Effect::LoadSound(B::StudioKit)
        ]
    );
    assert_eq!(m.view().shelf.loading, Some(StudioKit));
    // One more: now nothing wants the organ.
    assert_eq!(
        sound(&command(m, e, select(Sine))),
        [
            made(AUDITION_SEAT, InstrumentRef::SINE),
            Effect::DropSound(B::Organ)
        ]
    );
    assert_eq!(
        m.view().shelf.loading,
        None,
        "a computed one sounds at once"
    );
    // It was read all the same: let go of, and on no seat.
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::Organ))),
        [Effect::DropSound(B::Organ)]
    );
    // The kit, the row before, is read and kept: going back to it sounds at once.
    assert_eq!(sound(&go(m, e, Input::SoundLoaded(B::StudioKit))), []);
    assert_eq!(
        sound(&command(m, e, select(StudioKit))),
        [made(AUDITION_SEAT, studio_kit.clone())]
    );
    assert_eq!(m.view().shelf.loading, None);

    // Entered while read: the track has it. Closing the shelf lets go of the row before.
    command(m, e, select(Organ));
    assert_eq!(m.view().shelf.loading, Some(Organ));
    let effects = command(m, e, Command::ShelfConfirm);
    assert_eq!(
        sound(&effects),
        [Effect::Engine(EngineOp::MoveInstrument {
            from: AUDITION_SEAT,
            to: 1
        })]
    );
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::Organ))),
        [made(1, ORGAN)],
        "the new track's seat"
    );
    assert_eq!(
        sound(&command(m, e, Command::ToggleBand { band: Band::Shelf })),
        [Effect::DropSound(B::StudioKit)]
    );
}

#[test]
fn samples_that_do_not_read_take_the_instrument_off_the_shelf() {
    let (m, e) = &mut rig(&[B::GrandPiano, B::Organ]);
    command(m, e, open(ShelfMode::NewTrack));
    command(m, e, select(Organ));
    assert_eq!(
        sound(&go(m, e, Input::SoundUnreadable(B::Organ))),
        [Effect::Engine(EngineOp::ClearSeat {
            seat: AUDITION_SEAT
        })]
    );
    let shelf = m.view().shelf;
    assert_eq!((shelf.selected, shelf.loading), (None, None));
    assert!(!shelf.available.contains(&Organ) && shelf.available.contains(&GrandPiano));
    assert_eq!(sound(&command(m, e, select(Organ))), [], "no row any more");

    // The piano of the song: the track keeps it and sounds as the sine wave.
    assert_eq!(
        sound(&go(m, e, Input::SoundUnreadable(B::GrandPiano))),
        [made(0, PIANO)]
    );
    assert!(!m.view().shelf.available.contains(&GrandPiano));
    assert_eq!(m.view().song.tracks[0].instrument, GrandPiano);
    assert_eq!(sound(&go(m, e, Input::Tick)), []);
    assert_eq!(sound(&go(m, e, Input::SoundUnreadable(B::GrandPiano))), []);
}

#[test]
fn the_tracks_own_instrument_is_made_again_under_a_trial() {
    let (m, e) = &mut rig(&[B::GrandPiano]);
    command(m, e, open(ShelfMode::Replace { track: 1 }));
    command(m, e, select(Sine));
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::GrandPiano))),
        [
            Effect::Engine(EngineOp::EndTrial {
                seat: 0,
                keep: false
            }),
            made(0, PIANO),
            tried(0, InstrumentRef::SINE),
        ]
    );
    assert_eq!(e.sounding(0), Some(Sine));
    command(m, e, Command::ShelfCancel);
    assert_eq!(e.sounding(0), Some(GrandPiano));

    // The track's own instrument as the row, read while it is tried: once each.
    let (m, e) = &mut rig(&[B::GrandPiano]);
    command(m, e, open(ShelfMode::Replace { track: 1 }));
    command(m, e, select(GrandPiano));
    assert_eq!(m.view().shelf.loading, Some(GrandPiano));
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::GrandPiano))),
        [
            Effect::Engine(EngineOp::EndTrial {
                seat: 0,
                keep: false
            }),
            made(0, PIANO),
            tried(0, PIANO),
        ]
    );
}

fn play_sample(instrument: InstrumentKind) -> Command {
    Command::PlaySample { instrument }
}

fn sample_op(instrument: B) -> Effect {
    Effect::Engine(EngineOp::PlaySample {
        instrument: InstrumentRef::Builtin(instrument),
    })
}

const STOP: Effect = Effect::Engine(EngineOp::StopSample);

#[test]
fn a_sample_plays_stops_and_ends_by_itself() {
    let (m, e) = &mut rig(&[]);
    go(m, e, Input::AudioFrames(Some(256)));
    let latency_us = (m.view().audio.latency_ms.unwrap() * 1000.0) as u64;
    assert!(latency_us > 5_000);

    // Only in the shelf.
    assert_eq!(sound(&command(m, e, play_sample(SynthLead))), []);
    command(m, e, open(ShelfMode::NewTrack));
    m.set_now(1_000_000, None);
    assert_eq!(
        sound(&command(m, e, play_sample(SynthLead))),
        [sample_op(B::SynthLead)]
    );
    let playing = |instrument, at_us: u64| {
        Some(SampleView {
            instrument,
            started_us: at_us + latency_us,
            length_us: 4_000_000,
        })
    };
    let shelf = m.view().shelf;
    assert_eq!(shelf.sample, playing(SynthLead, 1_000_000));
    assert_eq!(shelf.selected, None, "it does not select the row");
    assert_eq!(
        status(m),
        Some(StatusMessage::SamplePlaying {
            instrument: SynthLead
        })
    );
    assert_eq!(m.view().song.undo_count, 0);
    assert!(m.view().transport.can_record);

    // Another one: the engine lets the first fade out under it.
    m.set_now(2_000_000, None);
    assert_eq!(
        sound(&command(m, e, play_sample(ElectroKit))),
        [sample_op(B::ElectroKit)]
    );
    assert_eq!(m.view().shelf.sample, playing(ElectroKit, 2_000_000));
    // A row that is auditioned has the status line; the sample plays on.
    command(m, e, select(Sine));
    assert_eq!(
        status(m),
        Some(StatusMessage::AuditionNewTrack { instrument: Sine })
    );
    assert_eq!(m.view().shelf.sample, playing(ElectroKit, 2_000_000));
    command(m, e, Command::ShelfCancel);

    assert_eq!(sound(&command(m, e, Command::StopSample)), [STOP]);
    assert_eq!(m.view().shelf.sample, None);
    assert_eq!(sound(&command(m, e, Command::StopSample)), []);

    // It ends by itself after its two bars and the tail.
    command(m, e, play_sample(Sine));
    let started = 2_000_000 + latency_us;
    m.set_now(started + 5_900_000, None);
    assert!(m.sample_playing(), "the time is to be told");
    assert_eq!(sound(&go(m, e, Input::Tick)), []);
    assert_eq!(m.view().shelf.sample, playing(Sine, 2_000_000));
    m.set_now(started + 6_000_000, None);
    assert_eq!(sound(&go(m, e, Input::Tick)), [STOP]);
    assert_eq!(m.view().shelf.sample, None);
    assert!(!m.sample_playing());

    // Closing the shelf stops it.
    command(m, e, play_sample(Sine));
    assert_eq!(
        sound(&command(m, e, Command::ToggleBand { band: Band::Shelf })),
        [STOP]
    );
    assert_eq!(m.view().shelf.sample, None);

    // An instrument that is no row has no sample.
    command(m, e, open(ShelfMode::NewTrack));
    assert_eq!(sound(&command(m, e, play_sample(GrandPiano))), []);

    // The output goes away: the engine's sample seat is emptied with the stream.
    command(m, e, play_sample(Sine));
    go(m, e, Input::AudioLost);
    assert_eq!(m.view().shelf.sample, None);
}

#[test]
fn a_sample_waits_for_its_samples_and_nothing_plays_without_an_output() {
    let (m, e) = &mut rig(&[B::Organ, B::StudioKit]);
    command(m, e, open(ShelfMode::NewTrack));
    assert_eq!(
        sound(&command(m, e, play_sample(Organ))),
        [Effect::LoadSound(B::Organ)]
    );
    assert_eq!(m.view().shelf.sample, None);
    assert_eq!(
        m.view().shelf.loading,
        None,
        "that is about the selected row"
    );
    m.set_now(3_000_000, None);
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::Organ))),
        [sample_op(B::Organ)]
    );
    assert_eq!(m.view().shelf.sample.unwrap().instrument, Organ);
    assert_eq!(m.view().shelf.sample.unwrap().started_us, 3_000_000);

    // ▶ on another one that is not read: the one playing stops, and only the last one asked
    // for plays when it is there. The organ is let go of.
    assert_eq!(
        sound(&command(m, e, play_sample(StudioKit))),
        [
            STOP,
            Effect::DropSound(B::Organ),
            Effect::LoadSound(B::StudioKit)
        ]
    );
    assert_eq!(
        sound(&command(m, e, Command::StopSample)),
        [Effect::DropSound(B::StudioKit)]
    );
    assert_eq!(
        sound(&go(m, e, Input::SoundLoaded(B::StudioKit))),
        [Effect::DropSound(B::StudioKit)]
    );
    assert_eq!(m.view().shelf.sample, None);

    // No output: nothing would be heard, so nothing plays.
    let mut silent = Model::new(Settings::default());
    cmd(&mut silent, open(ShelfMode::NewTrack));
    assert_eq!(sound(&cmd(&mut silent, play_sample(Sine))), []);
    assert_eq!(silent.view().shelf.sample, None);
}

#[test]
fn the_scale_lock_does_not_hold_on_a_drum_track() {
    let (m, e) = &mut rig(&[]);
    command(
        m,
        e,
        Command::SetMusicalScale {
            root: 0,
            mode: ScaleMode::Major,
            lock: true,
        },
    );
    let write = |key: u8| Command::AddNote {
        take: None,
        start: 0,
        length: 120,
        key,
        velocity: 100,
    };
    // The piano: C# is not in C major.
    command(m, e, write(37));
    assert_eq!(
        status(m),
        Some(StatusMessage::OutOfScale {
            key: 37,
            root: 0,
            mode: ScaleMode::Major
        })
    );
    assert_eq!(m.view().song.undo_count, 0);

    // A drum kit: key 37 is its side stick.
    command(m, e, open(ShelfMode::NewTrack));
    command(m, e, select(ElectroKit));
    command(m, e, Command::ShelfConfirm);
    assert_eq!(m.view().song.undo_count, 1);
    command(m, e, write(37));
    assert_eq!(m.view().song.undo_count, 2, "written");
    // Into the clip that is shown now, too.
    let take = m.view().piano_roll.take.expect("the new clip is shown");
    command(
        m,
        e,
        Command::AddNote {
            take: Some(take),
            start: 240,
            length: 120,
            key: 39,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 3);
}

#[test]
fn a_key_on_a_track_whose_samples_are_not_read_says_so_until_they_are() {
    let loading = |instrument| Some(StatusMessage::InstrumentLoading { instrument });
    let (m, e) = &mut rig(&[B::GrandPiano, B::Organ]);
    assert_ne!(status(m), loading(GrandPiano), "nothing before a key");
    assert!(m.wants_loading_key());

    go(m, e, Input::KeyWhileLoading);
    assert_eq!(status(m), loading(GrandPiano));
    assert!(!m.wants_loading_key(), "one key is enough");
    // What the user does meanwhile does not put it away.
    command(m, e, Command::ToggleMetronome);
    assert_eq!(status(m), loading(GrandPiano));

    go(m, e, Input::SoundLoaded(B::GrandPiano));
    assert_ne!(status(m), loading(GrandPiano), "read: gone");
    assert!(!m.wants_loading_key());
    // A key that was on its way while the samples came in.
    go(m, e, Input::KeyWhileLoading);
    assert_ne!(status(m), loading(GrandPiano));

    // A row is auditioned: the shelf says it, as before, and the status line is the shelf's.
    command(m, e, open(ShelfMode::NewTrack));
    command(m, e, select(Organ));
    assert_eq!(m.view().shelf.loading, Some(Organ));
    assert!(!m.wants_loading_key());
    go(m, e, Input::KeyWhileLoading);
    let auditioning = Some(StatusMessage::AuditionNewTrack { instrument: Organ });
    assert_eq!(status(m), auditioning);
    // Entered before they are read: the new track is the one the keys play, silent still.
    command(m, e, Command::ShelfConfirm);
    assert!(m.wants_loading_key());
    // The keys column of the piano roll counts as a key.
    let keys = Command::Audition {
        keys: vec![60],
        velocity: 100,
    };
    command(m, e, keys.clone());
    assert_eq!(status(m), loading(Organ));
    // Another track, whose instrument sounds: nothing to say, with a key or without.
    command(
        m,
        e,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    assert_ne!(status(m), loading(Organ));
    assert!(!m.wants_loading_key());
    command(m, e, keys);
    go(m, e, Input::KeyWhileLoading);
    assert!(!matches!(
        status(m),
        Some(StatusMessage::InstrumentLoading { .. })
    ));
    // Back on the silent track: said again only at the next key.
    command(
        m,
        e,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );
    assert_ne!(status(m), loading(Organ));
    assert!(m.wants_loading_key());
}

#[test]
fn without_the_sound_pack_a_new_song_starts_with_the_sine_wave_named_synth() {
    // No pack: the song the app starts with, and every new song after it.
    let (mut m, mut engine) = rig(&[]);
    for _ in 0..2 {
        let song = m.play().song().clone();
        assert_eq!(song.doc.tracks.len(), 1);
        let track = &song.doc.tracks[0];
        assert_eq!(
            (track.name.as_str(), &track.instrument),
            ("シンセ", &InstrumentRef::SINE)
        );
        let view = m.view();
        assert_eq!(view.song.tracks[0].stand_in, None, "it is no stand-in");
        // The rest is the song as ever.
        assert_eq!(song, crate::play::tests::new_song_without_pack());
        command(&mut m, &mut engine, Command::NewSong);
    }

    // With the pack: the grand piano, as ever.
    let (mut m, mut engine) = rig(&[B::GrandPiano]);
    for _ in 0..2 {
        assert_eq!(*m.play().song(), Song::new_default());
        command(&mut m, &mut engine, Command::NewSong);
    }
}
