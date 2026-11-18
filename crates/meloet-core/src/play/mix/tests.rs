use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{InstrumentRef, Song, Track, TrackColor, TrackId};
use meloet_protocol::{Command, SongFile, TransportState};

use crate::play::EngineOp;
use crate::play::tests::{cmd, ready, started};
use crate::state::{Effect, Input, Model};

/// What the engine was told last about the silent tracks, in these effects.
fn silent(effects: &[Effect]) -> Option<Vec<usize>> {
    effects.iter().rev().find_map(|e| match e {
        Effect::Engine(EngineOp::SilentTracks { tracks }) => Some(tracks.clone()),
        _ => None,
    })
}

fn flags(m: &Model) -> Vec<(bool, bool)> {
    m.view()
        .song
        .tracks
        .iter()
        .map(|t| (t.muted, t.soloed))
        .collect()
}

/// A song of three tracks, opened (as if read from a file).
pub(crate) fn three_tracks(m: &mut Model) -> Vec<Effect> {
    let mut song = Song::new_default();
    for id in [2, 3] {
        song.doc.tracks.push(Arc::new(Track {
            id: TrackId(id),
            name: format!("トラック {id}"),
            color: TrackColor::for_index(id as usize - 1),
            instrument: InstrumentRef::SINE,
            gain_db: 0.0,
            muted: false,
            clips: Vec::new(),
        }));
    }
    cmd(m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    })
}

#[test]
fn mute_is_a_step_and_the_engine_follows_undo_and_redo() {
    let m = &mut ready();
    three_tracks(m);
    let effects = cmd(m, Command::ToggleMute { track: 2 });
    assert_eq!(silent(&effects), Some(vec![1]));
    assert_eq!(flags(m), [(false, false), (true, false), (false, false)]);
    assert_eq!(m.view().song.undo_count, 1);
    assert_eq!(m.view().song.file, SongFile::Changed);

    assert_eq!(silent(&cmd(m, Command::Undo)), Some(vec![]));
    assert_eq!(flags(m)[1], (false, false));
    assert_eq!(m.view().song.file, SongFile::Saved);
    assert_eq!(silent(&cmd(m, Command::Redo)), Some(vec![1]));
    assert_eq!(flags(m)[1], (true, false));
    // Nothing that changes what plays tells the engine anything.
    assert_eq!(silent(&cmd(m, Command::SetTempo { bpm: 100.0 })), None);
    // A track that is not there.
    let effects = cmd(m, Command::ToggleMute { track: 9 });
    assert_eq!(silent(&effects), None);
    assert_eq!(m.view().song.undo_count, 2);
}

#[test]
fn solo_is_not_a_step_and_mute_beats_it() {
    let m = &mut ready();
    three_tracks(m);
    assert_eq!(
        silent(&cmd(m, Command::ToggleSolo { track: 1 })),
        Some(vec![1, 2])
    );
    assert_eq!(m.view().song.undo_count, 0);
    assert_eq!(m.view().song.file, SongFile::Saved);
    assert_eq!(
        silent(&cmd(m, Command::ToggleSolo { track: 2 })),
        Some(vec![2]),
        "any number of tracks"
    );
    assert_eq!(
        silent(&cmd(m, Command::ToggleMute { track: 2 })),
        Some(vec![1, 2]),
        "muted and soloed: silent"
    );
    assert_eq!(flags(m), [(false, true), (true, true), (false, false)]);
    // Undo takes back the mute, not the solo.
    assert_eq!(silent(&cmd(m, Command::Undo)), Some(vec![2]));
    assert_eq!(flags(m), [(false, true), (false, true), (false, false)]);
    cmd(m, Command::ToggleSolo { track: 1 });
    assert_eq!(
        silent(&cmd(m, Command::ToggleSolo { track: 2 })),
        Some(vec![])
    );
    assert_eq!(silent(&cmd(m, Command::ToggleSolo { track: 9 })), None);
}

#[test]
fn another_song_drops_the_solo_and_the_engine_hears_it() {
    let m = &mut ready();
    three_tracks(m);
    cmd(m, Command::ToggleSolo { track: 3 });
    // Opening the song again: no solo, nothing silent.
    let effects = three_tracks(m);
    assert_eq!(silent(&effects), Some(vec![]));
    assert_eq!(flags(m), [(false, false); 3]);

    cmd(m, Command::ToggleMute { track: 1 });
    cmd(m, Command::ToggleSolo { track: 1 });
    cmd(m, Command::NewSong);
    cmd(
        m,
        Command::AnswerSongDialog {
            dialog: m.view().workspace.song_dialog.unwrap().id,
            answer: meloet_protocol::SongAnswer::DontSave,
        },
    );
    assert_eq!(flags(m), [(false, false)]);
}

#[test]
fn a_stream_that_opens_hears_what_is_silent() {
    let m = &mut ready();
    three_tracks(m);
    cmd(m, Command::ToggleMute { track: 3 });
    let effects = m.handle(Input::AudioOpened {
        node_name: "alsa_output.card.analog-stereo".into(),
        sample_rate: 48_000,
    });
    assert_eq!(silent(&effects), Some(vec![2]));
}

#[test]
fn mute_waits_while_recording_and_the_recording_goes_on() {
    let m = &mut ready();
    cmd(m, Command::ToggleMute { track: 1 });
    cmd(m, Command::SetCountIn { bars: 0 });
    let effects = cmd(m, Command::Record);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::Record { .. }))),
        "a muted track records"
    );
    started(m);
    assert_eq!(m.view().transport.state, TransportState::Recording);
    let effects = cmd(m, Command::ToggleMute { track: 1 });
    assert_eq!(silent(&effects), None);
    assert!(m.view().song.tracks[0].muted);
    // Solo is only listening: it is taken.
    assert_eq!(
        silent(&cmd(m, Command::ToggleSolo { track: 1 })),
        None,
        "track 1 is muted: still silent"
    );
    assert!(m.view().song.tracks[0].soloed);
}
