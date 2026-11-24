//! The settings of the output sets of a plugin instrument (docs/architecture.md 20-15): how
//! they reach the plugin processes, what the history and the AI link say of them.

use std::path::PathBuf;

use meloet_model::{Gain, OutputChange, Pan, Song, TickRange, TrackId};
use meloet_protocol::{Command, StepName};

use super::*;
use crate::ai::{AiAnswer, AiAsk, AiInstrument, AiOutputsMeasure, AiRefusal};
use crate::catalog::tests::SURGE_LV2;
use crate::export::told::Told;
use crate::play::tests::cmd;
use crate::state::added_tests::rigged;
use crate::state::{Input, Model, PluginSaid};

/// a: a stereo pair, b: mono in the middle, c: mono on the left, d: mono and off.
fn sets() -> Vec<OutputSet> {
    let set = |id: &str, name: &str, channels: u8, on: bool, pan: f32| OutputSet {
        id: id.into(),
        name: name.into(),
        channels,
        on,
        pan,
    };
    vec![
        set("a", "Main", 2, true, 0.0),
        set("b", "Kick", 1, true, 0.0),
        set("c", "Snare", 1, true, -1.0),
        set("d", "Room", 1, false, 0.0),
    ]
}

fn ask(m: &mut Model, ask: AiAsk) -> AiAnswer {
    m.handle(Input::Ai(ask));
    m.take_ai_answer().expect("an answer")
}

/// Asks for a change of the output `id` of `track`; returns the effects.
fn change(m: &mut Model, track: TrackId, id: &str, change: OutputChange) -> Vec<Effect> {
    let effects = m.handle(Input::Ai(AiAsk::SetPluginOutput {
        track: track.0,
        output: id.into(),
        change,
    }));
    let answer = m.take_ai_answer().expect("an answer");
    assert!(matches!(answer, AiAnswer::Done(_)), "{answer:?}");
    effects
}

fn gain(db: f64) -> OutputChange {
    OutputChange {
        gain: Some(Gain::from_db(db)),
        ..OutputChange::default()
    }
}

fn off() -> OutputChange {
    OutputChange {
        on: Some(Some(false)),
        ..OutputChange::default()
    }
}

/// The settings sent to plugins that are loaded.
fn sent(effects: &[Effect]) -> Vec<(u64, PluginOutputs)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::SetPluginOutputs { ticket, outputs } => Some((*ticket, outputs.clone())),
            _ => None,
        })
        .collect()
}

/// The loads asked for, with the settings they carry.
fn loads(effects: &[Effect]) -> Vec<(u64, PluginOutputs)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::LoadPlugin {
                ticket, outputs, ..
            } => Some((*ticket, outputs.clone())),
            _ => None,
        })
        .collect()
}

/// Nothing is loaded, let go of or given a state.
fn no_reload(effects: &[Effect]) {
    let reloads = (effects.iter()).filter(|e| {
        matches!(
            e,
            Effect::LoadPlugin { .. }
                | Effect::ReloadPlugin { .. }
                | Effect::DropPlugin { .. }
                | Effect::SetPluginState { .. }
        )
    });
    assert_eq!(reloads.count(), 0, "{effects:?}");
}

/// The table of the track's instrument plugin in the song.
fn table(m: &Model, track: TrackId) -> PluginOutputs {
    match &m.ai_song().song.doc.track(track).unwrap().instrument {
        InstrumentRef::Plugin(p) => p.outputs.clone(),
        other => panic!("{other:?}"),
    }
}

/// A model with a Surge XT track added by the AI link, its load asked for and not answered.
fn adding() -> (Model, TrackId, u64, Vec<Effect>) {
    let mut m = rigged();
    let effects = m.handle(Input::Ai(AiAsk::AddTrack {
        instrument: AiInstrument::Plugin(SURGE_LV2.into()),
    }));
    assert!(matches!(m.take_ai_answer(), Some(AiAnswer::Done(_))));
    let track = m.ai_song().song.doc.tracks.last().unwrap().id;
    let [(ticket, _)] = loads(&effects)[..] else {
        panic!("one load: {effects:?}");
    };
    (m, track, ticket, effects)
}

/// The plugin of `ticket` is loaded and says its output sets, as the plugins' thread tells.
fn loaded(m: &mut Model, ticket: u64) -> Vec<Effect> {
    m.handle(Input::PluginSaid {
        ticket,
        said: PluginSaid::Outputs(sets()),
    });
    m.handle(Input::PluginLoaded { ticket })
}

fn with_loaded() -> (Model, TrackId, u64) {
    let (mut m, track, ticket, _) = adding();
    loaded(&mut m, ticket);
    (m, track, ticket)
}

#[test]
fn a_song_with_no_settings_loads_with_the_default() {
    let (_, _, _, effects) = adding();
    assert_eq!(loads(&effects)[0].1, PluginOutputs::default());
}

#[test]
fn a_change_goes_to_the_loaded_plugin_and_undo_and_redo_send_the_tables() {
    let (mut m, track, ticket) = with_loaded();
    let steps = m.ai_song().undo_count;

    let effects = change(&mut m, track, "b", gain(-6.0));
    no_reload(&effects);
    let set_b = table(&m, track);
    assert_eq!(set_b.get("b").unwrap().gain, Gain::from_db(-6.0));
    assert_eq!(sent(&effects), [(ticket, set_b.clone())]);
    assert_eq!(m.ai_song().undo_count, steps + 1);

    let effects = change(
        &mut m,
        track,
        "d",
        OutputChange {
            on: Some(Some(true)),
            pan: Some(Some(Pan::new(30))),
            ..OutputChange::default()
        },
    );
    no_reload(&effects);
    let both = table(&m, track);
    assert_eq!(sent(&effects), [(ticket, both.clone())]);

    let effects = cmd(&mut m, Command::Undo);
    no_reload(&effects);
    assert_eq!(sent(&effects), [(ticket, set_b.clone())]);
    let effects = cmd(&mut m, Command::Undo);
    no_reload(&effects);
    assert_eq!(sent(&effects), [(ticket, PluginOutputs::default())]);
    let effects = cmd(&mut m, Command::Redo);
    no_reload(&effects);
    assert_eq!(sent(&effects), [(ticket, set_b)]);
    let effects = cmd(&mut m, Command::Redo);
    assert_eq!(sent(&effects), [(ticket, both)]);
}

#[test]
fn the_same_values_again_are_no_step_and_send_nothing() {
    let (mut m, track, _) = with_loaded();
    change(&mut m, track, "b", gain(-6.0));
    let steps = m.ai_song().undo_count;
    let effects = m.handle(Input::Ai(AiAsk::SetPluginOutput {
        track: track.0,
        output: "b".into(),
        change: gain(-6.0),
    }));
    assert_eq!(
        m.take_ai_answer(),
        Some(AiAnswer::Refused(AiRefusal::Unchanged))
    );
    assert!(sent(&effects).is_empty());
    assert_eq!(m.ai_song().undo_count, steps);
}

#[test]
fn a_change_while_the_plugin_loads_goes_once_the_load_answers() {
    let (mut m, track, ticket, _) = adding();
    // Not loaded: any id is taken.
    let effects = change(&mut m, track, "c", off());
    no_reload(&effects);
    assert!(sent(&effects).is_empty(), "{effects:?}");
    let effects = loaded(&mut m, ticket);
    assert_eq!(sent(&effects), [(ticket, table(&m, track))]);
    // From now on each change goes once.
    let effects = change(&mut m, track, "c", gain(-2.0));
    assert_eq!(sent(&effects), [(ticket, table(&m, track))]);
}

#[test]
fn a_load_carries_the_settings_the_song_has_now() {
    let (mut m, track, _) = with_loaded();
    change(&mut m, track, "a", gain(-3.0));
    // Another sample rate: every plugin is loaded again.
    let mut effects = Vec::new();
    m.play_mut().set_plug_rate(44_100, &mut effects);
    assert_eq!(loads(&effects), [(loads(&effects)[0].0, table(&m, track))]);
    // A plugin that never loaded is loaded as the song has it.
    let (mut m, track, ticket, _) = adding();
    change(&mut m, track, "a", gain(-3.0));
    m.handle(Input::PluginUnreadable { ticket });
    let slot = meloet_protocol::PluginSlotRef::Instrument { track: track.0 };
    let effects = cmd(&mut m, Command::ReloadPlugin { slot });
    assert_eq!(loads(&effects)[0].1, table(&m, track));
}

#[test]
fn a_plugin_that_stopped_is_told_too_for_when_it_comes_back() {
    let (mut m, track, ticket) = with_loaded();
    m.handle(Input::PluginStopped { ticket });
    let effects = change(&mut m, track, "b", off());
    assert_eq!(sent(&effects), [(ticket, table(&m, track))]);
}

#[test]
fn a_song_opened_with_settings_loads_with_them() {
    let (mut m, track, _) = with_loaded();
    change(&mut m, track, "b", gain(-9.5));
    change(&mut m, track, "d", off());
    let song = m.ai_song().song;
    // Through the song file and back.
    let song: Song = crate::songfile::decode(&crate::songfile::encode(&song)).unwrap();
    let mut m = rigged();
    cmd(&mut m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    let effects = m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    });
    let opened = table(&m, track);
    assert_eq!(opened.sets.len(), 2, "{opened:?}");
    assert_eq!(loads(&effects).len(), 1, "{effects:?}");
    assert_eq!(loads(&effects)[0].1, opened);
}

#[test]
fn the_export_loads_with_the_settings_of_the_song() {
    let (mut m, track, _) = with_loaded();
    change(&mut m, track, "b", gain(-6.0));
    let doc = m.ai_song().song.doc;
    let told = Told::of(&doc, None, &m.known_plugins(), &[]);
    let plugin = (told.plugins.iter())
        .find(|p| p.place == PluginPlace::Instrument(track))
        .expect("the plugin is loaded for the export");
    assert_eq!(plugin.outputs, table(&m, track));
}

#[test]
fn an_output_the_loaded_plugin_does_not_have_is_refused_with_the_ones_it_has() {
    let (mut m, track, ticket, _) = adding();
    // Kept from before the plugin said its sets.
    change(&mut m, track, "x", gain(-1.0));
    loaded(&mut m, ticket);
    let answer = ask(
        &mut m,
        AiAsk::SetPluginOutput {
            track: track.0,
            output: "nope".into(),
            change: off(),
        },
    );
    let ids = ["a", "b", "c", "d"].map(String::from).to_vec();
    assert_eq!(
        answer,
        AiAnswer::Refused(AiRefusal::NoSuchOutput {
            asked: "nope".into(),
            ids
        })
    );
    // What the song keeps of a set it does not have can still be changed or reset.
    let reset = OutputChange {
        reset: true,
        ..OutputChange::default()
    };
    change(&mut m, track, "x", reset);
    assert_eq!(table(&m, track).get("x"), None);
}

#[test]
fn a_track_whose_instrument_is_no_plugin_has_no_outputs() {
    let mut m = rigged();
    let track = m.ai_song().song.doc.tracks[0].id;
    let answer = ask(
        &mut m,
        AiAsk::SetPluginOutput {
            track: track.0,
            output: "a".into(),
            change: off(),
        },
    );
    assert_eq!(answer, AiAnswer::Refused(AiRefusal::NoSuchPlugin));
    let measure = AiOutputsMeasure {
        span: TickRange::new(0, 7680).unwrap(),
        track: 1,
    };
    assert_eq!(
        m.ai_outputs_job(&measure, 1).unwrap_err(),
        AiRefusal::NoSuchPlugin
    );
}

#[test]
fn the_ai_link_reads_the_sets_once_the_plugin_says_them() {
    let (mut m, track, ticket, _) = adding();
    assert!(m.ai_song().instrument_sets.is_empty());
    loaded(&mut m, ticket);
    let read = m.ai_song();
    assert_eq!(read.instrument_sets.len(), 1);
    assert_eq!(read.instrument_sets[0].0, track);
    assert_eq!(&read.instrument_sets[0].1[..], &sets()[..]);
    // Another instrument: the sets of the plugin that was there are gone.
    let answer = ask(
        &mut m,
        AiAsk::SetInstrument {
            track: track.0,
            instrument: AiInstrument::Builtin(meloet_model::BuiltinInstrument::GrandPiano),
        },
    );
    assert!(matches!(answer, AiAnswer::Done(_)), "{answer:?}");
    assert!(m.ai_song().instrument_sets.is_empty());
}

#[test]
fn the_history_row_names_the_output() {
    let (mut m, track, ticket, _) = adding();
    change(&mut m, track, "c", off());
    let row = |m: &Model| {
        let n = m.ai_song().undo_count;
        m.play().history_rows(n, n).pop().unwrap().name
    };
    let surge = |param: &str| StepName::SetPluginParam {
        plugin: "Surge XT".into(),
        param: Some(param.into()),
    };
    // The plugin has not said its sets: the id.
    assert_eq!(row(&m), surge("c"));
    loaded(&mut m, ticket);
    assert_eq!(row(&m), surge("Snare"));
    change(
        &mut m,
        track,
        "c",
        OutputChange {
            reset: true,
            ..OutputChange::default()
        },
    );
    assert_eq!(row(&m), surge("Snare"));
}

#[test]
fn a_measurement_of_the_outputs_is_one_rendering_of_the_track_alone() {
    let (m, track, _) = with_loaded();
    let number = m.ai_song().song.doc.tracks.len() as u32;
    let bar = 4 * meloet_model::TICKS_PER_QUARTER;
    let measure = AiOutputsMeasure {
        span: TickRange::new(0, 2 * bar).unwrap(),
        track: number,
    };
    let (job, alone) = m.ai_outputs_job(&measure, 7).unwrap();
    assert_eq!(alone.0, number);
    assert_eq!(job.doc.tracks.len(), 1);
    assert_eq!(job.doc.tracks[0].id, track);
    let measuring = job.measure.unwrap();
    assert_eq!(measuring.alone, [track]);
    assert_eq!(measuring.capture, Some(track));
    assert!(measuring.ignore_mute);

    // Four sets at 48 kHz: the plugin host keeps at most 174 s of them.
    let tempo = m.ai_song().song.doc.timeline.tempo.bpm();
    let bars = (200.0 * tempo / 240.0).ceil() as u64;
    let long = AiOutputsMeasure {
        span: TickRange::new(0, bars * bar).unwrap(),
        track: number,
    };
    match m.ai_outputs_job(&long, 8).unwrap_err() {
        AiRefusal::CaptureTooLong { sets, longest_s } => {
            assert_eq!(sets, 4);
            assert!((longest_s - 174.76).abs() < 0.01, "{longest_s}");
        }
        other => panic!("{other:?}"),
    }
}
