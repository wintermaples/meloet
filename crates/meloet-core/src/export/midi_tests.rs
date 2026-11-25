//! MIDI in the rules of the export (protocol 36; spec 3 "MIDI export", spec 7 A-31): the
//! choices of 「入れるトラック」 are kept, the choices of audio stay for the audio formats,
//! nothing is measured, and 「書き出す」 writes the files at once.

use std::path::{Path, PathBuf};

use meloet_model::{Song, TickRange, TrackId};
use meloet_protocol::{
    Command, ExportChange, ExportCheck, ExportFailure, ExportFormat, ExportMidiProblem,
    ExportMidiTrack, ExportMidiTracks, ExportRange, ExportRunView, RecordFormat,
};

use super::{ExportAsk, ExportDirs, midi};
use crate::midi::smf;
use crate::midi::tests::{BAR, Parts, song};
use crate::settings::{Settings, temp_dir};
use crate::state::{Effect, Input, Model};

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn asks(effects: Vec<Effect>) -> Vec<ExportAsk> {
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Export(ask) => Some(ask),
            _ => None,
        })
        .collect()
}

fn set(m: &mut Model, change: ExportChange) -> Vec<ExportAsk> {
    asks(cmd(m, Command::SetExport { change }))
}

fn dialog(m: &Model) -> meloet_protocol::ExportDialogView {
    m.view().export.dialog.expect("the dialog")
}

#[test]
fn midi_is_the_last_format_and_keeps_the_choices_of_audio_and_its_own() {
    let mut m = Model::new(Settings::default()).with_export_dirs(ExportDirs {
        home: Some(PathBuf::from("/home/me")),
        usual: Some(PathBuf::from("/home/me/音楽")),
    });
    cmd(
        &mut m,
        Command::AddNote {
            take: None,
            start: 0,
            length: 480,
            key: 60,
            velocity: 100,
        },
    );
    cmd(&mut m, Command::OpenExport);
    let d = dialog(&m);
    assert_eq!(d.formats.last(), Some(&ExportFormat::Midi));
    assert_eq!(
        (d.midi_tracks, d.midi_numbers.as_str()),
        (ExportMidiTracks::All, "")
    );
    set(
        &mut m,
        ExportChange::Bits {
            bits: RecordFormat::Bits16,
        },
    );
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Midi,
        },
    );
    let d = dialog(&m);
    assert!(
        d.place.starts_with("~/音楽/") && d.place.ends_with(".mid"),
        "{}",
        d.place
    );
    assert!(d.bits_choices.is_empty());
    assert!(!d.picture_usable);

    set(
        &mut m,
        ExportChange::MidiTracks {
            tracks: ExportMidiTracks::Numbers,
        },
    );
    set(
        &mut m,
        ExportChange::MidiNumbers {
            text: "1-3, 5".to_owned(),
        },
    );
    // Kept while meloet runs, as the other choices.
    cmd(&mut m, Command::CloseExport);
    cmd(&mut m, Command::OpenExport);
    let d = dialog(&m);
    assert_eq!(d.format, ExportFormat::Midi);
    assert_eq!(
        (d.midi_tracks, d.midi_numbers.as_str()),
        (ExportMidiTracks::Numbers, "1-3, 5")
    );

    // Back to wav: the bits chosen before.
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Wav,
        },
    );
    let d = dialog(&m);
    assert_eq!(d.bits, RecordFormat::Bits16);
    assert!(d.place.ends_with(".wav"), "{}", d.place);
}

/// The song of [`song`] (「ピアノ」, 「ドラム」, 「ボーカル」, 「ベース」 muted, 「空」),
/// opened from `<root>/曲/曲.meloet` with the vocal track selected, and a loop of bars 2 and 3.
fn opened(root: &Path) -> (Model, Parts) {
    let (doc, parts) = song();
    let mut song = Song::new_default();
    song.doc = doc;
    song.doc.loop_range = TickRange::new(BAR, 3 * BAR);
    song.session.play_target = Some(parts.vocals);
    let mut m = Model::new(Settings::default()).with_export_dirs(ExportDirs {
        home: Some(root.to_path_buf()),
        usual: Some(root.join("音楽")),
    });
    cmd(&mut m, Command::OpenSong);
    let path = root.join("曲").join("曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    });
    assert_eq!(m.view().song.tracks.len(), 5);
    (m, parts)
}

fn track(number: u32, name: &str) -> ExportMidiTrack {
    ExportMidiTrack {
        number,
        name: name.to_owned(),
    }
}

fn numbers(m: &mut Model, text: &str) {
    set(
        m,
        ExportChange::MidiNumbers {
            text: text.to_owned(),
        },
    );
}

fn midi_format(m: &mut Model) -> Vec<ExportAsk> {
    set(
        m,
        ExportChange::Format {
            format: ExportFormat::Midi,
        },
    )
}

fn id(t: TrackId) -> u32 {
    t.0
}

#[test]
fn under_midi_nothing_is_measured_and_export_can_be_pressed_at_once() {
    let root = temp_dir("midi-check");
    let (mut m, _) = opened(&root);
    let opened = asks(cmd(&mut m, Command::OpenExport));
    let [ExportAsk::Render(job)] = &opened[..] else {
        panic!("{opened:?}");
    };
    let job = job.id;
    assert!(!dialog(&m).can_export);
    // The check of the audio is dropped, and none starts.
    assert_eq!(midi_format(&mut m), [ExportAsk::Drop { job }]);
    let d = dialog(&m);
    assert_eq!(d.check, ExportCheck::Waiting);
    assert!(d.can_export && d.midi_problem.is_none());
    assert_eq!(d.place, "~/曲/テストの曲.mid");
    // The other choices do not start one either.
    set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Loop,
        },
    );
    assert!(dialog(&m).can_export);
    // Back to audio: the check starts again.
    let again = set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Flac,
        },
    );
    assert!(matches!(&again[..], [ExportAsk::Render(j)] if j.id > job));
    assert!(!dialog(&m).can_export);
    midi_format(&mut m);
    cmd(&mut m, Command::CloseExport);
    // A dialog opened under MIDI starts nothing.
    assert!(asks(cmd(&mut m, Command::OpenExport)).is_empty());
    assert!(dialog(&m).can_export);
}

#[test]
fn what_each_choice_takes_and_why_it_cannot_be_written() {
    let root = temp_dir("midi-taken");
    let (mut m, parts) = opened(&root);
    cmd(&mut m, Command::OpenExport);
    midi_format(&mut m);
    let d = dialog(&m);
    // 「ぜんぶ」 passes the vocal track by without a word.
    let all = vec![
        track(1, "ピアノ"),
        track(2, "ドラム"),
        track(4, "ベース"),
        track(5, "空"),
    ];
    assert_eq!((&d.midi_taken, d.midi_vocal.len(), d.tracks), (&all, 0, 4));
    assert!(d.midi_problem.is_none());

    // The vocal track is selected: 「選んでいるトラック」 cannot be chosen.
    assert!(!d.midi_selected_usable);
    let selected = ExportChange::MidiTracks {
        tracks: ExportMidiTracks::Selected,
    };
    set(&mut m, selected.clone());
    assert_eq!(dialog(&m).midi_tracks, ExportMidiTracks::All);
    let bass = id(parts.bass);
    cmd(
        &mut m,
        Command::SelectTrack {
            track: bass,
            from_arm: false,
        },
    );
    assert!(dialog(&m).midi_selected_usable);
    set(&mut m, selected);
    let d = dialog(&m);
    assert_eq!(d.midi_tracks, ExportMidiTracks::Selected);
    assert_eq!((d.midi_taken, d.tracks), (vec![track(4, "ベース")], 1));
    assert!(d.can_export);
    // A vocal track selected meanwhile takes nothing; the next opening is 「ぜんぶ」 again.
    let vocals = id(parts.vocals);
    cmd(
        &mut m,
        Command::SelectTrack {
            track: vocals,
            from_arm: false,
        },
    );
    let d = dialog(&m);
    assert_eq!(d.midi_problem, Some(ExportMidiProblem::NoTrack));
    assert!(d.midi_taken.is_empty() && !d.can_export);
    cmd(&mut m, Command::CloseExport);
    cmd(&mut m, Command::OpenExport);
    assert_eq!(dialog(&m).midi_tracks, ExportMidiTracks::All);

    set(
        &mut m,
        ExportChange::MidiTracks {
            tracks: ExportMidiTracks::Numbers,
        },
    );
    // An empty field takes nothing.
    let d = dialog(&m);
    assert_eq!(d.midi_problem, Some(ExportMidiProblem::NoTrack));
    assert!(!d.can_export);
    numbers(&mut m, "１－３、9");
    let d = dialog(&m);
    assert_eq!(
        d.midi_problem,
        Some(ExportMidiProblem::NoSuchTrack { number: 9 })
    );
    assert_eq!(d.midi_taken, [track(1, "ピアノ"), track(2, "ドラム")]);
    assert_eq!(d.midi_vocal, [track(3, "ボーカル")]);
    assert!(!d.can_export);
    numbers(&mut m, "5, 3-2");
    let d = dialog(&m);
    assert!(d.midi_problem.is_none() && d.can_export);
    assert_eq!(d.midi_taken, [track(2, "ドラム"), track(5, "空")]);
    assert_eq!(d.midi_vocal, [track(3, "ボーカル")]);
    assert_eq!(d.tracks, 2);
    numbers(&mut m, "3");
    let d = dialog(&m);
    assert_eq!(d.midi_problem, Some(ExportMidiProblem::NoTrack));
    assert_eq!(d.midi_vocal, [track(3, "ボーカル")]);
    // 「トラックごと」 writes no file: it cannot be chosen.
    let tracks = ExportChange::Range {
        range: ExportRange::Tracks,
    };
    set(&mut m, tracks.clone());
    assert_eq!(dialog(&m).range, ExportRange::Song);
    numbers(&mut m, "1 2");
    let d = dialog(&m);
    assert_eq!(d.midi_problem, Some(ExportMidiProblem::Unreadable));
    assert!(d.midi_taken.is_empty() && d.midi_vocal.is_empty());
    numbers(&mut m, "0, 2");
    assert_eq!(
        dialog(&m).midi_problem,
        Some(ExportMidiProblem::NoSuchTrack { number: 0 })
    );
    numbers(&mut m, "4-1");
    set(&mut m, tracks);
    let d = dialog(&m);
    assert_eq!((d.range, d.tracks), (ExportRange::Tracks, 3));
    assert!(d.place.ends_with("/曲/"), "{}", d.place);

    // Under audio the numbers say nothing, and 「トラックごと」 counts every track.
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Wav,
        },
    );
    numbers(&mut m, "x");
    let d = dialog(&m);
    assert_eq!((d.midi_problem, d.tracks), (None, 5));
    // Not a step: the song is as it was.
    assert_eq!(m.view().song.undo_count, 0);
}

/// 「書き出す」 under MIDI: one ask to write, the dialog closed, and what the run says once
/// the worker has written the files.
fn started(m: &mut Model) -> (u64, Vec<midi::MidiOut>) {
    let started = asks(cmd(m, Command::StartExport));
    let [ExportAsk::WriteMidi { job, files }] = &started[..] else {
        panic!("{started:?}");
    };
    assert!(m.view().export.dialog.is_none());
    assert!(matches!(
        m.view().export.run,
        Some(ExportRunView::Running { percent: 0, .. })
    ));
    (*job, files.clone())
}

fn written(m: &mut Model, job: u64, files: &[midi::MidiOut]) -> ExportRunView {
    let news = midi::write(job, files);
    m.handle(Input::Export(news));
    m.view().export.run.expect("the run")
}

#[test]
fn export_writes_one_file_of_the_tracks_taken_at_once() {
    let root = temp_dir("midi-one");
    let (mut m, _) = opened(&root);
    cmd(&mut m, Command::OpenExport);
    midi_format(&mut m);
    let (job, files) = started(&mut m);
    let path = root.join("曲").join("テストの曲.mid");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, path);
    let run = written(&mut m, job, &files);
    let ExportRunView::Done {
        file,
        files,
        place,
        length_s,
        wave,
        lufs,
        true_peak_db,
        mp3_kbps,
        midi_tracks,
        ..
    } = run
    else {
        panic!("{run:?}");
    };
    assert_eq!((file.as_str(), files), ("テストの曲.mid", 1));
    assert_eq!(place, "~/曲/テストの曲.mid");
    // A bar at 120 (2 s), one slowing to 60 (2.8 s), one at 60 (4 s), one at 100 (2.4 s).
    assert_eq!(length_s, 11);
    assert!(wave.is_empty() && lufs.is_none() && true_peak_db.is_none());
    assert_eq!((mp3_kbps, midi_tracks), (None, Some(4)));

    let read = smf::read(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        (read.format, read.division, read.tracks.len()),
        (1, 1920, 5)
    );
    let names: Vec<_> = (read.tracks.iter())
        .map(|t| t.name.clone().unwrap())
        .collect();
    assert_eq!(names, ["テストの曲", "ピアノ", "ドラム", "ベース", "空"]);
    assert_eq!(read.tracks[0].length, 4 * BAR);
    assert_eq!(read.tracks[3].notes.len(), 1, "the muted track is in");
    assert!(read.tracks[2].notes.iter().all(|n| n.channel == 9));

    // 「フォルダをひらく」 shows the file; there is nothing to play.
    let shown = cmd(&mut m, Command::ShowExported);
    assert!(shown.contains(&Effect::ShowFile(path.clone())), "{shown:?}");
    let played = cmd(&mut m, Command::PlayExported);
    assert!(!played.iter().any(|e| matches!(e, Effect::OpenFile(_))));
    assert_eq!(m.view().song.undo_count, 0);

    // The loop: its two bars, the notes cut at its end.
    cmd(&mut m, Command::OpenExport);
    set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Loop,
        },
    );
    // The file is there now: 「書き出す」 asks first, and pressed again writes over it.
    assert!(asks(cmd(&mut m, Command::StartExport)).is_empty());
    assert!(dialog(&m).replace.is_some());
    let (job, files) = started(&mut m);
    let run = written(&mut m, job, &files);
    assert!(
        matches!(run, ExportRunView::Done { length_s: 7, .. }),
        "{run:?}"
    );
    let read = smf::read(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(read.tracks[0].length, 2 * BAR);
    let piano = &read.tracks[1].notes;
    assert_eq!(piano.len(), 1);
    assert_eq!((piano[0].start, piano[0].end), (BAR, BAR + 1_920));
}

#[test]
fn per_track_writes_a_file_for_each_track_named_after_it() {
    let root = temp_dir("midi-tracks");
    let (mut m, parts) = opened(&root);
    // Two tracks of one name.
    cmd(
        &mut m,
        Command::RenameTrack {
            track: id(parts.empty),
            name: "ピアノ".into(),
        },
    );
    cmd(&mut m, Command::OpenExport);
    midi_format(&mut m);
    set(
        &mut m,
        ExportChange::MidiTracks {
            tracks: ExportMidiTracks::Numbers,
        },
    );
    numbers(&mut m, "5, 1, 2");
    set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Tracks,
        },
    );
    let (job, files) = started(&mut m);
    let folder = root.join("曲");
    let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    assert_eq!(
        paths,
        [
            folder.join("ピアノ.mid"),
            folder.join("ドラム.mid"),
            folder.join("ピアノ 2.mid")
        ]
    );
    let run = written(&mut m, job, &files);
    let ExportRunView::Done {
        file,
        files: count,
        place,
        midi_tracks,
        wave,
        ..
    } = run
    else {
        panic!("{run:?}");
    };
    assert_eq!(
        (file.as_str(), count, midi_tracks),
        ("ピアノ.mid", 3, Some(3))
    );
    assert_eq!(place, "~/曲");
    assert!(wave.is_empty());
    for (path, name, notes) in [
        (&paths[0], "ピアノ", 4),
        (&paths[1], "ドラム", 8),
        (&paths[2], "ピアノ", 0),
    ] {
        let read = smf::read(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(read.tracks.len(), 2, "{path:?}");
        assert_eq!(read.tracks[0].name.as_deref(), Some("テストの曲"));
        assert_eq!(read.tracks[0].tempos.len(), 19);
        assert_eq!(read.tracks[1].name.as_deref(), Some(name));
        assert_eq!(read.tracks[1].notes.len(), notes, "{path:?}");
    }
    let shown = cmd(&mut m, Command::ShowExported);
    assert!(shown.contains(&Effect::ShowFile(paths[0].clone())));
}

#[test]
fn a_file_that_cannot_be_written_fails_as_audio_does_and_leaves_nothing() {
    let root = temp_dir("midi-fails");
    let (mut m, _) = opened(&root);
    cmd(&mut m, Command::OpenExport);
    midi_format(&mut m);
    set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Tracks,
        },
    );
    let (job, mut files) = started(&mut m);
    // The second file's folder is a file.
    std::fs::write(root.join("塞ぐ"), b"").unwrap();
    files[1].path = root.join("塞ぐ").join("ドラム.mid");
    let run = written(&mut m, job, &files);
    assert_eq!(
        run,
        ExportRunView::Failed {
            reason: ExportFailure::CannotWrite
        }
    );
    assert!(
        !files[0].path.exists(),
        "the first file is taken away again"
    );
    assert!(m.view().export.dialog.is_none());
    // A news of another job does nothing.
    cmd(&mut m, Command::OpenExport);
    m.handle(Input::Export(midi::write(job + 100, &[])));
    assert!(m.view().export.run.is_none());
}
