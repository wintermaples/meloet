//! Protocol 33 (round 12, waves 3 and 4): every new intent is taken, and until its feature is
//! built it changes nothing, as an intent with an old id.

use meloet_protocol::{
    Arp, ArpRate, ArpShape, AutoTarget, CompPiece, LaneHome, Language, StretchChange, TickSpan,
};

use super::*;

fn fresh() -> Model {
    Model::new(Settings::default())
}

fn cmd(model: &mut Model, command: Command) -> Vec<Effect> {
    model.handle(Input::Command(command))
}

#[test]
fn the_commands_of_round_12_waves_3_and_4_are_taken_and_change_nothing_yet() {
    let mut m = fresh();
    let before = m.view();
    let track = before.song.tracks[0].id;
    assert_eq!(before.song.tracks[0].arp, Arp::default());
    assert!(before.song.master.lanes.is_empty());
    assert!(before.workspace.spectrum);
    assert!(!before.workspace.mono_listen);
    assert_eq!(before.workspace.language, Language::Ja);
    assert_eq!(before.audio_work.bounce, None);
    let arp = Arp {
        on: true,
        shape: ArpShape::UpDown,
        rate: ArpRate::Eighth,
        triplet: true,
        octaves: 2,
        hold: true,
    };
    let master = LaneHome::Master;
    let volume = AutoTarget::Volume;
    let commands = [
        Command::SetArp { track, arp },
        Command::SetMonoListen { on: true },
        Command::DuplicateSection { marker: 99 },
        Command::DeleteSection { marker: 99 },
        Command::InsertSpace {
            marker: 99,
            bars: 4,
        },
        Command::BounceToAudio { clips: vec![99] },
        Command::BounceTrack { track },
        Command::CancelBounce,
        Command::AddLane {
            home: master,
            target: volume,
        },
        Command::FoldLane {
            home: master,
            target: volume,
            folded: true,
        },
        Command::SetLaneWrite {
            home: master,
            target: volume,
            on: true,
        },
        Command::RestoreLane {
            home: master,
            target: volume,
        },
        Command::DeleteLane {
            home: master,
            target: volume,
        },
        Command::AddFadeOut { bars: 4 },
        Command::SetComp {
            clip: 99,
            span: TickSpan {
                start: 0,
                end: 1920,
            },
            pieces: vec![CompPiece { from: 0, take: 98 }],
        },
        Command::ClearComp { clip: 99 },
        Command::SetClipStretch {
            clip: 99,
            change: StretchChange::TuneStrength { percent: 50.0 },
        },
        Command::SetClipStretch {
            clip: 99,
            change: StretchChange::TuneScale,
        },
        Command::SetClipsStretch {
            clips: vec![99],
            change: StretchChange::TuneStrength { percent: 50.0 },
        },
        Command::SetLanguage {
            language: Language::En,
        },
    ];
    for command in commands {
        assert_eq!(cmd(&mut m, command.clone()), [], "{command:?}");
    }
    assert_eq!(AppState::changes_from(&before, &m.view()), []);
    assert_eq!(m.play().undo_count(), 0);
}

/// E7 「音の動きを見せる」: a setting, not a step; the same value again saves nothing.
#[test]
fn the_spectrum_switch_is_kept_in_the_settings() {
    let mut m = fresh();
    let before = m.view();
    let effects = cmd(&mut m, Command::SetSpectrum { on: false });
    let [Effect::SaveSettings(saved)] = &effects[..] else {
        panic!("{effects:?}");
    };
    assert!(!saved.spectrum);
    assert!(!m.view().workspace.spectrum);
    assert_eq!(m.play().undo_count(), 0);
    assert_eq!(cmd(&mut m, Command::SetSpectrum { on: false }), []);
    assert!(matches!(
        &cmd(&mut m, Command::SetSpectrum { on: true })[..],
        [Effect::SaveSettings(s)] if s.spectrum
    ));
    assert_eq!(AppState::changes_from(&before, &m.view()), []);
}

/// The settings of E7 and D4 are read and written back as they are; the screen stays
/// Japanese until D4.
#[test]
fn the_spectrum_and_the_language_are_settings() {
    let settings = Settings {
        spectrum: false,
        language: Some(Language::En),
        ..Settings::default()
    };
    let m = Model::new(settings.clone());
    let ws = m.view().workspace;
    assert!(!ws.spectrum);
    assert_eq!(ws.language, Language::Ja);
    assert_eq!(*m.settings(), settings);
}
