use std::fs;
use std::path::{Path, PathBuf};

use meloet_model::{Song, StepKind};
use meloet_protocol::{
    Command, Hint, RecoveryCause, RecoveryView, SongAnswer, SongDialogKind, SongFile, StatusMessage,
};

use super::QUIET_US;
use crate::play::tests::{
    cmd, event, new_song_without_pack, on, ready, record_once, report, started, status,
};
use crate::play::{Heard, RecordReport};
use crate::recovery;
use crate::service::{recovery_start, save_song};
use crate::settings::temp_dir;
use crate::songfile;
use crate::state::{Effect, Input, Model};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn at(m: &mut Model, t_us: u64, input: Input) -> Vec<Effect> {
    m.set_now(t_us, None);
    m.handle(input)
}

fn tick(m: &mut Model, t_us: u64) -> Vec<Effect> {
    at(m, t_us, Input::Tick)
}

/// The recovery files asked for: (tempo of the song, cause, ending).
fn writes(effects: &[Effect]) -> Vec<(f64, RecoveryCause, bool)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::WriteRecovery {
                song,
                cause,
                ending,
                ..
            } => Some((song.doc.timeline.tempo.bpm(), *cause, *ending)),
            _ => None,
        })
        .collect()
}

fn removes(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::RemoveRecovery))
        .count()
}

fn song(m: &Model) -> Song {
    m.play.song().clone()
}

fn file(m: &Model) -> SongFile {
    m.view().song.file
}

/// The song as saved; "戻さない" keeps the ids given out after it.
fn assert_saved(m: &Model, saved: &Song) {
    let now = song(m).doc;
    assert_eq!(
        meloet_model::Doc {
            next_id: saved.doc.next_id,
            ..now
        },
        saved.doc
    );
}

fn recovery_view(m: &Model) -> Option<RecoveryView> {
    m.view().workspace.recovery
}

/// A button of the song dialog up now.
fn answer(m: &mut Model, answer: SongAnswer) -> Vec<Effect> {
    let dialog = m.view().workspace.song_dialog.expect("a song dialog").id;
    cmd(m, Command::AnswerSongDialog { dialog, answer })
}

/// Writes and removes the recovery file in `dir` as the writer thread does.
fn on_disk(dir: &Path, effects: &[Effect]) {
    for effect in effects {
        match effect {
            Effect::WriteRecovery {
                song, path, cause, ..
            } => recovery::write(dir, &recovery::encode(song, path.as_deref(), *cause, 1)).unwrap(),
            Effect::RemoveRecovery => recovery::remove(dir).unwrap(),
            _ => {}
        }
    }
}

/// Saves the way the core thread does; returns the effects that came after.
fn save(m: &mut Model, songs: &Path) -> Vec<Effect> {
    let mut effects = cmd(m, Command::SaveSong);
    let mut after = Vec::new();
    while let Some(i) = effects
        .iter()
        .position(|e| matches!(e, Effect::SaveSong { .. }))
    {
        let Effect::SaveSong { song, to } = effects.remove(i) else {
            unreachable!()
        };
        let store = crate::audiofiles::AudioStore::new(None);
        let answer = save_song(Some(songs), &song, to, &store, &mut |_| {});
        let more = m.handle(answer);
        after.extend(more.iter().cloned());
        effects.extend(more);
    }
    after
}

#[test]
fn a_change_is_written_when_the_hands_rest_for_two_seconds() {
    let m = &mut ready();
    assert!(writes(&tick(m, 10_000_000)).is_empty(), "nothing changed");

    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    assert!(m.recovery_waiting());
    assert!(writes(&tick(m, 1_000_000 + QUIET_US - 1)).is_empty());
    // Another change starts the two seconds again.
    at(
        m,
        2_000_000,
        Input::Command(Command::SetTempo { bpm: 110.0 }),
    );
    assert!(writes(&tick(m, 1_000_000 + QUIET_US)).is_empty());
    assert_eq!(
        writes(&tick(m, 2_000_000 + QUIET_US)),
        [(110.0, RecoveryCause::Sudden, false)]
    );
    assert!(!m.recovery_waiting());
    assert!(writes(&tick(m, 9_000_000)).is_empty(), "written once");

    // A change that brings the song back to its file leaves nothing unsaved to write.
    at(m, 10_000_000, Input::Command(Command::Undo));
    at(m, 10_000_001, Input::Command(Command::Undo));
    assert_eq!(file(m), SongFile::NeverSaved);
    assert!(writes(&tick(m, 20_000_000)).is_empty());
}

#[test]
fn nothing_is_written_while_recording_and_the_take_is_written_after() {
    let m = &mut ready();
    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    cmd(m, Command::SetCountIn { bars: 0 });
    // The change still waits when the recording starts: it is written at once, so that a
    // start after meloet ended in the middle finds the song the voice was recorded over.
    let record = at(m, 1_500_000, Input::Command(Command::Record));
    assert_eq!(writes(&record), [(100.0, RecoveryCause::Sudden, false)]);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    assert!(m.play.busy());
    assert!(writes(&tick(m, 9_000_000)).is_empty(), "recording");
    m.set_now(
        9_100_000,
        Some(Heard {
            tick: 400.0,
            state: meloet_protocol::TransportState::Recording,
            lap: 0,
        }),
    );
    cmd(m, Command::Record);
    let stopped = report(
        m,
        RecordReport::Stopped {
            t_us: 9_100_000,
            tick: 400.0,
            lap: 0,
        },
    );
    assert!(writes(&stopped).is_empty());
    assert_eq!(m.view().song.tracks[0].clips.len(), 1);
    assert!(writes(&tick(m, 9_100_000 + QUIET_US - 1)).is_empty());
    let written = tick(m, 9_100_000 + QUIET_US);
    let Some(Effect::WriteRecovery { song, .. }) = written
        .iter()
        .find(|e| matches!(e, Effect::WriteRecovery { .. }))
    else {
        panic!("the take is written");
    };
    assert_eq!(song.doc.tracks[0].clips.len(), 1);
}

#[test]
fn every_recording_over_a_waiting_change_writes_it_first() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    at(m, 1_000_000, Input::Command(Command::Record));
    started(m);
    m.set_now(1_100_000, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 1_100_000,
            tick: 400.0,
            lap: 0,
        },
    );
    assert!(!m.play.busy());
    at(
        m,
        2_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    let record = at(m, 2_500_000, Input::Command(Command::Record));
    assert_eq!(writes(&record), [(100.0, RecoveryCause::Sudden, false)]);
}

#[test]
fn saving_removes_the_file_and_so_does_closing_without_saving() {
    let songs = temp_dir("recovery-save");
    let m = &mut ready();
    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    assert_eq!(writes(&tick(m, 4_000_000)).len(), 1);
    assert_eq!(removes(&save(m, &songs)), 1);
    assert!(
        writes(&tick(m, 9_000_000)).is_empty(),
        "saved: nothing to write"
    );

    at(
        m,
        10_000_000,
        Input::Command(Command::SetTempo { bpm: 120.0 }),
    );
    let closing = at(m, 10_000_001, Input::CloseRequested);
    assert_eq!(removes(&closing), 0, "the dialog asks first");
    let dialog = m.view().workspace.song_dialog.unwrap().id;
    let effects = cmd(
        m,
        Command::AnswerSongDialog {
            dialog,
            answer: SongAnswer::DontSave,
        },
    );
    assert_eq!(removes(&effects), 1);
    assert!(effects.contains(&Effect::Exit));
    assert!(
        writes(&tick(m, 20_000_000)).is_empty(),
        "nothing more once meloet ends"
    );
    let _ = fs::remove_dir_all(songs);
}

#[test]
fn a_new_song_lets_the_file_go() {
    let m = &mut ready();
    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    cmd(m, Command::NewSong);
    let dialog = m.view().workspace.song_dialog.unwrap().id;
    let effects = cmd(
        m,
        Command::AnswerSongDialog {
            dialog,
            answer: SongAnswer::DontSave,
        },
    );
    assert_eq!(removes(&effects), 1);
    assert!(writes(&tick(m, 9_000_000)).is_empty());
}

#[test]
fn the_pc_ending_writes_at_once_or_says_there_is_nothing() {
    let m = &mut ready();
    assert_eq!(at(m, 1, Input::PcEnding), [Effect::EndingReady]);

    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    let effects = at(m, 1_000_001, Input::PcEnding);
    assert_eq!(writes(&effects), [(100.0, RecoveryCause::PcEnding, true)]);
    assert!(
        writes(&tick(m, 9_000_000)).is_empty(),
        "the change waits no more"
    );
    // Asked again (logind first, then the signal): the file holds it already.
    assert_eq!(at(m, 9_000_001, Input::PcEnding), [Effect::EndingReady]);
}

#[test]
fn the_pc_ending_writes_over_a_file_written_every_now_and_then() {
    let m = &mut ready();
    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    assert_eq!(
        writes(&tick(m, 4_000_000)),
        [(100.0, RecoveryCause::Sudden, false)]
    );
    assert_eq!(
        writes(&at(m, 5_000_000, Input::PcEnding)),
        [(100.0, RecoveryCause::PcEnding, true)]
    );
}

#[test]
fn the_pc_ending_stops_a_recording_and_writes_the_take() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    event(m, on(60), 10, 100.0, 0);
    m.set_now(
        200,
        Some(Heard {
            tick: 400.0,
            state: meloet_protocol::TransportState::Recording,
            lap: 0,
        }),
    );
    let asked = m.handle(Input::PcEnding);
    assert!(writes(&asked).is_empty());
    assert!(!asked.contains(&Effect::EndingReady), "waits for the take");
    let effects = report(
        m,
        RecordReport::Stopped {
            t_us: 200,
            tick: 400.0,
            lap: 0,
        },
    );
    let Some(Effect::WriteRecovery { song, ending, .. }) = effects
        .iter()
        .find(|e| matches!(e, Effect::WriteRecovery { .. }))
    else {
        panic!("written once the take is in: {effects:?}");
    };
    assert!(ending);
    assert_eq!(song.doc.tracks[0].clips.len(), 1);
}

/// A song with a take, saved, then changed (tempo), and the recovery file written as the PC
/// ends. Returns the saved song, the changed song and the song's path.
fn saved_then_changed(dir: &Path, songs: &Path) -> (Song, Song, PathBuf) {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], BAR as f64);
    save(m, songs);
    let saved = song(m);
    at(
        m,
        1_000_000,
        Input::Command(Command::SetTempo { bpm: 140.0 }),
    );
    record_once(m, &[(62, 0.0, 100.0)], BAR as f64);
    let effects = at(m, 2_000_000, Input::PcEnding);
    let Some(Effect::WriteRecovery {
        song, path, cause, ..
    }) = effects
        .into_iter()
        .find(|e| matches!(e, Effect::WriteRecovery { .. }))
    else {
        panic!("written");
    };
    recovery::write(
        dir,
        &recovery::encode(&song, path.as_deref(), cause, 1_794_750_060_000),
    )
    .unwrap();
    (saved, *song, path.unwrap())
}

fn start_from(dir: &Path) -> (Model, Vec<Effect>) {
    let mut m = ready();
    let start = recovery_start(dir, meloet_model::Language::Ja).expect("a recovery file");
    let effects = m.handle(Input::RecoveryFound(Box::new(start)));
    (m, effects)
}

#[test]
fn the_start_brings_the_song_back_unsaved_with_the_hint() {
    let (dir, songs) = (temp_dir("recovery-start"), temp_dir("recovery-start-songs"));
    let (_, changed, path) = saved_then_changed(&dir, &songs);
    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    assert_eq!(song(m), changed);
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(file(m), SongFile::Changed);
    assert_eq!(
        recovery_view(m),
        Some(RecoveryView::Recovered {
            vocals: None,
            written_ms: 1_794_750_060_000,
            cause: RecoveryCause::PcEnding,
            song: Some("はじめての曲".into()),
            takes: 2,
        })
    );
    assert_eq!(m.view().workspace.hint, None);
    assert_eq!(status(m), Some(StatusMessage::Recovered));
    // Bringing it back is the first step, over the song as saved (mock F-Recover1: "1 / 1").
    assert_eq!(m.play.steps(), [StepKind::Recovered]);
    assert_eq!((m.view().song.undo_count, m.view().song.redo_count), (1, 0));
    assert!(dir.join(recovery::FILE_NAME).exists(), "kept until saved");

    // Playing and recording leave the hint up.
    record_once(m, &[(64, 0.0, 100.0)], BAR as f64);
    cmd(m, Command::DismissHint);
    assert!(matches!(
        recovery_view(m),
        Some(RecoveryView::Recovered { .. })
    ));

    // Saving puts it away, and the song goes where it came from.
    assert_eq!(removes(&save(m, &songs)), 1);
    assert_eq!(recovery_view(m), None);
    assert_eq!(songfile::read(&path).unwrap(), song(m));
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn dont_restore_is_the_second_step_and_undo_goes_back_through_both() {
    let (dir, songs) = (
        temp_dir("recovery-discard"),
        temp_dir("recovery-discard-songs"),
    );
    let (saved, changed, path) = saved_then_changed(&dir, &songs);
    let (mut m, _) = start_from(&dir);
    let m = &mut m;

    let effects = cmd(m, Command::DiscardRecovered);
    assert_eq!(removes(&effects), 0, "asked first");
    let effects = answer(m, SongAnswer::Keep);
    assert_eq!(
        removes(&effects),
        0,
        "「残す」: the file stays until the next save"
    );
    assert_saved(m, &saved);
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(recovery_view(m), None);
    assert_eq!(status(m), Some(StatusMessage::RecoveryDiscarded));
    assert_eq!(
        m.play.steps(),
        [StepKind::Recovered, StepKind::DiscardRecovered],
        "mock F-Recover2: 2 / 2"
    );
    assert_eq!(m.view().song.undo_count, 2);
    // The saved song's ids are below the ones given out in the song brought back.
    assert_eq!(song(m).doc.next_id, changed.doc.next_id);
    assert!(
        writes(&tick(m, 60_000_000)).is_empty(),
        "nothing unsaved to write"
    );

    cmd(m, Command::Undo);
    assert_eq!(song(m), changed);
    assert_eq!(file(m), SongFile::Changed);
    cmd(m, Command::Redo);
    assert_saved(m, &saved);
    assert_eq!(file(m), SongFile::Saved);
    cmd(m, Command::Undo);
    assert_eq!(song(m), changed);
    // Once more: the song as saved, before the recovered step, which matches the file.
    cmd(m, Command::Undo);
    assert_saved(m, &saved);
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(m.view().song.undo_count, 0);
    assert!(
        writes(&tick(m, 120_000_000)).is_empty(),
        "nothing unsaved to write"
    );
    // And forward through both again.
    cmd(m, Command::Redo);
    assert_eq!(song(m), changed);
    assert_eq!(file(m), SongFile::Changed);
    cmd(m, Command::Redo);
    assert_saved(m, &saved);
    assert_eq!(file(m), SongFile::Saved);
    cmd(m, Command::Undo);
    assert_eq!(song(m), changed);
    assert_eq!(file(m), SongFile::Changed);

    // Saved again: the file has the song brought back.
    save(m, &songs);
    assert_eq!(songfile::read(&path).unwrap(), changed);
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn dont_restore_asks_whether_the_file_goes_too() {
    let (dir, songs) = (
        temp_dir("recovery-delete"),
        temp_dir("recovery-delete-songs"),
    );
    let (saved, changed, _) = saved_then_changed(&dir, &songs);
    let (mut m, _) = start_from(&dir);
    let m = &mut m;

    let effects = at(m, 1_000_000, Input::Command(Command::DiscardRecovered));
    assert_eq!(removes(&effects), 0);
    let d = m.view().workspace.song_dialog.expect("asked");
    assert_eq!(
        (d.kind, d.title.as_str()),
        (SongDialogKind::DeleteRecovery, "はじめての曲")
    );
    assert_eq!(m.view().song.undo_count, 2, "the question is not a step");
    // A button this dialog does not have does nothing.
    answer(m, SongAnswer::Save);
    assert_eq!(m.view().workspace.song_dialog, Some(d.clone()));

    let effects = at(
        m,
        1_000_001,
        Input::Command(Command::AnswerSongDialog {
            dialog: d.id,
            answer: SongAnswer::Delete,
        }),
    );
    assert_eq!(removes(&effects), 1);
    on_disk(&dir, &effects);
    assert!(!dir.join(recovery::FILE_NAME).exists());
    assert_eq!(m.view().workspace.song_dialog, None);
    assert_eq!(status(m), Some(StatusMessage::RecoveryDiscarded));
    assert_eq!(m.view().song.undo_count, 2);
    assert_saved(m, &saved);
    on_disk(&dir, &tick(m, 60_000_000));
    assert!(
        recovery_start(&dir, meloet_model::Language::Ja).is_none(),
        "a start after a crash finds nothing"
    );

    // Undo still brings the changes back, unsaved, and they are written again.
    at(m, 70_000_000, Input::Command(Command::Undo));
    assert_eq!(song(m), changed);
    assert_eq!(file(m), SongFile::Changed);
    assert!(writes(&tick(m, 70_000_000 + QUIET_US - 1)).is_empty());
    let effects = tick(m, 70_000_000 + QUIET_US);
    assert_eq!(writes(&effects), [(140.0, RecoveryCause::Sudden, false)]);
    on_disk(&dir, &effects);
    let (again, _) = start_from(&dir);
    assert_eq!(song(&again), changed);
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn esc_on_the_question_keeps_the_file() {
    let (dir, songs) = (temp_dir("recovery-keep"), temp_dir("recovery-keep-songs"));
    saved_then_changed(&dir, &songs);
    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    cmd(m, Command::DiscardRecovered);
    let effects = cmd(m, Command::Back);
    assert_eq!(removes(&effects), 0);
    assert_eq!(m.view().workspace.song_dialog, None);
    assert_eq!(status(m), Some(StatusMessage::RecoveryDiscarded));
    assert!(dir.join(recovery::FILE_NAME).exists());
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn the_serial_goes_up_on_coming_back_on_dont_restore_and_on_undo_over_them() {
    let (dir, songs) = (
        temp_dir("recovery-serial"),
        temp_dir("recovery-serial-songs"),
    );
    saved_then_changed(&dir, &songs);
    let first = ready().view().song.serial;
    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    let serial = |m: &Model| m.view().song.serial;
    assert_eq!(serial(m), first + 1);
    cmd(m, Command::DiscardRecovered);
    assert_eq!(serial(m), first + 2);
    // Undo and redo over these two steps go between the recovered and the saved song.
    cmd(m, Command::Undo);
    assert_eq!(serial(m), first + 3);
    cmd(m, Command::Undo);
    assert_eq!(serial(m), first + 4);
    cmd(m, Command::Redo);
    assert_eq!(serial(m), first + 5);
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn a_new_song_never_saved_comes_back_and_dont_restore_empties_it() {
    let dir = temp_dir("recovery-new");
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], BAR as f64);
    let effects = at(m, 1_000_000, Input::PcEnding);
    let Some(Effect::WriteRecovery {
        song: written,
        path: None,
        ..
    }) = effects
        .into_iter()
        .find(|e| matches!(e, Effect::WriteRecovery { .. }))
    else {
        panic!("written without a path");
    };
    recovery::write(
        &dir,
        &recovery::encode(&written, None, RecoveryCause::Sudden, 7),
    )
    .unwrap();

    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    assert_eq!(song(m), *written);
    assert_eq!(m.song_path(), None);
    assert_eq!(file(m), SongFile::Changed);
    assert_eq!(
        recovery_view(m),
        Some(RecoveryView::Recovered {
            vocals: None,
            written_ms: 7,
            cause: RecoveryCause::Sudden,
            song: None,
            takes: 1,
        })
    );
    cmd(m, Command::DiscardRecovered);
    assert_eq!(song(m).doc.tracks, new_song_without_pack().doc.tracks);
    assert_eq!(file(m), SongFile::NeverSaved);
    cmd(m, Command::Undo);
    assert_eq!(song(m), *written);
    assert_eq!(file(m), SongFile::Changed);
    cmd(m, Command::Undo);
    assert_eq!(song(m).doc.tracks, new_song_without_pack().doc.tracks);
    assert_eq!(file(m), SongFile::NeverSaved);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn an_unreadable_file_opens_the_song_last_saved_and_is_kept() {
    let (dir, songs) = (
        temp_dir("recovery-unread"),
        temp_dir("recovery-unread-songs"),
    );
    let (saved, _, path) = saved_then_changed(&dir, &songs);
    // A newer meloet's file: the head reads, the song does not.
    let file_path = dir.join(recovery::FILE_NAME);
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&file_path).unwrap()).unwrap();
    v["version"] = 99.into();
    fs::write(&file_path, serde_json::to_vec(&v).unwrap()).unwrap();

    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    assert_eq!(song(m), saved);
    assert_eq!(m.song_path(), Some(path.as_path()));
    assert_eq!(file(m), SongFile::Saved);
    assert_eq!(
        recovery_view(m),
        Some(RecoveryView::Unreadable {
            song: Some("はじめての曲".into())
        })
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::RecoveryUnreadable { saved_song: true })
    );
    let kept: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(kept.len(), 1);
    assert!(!file_path.exists());

    assert_eq!(
        cmd(m, Command::ShowRecoveryFile),
        [Effect::ShowFile(kept[0].clone())]
    );
    assert!(
        recovery_view(m).is_some(),
        "showing the file keeps the hint"
    );
    assert_eq!(cmd(m, Command::DiscardRecovered), [], "no 戻さない here");
    cmd(m, Command::DismissHint);
    assert_eq!(recovery_view(m), None);
    assert!(kept[0].exists());
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}

#[test]
fn an_unreadable_file_of_a_song_never_saved_opens_a_new_song() {
    let dir = temp_dir("recovery-unread-new");
    fs::write(dir.join(recovery::FILE_NAME), b"{\"format\":").unwrap();
    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    assert_eq!(song(m), Song::new_default());
    assert_eq!(file(m), SongFile::NeverSaved);
    assert_eq!(
        recovery_view(m),
        Some(RecoveryView::Unreadable { song: None })
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::RecoveryUnreadable { saved_song: false })
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_hint_about_sound_waits_behind_the_recovery_hint() {
    let dir = temp_dir("recovery-hints");
    fs::write(dir.join(recovery::FILE_NAME), b"x").unwrap();
    let (mut m, _) = start_from(&dir);
    assert_eq!(m.view().workspace.hint, None);
    cmd(&mut m, Command::DismissHint);
    assert_eq!(m.view().workspace.recovery, None);
    assert_eq!(m.view().workspace.hint, Some(Hint::EmptyArrange));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_recovery_hint_waits_behind_the_output_chooser() {
    let dir = temp_dir("recovery-chooser");
    fs::write(dir.join(recovery::FILE_NAME), b"x").unwrap();
    let mut m = Model::new(crate::settings::Settings::default());
    m.handle(Input::KeyboardAdded(meloet_midi::Keyboard {
        id: "kb1".into(),
        name: "Keystation 49".into(),
        port_name: "MIDI 1".into(),
        client: 24,
        port: 0,
    }));
    m.handle(Input::RecoveryFound(Box::new(
        recovery_start(&dir, meloet_model::Language::Ja).unwrap(),
    )));
    assert!(m.view().workspace.chooser.is_some());
    assert_eq!(m.view().workspace.recovery, None);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_recording_over_a_song_that_is_written_already_writes_nothing_more() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    // Nothing changed: nothing to write.
    let record = at(m, 1_000_000, Input::Command(Command::Record));
    assert!(writes(&record).is_empty());
    started(m);
    m.set_now(1_100_000, None);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 1_100_000,
            tick: 400.0,
            lap: 0,
        },
    );
    // A change that was written when the hands rested.
    at(
        m,
        2_000_000,
        Input::Command(Command::SetTempo { bpm: 100.0 }),
    );
    assert_eq!(writes(&tick(m, 2_000_000 + QUIET_US)).len(), 1);
    let record = at(m, 9_000_000, Input::Command(Command::Record));
    assert!(writes(&record).is_empty(), "the file holds the song");
}

/// A recording that was going on when meloet ended: its note and its file in `dir`.
fn left_recording(dir: &Path, frames: usize) {
    use crate::vocalrec::{Marks, NoteFile, RecNote, WavFile, work_audio_dir, write_note};
    use meloet_protocol::RecordFormat;
    let audio = work_audio_dir(dir);
    fs::create_dir_all(&audio).unwrap();
    let mut wav =
        WavFile::create(&audio.join("歌 9.wav"), 48_000, 1, RecordFormat::Bits24).unwrap();
    wav.write(&vec![0.25; frames]).unwrap();
    wav.sync().unwrap();
    // No `finish`: the lengths in the head are the ones of a file still being written.
    drop(wav);
    let note = RecNote::new(
        None,
        48_000,
        RecordFormat::Bits24,
        vec![NoteFile {
            file: "歌 9.wav".into(),
            track: 77,
            track_name: "歌".into(),
            channels: 1,
        }],
        Marks {
            from: 0,
            loop_range: None,
            roll: Some((0.0, 0.0)),
            laps: Vec::new(),
        },
    );
    write_note(dir, &note).unwrap();
}

#[test]
fn a_kept_recording_is_written_at_once_and_its_note_stays_until_then() {
    let dir = temp_dir("recovery-kept-voice");
    left_recording(&dir, 48_000);
    let note = dir.join(crate::vocalrec::REC_NOTE);
    let (m, effects) = start_from(&dir);
    assert!(matches!(
        recovery_view(&m),
        Some(RecoveryView::Recovered {
            vocals: Some(meloet_protocol::KeptVocals { count: 1, .. }),
            ..
        })
    ));
    // Not two seconds later: until the file holds the clip, only the note knows where it
    // belongs.
    let Some(Effect::WriteRecovery { song, path, .. }) = effects
        .iter()
        .find(|e| matches!(e, Effect::WriteRecovery { .. }))
    else {
        panic!("written at the start: {effects:?}");
    };
    assert_eq!(song.doc.audio.len(), 1);
    assert_eq!(song.doc.audio[0].file, "歌 9.wav");
    assert!(note.exists(), "the note waits for the recovery file");

    // meloet ends again before the note is gone, the recovery file written or not: the
    // voice comes back once, not twice.
    let (again, _) = start_from(&dir);
    assert_eq!(again.play.song().doc.audio.len(), 1);
    recovery::write(
        &dir,
        &recovery::encode(song, path.as_deref(), RecoveryCause::Sudden, 1),
    )
    .unwrap();
    assert!(note.exists());
    let (again, effects) = start_from(&dir);
    assert_eq!(again.play.song().doc.audio.len(), 1);
    assert_eq!(again.play.song().doc.tracks.len(), song.doc.tracks.len());
    assert!(matches!(
        recovery_view(&again),
        Some(RecoveryView::Recovered { vocals: None, .. })
    ));
    assert!(writes(&effects).is_empty());
    assert!(!note.exists(), "nothing of it was left to keep");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_file_with_only_its_head_is_not_left_behind() {
    // meloet ended while counting in: the file was made, and nothing was written into it.
    let dir = temp_dir("recovery-head-only");
    left_recording(&dir, 0);
    let file = crate::vocalrec::work_audio_dir(&dir).join("歌 9.wav");
    assert_eq!(fs::metadata(&file).unwrap().len(), 44);
    assert!(
        recovery_start(&dir, meloet_model::Language::Ja).is_some_and(|start| matches!(
            start,
            crate::state::RecoveryStart::Read { vocals: None, .. }
        ))
    );
    assert!(!file.exists());
    assert!(!dir.join(crate::vocalrec::REC_NOTE).exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_file_handed_at_the_start_asks_about_the_song_brought_back_first() {
    let (dir, songs) = (temp_dir("recovery-given"), temp_dir("recovery-given-songs"));
    let (_, changed, path) = saved_then_changed(&dir, &songs);
    let other = songs.join("other.meloet");
    songfile::save(&other, &Song::new_default()).unwrap();
    let (mut m, _) = start_from(&dir);
    let m = &mut m;
    m.handle(Input::OpenGiven(other.clone()));
    assert_eq!(
        m.view().workspace.song_dialog,
        None,
        "not before the screen"
    );
    m.handle(Input::ScreenUp);
    let d = m.view().workspace.song_dialog.expect("asked");
    assert_eq!(
        (d.kind, d.title.as_str()),
        (meloet_protocol::SongDialogKind::Unsaved, "はじめての曲")
    );
    cmd(
        m,
        Command::AnswerSongDialog {
            dialog: d.id,
            answer: SongAnswer::Cancel,
        },
    );
    assert_eq!(song(m), changed);
    assert!(recovery_view(m).is_some(), "still brought back");

    // The song brought back is that file's: it stays as it is.
    m.handle(Input::OpenGiven(path));
    assert_eq!(m.view().workspace.song_dialog, None);
    assert_eq!(song(m), changed);

    m.handle(Input::OpenGiven(other.clone()));
    let d = m.view().workspace.song_dialog.expect("asked");
    let effects = cmd(
        m,
        Command::AnswerSongDialog {
            dialog: d.id,
            answer: SongAnswer::DontSave,
        },
    );
    let read = effects.into_iter().find_map(|e| match e {
        Effect::ReadSong { path, previous } => Some((path, previous)),
        _ => None,
    });
    assert_eq!(read, Some((other.clone(), false)));
    let effects = m.handle(crate::service::read_song(other.clone(), false));
    assert_eq!(removes(&effects), 1, "the song brought back was let go");
    assert_eq!(m.song_path(), Some(other.as_path()));
    assert_eq!(recovery_view(m), None);
    let _ = (fs::remove_dir_all(dir), fs::remove_dir_all(songs));
}
