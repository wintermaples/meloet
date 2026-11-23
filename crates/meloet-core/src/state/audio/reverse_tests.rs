use meloet_model::{ImportedAudio, NewAudio, PlayedFrames};
use meloet_protocol::{ClipView, StepName, TransportState};

use super::*;
use crate::play::tests::{report, started};
use crate::play::{Heard, RecordReport};
use crate::state::Input;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

/// A model with a stream open at 48 kHz, at 120 a minute.
fn model() -> Model {
    let mut m = crate::play::tests::ready().with_cache_dir(Some("/cache/meloet".into()));
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    m
}

/// A file of `seconds` at 48 kHz brought in at the play line, on a new vocal track.
/// Returns its clip.
fn bring(m: &mut Model, name: &str, seconds: u64) -> u32 {
    cmd(m, Command::ImportAudio { track: None });
    let effects = m.handle(Input::AudioPicked(vec![PathBuf::from("/in").join(name)]));
    let ticket = (effects.iter())
        .find_map(|e| match e {
            Effect::BringAudio { ticket, .. } => Some(*ticket),
            _ => None,
        })
        .expect("the file is brought in");
    let file = Brought::In(ImportedAudio {
        audio: NewAudio {
            uid: [name.len() as u8; 16],
            file: name.to_owned(),
            frames: seconds * 48_000,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
        },
        sha256: [1; 32],
        name: name.to_owned(),
    });
    m.handle(Input::Audio(AudioNews::Brought {
        ticket,
        files: vec![file],
    }));
    let clip = clips(m)
        .into_iter()
        .find(|c| c.audio.as_ref().is_some_and(|a| a.file == name));
    clip.expect("the clip is in").id
}

fn clips(m: &Model) -> Vec<ClipView> {
    (m.view().song.tracks.into_iter())
        .flat_map(|t| t.clips)
        .collect()
}

fn clip(m: &Model, id: u32) -> ClipView {
    clips(m).into_iter().find(|c| c.id == id).unwrap()
}

fn gain(m: &Model, id: u32) -> f32 {
    clip(m, id).audio.unwrap().gain_db
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

fn measured(effects: &[Effect]) -> (u64, Vec<PlayedFrames>) {
    (effects.iter())
        .find_map(|e| match e {
            Effect::MeasureAudio { ticket, played } => Some((*ticket, played.clone())),
            _ => None,
        })
        .expect("the audio thread is asked to measure")
}

fn asked_to_reverse(effects: &[Effect]) -> (u64, Vec<(PlayedFrames, String)>, Vec<String>) {
    (effects.iter())
        .find_map(|e| match e {
            Effect::ReverseAudio {
                ticket,
                played,
                taken,
            } => Some((*ticket, played.clone(), taken.clone())),
            _ => None,
        })
        .expect("the audio thread is asked to reverse")
}

/// The files the audio thread would make for what it was asked.
fn made(played: &[(PlayedFrames, String)]) -> Vec<Option<NewAudio>> {
    (played.iter().enumerate())
        .map(|(i, (p, name))| {
            Some(NewAudio {
                uid: [100 + i as u8; 16],
                file: name.clone(),
                frames: p.backwards_frames(),
                sample_rate: p.source.sample_rate,
                channels: p.source.channels,
                bits: p.source.bits,
            })
        })
        .collect()
}

#[test]
fn normalize_measures_first_and_is_one_step_when_the_answer_comes() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let b = bring(&mut m, "歌 2.wav", 2);
    let before = steps(&m);
    let effects = cmd(
        &mut m,
        Command::NormalizeClips {
            clips: vec![b, 9_999, a, b],
        },
    );
    let (ticket, played) = measured(&effects);
    // In the order asked, each once; an id of nothing is passed over.
    assert_eq!(played.iter().map(|p| p.clip.0).collect::<Vec<_>>(), [b, a]);
    assert_eq!(
        (played[0].from, played[0].to, played[0].audio_end),
        (0, 96_000, 96_000)
    );
    assert_eq!(steps(&m), before);

    // The loudest of `b` is at half of full scale; `a` is silent.
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.5), Some(0.0)],
    }));
    assert_eq!(steps(&m), before + 1);
    assert_eq!(newest(&m), StepName::NormalizeClips);
    assert!((gain(&m, b) - 5.02).abs() < 1e-4);
    assert_eq!(gain(&m, a), 0.0);
    assert_eq!(
        status(&m),
        Some(StatusMessage::NormalizeSilent { count: 1 })
    );
    // The same answer again does nothing: the ticket is spent.
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.1), Some(0.1)],
    }));
    assert_eq!(steps(&m), before + 1);

    cmd(&mut m, Command::Undo);
    assert_eq!(gain(&m, b), 0.0);
    cmd(&mut m, Command::Redo);
    assert!((gain(&m, b) - 5.02).abs() < 1e-4);

    // Too quiet to reach −1 dBFS: up to +12, and told.
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a] });
    let (ticket, _) = measured(&effects);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.01)],
    }));
    assert_eq!(gain(&m, a), 12.0);
    assert_eq!(
        status(&m),
        Some(StatusMessage::NormalizeLimited { count: 1 })
    );
    // A file that could not be read: nothing, and it is said.
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![b] });
    let (ticket, _) = measured(&effects);
    let steps_now = steps(&m);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![None],
    }));
    assert_eq!(steps(&m), steps_now);
    assert_eq!(status(&m), Some(StatusMessage::ClipWorkFailed { count: 1 }));
    // One read and one not: the one read is a step, and the other is said first.
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a, b] });
    let (ticket, _) = measured(&effects);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.0), None],
    }));
    assert_eq!(status(&m), Some(StatusMessage::ClipWorkFailed { count: 1 }));
}

#[test]
fn an_answer_for_a_clip_that_changed_since_does_nothing() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let b = bring(&mut m, "歌 2.wav", 2);
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a, b] });
    let (ticket, _) = measured(&effects);
    // `a` is trimmed and `b` deleted while the files are read.
    cmd(
        &mut m,
        Command::TrimClip {
            clip: a,
            edge: meloet_protocol::ClipEdge::Start,
            ticks: 1_920,
        },
    );
    cmd(&mut m, Command::SelectClip { clip: Some(b) });
    cmd(&mut m, Command::DeleteSelected);
    let before = steps(&m);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.5), Some(0.5)],
    }));
    assert_eq!(steps(&m), before);
    assert!(!matches!(
        status(&m),
        Some(StatusMessage::NormalizeSilent { .. } | StatusMessage::NormalizeLimited { .. })
    ));

    // Undone back to where it was measured: then it goes in.
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a] });
    let (ticket, _) = measured(&effects);
    cmd(&mut m, Command::Undo);
    cmd(&mut m, Command::Undo);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.5)],
    }));
    assert_eq!(gain(&m, a), 0.0);

    // Another song: the answer is for the one before.
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a] });
    let (ticket, _) = measured(&effects);
    cmd(&mut m, Command::NewSong);
    let dialog = m
        .view()
        .workspace
        .song_dialog
        .expect("the song has changes")
        .id;
    cmd(
        &mut m,
        Command::AnswerSongDialog {
            dialog,
            answer: meloet_protocol::SongAnswer::DontSave,
        },
    );
    assert!(clips(&m).is_empty());
    let before = steps(&m);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.5)],
    }));
    assert_eq!(steps(&m), before);
}

#[test]
fn nothing_is_asked_while_recording_and_an_answer_waits_for_it_to_end() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a] });
    let (ticket, _) = measured(&effects);
    cmd(&mut m, Command::Record);
    started(&mut m);
    assert_ne!(m.view().transport.state, TransportState::Stopped);
    for command in [
        Command::NormalizeClips { clips: vec![a] },
        Command::ReverseClips { clips: vec![a] },
    ] {
        let effects = cmd(&mut m, command);
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::MeasureAudio { .. } | Effect::ReverseAudio { .. }))
        );
    }
    let before = steps(&m);
    m.handle(Input::Audio(AudioNews::Measured {
        ticket,
        peaks: vec![Some(0.5)],
    }));
    assert_eq!(gain(&m, a), 0.0);
    let heard = Heard {
        tick: 1_000.0,
        state: TransportState::Recording,
        lap: 0,
    };
    m.set_now(100, Some(heard));
    cmd(&mut m, Command::Record);
    report(
        &mut m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 1_000.0,
            lap: 0,
        },
    );
    assert!(!m.play.busy());
    assert!((gain(&m, a) - 5.02).abs() < 1e-4);
    assert!(steps(&m) > before);
    assert_eq!(newest(&m), StepName::NormalizeClips);
}

#[test]
fn reverse_puts_a_new_take_in_one_step_once_the_files_are_made() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let b = bring(&mut m, "歌 2.wav", 2);
    let before = steps(&m);
    let old = clip(&m, a);
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a, b] });
    let (ticket, played, taken) = asked_to_reverse(&effects);
    assert_eq!(played.len(), 2);
    // Named as a recording names its file, with the numbers the step gives.
    let track = m
        .view()
        .song
        .tracks
        .iter()
        .find(|t| t.clips.iter().any(|c| c.id == a))
        .unwrap()
        .name
        .clone();
    assert!(played[0].1.starts_with(&format!("{track} ")) && played[0].1.ends_with(".wav"));
    assert_ne!(played[0].1, played[1].1);
    assert_eq!(taken, ["歌 1.wav", "歌 2.wav"]);
    // A clip that waits for its file is not asked again.
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a] });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::ReverseAudio { .. }))
    );
    let effects = cmd(&mut m, Command::NormalizeClips { clips: vec![a] });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::MeasureAudio { .. }))
    );

    let files = made(&played);
    m.handle(Input::Audio(AudioNews::Reversed {
        ticket,
        files: files.clone(),
    }));
    assert_eq!(steps(&m), before + 1);
    assert_eq!(newest(&m), StepName::ReverseClips);
    let new = clip(&m, a);
    assert_ne!(new.take_id, old.take_id);
    assert_eq!(
        (new.start, new.end, new.take_offset),
        (old.start, old.end, 0)
    );
    let audio = new.audio.unwrap();
    assert_eq!(audio.file, files[0].as_ref().unwrap().file);
    assert_eq!((audio.first_frame, audio.frames), (0, 96_000));
    assert_eq!(status(&m), None);
    // Asked again now: the new take.
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a] });
    let (_, again, taken) = asked_to_reverse(&effects);
    assert_eq!(again[0].0.take.0, new.take_id);
    assert!(taken.contains(&audio.file));

    cmd(&mut m, Command::Undo);
    assert_eq!(clip(&m, a).take_id, old.take_id);
    cmd(&mut m, Command::Redo);
    assert_eq!(clip(&m, a).take_id, new.take_id);
}

#[test]
fn a_backwards_file_that_could_not_be_written_is_said() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let b = bring(&mut m, "歌 2.wav", 2);
    let before = steps(&m);
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a, b] });
    let (ticket, played, _) = asked_to_reverse(&effects);
    m.handle(Input::Audio(AudioNews::Reversed {
        ticket,
        files: vec![None, None],
    }));
    assert_eq!(steps(&m), before);
    assert_eq!(status(&m), Some(StatusMessage::ClipWorkFailed { count: 2 }));
    // One made and one not: the one made is a step, and the other is said.
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a, b] });
    let (ticket, again, _) = asked_to_reverse(&effects);
    assert_eq!(again, played);
    let mut files = made(&again);
    files[1] = None;
    m.handle(Input::Audio(AudioNews::Reversed { ticket, files }));
    assert_eq!(steps(&m), before + 1);
    assert_eq!(status(&m), Some(StatusMessage::ClipWorkFailed { count: 1 }));
}

#[test]
fn a_late_reverse_for_a_clip_that_shows_another_take_does_nothing() {
    let mut m = model();
    let a = bring(&mut m, "歌 1.wav", 2);
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a] });
    let (ticket, played, _) = asked_to_reverse(&effects);
    // Moved a bar on: the tempo is the same there, so it plays the same frames.
    cmd(
        &mut m,
        Command::MoveClips {
            clips: vec![a],
            ticks: 7_680,
            tracks: 0,
            copy: false,
        },
    );
    let start = clip(&m, a).start;
    m.handle(Input::Audio(AudioNews::Reversed {
        ticket,
        files: made(&played),
    }));
    assert_eq!(newest(&m), StepName::ReverseClips);
    assert_eq!(clip(&m, a).start, start);

    // The file could not be made, or the clip is longer now: nothing.
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a] });
    let (ticket, played, _) = asked_to_reverse(&effects);
    let before = steps(&m);
    m.handle(Input::Audio(AudioNews::Reversed {
        ticket,
        files: vec![None],
    }));
    assert_eq!(steps(&m), before);
    let effects = cmd(&mut m, Command::ReverseClips { clips: vec![a] });
    let (ticket, played_too, _) = asked_to_reverse(&effects);
    assert_eq!(played_too[0].0, played[0].0);
    cmd(
        &mut m,
        Command::TrimClip {
            clip: a,
            edge: meloet_protocol::ClipEdge::End,
            ticks: 1_920,
        },
    );
    let before = steps(&m);
    m.handle(Input::Audio(AudioNews::Reversed {
        ticket,
        files: made(&played_too),
    }));
    assert_eq!(steps(&m), before);
}
