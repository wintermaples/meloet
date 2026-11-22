//! The rules of the sound pack's fetch: the question, the answers, what the thread says.

use meloet_model::BuiltinInstrument as B;
use meloet_protocol::{PackFailure, PackFetchView, StandIn};

use super::*;
use crate::sounds::fetch::{Job, Numbers, Outlook, PackNews};

fn sampled() -> Vec<B> {
    B::all().filter(|b| instruments::sampled(*b)).collect()
}

fn start(fresh: bool) -> PackStart {
    PackStart {
        instruments: sampled(),
        outlook: Outlook {
            numbers: Numbers {
                done: 0,
                total: 240_000_000,
            },
            fresh,
            update_bytes: 0,
            missing: true,
            fetched_any: false,
        },
        pack_bytes: 240_000_000,
        place: "~/.local/share/meloet/sounds".into(),
    }
}

fn keyboard() -> Input {
    Input::KeyboardAdded(meloet_midi::Keyboard {
        id: "kb".into(),
        name: "Keystation".into(),
        port_name: "MIDI 1".into(),
        client: 20,
        port: 0,
    })
}

/// A first start: nothing of the pack is here.
fn first_start(settings: Settings) -> Model {
    let settings = Settings {
        output: Some("out".into()),
        ..settings
    };
    let mut m = Model::new(settings)
        .with_sounds(Vec::new())
        .with_pack(start(true));
    m.handle(keyboard());
    m
}

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

fn saved(effects: &[Effect]) -> Option<bool> {
    effects.iter().find_map(|e| match e {
        Effect::SaveSettings(s) => Some(s.pack_fetch),
        _ => None,
    })
}

fn asked(effects: &[Effect]) -> Vec<PackAsk> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Pack(ask) => Some(*ask),
            _ => None,
        })
        .collect()
}

#[test]
fn the_first_start_asks_and_starts_with_the_grand_piano_as_the_sine_wave() {
    let m = first_start(Settings::default());
    let view = m.view();
    assert!(view.sounds.ask);
    assert_eq!(view.sounds.fetch, PackFetchView::Idle);
    assert_eq!((view.sounds.have, view.sounds.total), (5, 31));
    assert_eq!(view.sounds.pack_bytes, 240_000_000);
    assert_eq!(view.sounds.place, "~/.local/share/meloet/sounds");
    // The new song's first track is the grand piano, sounding as the sine wave until it comes.
    let first = &view.song.tracks[0];
    assert_eq!(
        m.play().song().doc.tracks[0].instrument,
        InstrumentRef::Builtin(B::GrandPiano)
    );
    assert_eq!(first.stand_in, Some(StandIn::PackMissing));
    // The shelf: the five computed ones, and the 26 others as 「まだ」 in the shelf's order.
    assert_eq!(view.shelf.available.len(), 5);
    assert_eq!(view.shelf.not_yet.len(), 26);
    assert_eq!(view.shelf.not_yet[0], InstrumentKind::GrandPiano);
    assert_eq!(view.status.message, Some(StatusMessage::PackAsking));
    // Nothing is fetched before the answer, also not when the page is up.
    let mut m = m;
    let effects = m.handle(Input::ScreenUp);
    assert_eq!(asked(&effects), [PackAsk::Job(Job::Check)]);
}

#[test]
fn the_answer_fetches_and_each_instrument_sounds_as_it_comes() {
    let mut m = first_start(Settings::default());
    let effects = cmd(&mut m, Command::FetchPack);
    assert_eq!(asked(&effects), [PackAsk::Job(Job::Fetch { round: 1 })]);
    assert_eq!(saved(&effects), Some(true), "the answer is kept");
    let view = m.view().sounds;
    assert!(!view.ask);
    assert!(matches!(
        view.fetch,
        PackFetchView::Fetching {
            total_bytes: 240_000_000,
            ..
        }
    ));
    // A second 「取りにいく」 while fetching does nothing.
    assert!(cmd(&mut m, Command::FetchPack).is_empty());

    let numbers = Numbers {
        done: 1_000_000,
        total: 240_000_000,
    };
    m.handle(Input::Pack(PackNews::Progress {
        round: 1,
        instrument: Some(B::GrandPiano),
        numbers,
        remaining_s: Some(120),
    }));
    assert_eq!(
        m.view().sounds.fetch,
        PackFetchView::Fetching {
            instrument: Some(InstrumentKind::GrandPiano),
            done_bytes: 1_000_000,
            total_bytes: 240_000_000,
            remaining_s: Some(120),
        }
    );
    // What an older round says is not taken.
    m.handle(Input::Pack(PackNews::Progress {
        round: 0,
        instrument: None,
        numbers: Numbers::default(),
        remaining_s: None,
    }));
    assert!(matches!(
        m.view().sounds.fetch,
        PackFetchView::Fetching {
            done_bytes: 1_000_000,
            ..
        }
    ));

    let effects = m.handle(Input::Pack(PackNews::Installed {
        round: 1,
        instrument: Some(B::GrandPiano),
        json: "/x/grandPiano/v/instrument.json".into(),
    }));
    assert!(
        effects.contains(&Effect::LoadSound(B::GrandPiano)),
        "the track wants it"
    );
    let view = m.view();
    assert_eq!(
        view.status.message,
        Some(StatusMessage::SoundReady {
            instrument: InstrumentKind::GrandPiano
        })
    );
    assert_eq!(view.sounds.have, 6);
    assert_eq!(view.shelf.not_yet.len(), 25);
    assert!(view.shelf.available.contains(&InstrumentKind::GrandPiano));
    // Read: the seat gets the piano in place of the sine wave.
    let effects = m.handle(Input::SoundLoaded(B::GrandPiano));
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Instrument {
            instrument: InstrumentRef::Builtin(B::GrandPiano),
            ..
        })
    )));
    assert_eq!(m.view().song.tracks[0].stand_in, None);

    let effects = m.handle(Input::Pack(PackNews::Done { round: 1 }));
    assert_eq!(saved(&effects), Some(false), "nothing more to go on with");
    let view = m.view();
    assert_eq!(view.sounds.fetch, PackFetchView::Idle);
    assert_eq!(
        view.status.message,
        Some(StatusMessage::PackFetched { count: 6 })
    );
    assert!(
        asked(&cmd(&mut m, Command::FetchPack)).is_empty(),
        "nothing to fetch"
    );
}

#[test]
fn a_failure_is_said_and_later_and_stop_put_the_band_away() {
    let mut m = first_start(Settings::default());
    cmd(&mut m, Command::FetchPack);
    let numbers = Numbers {
        done: 92_000_000,
        total: 240_000_000,
    };
    m.handle(Input::Pack(PackNews::Failed {
        round: 1,
        failure: PackFailure::CutOff,
        numbers,
    }));
    let view = m.view();
    assert_eq!(
        view.sounds.fetch,
        PackFetchView::Failed {
            failure: PackFailure::CutOff,
            done_bytes: 92_000_000,
            total_bytes: 240_000_000,
        }
    );
    assert_eq!(
        view.status.message,
        Some(StatusMessage::PackFailed {
            failure: PackFailure::CutOff,
            done_bytes: 92_000_000,
            total_bytes: 240_000_000,
        })
    );
    // 「続きから」 starts a new round from what is there.
    let effects = cmd(&mut m, Command::FetchPack);
    assert_eq!(asked(&effects), [PackAsk::Job(Job::Fetch { round: 2 })]);
    assert!(matches!(
        m.view().sounds.fetch,
        PackFetchView::Fetching {
            done_bytes: 92_000_000,
            ..
        }
    ));
    // 「やめる」.
    let effects = cmd(&mut m, Command::StopPackFetch);
    assert_eq!(asked(&effects), [PackAsk::Stop { round: 2 }]);
    assert_eq!(saved(&effects), Some(false), "the next start asks again");
    assert_eq!(m.view().sounds.fetch, PackFetchView::Idle);
    // What the stopped round says late is not taken.
    m.handle(Input::Pack(PackNews::Done { round: 2 }));
    m.handle(Input::Pack(PackNews::Failed {
        round: 2,
        failure: PackFailure::Offline,
        numbers,
    }));
    assert_eq!(m.view().sounds.fetch, PackFetchView::Idle);
    // A broken pack: a new song starts with the sine wave (85).
    let effects = cmd(&mut m, Command::FetchPack);
    assert_eq!(asked(&effects), [PackAsk::Job(Job::Fetch { round: 4 })]);
    m.handle(Input::Pack(PackNews::Failed {
        round: 4,
        failure: PackFailure::Corrupt,
        numbers,
    }));
    assert_eq!(
        m.play().new_song().doc.tracks[0].instrument,
        InstrumentRef::SINE
    );
    // 「あとで」 after a failure.
    let effects = cmd(&mut m, Command::PackLater);
    assert_eq!(saved(&effects), Some(false));
    assert_eq!(m.view().sounds.fetch, PackFetchView::Idle);
    assert!(cmd(&mut m, Command::PackLater).is_empty());
}

#[test]
fn later_asks_again_at_the_next_start_and_a_kept_answer_goes_on() {
    let mut m = first_start(Settings::default());
    let effects = cmd(&mut m, Command::PackLater);
    assert_eq!(saved(&effects), Some(false));
    assert!(!m.view().sounds.ask);
    assert_ne!(m.view().status.message, Some(StatusMessage::PackAsking));
    // The next start, with 「取りにいく」 kept: no question, the fetch goes on once the page
    // is up, after the look at the catalog.
    let mut m = first_start(Settings {
        pack_fetch: true,
        ..Settings::default()
    });
    assert!(!m.view().sounds.ask);
    let effects = m.handle(Input::ScreenUp);
    assert_eq!(
        asked(&effects),
        [
            PackAsk::Job(Job::Check),
            PackAsk::Job(Job::Fetch { round: 1 })
        ]
    );
    assert!(m.handle(Input::ScreenUp).is_empty(), "once");
}

#[test]
fn a_new_pack_is_said_once_and_fetched_on_the_answer() {
    let mut s = start(false);
    s.outlook.missing = false;
    let mut m = Model::new(Settings::default())
        .with_sounds(sampled())
        .with_pack(s);
    let view = m.view().sounds;
    assert!(!view.ask);
    assert_eq!((view.have, view.total), (31, 31));
    assert!(asked(&cmd(&mut m, Command::FetchPack)).is_empty());
    m.handle(Input::Pack(PackNews::NewPack {
        bytes: 12_000_000,
        pack_bytes: 241_000_000,
        instruments: sampled(),
    }));
    let view = m.view();
    assert_eq!(view.sounds.new_pack_bytes, Some(12_000_000));
    assert_eq!(
        view.status.message,
        Some(StatusMessage::NewPack { bytes: 12_000_000 })
    );
    let effects = cmd(&mut m, Command::FetchPack);
    assert_eq!(asked(&effects), [PackAsk::Job(Job::Fetch { round: 1 })]);
    // A newer version of an instrument that is here is read again; no 「鳴るようになりました」.
    m.handle(Input::SoundLoaded(B::GrandPiano));
    let effects = m.handle(Input::Pack(PackNews::Installed {
        round: 1,
        instrument: Some(B::GrandPiano),
        json: "/x".into(),
    }));
    assert!(!matches!(
        m.view().status.message,
        Some(StatusMessage::SoundReady { .. })
    ));
    assert!(effects.contains(&Effect::DropSound(B::GrandPiano)));
    assert!(effects.contains(&Effect::LoadSound(B::GrandPiano)));
    m.handle(Input::Pack(PackNews::Done { round: 1 }));
    assert_eq!(m.view().sounds.new_pack_bytes, None);
}

#[test]
fn without_a_fetching_thread_the_pack_counts_as_there() {
    let mut m = Model::new(Settings::default()).with_sounds(Vec::new());
    let view = m.view();
    assert_eq!((view.sounds.have, view.sounds.total), (31, 31));
    assert!(!view.sounds.ask);
    assert!(view.shelf.not_yet.is_empty());
    assert_eq!(view.shelf.available, [InstrumentKind::Sine], "spec 3, 85");
    assert_eq!(
        m.play().song().doc.tracks[0].instrument,
        InstrumentRef::SINE
    );
    assert!(asked(&cmd(&mut m, Command::FetchPack)).is_empty());
    assert!(asked(&m.handle(Input::ScreenUp)).is_empty());
}
