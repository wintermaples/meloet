//! 「書き出す」 asks before it writes over files that are there already (the owner,
//! 2026-11-25), a second press under MIDI writes nothing more, and the files of a run
//! cancelled once they were in place are taken away.

use std::path::{Path, PathBuf};

use meloet_model::Song;
use meloet_protocol::{
    Command, ExportChange, ExportFormat, ExportRange, ExportReplace, ExportRunView,
};

use super::{ExportAsk, ExportDirs, ExportNews};
use crate::midi::tests::song;
use crate::settings::{Settings, temp_dir};
use crate::state::{Effect, Input, Model};

fn asks(effects: Vec<Effect>) -> Vec<ExportAsk> {
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Export(ask) => Some(ask),
            _ => None,
        })
        .collect()
}

fn cmd(m: &mut Model, command: Command) -> Vec<ExportAsk> {
    asks(m.handle(Input::Command(command)))
}

fn news(m: &mut Model, news: ExportNews) -> Vec<ExportAsk> {
    asks(m.handle(Input::Export(news)))
}

fn set(m: &mut Model, change: ExportChange) -> Vec<ExportAsk> {
    cmd(m, Command::SetExport { change })
}

/// The song of the MIDI tests (「ピアノ」, 「ドラム」, 「ボーカル」, 「ベース」, 「空」),
/// saved as `root/曲/曲.meloet`: it is written out into `root/曲`.
fn opened(root: &Path) -> Model {
    let mut saved = Song::new_default();
    saved.doc = song().0;
    let mut m = Model::new(Settings::default()).with_export_dirs(ExportDirs {
        home: Some(root.to_path_buf()),
        usual: Some(root.join("音楽")),
    });
    cmd(&mut m, Command::OpenSong);
    let path = root.join("曲").join("曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(saved),
        from_previous: false,
    });
    std::fs::create_dir_all(root.join("曲")).unwrap();
    m
}

fn done(m: &mut Model, job: u64) {
    news(
        m,
        ExportNews::Counted {
            job,
            percent: 100,
            peak_db: Some(-6.0),
            over: 0,
            length_s: 10,
            remaining_s: None,
            lufs: None,
            true_peak_db: None,
            done: true,
        },
    );
}

/// Opens the dialog and lets its check end; the job's id.
fn open(m: &mut Model) -> u64 {
    let opened = cmd(m, Command::OpenExport);
    let [ExportAsk::Render(job)] = &opened[..] else {
        panic!("{opened:?}");
    };
    let job = job.id;
    done(m, job);
    job
}

fn replace(m: &Model) -> Option<ExportReplace> {
    m.view().export.dialog.expect("the dialog").replace
}

fn asked(count: u32, file: &str) -> Option<ExportReplace> {
    Some(ExportReplace {
        count,
        file: file.to_owned(),
    })
}

/// 「書き出す」 went on: the job goes to `path`.
fn goes(asks: &[ExportAsk], path: &Path) -> u64 {
    match asks {
        [ExportAsk::Finish { job, target }] if target.path == path => *job,
        other => panic!("not to {path:?}: {other:?}"),
    }
}

fn picked(m: &mut Model, path: PathBuf) {
    assert_eq!(cmd(m, Command::ChooseExportPlace).len(), 1);
    assert!(news(m, ExportNews::PlacePicked(Some(path))).is_empty());
}

#[test]
fn an_earlier_export_in_the_usual_place_is_asked_about_first() {
    let root = temp_dir("export-replace-usual");
    let mut m = opened(&root);
    let file = root.join("曲").join("テストの曲.wav");

    // Nothing is there: no question.
    let job = open(&mut m);
    assert_eq!(goes(&cmd(&mut m, Command::StartExport), &file), job);
    std::fs::write(&file, b"earlier").unwrap();
    news(
        &mut m,
        ExportNews::Finished {
            job,
            files: vec![file.clone()],
            wave: Vec::new(),
        },
    );

    // The same name again: the dialog stays and asks; nothing is written.
    open(&mut m);
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "テストの曲.wav"));
    assert!(m.view().export.dialog.unwrap().can_export);
    assert_eq!(m.view().export.run, None);
    // 「やめる」 and Esc: back to the dialog, its job kept.
    assert!(cmd(&mut m, Command::CloseExport).is_empty());
    assert_eq!(replace(&m), None);
    // Asked again, 「このまま書き出す」 writes over it.
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "テストの曲.wav"));
    goes(&cmd(&mut m, Command::StartExport), &file);
    assert!(m.view().export.dialog.is_none());
    assert_eq!(std::fs::read(&file).unwrap(), b"earlier");
}

#[test]
fn the_file_the_save_window_asked_about_is_not_asked_about_again_but_another_is() {
    let root = temp_dir("export-replace-picked");
    let mut m = opened(&root);
    let there = root.join("外");
    std::fs::create_dir_all(&there).unwrap();
    for file in ["a.wav", "a.flac"] {
        std::fs::write(there.join(file), b"").unwrap();
    }
    open(&mut m);
    // The window asked about 「a.wav」 when it was picked.
    picked(&mut m, there.join("a.wav"));
    // Another format writes another file: that one is asked about.
    let flac = ExportFormat::Flac;
    set(&mut m, ExportChange::Format { format: flac });
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "a.flac"));
    // A choice takes the question away.
    let wav = ExportFormat::Wav;
    set(&mut m, ExportChange::Format { format: wav });
    assert_eq!(replace(&m), None);
    let job = goes(&cmd(&mut m, Command::StartExport), &there.join("a.wav"));
    news(
        &mut m,
        ExportNews::Finished {
            job,
            files: vec![there.join("a.wav")],
            wave: Vec::new(),
        },
    );
    // The next export of it is asked about: the window asked only once.
    open(&mut m);
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "a.wav"));

    // A file that comes after its place was picked is asked about.
    picked(&mut m, there.join("b.wav"));
    assert_eq!(replace(&m), None, "picking takes the question away");
    std::fs::write(there.join("b.wav"), b"").unwrap();
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "b.wav"));
    // So is one the window asked about while it was not there yet.
    picked(&mut m, there.join("c.wav"));
    std::fs::write(there.join("c.wav"), b"").unwrap();
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "c.wav"));
}

#[test]
fn each_track_asks_about_its_files_there_already_and_about_those_that_came_meanwhile() {
    let root = temp_dir("export-replace-tracks");
    let mut m = opened(&root);
    let folder = root.join("曲");
    // Two tracks of one name: 「ピアノ.wav」 and 「ピアノ 2.wav」, as the job names them.
    let empty = m.view().song.tracks[4].id;
    m.handle(Input::Command(Command::RenameTrack {
        track: empty,
        name: "ピアノ".into(),
    }));
    for file in ["ドラム.wav", "ピアノ 2.wav", "ほか.wav"] {
        std::fs::write(folder.join(file), b"").unwrap();
    }
    open(&mut m);
    let tracks = ExportRange::Tracks;
    let again = set(&mut m, ExportChange::Range { range: tracks });
    let [ExportAsk::Drop { .. }, ExportAsk::Render(job)] = &again[..] else {
        panic!("{again:?}");
    };
    let job = job.id;
    done(&mut m, job);
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(2, "ドラム.wav"));
    // One more came while it asked: it asks again.
    std::fs::write(folder.join("ベース.wav"), b"").unwrap();
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(3, "ドラム.wav"));
    // One went: what is left was asked about.
    std::fs::remove_file(folder.join("ドラム.wav")).unwrap();
    assert_eq!(goes(&cmd(&mut m, Command::StartExport), &folder), job);
}

#[test]
fn midi_asks_too_and_a_second_press_writes_nothing_more() {
    let root = temp_dir("export-replace-midi");
    let mut m = opened(&root);
    let file = root.join("曲").join("テストの曲.mid");
    std::fs::write(&file, b"").unwrap();
    cmd(&mut m, Command::OpenExport);
    let midi = ExportFormat::Midi;
    set(&mut m, ExportChange::Format { format: midi });
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert_eq!(replace(&m), asked(1, "テストの曲.mid"));
    let started = cmd(&mut m, Command::StartExport);
    let [ExportAsk::WriteMidi { files, .. }] = &started[..] else {
        panic!("{started:?}");
    };
    assert_eq!(files[0].path, file);
    // A fast second press: the dialog has closed, nothing more is written.
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert!(matches!(
        m.view().export.run,
        Some(ExportRunView::Running { percent: 0, .. })
    ));
}

#[test]
fn a_cancel_that_comes_once_the_files_are_in_place_takes_them_away() {
    let root = temp_dir("export-replace-cancel");
    let mut m = opened(&root);
    let file = root.join("曲").join("テストの曲.wav");
    open(&mut m);
    let job = goes(&cmd(&mut m, Command::StartExport), &file);
    assert_eq!(
        cmd(&mut m, Command::CancelExport),
        [ExportAsk::Drop { job }]
    );
    // The job had moved its file and said so before it heard of the cancel.
    let finished = ExportNews::Finished {
        job,
        files: vec![file.clone()],
        wave: Vec::new(),
    };
    assert_eq!(
        news(&mut m, finished.clone()),
        [ExportAsk::Remove {
            files: vec![file.clone()]
        }]
    );
    assert_eq!(m.view().export.run, Some(ExportRunView::Cancelled));
    // Said again, or of another job, nothing is taken away.
    assert!(news(&mut m, finished).is_empty());
    let other = ExportNews::Finished {
        job: job + 100,
        files: vec![file],
        wave: Vec::new(),
    };
    assert!(news(&mut m, other).is_empty());
}
