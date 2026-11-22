//! The dialog 「書き出し」 and the run, through the whole of the rules.

use std::path::PathBuf;

use meloet_protocol::{
    Command, ExportChange, ExportCheck, ExportFailure, ExportFormat, ExportRange, ExportRunView,
    ExportSolo, ExportTail, ExportView, RecordFormat, StatusMessage, TickSpan, TrackRef,
};

use super::{ExportAsk, ExportDirs, ExportJob, ExportNews, ExportTarget};
use crate::settings::Settings;
use crate::state::{Effect, Input, Model};

const BAR: u64 = 4 * 1920;

fn cmd(m: &mut Model, command: Command) -> Vec<ExportAsk> {
    asks(m.handle(Input::Command(command)))
}

fn news(m: &mut Model, news: ExportNews) -> Vec<ExportAsk> {
    asks(m.handle(Input::Export(news)))
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
    cmd(m, Command::SetExport { change })
}

/// A song with one clip of four bars on its first track, and nowhere to save it yet.
fn song() -> Model {
    let mut m = Model::new(Settings::default()).with_export_dirs(ExportDirs {
        home: Some(PathBuf::from("/home/me")),
        usual: Some(PathBuf::from("/home/me/音楽")),
    });
    m.handle(Input::Command(Command::AddNote {
        take: None,
        start: 0,
        length: 480,
        key: 60,
        velocity: 100,
    }));
    assert_eq!(m.view().song.tracks[0].clips.len(), 1);
    m
}

fn export(m: &Model) -> ExportView {
    m.view().export
}

fn the_job(asks: &[ExportAsk]) -> &ExportJob {
    match asks {
        [ExportAsk::Render(job)] => job,
        other => panic!("not one job: {other:?}"),
    }
}

fn counted(job: u64, percent: u8, peak_db: Option<f32>, done: bool) -> ExportNews {
    ExportNews::Counted {
        job,
        percent,
        peak_db,
        // One place for a peak over 0 dB; the counting itself is tried in `job/tests.rs`.
        over: u32::from(peak_db.is_some_and(|db| db > 0.0)),
        length_s: 10,
        remaining_s: (!done).then_some(4),
        done,
    }
}

#[test]
fn opening_the_dialog_starts_the_check_and_closing_it_throws_the_job_away() {
    let mut m = song();
    assert_eq!(export(&m), ExportView::default());
    let asked = cmd(&mut m, Command::OpenExport);
    let job = the_job(&asked);
    assert_eq!((job.span.start(), job.span.end()), (0, 4 * BAR));
    assert!(!job.per_track && !job.realtime);
    assert_eq!(job.tail, ExportTail::UntilSilent);
    assert_eq!(job.sample_rate, 48_000);
    assert_eq!(job.doc.tracks[0].clips.len(), 1);
    let id = job.id;

    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.range, ExportRange::Song);
    assert_eq!(dialog.loop_range, None);
    assert_eq!(dialog.tracks, 1);
    assert_eq!(
        dialog.span,
        TickSpan {
            start: 0,
            end: 4 * BAR
        }
    );
    assert_eq!(dialog.formats, [ExportFormat::Wav, ExportFormat::Flac]);
    let name = m.view().song.name;
    assert_eq!(dialog.place, format!("~/音楽/{name}.wav"));
    assert_eq!(dialog.check, ExportCheck::Waiting);
    assert!(dialog.stopped_plugins.is_empty());
    assert_eq!(
        (dialog.sample_rate, dialog.bits),
        (48_000, RecordFormat::Bits24)
    );
    assert!(dialog.sample_rate_choices.contains(&44_100));
    assert_eq!(dialog.bits_choices, RecordFormat::ALL);
    assert!(!dialog.advanced_open && !dialog.realtime && !dialog.realtime_forced);
    assert!(!dialog.can_export);
    assert_eq!(
        m.view().status.message,
        Some(StatusMessage::ExportDialogOpen)
    );
    // The song did not change.
    assert_eq!(m.view().song.undo_count, 1);

    // A second open does nothing.
    assert!(cmd(&mut m, Command::OpenExport).is_empty());

    assert!(news(&mut m, counted(id, 38, Some(-3.5), false)).is_empty());
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(
        dialog.check,
        ExportCheck::Counting {
            percent: 38,
            peak_db: Some(-3.5),
            clips: false,
            over: 0,
        }
    );
    assert!(dialog.can_export);

    assert_eq!(
        cmd(&mut m, Command::CloseExport),
        [ExportAsk::Drop { job: id }]
    );
    assert_eq!(export(&m), ExportView::default());
    // News of a job that is gone changes nothing.
    news(&mut m, counted(id, 100, Some(-0.5), true));
    assert_eq!(export(&m), ExportView::default());
    assert_eq!(m.view().song.undo_count, 1);
}

#[test]
fn only_what_changes_the_audio_starts_the_check_anew() {
    let mut m = song();
    let first = the_job(&cmd(&mut m, Command::OpenExport)).id;
    news(&mut m, counted(first, 100, Some(0.5), true));
    let check = |m: &Model| export(m).dialog.unwrap().check;
    assert_eq!(
        check(&m),
        ExportCheck::Done {
            peak_db: Some(0.5),
            clips: true,
            over: 1,
        }
    );

    // The last step of the export: the job stays, and floats do not clip.
    assert!(
        set(
            &mut m,
            ExportChange::Bits {
                bits: RecordFormat::Bits32
            }
        )
        .is_empty()
    );
    assert_eq!(
        check(&m),
        ExportCheck::Done {
            peak_db: Some(0.5),
            clips: false,
            over: 1,
        }
    );
    // A flac file holds no floats: back to 24 bits, and the name follows the format.
    assert!(
        set(
            &mut m,
            ExportChange::Format {
                format: ExportFormat::Flac
            }
        )
        .is_empty()
    );
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.bits, RecordFormat::Bits24);
    assert_eq!(
        dialog.bits_choices,
        [RecordFormat::Bits16, RecordFormat::Bits24]
    );
    assert!(dialog.place.ends_with(".flac"));
    assert!(matches!(
        dialog.check,
        ExportCheck::Done { clips: true, .. }
    ));
    assert!(
        set(
            &mut m,
            ExportChange::Bits {
                bits: RecordFormat::Bits32
            }
        )
        .is_empty()
    );
    assert_eq!(export(&m).dialog.unwrap().bits, RecordFormat::Bits24);
    assert!(set(&mut m, ExportChange::SampleRate { hz: 44_100 }).is_empty());
    assert!(set(&mut m, ExportChange::SampleRate { hz: 12_345 }).is_empty());
    assert_eq!(export(&m).dialog.unwrap().sample_rate, 44_100);
    assert!(set(&mut m, ExportChange::AdvancedOpen { open: true }).is_empty());
    assert!(export(&m).dialog.unwrap().advanced_open);

    // The tail, the speed and the range make other audio: a new job each time.
    let asked = set(
        &mut m,
        ExportChange::Tail {
            tail: ExportTail::Off,
        },
    );
    let [ExportAsk::Drop { job }, ExportAsk::Render(second)] = &asked[..] else {
        panic!("{asked:?}");
    };
    assert_eq!(*job, first);
    assert_eq!(second.tail, ExportTail::Off);
    assert_eq!(check(&m), ExportCheck::Waiting);
    let asked = set(&mut m, ExportChange::Realtime { on: true });
    let [ExportAsk::Drop { .. }, ExportAsk::Render(third)] = &asked[..] else {
        panic!("{asked:?}");
    };
    assert!(third.realtime);
    let asked = set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Tracks,
        },
    );
    let [ExportAsk::Drop { .. }, ExportAsk::Render(fourth)] = &asked[..] else {
        panic!("{asked:?}");
    };
    assert!(fourth.per_track);
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.range, ExportRange::Tracks);
    assert_eq!(dialog.place, "~/音楽/");

    // There is no loop: the choice does nothing.
    assert!(
        set(
            &mut m,
            ExportChange::Range {
                range: ExportRange::Loop
            }
        )
        .is_empty()
    );
    assert_eq!(export(&m).dialog.unwrap().range, ExportRange::Tracks);

    // What was chosen is there again the next time.
    cmd(&mut m, Command::CloseExport);
    cmd(&mut m, Command::OpenExport);
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.range, ExportRange::Tracks);
    assert_eq!(dialog.format, ExportFormat::Flac);
    assert_eq!(dialog.sample_rate, 44_100);
    assert_eq!(dialog.tail, ExportTail::Off);
    assert!(dialog.realtime && dialog.advanced_open);
}

#[test]
fn the_loop_is_a_range_once_the_song_has_one() {
    let mut m = song();
    m.handle(Input::Command(Command::SetLoop {
        from_bar: 2,
        to_bar: 3,
    }));
    let looped = m.view().song.loop_range.unwrap();
    cmd(&mut m, Command::OpenExport);
    assert_eq!(export(&m).dialog.unwrap().loop_range, Some(looped));
    let asked = set(
        &mut m,
        ExportChange::Range {
            range: ExportRange::Loop,
        },
    );
    let [ExportAsk::Drop { .. }, ExportAsk::Render(job)] = &asked[..] else {
        panic!("{asked:?}");
    };
    assert_eq!(
        (job.span.start(), job.span.end()),
        (looped.start, looped.end)
    );
    assert_eq!(export(&m).dialog.unwrap().span, looped);
}

#[test]
fn the_place_is_chosen_in_the_pcs_window() {
    let mut m = song();
    cmd(&mut m, Command::OpenExport);
    let name = m.view().song.name;
    let asked = cmd(&mut m, Command::ChooseExportPlace);
    assert_eq!(
        asked,
        [ExportAsk::PickPlace {
            folder: Some(PathBuf::from("/home/me/音楽")),
            name: format!("{name}.wav"),
            folder_only: false,
        }]
    );
    // The window is up: a second press asks for no second one.
    assert!(cmd(&mut m, Command::ChooseExportPlace).is_empty());
    // Put away: nothing changes, and it can be asked for again.
    news(&mut m, ExportNews::PlacePicked(None));
    assert_eq!(
        export(&m).dialog.unwrap().place,
        format!("~/音楽/{name}.wav")
    );
    assert_eq!(cmd(&mut m, Command::ChooseExportPlace).len(), 1);
    let picked = PathBuf::from("/home/me/デスクトップ/できた.wav");
    assert!(news(&mut m, ExportNews::PlacePicked(Some(picked))).is_empty());
    assert_eq!(
        export(&m).dialog.unwrap().place,
        "~/デスクトップ/できた.wav"
    );
    // The ending is the format's, whatever was typed.
    set(
        &mut m,
        ExportChange::Format {
            format: ExportFormat::Flac,
        },
    );
    assert_eq!(
        export(&m).dialog.unwrap().place,
        "~/デスクトップ/できた.flac"
    );
}

#[test]
fn an_export_runs_in_the_status_line_and_ends() {
    let mut m = song();
    let job = the_job(&cmd(&mut m, Command::OpenExport)).id;
    // Nothing is known of the job yet.
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert!(export(&m).dialog.is_some());

    news(&mut m, counted(job, 40, Some(-6.0), false));
    let name = m.view().song.name;
    let path = PathBuf::from(format!("/home/me/音楽/{name}.wav"));
    assert_eq!(
        cmd(&mut m, Command::StartExport),
        [ExportAsk::Finish {
            job,
            target: ExportTarget {
                path: path.clone(),
                format: ExportFormat::Wav,
                sample_rate: 48_000,
                bits: RecordFormat::Bits24,
            },
        }]
    );
    let running = |percent: u8, remaining_s: Option<u32>, forced: bool| ExportView {
        dialog: None,
        run: Some(ExportRunView::Running {
            percent,
            file: format!("{name}.wav"),
            remaining_s,
            realtime: forced,
            realtime_forced: forced,
            length_s: 10,
        }),
    };
    assert_eq!(export(&m), running(40, Some(4), false));
    assert_ne!(
        m.view().status.message,
        Some(StatusMessage::ExportDialogOpen)
    );
    // While it runs the dialog does not open, and the keys and the song go on.
    assert!(cmd(&mut m, Command::OpenExport).is_empty());
    assert_eq!(export(&m), running(40, Some(4), false));

    news(&mut m, ExportNews::MustBeRealtime { job });
    news(&mut m, counted(job, 100, Some(-6.0), true));
    assert_eq!(export(&m), running(99, None, true));

    news(
        &mut m,
        ExportNews::Finished {
            job,
            files: vec![path.clone()],
        },
    );
    let done = ExportView {
        dialog: None,
        run: Some(ExportRunView::Done {
            file: format!("{name}.wav"),
            files: 1,
            place: format!("~/音楽/{name}.wav"),
            length_s: 10,
            sample_rate: 48_000,
            bits: RecordFormat::Bits24,
        }),
    };
    assert_eq!(export(&m), done);

    // 「フォルダをひらく」 shows the file and leaves the line.
    let effects = m.handle(Input::Command(Command::ShowExported));
    assert_eq!(effects, [Effect::ShowFile(path)]);
    assert_eq!(export(&m), done);
    // The next thing the person does puts the line away.
    m.handle(Input::Command(Command::ToggleMetronome));
    assert_eq!(export(&m), ExportView::default());
    assert!(m.handle(Input::Command(Command::ShowExported)).is_empty());
}

#[test]
fn a_chosen_real_time_export_is_not_said_to_be_a_plugins() {
    let mut m = song();
    cmd(&mut m, Command::OpenExport);
    let asked = set(&mut m, ExportChange::Realtime { on: true });
    let [ExportAsk::Drop { .. }, ExportAsk::Render(job)] = &asked[..] else {
        panic!("{asked:?}");
    };
    let job = job.id;
    news(&mut m, counted(job, 100, Some(-6.0), true));
    cmd(&mut m, Command::StartExport);
    let said = |m: &Model| match export(m).run {
        Some(ExportRunView::Running {
            realtime,
            realtime_forced,
            ..
        }) => (realtime, realtime_forced),
        other => panic!("{other:?}"),
    };
    assert_eq!(said(&m), (true, false));
    news(&mut m, ExportNews::MustBeRealtime { job });
    assert_eq!(said(&m), (true, true));
}

#[test]
fn an_export_is_cancelled_or_fails() {
    let mut m = song();
    let job = the_job(&cmd(&mut m, Command::OpenExport)).id;
    news(&mut m, counted(job, 100, None, true));
    assert_eq!(
        export(&m).dialog.unwrap().check,
        ExportCheck::Done {
            peak_db: None,
            clips: false,
            over: 0,
        }
    );
    cmd(&mut m, Command::StartExport);
    assert_eq!(
        cmd(&mut m, Command::CancelExport),
        [ExportAsk::Drop { job }]
    );
    assert_eq!(export(&m).run, Some(ExportRunView::Cancelled));
    // What the worker still says of the job is not heard.
    news(
        &mut m,
        ExportNews::Finished {
            job,
            files: vec![PathBuf::from("/home/me/音楽/a.wav")],
        },
    );
    assert_eq!(export(&m).run, Some(ExportRunView::Cancelled));
    assert!(cmd(&mut m, Command::CancelExport).is_empty());

    // The check fails: the dialog says so and cannot export.
    let job = the_job(&cmd(&mut m, Command::OpenExport)).id;
    assert_eq!(export(&m).run, None);
    let plugin = ExportFailure::Plugin {
        name: "Surge XT".into(),
    };
    news(
        &mut m,
        ExportNews::Failed {
            job,
            reason: plugin.clone(),
        },
    );
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.check, ExportCheck::Failed { reason: plugin });
    assert!(!dialog.can_export);
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert!(export(&m).dialog.is_some());

    // The run fails.
    cmd(&mut m, Command::CloseExport);
    let job = the_job(&cmd(&mut m, Command::OpenExport)).id;
    news(&mut m, counted(job, 10, Some(-12.0), false));
    cmd(&mut m, Command::StartExport);
    news(
        &mut m,
        ExportNews::Failed {
            job,
            reason: ExportFailure::CannotWrite,
        },
    );
    assert_eq!(
        export(&m).run,
        Some(ExportRunView::Failed {
            reason: ExportFailure::CannotWrite
        })
    );
}

#[test]
fn an_empty_song_has_nothing_to_export() {
    let mut m = Model::new(Settings::default());
    assert!(cmd(&mut m, Command::OpenExport).is_empty());
    let dialog = export(&m).dialog.unwrap();
    assert_eq!(dialog.span, TickSpan { start: 0, end: 0 });
    assert_eq!(dialog.check, ExportCheck::Waiting);
    assert!(!dialog.can_export);
    assert!(cmd(&mut m, Command::StartExport).is_empty());
    assert!(cmd(&mut m, Command::CloseExport).is_empty());
    assert_eq!(export(&m), ExportView::default());
}

#[test]
fn a_sink_is_not_made_where_nothing_can_be_written() {
    let spec = super::sink::SinkSpec {
        format: ExportFormat::Wav,
        sample_rate: 48_000,
        channels: 2,
        bits: RecordFormat::Bits24,
    };
    let made = super::sink::create(&PathBuf::from("/nowhere/a.wav"), spec);
    assert_eq!(
        made.err().map(|e| e.kind()),
        Some(std::io::ErrorKind::NotFound)
    );
}

#[test]
fn the_check_names_the_soloed_tracks_and_the_job_stays() {
    let mut m = song();
    cmd(&mut m, Command::AddVocalTrack);
    cmd(&mut m, Command::AddVocalTrack);
    let tracks = m.view().song.tracks.clone();
    assert_eq!(tracks.len(), 3);
    cmd(&mut m, Command::OpenExport);
    assert_eq!(export(&m).dialog.unwrap().solo, None);

    // Soloing is not in what is written: no new job.
    assert!(
        cmd(
            &mut m,
            Command::ToggleSolo {
                track: tracks[2].id
            }
        )
        .is_empty()
    );
    let solo = |m: &Model| export(m).dialog.unwrap().solo;
    assert_eq!(
        solo(&m),
        Some(ExportSolo {
            first: TrackRef {
                number: 3,
                name: tracks[2].name.clone()
            },
            others: 0
        })
    );
    assert!(
        cmd(
            &mut m,
            Command::ToggleSolo {
                track: tracks[1].id
            }
        )
        .is_empty()
    );
    assert_eq!(
        solo(&m),
        Some(ExportSolo {
            first: TrackRef {
                number: 2,
                name: tracks[1].name.clone()
            },
            others: 1
        })
    );
    cmd(
        &mut m,
        Command::ToggleSolo {
            track: tracks[1].id,
        },
    );
    cmd(
        &mut m,
        Command::ToggleSolo {
            track: tracks[2].id,
        },
    );
    assert_eq!(solo(&m), None);
}
