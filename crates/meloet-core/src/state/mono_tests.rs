//! M3 「仕上げのメーター」: 「モノで聞く」, only for listening, and what the analysis thread is
//! asked to work.

use super::tests::{HP_NODE, playing};
use super::*;
use crate::analysis::Wants;

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

#[test]
fn mono_listen_tells_the_playing_engine_only_and_changes_nothing_of_the_song() {
    let mut m = playing();
    let before = m.view();
    let settings = m.settings();
    let effects = cmd(&mut m, Command::SetMonoListen { on: true });
    assert_eq!(effects, [Effect::MonoListen(true)]);
    let after = m.view();
    assert!(after.workspace.mono_listen);
    assert_eq!(status(&m), Some(StatusMessage::MonoListening));
    // Not a step, nothing of the song (the export renders the song), not a setting.
    assert_eq!(m.play().undo_count(), 0);
    assert_eq!(after.song, before.song);
    assert_eq!(m.settings(), settings);
    assert_eq!(cmd(&mut m, Command::SetMonoListen { on: true }), []);

    // It is said in place of the summary while it stays on.
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), Some(StatusMessage::MonoListening));

    assert_eq!(
        cmd(&mut m, Command::SetMonoListen { on: false }),
        [Effect::MonoListen(false)]
    );
    assert!(!m.view().workspace.mono_listen);
    assert_ne!(status(&m), Some(StatusMessage::MonoListening));
    assert_eq!(m.play().undo_count(), 0);
}

#[test]
fn a_stream_that_opens_is_told_again_and_meloet_starts_with_it_off() {
    let mut m = playing();
    cmd(&mut m, Command::SetMonoListen { on: true });
    let effects = m.handle(Input::AudioOpened {
        node_name: HP_NODE.into(),
        sample_rate: 48_000,
    });
    assert!(effects.contains(&Effect::MonoListen(true)), "{effects:?}");
    assert!(!effects.iter().any(|e| matches!(e, Effect::SaveSettings(_))));

    let next = Model::new((*m.settings()).clone());
    assert!(!next.view().workspace.mono_listen);
}

#[test]
fn it_may_be_turned_while_counting_in_or_recording() {
    let mut m = playing();
    cmd(&mut m, Command::Record);
    assert!(m.play.busy());
    assert_eq!(
        cmd(&mut m, Command::SetMonoListen { on: true }),
        [Effect::MonoListen(true)]
    );
}

#[test]
fn the_analysis_works_what_the_screen_shows() {
    let mut m = playing();
    cmd(
        &mut m,
        Command::SetViewport {
            width: 1920.0,
            height: 1080.0,
        },
    );
    let wants = |spectrum, loudness| Wants { spectrum, loudness };
    assert_eq!(m.analysis_wants(), wants(true, false));
    cmd(&mut m, Command::ToggleBand { band: Band::Mixer });
    assert_eq!(m.analysis_wants(), wants(true, true));
    // 「音の動きを見せる」 off: the mixer's box still shows the bands.
    cmd(&mut m, Command::SetSpectrum { on: false });
    assert_eq!(m.analysis_wants(), wants(true, true));
    cmd(&mut m, Command::ToggleBand { band: Band::Mixer });
    assert_eq!(m.analysis_wants(), wants(false, false));
    cmd(&mut m, Command::SetSpectrum { on: true });
    assert_eq!(m.analysis_wants(), wants(true, false));
}
