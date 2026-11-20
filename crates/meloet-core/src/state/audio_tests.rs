//! What protocol 24 added to the rules so far: audio files held over the window, the 160
//! row, and the intents that are accepted and do nothing yet (docs/architecture.md 22-17).

use meloet_protocol::{ClipField, DropMode, Event};

use super::*;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

fn file_events(effects: &[Effect]) -> Vec<Event> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::FileEvent(event) => Some(*event),
            _ => None,
        })
        .collect()
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|n| PathBuf::from("/home/me").join(n))
        .collect()
}

#[test]
fn audio_files_held_over_the_window_say_where_the_pointer_is() {
    let mut m = Model::new(Settings::default());
    let held = paths(&["メモ.txt", "ドラム キック.wav", "FluidR3_GM.sf2", "声.MP3"]);
    assert!(m.handle(Input::FilesOver(held.clone())).is_empty());
    assert_eq!(
        m.view().workspace.file_drop,
        Some(FileDropView {
            // The first audio file, not the first file.
            name: "ドラム キック.wav".into(),
            addable: true,
            files: 4,
            sf2: 1,
            audio: 2,
        })
    );
    assert_eq!(
        status(&m),
        Some(StatusMessage::AudioDragging { audio: 2, sf2: 1 })
    );
    assert_eq!(
        file_events(&m.handle(Input::FilesAt { x: 520, y: 385 })),
        [Event::FileDrag { x: 520, y: 385 }]
    );

    // Let go: the place goes to the screen with a number, the SF2 file is added as before
    // and the audio files are not counted as files that could not be added.
    m.handle(Input::FilesAt { x: 600, y: 540 });
    let effects = m.handle(Input::FilesDropped(held.clone()));
    assert_eq!(
        file_events(&effects),
        [Event::FileDrop {
            drop: 1,
            x: 600,
            y: 540
        }]
    );
    assert!(effects.contains(&Effect::AddSf2(held[2].clone())));
    assert_eq!(m.view().workspace.file_drop, None);
    assert_eq!(m.pending_drop.as_ref().map(|p| p.paths.len()), Some(2));
    assert_eq!(m.sf2_batch.as_ref().map(|b| b.skipped), Some(1));

    // An answer to another drop is not this one's.
    let answer = |drop| Command::DropAudio {
        drop,
        track: None,
        tick: 0,
        mode: DropMode::InRow,
    };
    cmd(&mut m, answer(7));
    assert!(m.pending_drop.is_some());
    cmd(&mut m, answer(1));
    assert!(m.pending_drop.is_none());

    // The next drop has the next number; files coming over the window let an unanswered one go.
    m.handle(Input::FilesOver(held.clone()));
    let effects = m.handle(Input::FilesDropped(held.clone()));
    assert!(matches!(
        file_events(&effects)[..],
        [Event::FileDrop { drop: 2, .. }]
    ));
    m.handle(Input::FilesOver(held));
    assert!(m.pending_drop.is_none());
}

#[test]
fn files_with_no_audio_and_a_drag_given_up_send_no_place() {
    let mut m = Model::new(Settings::default());
    let sf2 = paths(&["FluidR3_GM.sf2"]);
    m.handle(Input::FilesOver(sf2.clone()));
    assert_eq!(m.view().workspace.file_drop.map(|d| d.audio), Some(0));
    assert_eq!(status(&m), Some(StatusMessage::Sf2Dragging));
    assert!(m.handle(Input::FilesAt { x: 1, y: 2 }).is_empty());
    assert!(file_events(&m.handle(Input::FilesDropped(sf2))).is_empty());

    // Esc while audio is held: no frame any more, and letting go brings nothing in.
    let wav = paths(&["ドラム.wav"]);
    m.handle(Input::FilesOver(wav.clone()));
    assert_eq!(
        status(&m),
        Some(StatusMessage::AudioDragging { audio: 1, sf2: 0 })
    );
    cmd(&mut m, Command::CancelFileDrop);
    assert!(m.handle(Input::FilesAt { x: 3, y: 4 }).is_empty());
    assert!(m.handle(Input::FilesDropped(wav.clone())).is_empty());
    assert!(m.pending_drop.is_none());

    // Taken out of the window again.
    m.handle(Input::FilesOver(wav));
    m.handle(Input::FilesLeft);
    assert!(m.handle(Input::FilesAt { x: 3, y: 4 }).is_empty());
}

#[test]
fn only_a_vocal_track_gets_the_tall_row() {
    let mut m = Model::new(Settings::default());
    let tall = |m: &Model| m.view().arrange.tall_track;
    let keys = m.view().song.tracks[0].id;
    cmd(&mut m, Command::AddVocalTrack);
    let vocal = m.view().song.tracks[1].id;
    assert_eq!(tall(&m), None);

    cmd(&mut m, Command::SetTallTrack { track: Some(keys) });
    assert_eq!(tall(&m), None);
    cmd(&mut m, Command::SetTallTrack { track: Some(99) });
    assert_eq!(tall(&m), None);
    cmd(&mut m, Command::SetTallTrack { track: Some(vocal) });
    assert_eq!(tall(&m), Some(vocal));
    // No step: the song is as it was.
    let undo = m.view().song.undo_count;
    // The same track again, and `None`, put every row back.
    cmd(&mut m, Command::SetTallTrack { track: Some(vocal) });
    assert_eq!(tall(&m), None);
    cmd(&mut m, Command::SetTallTrack { track: Some(vocal) });
    cmd(&mut m, Command::SetTallTrack { track: None });
    assert_eq!(tall(&m), None);
    assert_eq!(m.view().song.undo_count, undo);

    // The track is deleted: no row is tall.
    cmd(&mut m, Command::SetTallTrack { track: Some(vocal) });
    cmd(&mut m, Command::DeleteTrack { track: vocal });
    assert_eq!(tall(&m), None);
}

#[test]
fn the_intents_of_vocal_clips_are_taken_and_do_nothing_yet() {
    let mut m = Model::new(Settings::default());
    let before = m.view();
    for command in [
        Command::SplitClips,
        Command::SetClipField {
            clip: 1,
            field: ClipField::GainDb,
            value: -3.0,
        },
        Command::ImportAudio { track: None },
        Command::FindAudio { audio: 31 },
        Command::RemoveMissing {
            missing: MissingRef::Audio { audio: 31 },
        },
        Command::DropAudio {
            drop: 1,
            track: None,
            tick: 0,
            mode: DropMode::Separate,
        },
    ] {
        assert!(cmd(&mut m, command.clone()).is_empty(), "{command:?}");
        assert_eq!(m.view(), before, "{command:?}");
    }
    assert_eq!(before.audio_work, AudioWorkView::default());
    assert_eq!(before.transport.recording_faults, None);
    assert_eq!(m.peaks_place(31).map(|_| ()), None);
}
