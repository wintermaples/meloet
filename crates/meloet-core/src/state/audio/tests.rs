use meloet_model::{ImportedAudio, NewAudio};
use meloet_protocol::{
    AudioJob, AudioJobKind, AudioWorkView, ClipField, ClipView, MissingRef, SelectMode, StepName,
    TrackRef,
};

use std::path::Path;

use super::*;
use crate::audiofiles::keeper::{Growing, SourceState};
use crate::peaks::{PeakBuilder, PeaksPlace};
use crate::state::Input;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

/// A model with a stream open at 48 kHz.
fn model() -> Model {
    let mut m = crate::play::tests::ready().with_cache_dir(Some("/cache/meloet".into()));
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    m
}

/// A file as the audio thread says it is in the work place: `seconds` long, stereo.
fn brought(name: &str, seconds: u64, rate: u32) -> Brought {
    Brought::In(ImportedAudio {
        audio: NewAudio {
            uid: [name.len() as u8; 16],
            file: name.to_owned(),
            frames: seconds * u64::from(rate),
            sample_rate: rate,
            channels: 2,
            bits: None,
        },
        sha256: [name.len() as u8; 32],
        name: name
            .rsplit_once('.')
            .map_or(name, |(stem, _)| stem)
            .to_owned(),
    })
}

/// The ticket and the paths the audio thread was asked to bring in.
fn asked(effects: &[Effect]) -> (u64, Vec<String>) {
    (effects.iter())
        .find_map(|e| match e {
            Effect::BringAudio { ticket, paths, .. } => Some((
                *ticket,
                (paths.iter())
                    .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                    .collect(),
            )),
            _ => None,
        })
        .expect("files are brought in")
}

/// Lets go of the files over the window and says where; then the audio thread's answer.
fn drop_files(
    m: &mut Model,
    names: &[&str],
    track: Option<u32>,
    tick: u64,
    mode: DropMode,
    answer: Vec<Brought>,
) -> Vec<Effect> {
    let paths: Vec<PathBuf> = names.iter().map(|n| PathBuf::from("/in").join(n)).collect();
    m.handle(Input::FilesOver(paths.clone()));
    let effects = m.handle(Input::FilesDropped(paths));
    let drop = (effects.iter())
        .find_map(|e| match e {
            Effect::FileEvent(meloet_protocol::Event::FileDrop { drop, .. }) => Some(*drop),
            _ => None,
        })
        .expect("the drop is told");
    let effects = cmd(
        m,
        Command::DropAudio {
            drop,
            track,
            tick,
            mode,
        },
    );
    let (ticket, _) = asked(&effects);
    m.handle(Input::Audio(AudioNews::Brought {
        ticket,
        files: answer,
    }))
}

fn clips(m: &Model) -> Vec<(u32, ClipView)> {
    (m.view().song.tracks.into_iter())
        .flat_map(|t| t.clips.into_iter().map(move |c| (t.id, c)))
        .collect()
}

fn vocal_track(m: &mut Model) -> u32 {
    cmd(m, Command::AddVocalTrack);
    m.view().song.tracks.last().unwrap().id
}

fn undo_count(m: &Model) -> u32 {
    m.view().history.current
}

#[test]
fn one_file_let_go_of_over_a_vocal_track_goes_there_as_one_step() {
    let mut m = model();
    let vocal = vocal_track(&mut m);
    let steps = undo_count(&m);
    drop_files(
        &mut m,
        &["ドラム.wav"],
        Some(vocal),
        4 * BAR,
        DropMode::Separate,
        vec![brought("ドラム.wav", 2, 48_000)],
    );
    let all = clips(&m);
    assert_eq!(all.len(), 1);
    let (track, clip) = &all[0];
    assert_eq!(*track, vocal);
    // 2 seconds at 96 a minute: 3.2 quarter notes.
    assert_eq!((clip.start, clip.end), (4 * BAR, 4 * BAR + 6_144));
    assert_eq!(clip.name, "ドラム");
    let audio = clip.audio.as_ref().unwrap();
    assert_eq!(
        (audio.file.as_str(), audio.frames, audio.resampled),
        ("ドラム.wav", 96_000, false)
    );
    assert!(!audio.stretch.follow);
    assert_eq!(undo_count(&m), steps + 1);
    assert_eq!(
        status(&m),
        Some(StatusMessage::AudioImported {
            file: "ドラム.wav".into(),
            track: TrackRef {
                number: 2,
                name: "歌".into()
            },
            tick: 4 * BAR,
            count: 1,
            skipped: 0,
        })
    );
    // The clip is selected: the parameters panel shows it.
    assert_eq!(m.view().arrange.selected_clips, [clip.id]);
    assert_eq!(
        m.view().params.selected,
        Some(SelectedParams::Clip(SelectedClip {
            track: vocal,
            clip: clip.id,
            missing: None,
            tune_usable: true,
        }))
    );
    let rows = m.history_rows(steps + 1, steps + 1);
    assert_eq!(rows[0].name, StepName::ImportAudio);
    cmd(&mut m, Command::Undo);
    assert!(clips(&m).is_empty());
    assert!(m.view().song.tracks.iter().any(|t| t.id == vocal));
}

#[test]
fn a_file_that_needs_a_new_track_past_sixty_four_is_said() {
    let mut m = model();
    while m.view().song.tracks.len() < crate::play::MAX_TRACKS {
        vocal_track(&mut m);
    }
    let steps = undo_count(&m);
    drop_files(
        &mut m,
        &["ドラム.wav"],
        None,
        0,
        DropMode::InRow,
        vec![brought("ドラム.wav", 2, 48_000)],
    );
    assert_eq!(status(&m), Some(StatusMessage::TooManyTracks));
    assert_eq!(undo_count(&m), steps);
    assert!(clips(&m).is_empty());
    assert_eq!(m.view().song.tracks.len(), crate::play::MAX_TRACKS);
    // Over a vocal track it still goes in: no track is added.
    let last = m.view().song.tracks.last().unwrap().id;
    drop_files(
        &mut m,
        &["ドラム.wav"],
        Some(last),
        0,
        DropMode::InRow,
        vec![brought("ドラム.wav", 2, 48_000)],
    );
    assert_eq!(clips(&m).len(), 1);
    assert_eq!(undo_count(&m), steps + 1);
}

#[test]
fn several_files_go_apart_or_in_a_row_as_one_step() {
    let files = || {
        vec![
            brought("a.mp3", 2, 44_100),
            Brought::Unreadable("b.mp3".into(), Unread::Broken),
            brought("c.ogg", 1, 44_100),
        ]
    };
    // Apart: a new vocal track for each, whatever they were let go of over.
    let mut m = model();
    let vocal = vocal_track(&mut m);
    let steps = undo_count(&m);
    let names = ["c.ogg", "b.mp3", "a.mp3"];
    let paths: Vec<PathBuf> = names.iter().map(|n| PathBuf::from("/in").join(n)).collect();
    m.handle(Input::FilesOver(paths.clone()));
    let told = m.handle(Input::FilesDropped(paths));
    let drop = (told.iter())
        .find_map(|e| match e {
            Effect::FileEvent(meloet_protocol::Event::FileDrop { drop, .. }) => Some(*drop),
            _ => None,
        })
        .unwrap();
    let effects = cmd(
        &mut m,
        Command::DropAudio {
            drop,
            track: Some(vocal),
            tick: BAR,
            mode: DropMode::Separate,
        },
    );
    // In the order of their names.
    let (ticket, order) = asked(&effects);
    assert_eq!(order, ["a.mp3", "b.mp3", "c.ogg"]);
    // An answer to the same drop a second time does nothing.
    assert!(
        cmd(
            &mut m,
            Command::DropAudio {
                drop,
                track: None,
                tick: 0,
                mode: DropMode::InRow
            }
        )
        .iter()
        .all(|e| !matches!(e, Effect::BringAudio { .. }))
    );
    m.handle(Input::Audio(AudioNews::Brought {
        ticket,
        files: files(),
    }));
    let all = clips(&m);
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .all(|(track, c)| *track != vocal && c.start == BAR)
    );
    assert_ne!(all[0].0, all[1].0);
    assert_eq!(m.view().song.tracks.len(), 4);
    assert_eq!(undo_count(&m), steps + 1);
    assert!(matches!(
        status(&m),
        Some(StatusMessage::AudioImported {
            count: 2,
            skipped: 1,
            ..
        })
    ));
    assert!(all.iter().all(|(_, c)| c.audio.as_ref().unwrap().resampled));
    cmd(&mut m, Command::Undo);
    assert_eq!((clips(&m).len(), m.view().song.tracks.len()), (0, 2));

    // In a row: on the track, one after the other.
    let mut m = model();
    let vocal = vocal_track(&mut m);
    drop_files(&mut m, &names, Some(vocal), BAR, DropMode::InRow, files());
    let all = clips(&m);
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|(track, _)| *track == vocal));
    assert_eq!(all[0].1.start, BAR);
    assert_eq!(all[1].1.start, all[0].1.end);
    assert_eq!(m.view().song.tracks.len(), 2);
}

#[test]
fn files_that_do_not_read_bring_nothing_in() {
    let mut m = model();
    let steps = undo_count(&m);
    // Each says why: not a kind meloet reads, broken, three channels or more (spec 6).
    for (name, why, said) in [
        (
            "声.mp3",
            Unread::NotAudio,
            StatusMessage::AudioUnreadable {
                file: "声.mp3".into(),
            },
        ),
        (
            "声.wav",
            Unread::Broken,
            StatusMessage::AudioBroken {
                file: "声.wav".into(),
            },
        ),
        (
            "8ch.wav",
            Unread::TooManyChannels,
            StatusMessage::AudioTooManyChannels {
                file: "8ch.wav".into(),
            },
        ),
    ] {
        drop_files(
            &mut m,
            &[name],
            None,
            0,
            DropMode::Separate,
            vec![Brought::Unreadable(name.into(), why)],
        );
        assert!(clips(&m).is_empty());
        assert_eq!(undo_count(&m), steps);
        assert_eq!(status(&m), Some(said));
    }
}

#[test]
fn the_menu_asks_the_pc_for_files_and_puts_them_at_the_play_line() {
    let mut m = model();
    let vocal = vocal_track(&mut m);
    cmd(&mut m, Command::Locate { tick: 2 * BAR });
    let effects = cmd(&mut m, Command::ImportAudio { track: None });
    assert!(effects.contains(&Effect::PickAudio { many: true }));
    // Put away: nothing.
    assert!(
        m.handle(Input::AudioPicked(Vec::new()))
            .iter()
            .all(|e| !matches!(e, Effect::BringAudio { .. }))
    );
    cmd(&mut m, Command::ImportAudio { track: None });
    let effects = m.handle(Input::AudioPicked(vec![
        "/in/b.wav".into(),
        "/in/a.wav".into(),
    ]));
    let (ticket, order) = asked(&effects);
    assert_eq!(order, ["a.wav", "b.wav"]);
    m.handle(Input::Audio(AudioNews::Brought {
        ticket,
        files: vec![brought("a.wav", 1, 48_000), brought("b.wav", 1, 48_000)],
    }));
    // The selected track is the vocal track: in a row from the play line.
    let all = clips(&m);
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|(track, _)| *track == vocal));
    assert_eq!((all[0].1.start, all[1].1.start), (2 * BAR, all[0].1.end));

    // With a keys track selected: a new vocal track.
    let keys = m.view().song.tracks[0].id;
    cmd(
        &mut m,
        Command::SelectTrack {
            track: keys,
            from_arm: false,
        },
    );
    cmd(&mut m, Command::ImportAudio { track: None });
    let effects = m.handle(Input::AudioPicked(vec!["/in/c.wav".into()]));
    let (ticket, _) = asked(&effects);
    m.handle(Input::Audio(AudioNews::Brought {
        ticket,
        files: vec![brought("c.wav", 1, 48_000)],
    }));
    assert_eq!(m.view().song.tracks.len(), 3);
    assert_eq!(clips(&m).last().unwrap().0, m.view().song.tracks[2].id);
}

#[test]
fn a_clip_is_cut_at_the_play_line_and_its_values_are_steps() {
    let mut m = model();
    let vocal = vocal_track(&mut m);
    drop_files(
        &mut m,
        &["歌.wav"],
        Some(vocal),
        0,
        DropMode::Separate,
        vec![brought("歌.wav", 5, 48_000)],
    );
    let clip = clips(&m)[0].1.clone();
    assert_eq!((clip.start, clip.end), (0, 2 * BAR));
    let steps = undo_count(&m);
    let field = |m: &mut Model, field, value| {
        cmd(
            m,
            Command::SetClipField {
                clip: clip.id,
                field,
                value,
            },
        )
    };
    field(&mut m, ClipField::FadeIn, Q as f64);
    field(&mut m, ClipField::FadeOut, 100.0 * BAR as f64);
    field(&mut m, ClipField::GainDb, -80.0);
    field(&mut m, ClipField::GainDb, f64::NAN);
    let audio = clips(&m)[0].1.audio.clone().unwrap();
    // Outside the range: put into it without a word.
    assert_eq!(
        (audio.fade_in, audio.fade_out, audio.gain_db),
        (Q, 2 * BAR - Q, -60.0)
    );
    assert_eq!(status(&m), None);
    assert_eq!(undo_count(&m), steps + 3);
    let names: Vec<StepName> = (m.history_rows(steps + 1, steps + 3).into_iter())
        .map(|r| r.name)
        .collect();
    assert_eq!(
        names,
        [
            StepName::SetClipFade,
            StepName::SetClipFade,
            StepName::SetClipGain
        ]
    );

    // S with the clip selected and the line inside it.
    cmd(&mut m, Command::Locate { tick: BAR });
    cmd(&mut m, Command::SplitClips);
    let all = clips(&m);
    assert_eq!(all.len(), 2);
    let (left, right) = (&all[0].1, &all[1].1);
    assert_eq!(
        (left.id, left.end, right.start, right.end),
        (clip.id, BAR, BAR, 2 * BAR)
    );
    assert_eq!(left.take_id, right.take_id);
    let (l, r) = (left.audio.as_ref().unwrap(), right.audio.as_ref().unwrap());
    assert_eq!(
        (l.fade_in, l.fade_out, r.fade_in, r.fade_out),
        (Q, 0, 0, BAR)
    );
    assert_eq!((l.first_frame, r.first_frame), (0, 0));
    assert_eq!((left.take_offset, right.take_offset), (0, -(BAR as i64)));
    assert_eq!(m.view().arrange.selected_clips, [right.id]);
    assert_eq!(
        status(&m),
        Some(StatusMessage::ClipSplit {
            name: "歌".into(),
            tick: BAR,
            count: 1
        })
    );
    assert_eq!(undo_count(&m), steps + 4);
    // The line on an edge of the selected clip: nothing.
    cmd(&mut m, Command::SplitClips);
    assert_eq!(
        (clips(&m).len(), undo_count(&m), status(&m)),
        (2, steps + 4, None)
    );
    // Nothing selected: the clip of the selected track under the line.
    cmd(
        &mut m,
        Command::SelectClips {
            clips: Vec::new(),
            mode: SelectMode::Replace,
        },
    );
    cmd(&mut m, Command::Locate { tick: Q });
    cmd(&mut m, Command::SplitClips);
    assert_eq!(clips(&m).len(), 3);

    // A typed position that would overlap is said; a typed length is taken.
    let right = clips(&m)[2].1.id;
    cmd(
        &mut m,
        Command::SetClipField {
            clip: right,
            field: ClipField::Start,
            value: 0.0,
        },
    );
    assert_eq!(status(&m), Some(StatusMessage::ClipsNotMoved));
    cmd(
        &mut m,
        Command::SetClipField {
            clip: right,
            field: ClipField::Start,
            value: 3.0 * BAR as f64,
        },
    );
    cmd(
        &mut m,
        Command::SetClipField {
            clip: right,
            field: ClipField::Length,
            value: Q as f64,
        },
    );
    let moved = clips(&m)[2].1.clone();
    assert_eq!((moved.start, moved.end), (3 * BAR, 3 * BAR + Q));
    assert_eq!(moved.audio.unwrap().fade_out, Q);
}

/// A model with one file in a clip, and what the audio thread was asked to watch.
fn one_clip(rate: u32) -> (Model, ClipView, [u8; 16]) {
    let mut m = model();
    let vocal = vocal_track(&mut m);
    let effects = drop_files(
        &mut m,
        &["歌.mp3"],
        Some(vocal),
        0,
        DropMode::Separate,
        vec![brought("歌.mp3", 5, rate)],
    );
    let watched = (effects.iter())
        .find_map(|e| match e {
            Effect::WatchAudio {
                sources,
                sample_rate,
                ..
            } => Some((sources.clone(), *sample_rate)),
            _ => None,
        })
        .expect("the new table is watched");
    assert_eq!(watched.1, Some(48_000));
    assert_eq!(watched.0.len(), 1);
    let clip = clips(&m)[0].1.clone();
    (m, clip, watched.0[0].uid)
}

fn states(m: &mut Model, uid: [u8; 16], state: SourceState) {
    let serial = m.song_serial();
    m.handle(Input::Audio(AudioNews::States {
        serial,
        states: vec![(uid, state)],
    }));
}

#[test]
fn a_file_that_is_being_read_shows_how_far_it_is() {
    let (mut m, clip, uid) = one_clip(44_100);
    let audio = clip.audio.unwrap().audio;
    assert_eq!(m.view().audio_work, AudioWorkView::default());
    states(&mut m, uid, SourceState::Loading);
    let state = |m: &Model| clips(m)[0].1.audio.as_ref().unwrap().state;
    assert_eq!(state(&m), AudioClipState::Loading);
    let job = |percent| AudioJob {
        kind: AudioJobKind::Loading,
        audio,
        clip: None,
        percent,
    };
    assert_eq!(m.view().audio_work.jobs, [job(0)]);
    // Its peaks are read from memory while they are made.
    let peaks = Growing(Arc::new(std::sync::Mutex::new(PeakBuilder::new(2))));
    m.handle(Input::Audio(AudioNews::Peaks { uid, peaks }));
    assert!(matches!(m.peaks_place(audio), Some(PeaksPlace::Growing(_))));
    m.handle(Input::Audio(AudioNews::Progress { uid, percent: 40 }));
    assert_eq!(m.view().audio_work.jobs, [job(40)]);
    m.handle(Input::Audio(AudioNews::Done { uid, ok: true }));
    assert_eq!(state(&m), AudioClipState::Ready);
    assert_eq!(m.view().audio_work, AudioWorkView::default());
    match m.peaks_place(audio) {
        Some(PeaksPlace::File(path)) => {
            assert!(
                path.starts_with("/cache/meloet/peaks"),
                "{}",
                path.display()
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(m.peaks_place(audio + 100).is_none());
    // A file that did not decode is as good as not there.
    states(&mut m, uid, SourceState::Loading);
    m.handle(Input::Audio(AudioNews::Done { uid, ok: false }));
    assert_eq!(state(&m), AudioClipState::Missing);
}

#[test]
fn a_missing_file_is_said_looked_for_and_taken_out() {
    let (mut m, clip, uid) = one_clip(48_000);
    let audio = clip.audio.as_ref().unwrap().audio;
    states(&mut m, uid, SourceState::Missing);
    assert_eq!(
        clips(&m)[0].1.audio.as_ref().unwrap().state,
        AudioClipState::Missing
    );
    assert_eq!(
        m.view().params.selected,
        Some(SelectedParams::Clip(SelectedClip {
            track: clips(&m)[0].0,
            clip: clip.id,
            missing: Some(MissingAudioView {
                audio,
                file: "歌.mp3".into(),
                was_at: None,
            }),
            tune_usable: false,
        }))
    );
    // 「さがす…」: the PC's window, then the audio thread; the song does not change.
    let steps = undo_count(&m);
    let effects = cmd(&mut m, Command::FindAudio { audio });
    assert!(effects.contains(&Effect::PickAudio { many: false }));
    let effects = m.handle(Input::AudioPicked(vec!["/else/歌.mp3".into()]));
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::FindAudio { source, path } if source.id.0 == audio && path == Path::new("/else/歌.mp3")
    )));
    // Not audio that reads: said, and still missing.
    m.handle(Input::Audio(AudioNews::Found {
        audio,
        file: "別.mp3".into(),
        found: Finding::Unreadable,
    }));
    assert_eq!(
        status(&m),
        Some(StatusMessage::AudioUnreadable {
            file: "別.mp3".into()
        })
    );
    // Another rate or other channels: not copied (spec 6 「さがしたファイルが合わない」).
    m.handle(Input::Audio(AudioNews::Found {
        audio,
        file: "別.mp3".into(),
        found: Finding::Mismatch,
    }));
    assert_eq!(status(&m), Some(StatusMessage::AudioFoundMismatch));
    // Found: the files are looked at again, and the clips that play it are counted.
    let effects = m.handle(Input::Audio(AudioNews::Found {
        audio,
        file: "別.mp3".into(),
        found: Finding::Taken,
    }));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::WatchAudio { .. }))
    );
    assert_eq!(
        status(&m),
        Some(StatusMessage::AudioFound {
            file: "歌.mp3".into(),
            clips: 1
        })
    );
    assert_eq!(undo_count(&m), steps);
    states(&mut m, uid, SourceState::Ready);
    assert_eq!(
        clips(&m)[0].1.audio.as_ref().unwrap().state,
        AudioClipState::Ready
    );
    // A file that is there cannot be looked for, or taken out by the notice.
    assert!(
        !cmd(&mut m, Command::FindAudio { audio }).contains(&Effect::PickAudio { many: false })
    );
    cmd(
        &mut m,
        Command::RemoveMissing {
            missing: MissingRef::Audio { audio },
        },
    );
    assert_eq!(clips(&m).len(), 1);

    // 「曲から外す」: one step, and undo brings the clip back.
    states(&mut m, uid, SourceState::Missing);
    cmd(
        &mut m,
        Command::RemoveMissing {
            missing: MissingRef::Audio { audio },
        },
    );
    assert!(clips(&m).is_empty());
    assert_eq!(undo_count(&m), steps + 1);
    assert_eq!(
        m.history_rows(steps + 1, steps + 1)[0].name,
        StepName::RemoveMissing
    );
    cmd(&mut m, Command::Undo);
    assert_eq!(clips(&m).len(), 1);
}

#[test]
fn the_panel_shows_the_clip_until_an_instrument_is_selected() {
    let (mut m, clip, _) = one_clip(48_000);
    let is_clip = |m: &Model| matches!(m.view().params.selected, Some(SelectedParams::Clip(_)));
    assert!(is_clip(&m));
    let keys = m.view().song.tracks[0].id;
    cmd(&mut m, Command::SelectInstrument { track: keys });
    assert!(!is_clip(&m));
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![clip.id],
            mode: SelectMode::Replace,
        },
    );
    assert!(is_clip(&m));
    cmd(
        &mut m,
        Command::SelectClips {
            clips: Vec::new(),
            mode: SelectMode::Replace,
        },
    );
    assert!(!is_clip(&m));
}

#[test]
fn another_song_is_watched_anew_and_its_missing_files_are_said() {
    let (mut m, _, uid) = one_clip(48_000);
    let song = m.play.song().clone();
    let effects = m.handle(Input::SongOpened);
    assert!(
        effects
            .iter()
            .all(|e| !matches!(e, Effect::WatchAudio { .. }))
    );
    // The song is put in place again, as an opened song is.
    let mut effects = Vec::new();
    m.replace_song(song, false, &mut effects);
    // (A command, as opening a song is: what was said before is gone.)
    let effects = cmd(&mut m, Command::Locate { tick: 0 });
    let serial = (effects.iter())
        .find_map(|e| match e {
            Effect::WatchAudio {
                serial, sources, ..
            } if sources.len() == 1 => Some(*serial),
            _ => None,
        })
        .expect("the song's files are watched");
    assert_eq!(serial, m.song_serial());
    // An answer for the song before is not this song's.
    m.handle(Input::Audio(AudioNews::States {
        serial: serial - 1,
        states: vec![(uid, SourceState::Missing)],
    }));
    assert!(!matches!(
        status(&m),
        Some(StatusMessage::AudioMissing { .. })
    ));
    m.handle(Input::Audio(AudioNews::States {
        serial,
        states: vec![(uid, SourceState::Missing)],
    }));
    assert_eq!(status(&m), Some(StatusMessage::AudioMissing { count: 1 }));
    // The rate of the stream changes: asked again.
    let effects = m.handle(Input::AudioOpened {
        node_name: m.wanted.clone().expect("a stream is open"),
        sample_rate: 44_100,
    });
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::WatchAudio {
            sample_rate: Some(44_100),
            ..
        }
    )));
}

fn stretch(m: &mut Model, clip: u32, change: meloet_protocol::StretchChange) -> Vec<Effect> {
    cmd(m, Command::SetClipStretch { clip, change })
}

#[test]
fn the_rows_of_stretching_and_pitch_are_steps_and_the_tempo_is_found() {
    use meloet_protocol::StretchChange as Row;
    // Five seconds: two bars of four at 96.
    let (mut m, clip, _) = one_clip(48_000);
    let id = clip.id;
    let view = |m: &Model| clips(m)[0].1.clone();
    let rows = |m: &Model| view(m).audio.unwrap().stretch;
    assert_eq!(
        (rows(&m).follow, rows(&m).source_bpm, rows(&m).ratio),
        (false, None, None)
    );
    let steps = undo_count(&m);

    cmd(&mut m, Command::FindSourceTempo { clip: id });
    assert_eq!(
        status(&m),
        Some(StatusMessage::SourceTempoFound {
            name: view(&m).name,
            bpm: 96.0,
            bars: 2,
            same: false,
        })
    );
    assert_eq!(rows(&m).source_bpm, Some(96.0));
    assert_eq!(undo_count(&m), steps + 1);
    let newest = |m: &Model| m.history_rows(undo_count(m), undo_count(m))[0].name.clone();
    assert_eq!(newest(&m), StepName::SetSourceTempo);
    // A new take in the same place; following stays off.
    assert_ne!(view(&m).take_id, clip.take_id);
    assert_eq!((rows(&m).follow, rows(&m).ratio), (false, None));
    assert!(view(&m).audio.unwrap().source_tempo.is_none());
    // Found again: said again, and no step.
    cmd(&mut m, Command::FindSourceTempo { clip: id });
    assert!(matches!(
        status(&m),
        Some(StatusMessage::SourceTempoFound {
            bars: 2,
            same: true,
            ..
        })
    ));
    assert_eq!(undo_count(&m), steps + 1);

    // Following: the song is at 96 as the audio.
    let effects = stretch(&mut m, id, Row::Follow { on: true });
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(crate::state::EngineOp::Clips { .. })))
    );
    assert_eq!((rows(&m).follow, rows(&m).ratio), (true, Some(1.0)));
    assert_eq!(
        view(&m).audio.unwrap().source_tempo.unwrap().tempo_bpm,
        96.0
    );
    assert_eq!(newest(&m), StepName::SetClipFollow);
    // The mark goes with the song's tempo, and with the tempo that is typed.
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    assert_eq!(rows(&m).ratio, Some(1.25));
    stretch(&mut m, id, Row::SourceBpm { bpm: 48.004 });
    assert_eq!(
        (rows(&m).source_bpm, rows(&m).ratio),
        (Some(48.0), Some(2.5))
    );
    stretch(&mut m, id, Row::SourceBpm { bpm: 1_000.0 });
    assert_eq!(rows(&m).source_bpm, Some(300.0));
    stretch(&mut m, id, Row::SourceBpm { bpm: 0.0 });
    assert_eq!(rows(&m).source_bpm, Some(30.0));

    // The pitch: the nearest whole number inside its range.
    let steps = undo_count(&m);
    stretch(&mut m, id, Row::Semitones { semitones: 2.4 });
    stretch(&mut m, id, Row::Cents { cents: -80.0 });
    stretch(&mut m, id, Row::KeepFormant { on: false });
    let now = rows(&m);
    assert_eq!(
        (now.semitones, now.cents, now.keep_formant),
        (2, -50, false)
    );
    assert_eq!(undo_count(&m), steps + 3);
    assert_eq!(newest(&m), StepName::SetClipPitch);
    // The same value, what is not a number, and a clip that is not there: nothing.
    stretch(&mut m, id, Row::Semitones { semitones: 2.0 });
    stretch(
        &mut m,
        id,
        Row::Semitones {
            semitones: f64::NAN,
        },
    );
    stretch(&mut m, id, Row::Cents { cents: f64::NAN });
    stretch(&mut m, id, Row::SourceBpm { bpm: f64::NAN });
    stretch(&mut m, id + 1_000, Row::Follow { on: false });
    cmd(&mut m, Command::FindSourceTempo { clip: id + 1_000 });
    assert_eq!(undo_count(&m), steps + 3);
    stretch(&mut m, id, Row::Semitones { semitones: -99.0 });
    assert_eq!(rows(&m).semitones, -12);
    // Each of them comes back with undo.
    cmd(&mut m, Command::Undo);
    cmd(&mut m, Command::Undo);
    assert!(rows(&m).keep_formant);
    assert_eq!((rows(&m).semitones, rows(&m).cents), (2, -50));

    // While a recording counts in or runs, nothing changes.
    cmd(&mut m, Command::Record);
    assert_ne!(
        m.view().transport.state,
        meloet_protocol::TransportState::Stopped
    );
    let before = rows(&m);
    stretch(&mut m, id, Row::Follow { on: false });
    stretch(&mut m, id, Row::Semitones { semitones: 5.0 });
    cmd(&mut m, Command::FindSourceTempo { clip: id });
    assert_eq!(rows(&m), before);
}

#[test]
fn tuning_is_a_step_with_the_songs_scale_of_that_moment() {
    use meloet_protocol::{ScaleMode, StretchChange as Row, TuneView};
    let (mut m, clip, _) = one_clip(48_000);
    let id = clip.id;
    let tune = |m: &Model| clips(m)[0].1.audio.as_ref().unwrap().stretch.tune;
    let newest = |m: &Model| m.history_rows(undo_count(m), undo_count(m))[0].name.clone();
    let scale = |m: &mut Model, root, mode| {
        cmd(
            m,
            Command::SetMusicalScale {
                root,
                mode,
                lock: false,
                highlight: true,
            },
        );
    };
    assert_eq!(tune(&m), None);
    assert!(matches!(
        m.view().params.selected,
        Some(SelectedParams::Clip(SelectedClip {
            tune_usable: true,
            ..
        }))
    ));
    let steps = undo_count(&m);
    // At strength 0 neither the strength nor the scale is a change.
    stretch(&mut m, id, Row::TuneStrength { percent: 0.0 });
    stretch(&mut m, id, Row::TuneScale);
    assert_eq!((tune(&m), undo_count(&m)), (None, steps));

    // Leaving 0 the clip takes the song's scale; the audio is asked for again.
    scale(&mut m, 9, ScaleMode::Minor);
    let effects = stretch(&mut m, id, Row::TuneStrength { percent: 59.6 });
    let a_minor = TuneView {
        root: 9,
        mode: ScaleMode::Minor,
        strength: 60,
    };
    assert_eq!(tune(&m), Some(a_minor));
    assert_eq!(
        (undo_count(&m), newest(&m)),
        (steps + 1, StepName::SetClipTune)
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(crate::state::EngineOp::Clips { .. })))
    );
    // The song's scale changes: the clip keeps its own, also with another strength.
    scale(&mut m, 2, ScaleMode::Dorian);
    assert_eq!(tune(&m), Some(a_minor));
    stretch(&mut m, id, Row::TuneStrength { percent: 250.0 });
    assert_eq!(
        tune(&m),
        Some(TuneView {
            strength: 100,
            ..a_minor
        })
    );
    assert_eq!(undo_count(&m), steps + 2);
    // 「いまのスケールにする」: one step, once.
    stretch(&mut m, id, Row::TuneScale);
    let d_dorian = TuneView {
        root: 2,
        mode: ScaleMode::Dorian,
        strength: 100,
    };
    assert_eq!(tune(&m), Some(d_dorian));
    assert_eq!(
        (undo_count(&m), newest(&m)),
        (steps + 3, StepName::SetClipTune)
    );
    stretch(&mut m, id, Row::TuneScale);
    // The same value, what is not a number and a clip that is not there: nothing.
    stretch(&mut m, id, Row::TuneStrength { percent: 100.0 });
    stretch(&mut m, id, Row::TuneStrength { percent: f64::NAN });
    stretch(&mut m, id + 1_000, Row::TuneStrength { percent: 30.0 });
    stretch(&mut m, id + 1_000, Row::TuneScale);
    assert_eq!(undo_count(&m), steps + 3);
    // Undo and redo, step by step.
    cmd(&mut m, Command::Undo);
    assert_eq!(tune(&m).map(|t| (t.root, t.strength)), Some((9, 100)));
    cmd(&mut m, Command::Undo);
    assert_eq!(tune(&m), Some(a_minor));
    cmd(&mut m, Command::Undo);
    assert_eq!(tune(&m), None);
    cmd(&mut m, Command::Redo);
    cmd(&mut m, Command::Redo);
    cmd(&mut m, Command::Redo);
    assert_eq!(tune(&m), Some(d_dorian));
    // To 0 the scale goes with it.
    stretch(&mut m, id, Row::TuneStrength { percent: -3.0 });
    assert_eq!((tune(&m), undo_count(&m)), (None, steps + 4));
    cmd(&mut m, Command::Undo);

    // With several clips it does nothing.
    cmd(
        &mut m,
        Command::SetClipsStretch {
            clips: vec![id],
            change: Row::TuneStrength { percent: 10.0 },
        },
    );
    assert_eq!(tune(&m), Some(d_dorian));

    // While a recording counts in or runs, nothing changes.
    cmd(&mut m, Command::Record);
    assert_ne!(
        m.view().transport.state,
        meloet_protocol::TransportState::Stopped
    );
    stretch(&mut m, id, Row::TuneStrength { percent: 20.0 });
    stretch(&mut m, id, Row::TuneStrength { percent: 0.0 });
    scale(&mut m, 0, ScaleMode::Major);
    stretch(&mut m, id, Row::TuneScale);
    assert_eq!(tune(&m), Some(d_dorian));
}

#[test]
fn audio_of_no_whole_bars_has_no_tempo_to_find() {
    let mut m = model();
    let vocal = vocal_track(&mut m);
    // 100 seconds: more than eight bars at 60.
    drop_files(
        &mut m,
        &["長い.wav"],
        Some(vocal),
        0,
        DropMode::Separate,
        vec![brought("長い.wav", 100, 48_000)],
    );
    let clip = clips(&m)[0].1.clone();
    let steps = undo_count(&m);
    cmd(&mut m, Command::FindSourceTempo { clip: clip.id });
    assert_eq!(
        status(&m),
        Some(StatusMessage::SourceTempoNotFound { name: clip.name })
    );
    assert_eq!(undo_count(&m), steps);
    assert_eq!(
        clips(&m)[0].1.audio.as_ref().unwrap().stretch.source_bpm,
        None
    );
}

#[test]
fn the_clips_whose_audio_is_being_made_are_shown_with_their_percent() {
    use crate::maker::MakerJob;
    let (mut m, clip, _) = one_clip(48_000);
    let audio = clip.audio.as_ref().unwrap().audio;
    let job = |clip: u32, percent| MakerJob {
        clip,
        audio,
        percent,
    };
    // A clip that is gone since the maker said so is left out.
    m.handle(Input::AudioMaking(vec![
        job(clip.id, 34),
        job(clip.id + 500, 0),
    ]));
    assert_eq!(
        m.view().audio_work.jobs,
        [AudioJob {
            kind: AudioJobKind::Rendering,
            audio,
            clip: Some(clip.id),
            percent: 34
        }]
    );
    m.handle(Input::AudioMaking(Vec::new()));
    assert_eq!(m.view().audio_work, AudioWorkView::default());
}

#[test]
fn two_or_more_vocal_clips_change_together_as_one_step() {
    use meloet_protocol::{SelectedClips, StretchChange as Row};
    let mut m = model();
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
    let keys_clip = clips(&m)[0].1.id;
    let vocal = vocal_track(&mut m);
    drop_files(
        &mut m,
        &["a.wav", "b.wav"],
        Some(vocal),
        0,
        DropMode::Separate,
        vec![brought("a.wav", 2, 48_000), brought("b.wav", 2, 48_000)],
    );
    let all = clips(&m);
    let (a, b) = (all[1].1.id, all[2].1.id);
    let tracks = vec![all[1].0, all[2].0];
    // Brought in together: both are selected, and the panel is 「まとめて変える」.
    assert_eq!(m.view().arrange.selected_clips, [a, b]);
    let panel = |m: &Model| m.view().params.clips;
    assert_eq!(m.view().params.selected, None);
    assert_eq!(
        panel(&m),
        Some(SelectedClips {
            clips: vec![a, b],
            tracks: tracks.clone(),
            gain_db: Some(0.0),
            follow: Some(false),
            semitones: Some(0),
            cents: Some(0),
            keep_formant: Some(true),
        })
    );
    let gain = |m: &Model, clip: u32| {
        let all = clips(m);
        let c = all.iter().find(|(_, c)| c.id == clip).unwrap();
        c.1.audio.as_ref().unwrap().gain_db
    };
    // One clip's own volume: the panel says 「—」.
    cmd(
        &mut m,
        Command::SetClipField {
            clip: b,
            field: ClipField::GainDb,
            value: 10.0,
        },
    );
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![a, b],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(panel(&m).unwrap().gain_db, None);
    let steps = undo_count(&m);

    // The line dragged by +3 dB: both move, each stops at +12.
    cmd(
        &mut m,
        Command::ShiftClipGains {
            clips: vec![a, b],
            by_db: 3.0,
        },
    );
    assert_eq!((gain(&m, a), gain(&m, b)), (3.0, 12.0));
    assert_eq!(undo_count(&m), steps + 1);
    let row = m.history_rows(steps + 1, steps + 1)[0].clone();
    assert_eq!(row.name, StepName::SetClipGain);
    assert_eq!(row.what, meloet_protocol::StepWhat::Clips { count: 2 });
    cmd(&mut m, Command::Undo);
    assert_eq!((gain(&m, a), gain(&m, b)), (0.0, 10.0));
    // The selection is back with the undo.
    assert_eq!(m.view().arrange.selected_clips, [a, b]);

    // 0, a value that is not a number and ids of no clip do nothing.
    for (clips, by_db) in [(vec![a, b], 0.0), (vec![a, b], f64::NAN), (vec![999], 2.0)] {
        cmd(&mut m, Command::ShiftClipGains { clips, by_db });
        assert_eq!(undo_count(&m), steps);
    }
    // Far down: each stops at −60.
    cmd(
        &mut m,
        Command::ShiftClipGains {
            clips: vec![a, b, 999],
            by_db: -100.0,
        },
    );
    assert_eq!((gain(&m, a), gain(&m, b)), (-60.0, -60.0));
    assert_eq!(panel(&m).unwrap().gain_db, Some(-60.0));
    cmd(&mut m, Command::Undo);

    // A value typed: both get it, as one step; typed again it is no step.
    cmd(
        &mut m,
        Command::SetClipGains {
            clips: vec![a, b],
            db: -3.5,
        },
    );
    assert_eq!((gain(&m, a), gain(&m, b)), (-3.5, -3.5));
    assert_eq!(panel(&m).unwrap().gain_db, Some(-3.5));
    assert_eq!(undo_count(&m), steps + 1);
    cmd(
        &mut m,
        Command::SetClipGains {
            clips: vec![a, b],
            db: -3.5,
        },
    );
    assert_eq!(undo_count(&m), steps + 1);

    // The rows of stretching and pitch.
    let rows = |m: &Model, clip: u32| {
        let all = clips(m);
        let c = all.iter().find(|(_, c)| c.id == clip).unwrap();
        c.1.audio.as_ref().unwrap().stretch
    };
    stretch(&mut m, a, Row::Semitones { semitones: 2.0 });
    assert_eq!(panel(&m).unwrap().semitones, None);
    let steps = undo_count(&m);
    let many = |m: &mut Model, change| {
        cmd(
            m,
            Command::SetClipsStretch {
                clips: vec![a, b],
                change,
            },
        )
    };
    many(&mut m, Row::Semitones { semitones: 20.0 });
    assert_eq!((rows(&m, a).semitones, rows(&m, b).semitones), (12, 12));
    many(&mut m, Row::Cents { cents: -30.4 });
    many(&mut m, Row::Follow { on: true });
    many(&mut m, Row::KeepFormant { on: false });
    let p = panel(&m).unwrap();
    assert_eq!(
        (p.semitones, p.cents, p.follow, p.keep_formant),
        (Some(12), Some(-30), Some(true), Some(false))
    );
    assert_eq!(undo_count(&m), steps + 4);
    let names: Vec<StepName> = (m.history_rows(steps + 1, steps + 4).into_iter())
        .map(|r| r.name)
        .collect();
    assert_eq!(
        names,
        [
            StepName::SetClipPitch,
            StepName::SetClipPitch,
            StepName::SetClipFollow,
            StepName::SetClipPitch
        ]
    );
    // What every clip has already, and the tempo of the audio, are no step.
    many(&mut m, Row::Follow { on: true });
    many(&mut m, Row::SourceBpm { bpm: 100.0 });
    assert_eq!(undo_count(&m), steps + 4);

    // An instrument selected afterwards takes the panel; the clips selected again bring it
    // back.
    let keys = m.view().song.tracks[0].id;
    cmd(&mut m, Command::SelectInstrument { track: keys });
    assert_eq!(panel(&m), None);
    assert!(m.view().params.selected.is_some());
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![a, b],
            mode: SelectMode::Replace,
        },
    );
    assert!(panel(&m).is_some());
    // A keys clip among them: not this panel (B-95-8); one clip: the panel of one clip.
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![a, b, keys_clip],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(panel(&m), None);
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![a],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(panel(&m), None);
    assert!(matches!(
        m.view().params.selected,
        Some(SelectedParams::Clip(_))
    ));
}

#[test]
fn audio_the_disk_did_not_give_in_time_is_said_once_until_the_song_stops() {
    let mut m = model();
    // Stopped: nothing to say.
    m.handle(Input::DiskTooSlow);
    assert_ne!(status(&m), Some(StatusMessage::DiskTooSlow));
    cmd(&mut m, Command::PlayStop);
    assert!(m.view().transport.state != meloet_protocol::TransportState::Stopped);
    m.handle(Input::DiskTooSlow);
    assert_eq!(status(&m), Some(StatusMessage::DiskTooSlow));
    // Put away by the next thing the person does, and not said again while it plays.
    cmd(
        &mut m,
        Command::SelectClips {
            clips: Vec::new(),
            mode: SelectMode::Replace,
        },
    );
    m.handle(Input::DiskTooSlow);
    assert_ne!(status(&m), Some(StatusMessage::DiskTooSlow));
    // Stopped and played again: said again.
    cmd(&mut m, Command::PlayStop);
    cmd(&mut m, Command::PlayStop);
    m.handle(Input::DiskTooSlow);
    assert_eq!(status(&m), Some(StatusMessage::DiskTooSlow));
}

/// Round 12 A2: a section with an audio clip over its end is copied; the engine is told the clips of the seat again,
/// and every clip plays the audio the one clip did.
#[test]
fn a_copied_section_cuts_an_audio_clip_and_tells_the_engine_its_clips() {
    let (mut m, clip, _) = one_clip(48_000);
    assert_eq!((clip.start, clip.end), (0, 2 * BAR));
    let mark = |m: &mut Model, tick: u64, name: &str| {
        cmd(
            m,
            Command::AddMarker {
                name: name.into(),
                tick: Some(tick),
            },
        );
        let id = m.view().timeline.renaming.unwrap();
        cmd(m, Command::CloseMarkerName);
        id
    };
    let intro = mark(&mut m, 0, "イントロ");
    mark(&mut m, BAR, "A メロ");
    let steps = undo_count(&m);
    let effects = cmd(&mut m, Command::DuplicateSection { marker: intro });
    assert_eq!(undo_count(&m), steps + 1);
    let all: Vec<ClipView> = clips(&m).into_iter().map(|(_, c)| c).collect();
    assert_eq!(
        all.iter().map(|c| (c.start, c.end)).collect::<Vec<_>>(),
        [(0, BAR), (BAR, 2 * BAR), (2 * BAR, 3 * BAR)]
    );
    assert_eq!(all[0].id, clip.id);
    assert!(all.iter().all(|c| c.take_id == clip.take_id));
    // The copy plays what bar 1 played; the later half plays on from where the cut is.
    assert_eq!(
        all.iter().map(|c| c.take_offset).collect::<Vec<_>>(),
        [0, 0, -(BAR as i64)]
    );
    let sent: Vec<usize> = (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(crate::state::EngineOp::Clips { clips, .. }) => Some(clips.len()),
            _ => None,
        })
        .collect();
    assert_eq!(sent, [3]);
    assert!(
        !(effects.iter()).any(|e| matches!(
            e,
            Effect::Engine(crate::state::EngineOp::AudioSources { .. })
        )),
        "the audio table is the same"
    );
    cmd(&mut m, Command::Undo);
    let back: Vec<ClipView> = clips(&m).into_iter().map(|(_, c)| c).collect();
    assert_eq!(back, [clip]);
}
