//! At 64 tracks every way of adding a track says 「トラックは 64 本まで…」 and adds nothing
//! (A3). Duplicate, bounce, patterns and imported audio have their own tests.

use meloet_model::BuiltinInstrument;
use meloet_protocol::{Command, InstrumentKind, ShelfMode, StatusMessage};

use crate::ai::{AiAnswer, AiAsk, AiInstrument, AiRefusal};
use crate::play::MAX_TRACKS;
use crate::play::tests::{cmd, ready, status};
use crate::state::{Input, Model};

/// A model whose song has 64 tracks, the last ones vocal tracks.
fn full() -> Model {
    let mut m = ready();
    while m.view().song.tracks.len() < MAX_TRACKS {
        cmd(&mut m, Command::AddVocalTrack);
    }
    m
}

fn tracks_and_steps(m: &Model) -> (usize, u32) {
    let song = m.view().song;
    (song.tracks.len(), song.undo_count)
}

#[test]
fn a_vocal_track_past_sixty_four_is_said() {
    let m = &mut full();
    let before = tracks_and_steps(m);
    cmd(m, Command::AddVocalTrack);
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(tracks_and_steps(m), before);
}

#[test]
fn the_shelfs_new_track_past_sixty_four_is_said() {
    let m = &mut full();
    let before = tracks_and_steps(m);
    cmd(
        m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    cmd(
        m,
        Command::ShelfSelect {
            instrument: Some(InstrumentKind::Sine),
        },
    );
    cmd(m, Command::ShelfConfirm);
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(tracks_and_steps(m), before);
}

#[test]
fn the_ais_track_past_sixty_four_is_refused_and_said() {
    let m = &mut full();
    let before = tracks_and_steps(m);
    m.handle(Input::Ai(AiAsk::AddTrack {
        instrument: AiInstrument::Builtin(BuiltinInstrument::Sine),
    }));
    assert!(matches!(
        m.take_ai_answer(),
        Some(AiAnswer::Refused(AiRefusal::TooManyTracks))
    ));
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(tracks_and_steps(m), before);

    m.handle(Input::Ai(AiAsk::Screen(Command::AddVocalTrack)));
    assert!(matches!(
        m.take_ai_answer(),
        Some(AiAnswer::Refused(AiRefusal::TooManyTracks))
    ));
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(tracks_and_steps(m), before);
}
