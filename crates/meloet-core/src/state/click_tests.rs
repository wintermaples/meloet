//! R3, 「メトロノーム ▾」: the click's volume, sound and when it sounds. A meloet setting, not
//! part of the song.

use meloet_protocol::{ClickSound, ClickWhen};

use super::*;
use crate::play::{EngineOp, Play};
use crate::settings::ClickSettings;

fn fresh() -> Model {
    Model::new(Settings::default())
}

fn set(
    m: &mut Model,
    volume_db: Option<f32>,
    sound: Option<ClickSound>,
    when: Option<ClickWhen>,
    commit: bool,
) -> Vec<Effect> {
    m.handle(Input::Command(Command::SetClick {
        volume_db,
        sound,
        when,
        commit,
    }))
}

fn shown(m: &Model) -> ClickSettings {
    let t = m.view().transport;
    ClickSettings {
        volume_db: t.click_volume_db,
        sound: t.click_sound,
        when: t.click_when,
    }
}

fn told(effects: &[Effect]) -> Vec<ClickSettings> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::Click { settings }) => Some(*settings),
            _ => None,
        })
        .collect()
}

fn saved(effects: &[Effect]) -> Vec<ClickSettings> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::SaveSettings(s) => Some(s.click),
            _ => None,
        })
        .collect()
}

const AS_MADE: ClickSettings = ClickSettings {
    volume_db: 0.0,
    sound: ClickSound::Beep,
    when: ClickWhen::Always,
};

#[test]
fn a_committed_change_is_heard_shown_and_saved_and_is_not_a_step() {
    let mut m = fresh();
    let before = m.view();
    let want = ClickSettings {
        volume_db: -12.0,
        sound: ClickSound::Cowbell,
        when: ClickWhen::RecordingOnly,
    };
    let effects = set(
        &mut m,
        Some(-12.0),
        Some(ClickSound::Cowbell),
        Some(ClickWhen::RecordingOnly),
        true,
    );
    assert_eq!(told(&effects), [want]);
    assert_eq!(saved(&effects), [want]);
    assert_eq!(effects.len(), 2, "{effects:?}");
    assert_eq!(shown(&m), want);
    assert_eq!(m.settings().click, want);
    let after = m.view();
    assert_eq!(after.song.undo_count, before.song.undo_count, "not a step");
    assert_eq!(after.song.file, before.song.file, "the song is not changed");

    // Only what is given changes.
    let effects = set(&mut m, None, Some(ClickSound::Tick), None, true);
    let tick = ClickSettings {
        sound: ClickSound::Tick,
        ..want
    };
    assert_eq!((told(&effects), saved(&effects)), (vec![tick], vec![tick]));
    // The same again, or nothing given: nothing to do.
    assert!(set(&mut m, None, Some(ClickSound::Tick), None, true).is_empty());
    assert!(set(&mut m, None, None, None, true).is_empty());
    assert!(set(&mut m, None, None, None, false).is_empty());
}

#[test]
fn the_volume_is_put_into_its_range_and_a_value_that_is_not_a_number_does_nothing() {
    let mut m = fresh();
    set(&mut m, Some(40.0), None, None, true);
    assert_eq!(shown(&m).volume_db, 6.0);
    set(&mut m, Some(-100.0), None, None, true);
    assert_eq!(shown(&m).volume_db, -24.0);
    assert!(set(&mut m, Some(f32::NAN), None, None, true).is_empty());
    assert!(set(&mut m, Some(f32::INFINITY), None, None, false).is_empty());
    assert_eq!(shown(&m).volume_db, -24.0);
}

/// A drag of the volume: each move changes only what is heard; the release keeps it.
#[test]
fn a_drag_is_heard_and_its_release_is_kept() {
    let mut m = fresh();
    let effects = set(&mut m, Some(-6.0), None, None, false);
    let at = |db| ClickSettings {
        volume_db: db,
        ..AS_MADE
    };
    assert_eq!((told(&effects), saved(&effects)), (vec![at(-6.0)], vec![]));
    assert_eq!(shown(&m), AS_MADE, "the view keeps the setting");
    assert_eq!(m.settings().click, AS_MADE);
    // The same value again tells the engine nothing.
    assert!(set(&mut m, Some(-6.0), None, None, false).is_empty());
    let effects = set(&mut m, Some(-9.0), None, None, false);
    assert_eq!(told(&effects), [at(-9.0)]);
    // A stream that opens meanwhile plays what is heard.
    let mut opened = Vec::new();
    m.play.stream_opened(&mut opened);
    assert_eq!(told(&opened), [at(-9.0)]);

    // Released where it was last heard: kept, and the engine has it already.
    let effects = set(&mut m, Some(-9.0), None, None, true);
    assert_eq!((told(&effects), saved(&effects)), (vec![], vec![at(-9.0)]));
    assert_eq!(shown(&m), at(-9.0));
}

/// Esc: the screen sends the setting back with `commit: false`; nothing is kept.
#[test]
fn a_cancelled_drag_goes_back_to_the_setting() {
    let mut m = fresh();
    set(&mut m, Some(-3.0), None, None, true);
    let effects = set(&mut m, Some(-20.0), None, None, false);
    assert_eq!(told(&effects)[0].volume_db, -20.0);
    let effects = set(&mut m, Some(-3.0), None, None, false);
    assert_eq!(told(&effects)[0].volume_db, -3.0);
    assert!(saved(&effects).is_empty());
    assert_eq!(shown(&m).volume_db, -3.0);
    // Nothing is left heard apart from the setting.
    let mut opened = Vec::new();
    m.play.stream_opened(&mut opened);
    assert_eq!(told(&opened)[0].volume_db, -3.0);
}

#[test]
fn every_stream_that_opens_is_told_the_setting() {
    let kept = ClickSettings {
        volume_db: -18.0,
        sound: ClickSound::Tick,
        when: ClickWhen::RecordingOnly,
    };
    let mut play = Play::new(true, 1).with_click(kept);
    let mut effects = Vec::new();
    play.stream_opened(&mut effects);
    assert_eq!(told(&effects), [kept]);
    let m = Model::new(Settings {
        click: kept,
        ..Settings::default()
    });
    assert_eq!(shown(&m), kept);
}

/// What the engine is told: the gain of the volume, the sound, and whether it is quiet while
/// playing.
#[test]
fn the_engine_gets_a_gain_and_the_sound() {
    use meloet_engine::{ClickSound as Made, EngineCommand};
    let cases = [
        (
            0.0,
            ClickSound::Beep,
            ClickWhen::Always,
            Made::Beep,
            1.0,
            false,
        ),
        (
            -24.0,
            ClickSound::Tick,
            ClickWhen::RecordingOnly,
            Made::Tick,
            0.063_095_73,
            true,
        ),
        (
            6.0,
            ClickSound::Cowbell,
            ClickWhen::Always,
            Made::Cowbell,
            1.995_262_3,
            false,
        ),
    ];
    for (volume_db, sound, when, made, want_gain, want_only) in cases {
        let settings = ClickSettings {
            volume_db,
            sound,
            when,
        };
        let EngineCommand::SetClick {
            sound,
            gain,
            recording_only,
        } = crate::service::click_command(settings)
        else {
            panic!("not SetClick");
        };
        assert_eq!((sound, recording_only), (made, want_only));
        assert!((gain - want_gain).abs() < 1e-6, "{volume_db} dB: {gain}");
    }
}

/// A setting of the PC: it is taken while counting in and recording too.
#[test]
fn it_is_taken_while_recording() {
    let mut m = super::tests::playing();
    m.handle(Input::Command(Command::Record));
    assert!(m.play.busy(), "{:?}", m.view().transport.state);
    let effects = set(&mut m, None, None, Some(ClickWhen::RecordingOnly), true);
    assert_eq!(told(&effects).len(), 1);
    assert_eq!(saved(&effects).len(), 1);
    assert_eq!(shown(&m).when, ClickWhen::RecordingOnly);
}
