//! When the look for a new version goes out, and when the status line says what it found.

use super::super::tests::playing;
use super::super::*;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn asked(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::CheckUpdates))
        .count()
}

fn said(m: &Model) -> bool {
    matches!(
        m.view().status.message,
        Some(StatusMessage::NewVersion { ref version }) if version == "0.4.0"
    )
}

/// The stream is open, the page is up and the look found 0.4.0.
fn found() -> Model {
    let mut m = playing();
    assert_eq!(asked(&m.handle(Input::ScreenUp)), 1);
    m.handle(Input::NewVersion("0.4.0".into()));
    m
}

#[test]
fn the_look_goes_out_once_after_the_greeting() {
    let mut m = playing();
    assert_eq!(m.view().updates, UpdatesView::default());
    assert_eq!(asked(&m.handle(Input::ScreenUp)), 1);
    assert_eq!(asked(&m.handle(Input::ScreenUp)), 0);
    assert_eq!(
        asked(&cmd(&mut m, Command::SetCheckUpdates { on: false })),
        0
    );
    assert_eq!(
        asked(&cmd(&mut m, Command::SetCheckUpdates { on: true })),
        0
    );
}

#[test]
fn switched_off_it_never_goes_out() {
    let mut m = Model::new(Settings {
        check_updates: false,
        ..Settings::default()
    });
    assert_eq!(asked(&m.handle(Input::ScreenUp)), 0);
    assert!(!m.view().updates.check);
    // Turned on later in the run: it goes out then, and the switch is kept.
    let effects = cmd(&mut m, Command::SetCheckUpdates { on: true });
    assert_eq!(asked(&effects), 1);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::SaveSettings(s) if s.check_updates))
    );
    assert!(m.view().updates.check);
    // The same value again is nothing.
    assert_eq!(cmd(&mut m, Command::SetCheckUpdates { on: true }), []);
}

#[test]
fn turned_on_before_the_greeting_it_waits_for_it() {
    let mut m = Model::new(Settings {
        check_updates: false,
        ..Settings::default()
    });
    assert_eq!(
        asked(&cmd(&mut m, Command::SetCheckUpdates { on: true })),
        0
    );
    assert_eq!(asked(&m.handle(Input::ScreenUp)), 1);
}

#[test]
fn a_new_version_is_said_while_stopped_and_in_about() {
    let mut m = found();
    let view = m.view();
    assert_eq!(view.updates.new_version.as_deref(), Some("0.4.0"));
    assert!(said(&m));
    // 「あとで」: gone from the status line for this run, still in 「meloet について」.
    assert_eq!(cmd(&mut m, Command::DismissNewVersion), []);
    assert!(!said(&m));
    assert_eq!(m.view().updates.new_version.as_deref(), Some("0.4.0"));
    // It is not a step and changes nothing of the song.
    assert_eq!(m.view().song.undo_count, 0);
}

#[test]
fn the_page_of_the_release_opens_in_the_browser() {
    let mut m = playing();
    assert_eq!(
        cmd(&mut m, Command::OpenReleasePage),
        [],
        "nothing found yet"
    );
    let mut m = found();
    let effects = cmd(&mut m, Command::OpenReleasePage);
    assert_eq!(
        effects,
        [Effect::OpenUrl(
            "https://github.com/wintermaples/meloet/releases/tag/v0.4.0".into()
        )]
    );
    assert!(!said(&m));
}

#[test]
fn found_while_playing_it_waits_until_stopped_and_is_said_once() {
    let mut m = playing();
    m.handle(Input::ScreenUp);
    cmd(&mut m, Command::PlayStop);
    assert!(m.play.is_moving());
    m.handle(Input::NewVersion("0.4.0".into()));
    assert!(!said(&m), "not while playing");
    cmd(&mut m, Command::PlayStop);
    assert!(!m.play.is_moving());
    assert!(said(&m), "said once stopped");
    // Playing again puts it away for this run.
    cmd(&mut m, Command::PlayStop);
    cmd(&mut m, Command::PlayStop);
    assert!(!m.play.is_moving());
    assert!(!said(&m));
}

#[test]
fn not_in_the_count_in() {
    let mut m = found();
    assert!(said(&m));
    cmd(&mut m, Command::Record);
    assert!(m.play.busy());
    assert!(!said(&m));
}

#[test]
fn switched_off_what_was_found_is_gone_and_a_late_answer_is_dropped() {
    let mut m = found();
    cmd(&mut m, Command::SetCheckUpdates { on: false });
    assert_eq!(m.view().updates.new_version, None);
    assert!(!said(&m));
    m.handle(Input::NewVersion("0.4.0".into()));
    assert_eq!(m.view().updates.new_version, None);
}
