//! M2 「音が大きすぎます」: the notice the telemetry thread's finding makes, and
//! 「全部のトラックを N dB 下げる」.

use super::tests::playing;
use super::*;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

fn found(m: &mut Model, by_db: f32, limiter: bool) {
    m.handle(Input::TooLoud { by_db, limiter });
}

fn loud(by_db: f32, lower_db: u8, limiter: bool) -> Option<StatusMessage> {
    Some(StatusMessage::TooLoud {
        by_db,
        lower_db,
        limiter,
    })
}

fn gain_db(m: &Model, number: usize) -> Option<f32> {
    m.view().song.tracks[number - 1].gain_db
}

#[test]
fn once_a_play_until_it_is_closed_or_the_next_play() {
    let mut m = playing();
    cmd(&mut m, Command::PlayStop);
    assert!(m.play.is_moving());
    found(&mut m, 5.62, true);
    assert_eq!(status(&m), loud(5.6, 6, true));
    // Once a play: a second finding changes nothing.
    found(&mut m, 2.06, false);
    assert_eq!(status(&m), loud(5.6, 6, true));
    // It stays over other operations, after their notices, and over the stop.
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), loud(5.6, 6, true));
    cmd(&mut m, Command::PlayStop);
    assert!(!m.play.is_moving());
    assert_eq!(status(&m), loud(5.6, 6, true));
    cmd(&mut m, Command::DismissTooLoud);
    assert_ne!(status(&m), loud(5.6, 6, true));

    // The next play finds anew, and puts away what is not answered.
    cmd(&mut m, Command::PlayStop);
    found(&mut m, 2.06, false);
    assert_eq!(status(&m), loud(2.1, 3, false));
    cmd(&mut m, Command::PlayStop);
    cmd(&mut m, Command::PlayStop);
    assert_ne!(status(&m), loud(2.1, 3, false));
    cmd(&mut m, Command::DismissTooLoud);
    found(&mut m, 4.0, true);
    assert_eq!(status(&m), loud(4.0, 4, true));
}

#[test]
fn closing_it_holds_until_the_next_play() {
    let mut m = playing();
    cmd(&mut m, Command::PlayStop);
    found(&mut m, 7.0, true);
    cmd(&mut m, Command::DismissTooLoud);
    found(&mut m, 7.0, true);
    assert_ne!(status(&m), loud(7.0, 7, true), "not again in this play");
    cmd(&mut m, Command::PlayStop);
    found(&mut m, 7.0, true);
    assert_ne!(status(&m), loud(7.0, 7, true), "nor after it stopped");
}

#[test]
fn another_song_puts_it_away() {
    let mut m = playing();
    cmd(&mut m, Command::PlayStop);
    found(&mut m, 7.0, true);
    cmd(&mut m, Command::PlayStop);
    assert_eq!(status(&m), loud(7.0, 7, true));
    let serial = m.play.serial();
    cmd(&mut m, Command::NewSong);
    assert_ne!(m.play.serial(), serial, "a new song is in place");
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_ne!(status(&m), loud(7.0, 7, true));
}

#[test]
fn found_while_recording_it_is_said_once_the_recording_is_over() {
    let mut m = playing();
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(&mut m, Command::Record);
    assert!(m.play.busy());
    found(&mut m, 6.0, true);
    assert_ne!(status(&m), loud(6.0, 6, true), "not over the recording");
    let undo = m.view().song.undo_count;
    cmd(&mut m, Command::LowerAllTracks { db: 6.0 });
    assert_eq!(gain_db(&m, 1), Some(0.0), "refused while recording");
    assert_eq!(m.view().song.undo_count, undo);
    cmd(&mut m, Command::PlayStop);
    m.handle(Input::Record(crate::play::RecordReport::Stopped {
        t_us: 100,
        tick: 0.0,
        lap: 0,
    }));
    assert!(!m.play.busy());
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), loud(6.0, 6, true));
}

#[test]
fn lowering_every_track_is_one_step_and_puts_the_notice_away() {
    let mut m = playing();
    cmd(&mut m, Command::DuplicateTrack { track: 1 });
    let second = m.view().song.tracks[1].id;
    cmd(
        &mut m,
        Command::SetGain {
            strip: meloet_protocol::StripRef::Track { id: second },
            db: Some(-58.0),
            commit: true,
        },
    );
    let undo = m.view().song.undo_count;
    cmd(&mut m, Command::PlayStop);
    found(&mut m, 6.0, true);
    cmd(&mut m, Command::LowerAllTracks { db: 6.0 });
    let view = m.view();
    assert_eq!(view.song.undo_count, undo + 1, "one step");
    assert_eq!((gain_db(&m, 1), gain_db(&m, 2)), (Some(-6.0), Some(-60.0)));
    assert_eq!(view.song.master.gain_db, Some(0.0), "not the master");
    assert_eq!(view.song.returns[0].gain_db, Some(0.0), "not the returns");
    assert_ne!(status(&m), loud(6.0, 6, true));
    found(&mut m, 6.0, true);
    assert_ne!(status(&m), loud(6.0, 6, true), "not again in this play");

    cmd(&mut m, Command::Undo);
    assert_eq!((gain_db(&m, 1), gain_db(&m, 2)), (Some(0.0), Some(-58.0)));
    cmd(&mut m, Command::Redo);
    assert_eq!((gain_db(&m, 1), gain_db(&m, 2)), (Some(-6.0), Some(-60.0)));
    // What is not a lowering does nothing.
    for db in [0.0, -6.0, f32::NAN] {
        cmd(&mut m, Command::LowerAllTracks { db });
    }
    assert_eq!(m.view().song.undo_count, undo + 1);
}
