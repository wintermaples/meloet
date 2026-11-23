use std::path::PathBuf;

use meloet_model::NewAudio;
use meloet_protocol::{
    Command, ExportFailure, ExportTail, RecordFormat, SongAnswer, StatusMessage, StepName,
    TrackKindView, TrackRef, TrackView, TransportState,
};

use super::*;
use crate::audiofiles::keeper::AudioNews;
use crate::play::tests::{ready, report, started};
use crate::play::{Heard, RecordReport};
use crate::state::Input;

const BAR: u64 = 7_680;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

/// A model with a stream open at 48 kHz, at 120 a minute, and two clips on track 1 「ピアノ」:
/// bars 1 to 4 and 9 to 12. Returns their ids.
fn model() -> (Model, u32, u32) {
    let mut m = ready();
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    cmd(&mut m, Command::CreateClip { track: 1, tick: 0 });
    cmd(
        &mut m,
        Command::CreateClip {
            track: 1,
            tick: 8 * BAR,
        },
    );
    let clips: Vec<u32> = tracks(&m)[0].clips.iter().map(|c| c.id).collect();
    (m, clips[0], clips[1])
}

fn tracks(m: &Model) -> Vec<TrackView> {
    m.view().song.tracks
}

fn steps(m: &Model) -> u32 {
    m.view().history.current
}

fn newest(m: &Model) -> StepName {
    m.history_rows(steps(m), steps(m))[0].name.clone()
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

fn shown(m: &Model) -> Option<BounceView> {
    m.view().audio_work.bounce
}

fn asked(effects: &[Effect]) -> Option<&BounceAsk> {
    effects.iter().find_map(|e| match e {
        Effect::Bounce(ask) => Some(ask),
        _ => None,
    })
}

fn bounce(m: &mut Model, command: Command) -> BounceAsk {
    let effects = cmd(m, command);
    asked(&effects).expect("a job is asked for").clone()
}

fn counted(job: u64, percent: u8) -> Input {
    Input::Export(ExportNews::Counted {
        job,
        percent,
        peak_db: Some(-6.0),
        over: 0,
        length_s: 8,
        remaining_s: Some(1),
        lufs: None,
        true_peak_db: None,
        done: percent == 100,
    })
}

fn temp(job: u64) -> PathBuf {
    PathBuf::from(format!("/state/work/音/.bounce-1-{job}.wav"))
}

fn finished(job: u64) -> Input {
    Input::Export(ExportNews::Finished {
        job,
        files: vec![temp(job)],
        wave: Vec::new(),
    })
}

/// What the rules ask the thread that reads audio files in to take.
fn to_take(effects: &[Effect]) -> Option<(u64, PathBuf, String)> {
    effects.iter().find_map(|e| match e {
        Effect::TakeBounced {
            ticket,
            temp,
            wanted,
            ..
        } => Some((*ticket, temp.clone(), wanted.clone())),
        _ => None,
    })
}

fn audio(file: &str, seconds: u64) -> NewAudio {
    NewAudio {
        uid: [9; 16],
        file: file.to_owned(),
        frames: seconds * 48_000,
        sample_rate: 48_000,
        channels: 2,
        bits: Some(24),
    }
}

fn taken(job: u64, audio: Option<NewAudio>) -> Input {
    Input::Audio(AudioNews::Adopted { ticket: job, audio })
}

fn dropped(effects: &[Effect]) -> Vec<String> {
    (effects.iter())
        .flat_map(|e| match e {
            Effect::DropVocalFiles(names) => names.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// The job renders, writes its file, and the file is taken in under `file`.
fn made(m: &mut Model, job: u64, file: &str) -> Vec<Effect> {
    m.handle(counted(job, 100));
    let effects = m.handle(finished(job));
    let (ticket, path, _) = to_take(&effects).expect("the file is to be taken in");
    assert_eq!((ticket, path), (job, temp(job)));
    m.handle(taken(job, Some(audio(file, 20))))
}

fn record(m: &mut Model) {
    cmd(m, Command::Record);
    started(m);
    assert_ne!(m.view().transport.state, TransportState::Stopped);
}

fn stop_recording(m: &mut Model) {
    let heard = Heard {
        tick: 1_000.0,
        state: TransportState::Recording,
        lap: 0,
    };
    m.set_now(100, Some(heard));
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 1_000.0,
            lap: 0,
        },
    );
    assert!(!m.play.busy());
}

#[test]
fn a_track_turned_into_audio_is_one_step_on_a_new_track_and_undo_takes_it_back() {
    let (mut m, _, second) = model();
    let before = steps(&m);
    let ask = bounce(&mut m, Command::BounceTrack { track: 1 });
    let job = &ask.job;
    // From the first clip's start to the last clip's end, the track alone, with its tail.
    let end = tracks(&m)[0].clips[1].end;
    assert_eq!((job.span.start(), job.span.end()), (0, end));
    assert_eq!(
        end,
        tracks(&m)[0]
            .clips
            .iter()
            .find(|c| c.id == second)
            .unwrap()
            .end
    );
    assert_eq!(job.doc.tracks.len(), 1);
    assert_eq!(job.tail, ExportTail::UntilSilent);
    assert!(!job.per_track && !job.realtime && job.measure.is_none());
    assert_eq!(job.sample_rate, 48_000);
    assert_eq!(ask.bits, RecordFormat::default());
    assert!((BOUNCE_JOBS..AI_JOBS).contains(&job.id));
    let job = job.id;
    assert_eq!(steps(&m), before, "no step before the audio is there");
    let piano = TrackRef {
        number: 1,
        name: "ピアノ".into(),
    };
    assert_eq!(
        shown(&m),
        Some(BounceView {
            track: piano.clone(),
            percent: 0
        })
    );
    m.handle(counted(job, 40));
    assert_eq!(shown(&m).unwrap().percent, 40);
    m.handle(counted(job, 100));
    assert_eq!(shown(&m).unwrap().percent, 99, "not done before the step");

    let effects = m.handle(finished(job));
    let (_, _, wanted) = to_take(&effects).unwrap();
    assert!(wanted.starts_with("ピアノ（音） ") && wanted.ends_with(".wav"));
    assert_eq!(steps(&m), before);
    assert!(to_take(&m.handle(Input::Tick)).is_none(), "asked once");

    m.handle(taken(job, Some(audio("ピアノ（音） 12.wav", 20))));
    assert_eq!(steps(&m), before + 1);
    assert_eq!(newest(&m), StepName::Bounce);
    assert_eq!(shown(&m), None);
    let t = tracks(&m);
    assert_eq!(t.len(), 2);
    assert!(t[0].muted, "the keys track is muted");
    assert_eq!(t[1].name, "ピアノ（音）");
    assert!(matches!(t[1].kind, TrackKindView::Vocal { .. }));
    let clip = &t[1].clips[0];
    assert_eq!(clip.start, 0);
    let audio = clip.audio.as_ref().unwrap();
    assert_eq!(audio.file, "ピアノ（音） 12.wav");
    assert!(!audio.stretch.follow);
    assert_eq!(
        status(&m),
        Some(StatusMessage::Bounced {
            track: TrackRef {
                number: 2,
                name: "ピアノ（音）".into()
            }
        })
    );
    // The answer again does nothing.
    m.handle(taken(job, Some(audio_again())));
    assert_eq!(steps(&m), before + 1);

    cmd(&mut m, Command::Undo);
    let t = tracks(&m);
    assert_eq!(t.len(), 1);
    assert!(!t[0].muted);
    cmd(&mut m, Command::Redo);
    assert_eq!(tracks(&m).len(), 2);
    assert!(tracks(&m)[0].muted);
}

fn audio_again() -> NewAudio {
    audio("ピアノ（音） 13.wav", 20)
}

#[test]
fn clips_of_one_track_from_the_earliest_start_to_the_latest_end() {
    let (mut m, first, second) = model();
    let ask = bounce(
        &mut m,
        Command::BounceToAudio {
            clips: vec![second, 9_999, first],
        },
    );
    assert_eq!(ask.job.span.start(), 0);
    assert_eq!(ask.job.span.end(), tracks(&m)[0].clips[1].end);
    cmd(&mut m, Command::CancelBounce);

    let ask = bounce(
        &mut m,
        Command::BounceToAudio {
            clips: vec![second],
        },
    );
    assert_eq!(ask.job.span.start(), 8 * BAR);
    cmd(&mut m, Command::CancelBounce);

    // No keys clip: nothing.
    for clips in [vec![], vec![9_999]] {
        let effects = cmd(&mut m, Command::BounceToAudio { clips });
        assert!(asked(&effects).is_none());
        assert_eq!(status(&m), None);
    }
}

#[test]
fn clips_of_two_tracks_do_nothing() {
    let (mut m, first, _) = model();
    cmd(&mut m, Command::DuplicateTrack { track: 1 });
    let t = tracks(&m);
    let copy = t[1].clips[0].id;
    let effects = cmd(
        &mut m,
        Command::BounceToAudio {
            clips: vec![first, copy],
        },
    );
    assert!(asked(&effects).is_none());
    assert!(shown(&m).is_none());
}

#[test]
fn not_while_recording_nor_while_another_job_runs() {
    let (mut m, first, _) = model();
    record(&mut m);
    for command in [
        Command::BounceTrack { track: 1 },
        Command::BounceToAudio { clips: vec![first] },
    ] {
        assert!(asked(&cmd(&mut m, command)).is_none());
    }
    assert!(shown(&m).is_none());
    stop_recording(&mut m);

    // A track with no clip says so; a vocal track does nothing.
    cmd(&mut m, Command::AddVocalTrack);
    let vocal = tracks(&m)[1].id;
    assert!(asked(&cmd(&mut m, Command::BounceTrack { track: vocal })).is_none());
    assert_eq!(status(&m), None);

    // Another bounce, or the export's dialog.
    bounce(&mut m, Command::BounceTrack { track: 1 });
    assert!(asked(&cmd(&mut m, Command::BounceToAudio { clips: vec![first] })).is_none());
    assert_eq!(status(&m), Some(StatusMessage::BounceBusy));
    // The export waits: its dialog does not open.
    cmd(&mut m, Command::OpenExport);
    assert_eq!(status(&m), Some(StatusMessage::ExportWaitsForBounce));
    assert!(m.view().export.dialog.is_none());
    cmd(&mut m, Command::CancelBounce);
    assert_eq!(status(&m), Some(StatusMessage::BounceCancelled));

    cmd(&mut m, Command::OpenExport);
    assert!(m.view().export.dialog.is_some());
    assert!(asked(&cmd(&mut m, Command::BounceTrack { track: 1 })).is_none());
    // The open dialog is what the status line says over it.
    assert_eq!(m.notice, Some(StatusMessage::BounceBusy));
    assert!(shown(&m).is_none());
    cmd(&mut m, Command::CloseExport);

    // The worker finds the AI link's export running.
    let ask = bounce(&mut m, Command::BounceTrack { track: 1 });
    m.handle(Input::BounceBusy { job: ask.job.id });
    assert_eq!(status(&m), Some(StatusMessage::BounceBusy));
    assert!(shown(&m).is_none());
}

#[test]
fn a_track_with_no_clip_and_a_song_of_64_tracks_say_so() {
    let mut m = ready();
    assert!(asked(&cmd(&mut m, Command::BounceTrack { track: 1 })).is_none());
    assert_eq!(status(&m), Some(StatusMessage::NothingToBounce));

    cmd(&mut m, Command::CreateClip { track: 1, tick: 0 });
    while tracks(&m).len() < MAX_TRACKS {
        cmd(&mut m, Command::AddVocalTrack);
    }
    assert!(asked(&cmd(&mut m, Command::BounceTrack { track: 1 })).is_none());
    assert_eq!(status(&m), Some(StatusMessage::TooManyTracks));
}

#[test]
fn cancel_makes_no_step_and_takes_the_file_away_whenever_it_comes() {
    let (mut m, _, _) = model();
    let before = steps(&m);

    // While it renders: the job is let go of; a file it wrote all the same is taken away.
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    let effects = cmd(&mut m, Command::CancelBounce);
    assert!(effects.contains(&Effect::Export(ExportAsk::Drop { job })));
    assert_eq!(status(&m), Some(StatusMessage::BounceCancelled));
    assert!(shown(&m).is_none());
    let effects = m.handle(finished(job));
    assert!(to_take(&effects).is_none());
    assert_eq!(dropped(&effects), [format!(".bounce-1-{job}.wav")]);

    // While the file is taken in: the answer is taken away.
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    to_take(&m.handle(finished(job))).unwrap();
    cmd(&mut m, Command::CancelBounce);
    let effects = m.handle(taken(job, Some(audio("ピアノ（音） 12.wav", 20))));
    assert_eq!(dropped(&effects), ["ピアノ（音） 12.wav"]);

    // While the step waits for a recording to end.
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    record(&mut m);
    made(&mut m, job, "ピアノ（音） 14.wav");
    assert_eq!(shown(&m).map(|b| b.percent), Some(99));
    let effects = cmd(&mut m, Command::CancelBounce);
    assert_eq!(dropped(&effects), ["ピアノ（音） 14.wav"]);
    stop_recording(&mut m);
    assert_eq!(tracks(&m).len(), 1);
    assert!(steps(&m) >= before);
    assert!(
        (m.play.song().doc.audio.iter()).all(|s| !s.file.starts_with("ピアノ（音）")),
        "no audio of the bounce is in the song"
    );
}

#[test]
fn the_step_waits_for_a_recording_to_end() {
    let (mut m, _, _) = model();
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    record(&mut m);
    let effects = made(&mut m, job, "ピアノ（音） 12.wav");
    assert!(dropped(&effects).is_empty());
    assert_eq!(tracks(&m).len(), 1, "not while recording");
    stop_recording(&mut m);
    assert_eq!(tracks(&m).len(), 2);
    assert_eq!(newest(&m), StepName::Bounce);
    assert!(shown(&m).is_none());
}

#[test]
fn a_track_that_is_gone_or_another_song_throws_the_audio_away() {
    let (mut m, _, _) = model();
    cmd(&mut m, Command::DuplicateTrack { track: 1 });
    let other = tracks(&m)[1].id;

    let job = bounce(&mut m, Command::BounceTrack { track: other }).job.id;
    cmd(&mut m, Command::DeleteTrack { track: other });
    let steps_now = steps(&m);
    let effects = made(&mut m, job, "ピアノ 2（音） 30.wav");
    assert_eq!(steps(&m), steps_now);
    assert_eq!(status(&m), Some(StatusMessage::BounceGone));
    assert_eq!(dropped(&effects), ["ピアノ 2（音） 30.wav"]);
    assert!(shown(&m).is_none());

    // Undo past the request takes the track away too.
    cmd(&mut m, Command::Undo);
    let job = bounce(&mut m, Command::BounceTrack { track: other }).job.id;
    cmd(&mut m, Command::Undo);
    assert!(tracks(&m).iter().all(|t| t.id != other));
    made(&mut m, job, "ピアノ 2（音） 31.wav");
    assert_eq!(status(&m), Some(StatusMessage::BounceGone));

    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    cmd(&mut m, Command::NewSong);
    let dialog = m.view().workspace.song_dialog.expect("changes").id;
    cmd(
        &mut m,
        Command::AnswerSongDialog {
            dialog,
            answer: SongAnswer::DontSave,
        },
    );
    let steps_now = steps(&m);
    let effects = made(&mut m, job, "ピアノ（音） 40.wav");
    assert_eq!(steps(&m), steps_now);
    assert_eq!(status(&m), Some(StatusMessage::BounceGone));
    assert_eq!(dropped(&effects), ["ピアノ（音） 40.wav"]);
}

#[test]
fn a_job_that_fails_says_why_and_leaves_the_song() {
    let (mut m, _, _) = model();
    let before = steps(&m);
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    let reason = ExportFailure::Plugin {
        name: "Surge XT".into(),
    };
    m.handle(Input::Export(ExportNews::Failed {
        job,
        reason: reason.clone(),
    }));
    assert_eq!(status(&m), Some(StatusMessage::BounceFailed { reason }));
    assert!(shown(&m).is_none());

    // The file could not be taken in.
    let job = bounce(&mut m, Command::BounceTrack { track: 1 }).job.id;
    to_take(&m.handle(finished(job))).unwrap();
    m.handle(taken(job, None));
    assert_eq!(
        status(&m),
        Some(StatusMessage::BounceFailed {
            reason: ExportFailure::CannotWrite
        })
    );
    assert_eq!(steps(&m), before);
    // The export's own news are not the bounce's.
    assert!(!m.bounce_export_news(&ExportNews::Failed {
        job: 1,
        reason: ExportFailure::CannotWrite
    }));
}
