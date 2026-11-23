//! 「トラックを複製」 (round 12, A3) through the rules: one step, right below the track and
//! selected, its plugins started from the ones that sound, the most tracks a song has, and
//! what is not copied.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::automation as auto;
use meloet_model::{
    AutoTarget, Gain, InstrumentRef, NewTake, Note, PluginPlace, PluginRef, Song, StripId, Track,
    TrackColor, TrackId, edit, mixer,
};
use meloet_protocol::{
    AutoTarget as TargetView, Command, LaneRef, PluginState, SoundView, StatusMessage,
    TrackKindView,
};

use crate::catalog::tests::{COMP_LV2, SURGE_VST3};
use crate::play::tests::{cmd, ready, started, status};
use crate::play::{EngineOp, MAX_TRACKS, PlugState};
use crate::state::added_tests::{loads, rigged};
use crate::state::{Effect, Input, Model, PluginAsk, PluginSaid, StateAsk};

const T1: TrackId = TrackId(1);
const Q: u64 = meloet_model::TICKS_PER_QUARTER;

fn opened(m: &mut Model, song: Song) -> Vec<Effect> {
    cmd(m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    })
}

fn ops(effects: &[Effect]) -> Vec<EngineOp> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op.clone()),
            _ => None,
        })
        .collect()
}

fn ids(m: &Model) -> Vec<u32> {
    m.view().song.tracks.iter().map(|t| t.id).collect()
}

fn names(m: &Model) -> Vec<String> {
    m.view().song.tracks.into_iter().map(|t| t.name).collect()
}

fn with_track(song: &mut Song, id: u32, name: &str, instrument: InstrumentRef) {
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(id),
        name: name.to_owned(),
        color: TrackColor::for_index(id as usize - 1),
        instrument,
        ..Track::clone(&song.doc.tracks[0])
    }));
}

/// The piano with a clip, its fader at −6 dB, a reverb in slot 0 and a lane for the reverb's
/// first parameter, and below it "ベース".
fn piano_and_bass() -> Song {
    let mut song = crate::play::tests::new_song_without_pack();
    let doc = &mut song.doc;
    let take = NewTake {
        origin: 0,
        length: 4 * Q,
        notes: vec![Note {
            start: 0,
            length: Q,
            key: 60,
            velocity: 100,
            from: None,
        }],
        controls: Vec::new(),
    };
    *doc = edit::add_recording(doc, T1, vec![take], 0).unwrap().0.doc;
    let strip = StripId::Track(T1);
    *doc = mixer::set_gain(doc, strip, Gain::from_db(-6.0))
        .unwrap()
        .doc;
    *doc = mixer::add_effect(doc, strip, 0, meloet_model::effects::EffectKind::Reverb)
        .unwrap()
        .doc;
    let reverb = doc.insert(strip, 0).unwrap().id;
    let target = AutoTarget::Insert {
        insert: reverb,
        param: 0,
    };
    *doc = auto::add_lane(doc, T1, target, false).unwrap().0.doc;
    with_track(&mut song, 2, "ベース", InstrumentRef::SINE);
    song
}

#[test]
fn a_copy_is_one_step_right_below_the_track_and_selected() {
    let m = &mut ready();
    opened(m, piano_and_bass());
    let view = m.view();
    let reverb = view.song.tracks[0].slots[0].clone().unwrap().id;
    let on_reverb = TargetView::Insert {
        insert: reverb,
        param: 0,
    };
    cmd(
        m,
        Command::FoldLane {
            track: 1,
            target: on_reverb,
            folded: true,
        },
    );
    cmd(
        m,
        Command::SelectTrack {
            track: 2,
            from_arm: false,
        },
    );

    let effects = cmd(m, Command::DuplicateTrack { track: 1 });
    let view = m.view();
    let copy = view.song.tracks[1].clone();
    assert_eq!(names(m), ["シンセ", "シンセ 2", "ベース"]);
    assert_eq!(ids(m), [1, copy.id, 2]);
    assert_eq!(copy.number, 2);
    assert_eq!(
        copy.color, view.song.tracks[0].color,
        "the color of the track"
    );
    assert_eq!(view.song.play_target, Some(copy.id), "selected");
    assert_eq!(view.song.undo_count, 1);
    assert!(!matches!(status(m), Some(StatusMessage::TooManyTracks)));
    assert_eq!(copy.gain_db, Some(-6.0));
    let source = &view.song.tracks[0];
    let insert = copy.slots[0].clone().unwrap();
    assert_ne!(insert.id, reverb);
    assert_eq!(insert.effect, source.slots[0].clone().unwrap().effect);
    let copied = TargetView::Insert {
        insert: insert.id,
        param: 0,
    };
    assert_eq!(copy.lanes.len(), 1);
    assert_eq!(
        copy.lanes[0].target, copied,
        "the lane moves the copy's reverb"
    );
    assert_ne!(copy.lanes[0].lane_id, source.lanes[0].lane_id);
    assert_eq!(copy.clips.len(), 1);
    assert_ne!(copy.clips[0].id, source.clips[0].id);
    assert_eq!(
        copy.clips[0].take_id, source.clips[0].take_id,
        "the take is shared"
    );
    assert!(view.automation.folded.contains(&LaneRef {
        track: copy.id,
        target: copied,
    }));

    // It has a seat of its own, with its instrument, its notes and its strip.
    let seat = m.track_seats()[1].unwrap();
    assert_eq!(m.track_seats(), [Some(0), Some(seat), Some(1)]);
    let told = ops(&effects);
    assert!(
        told.iter()
            .any(|op| matches!(op, EngineOp::Instrument { seat: s, .. } if *s == seat))
    );
    assert!(told.iter().any(|op| matches!(op, EngineOp::Sequence { track, events } if *track == seat && !events.is_empty())));
    assert!(told.iter().any(|op| matches!(op, EngineOp::Strip { strip: crate::play::StripSeat::Track(s), .. } if *s == seat)));

    // Undo takes it away, seat and all; redo brings it back as it was.
    let effects = cmd(m, Command::Undo);
    assert_eq!(ids(m), [1, 2]);
    assert!(matches!(
        status(m),
        Some(StatusMessage::Undone {
            name: meloet_protocol::StepName::DuplicateTrack,
            what: meloet_protocol::StepWhat::Track { track },
        }) if track.name == "シンセ 2"
    ));
    assert!(ops(&effects).contains(&EngineOp::ClearSeat { seat }));
    assert!(
        m.view()
            .automation
            .folded
            .iter()
            .all(|l| l.track != copy.id)
    );
    cmd(m, Command::Redo);
    assert_eq!(ids(m), [1, copy.id, 2]);
    assert_eq!(
        m.view().song.tracks[1].slots[0].clone().unwrap().id,
        insert.id
    );

    // A track that is not there.
    cmd(m, Command::DuplicateTrack { track: 99 });
    assert_eq!(m.view().song.undo_count, 1);
    assert_eq!(m.view().song.tracks.len(), 3);
}

#[test]
fn the_plugins_of_the_copy_start_from_the_ones_that_sound() {
    let mut m = rigged();
    let mut song = crate::play::tests::new_song_without_pack();
    let surge = PluginRef::new(
        SURGE_VST3,
        "Surge XT",
        "Surge Synth Team",
        "/p/Surge XT.vst3",
    )
    .with_state(Arc::from([1u8, 2, 3]));
    with_track(
        &mut song,
        2,
        "Surge XT",
        InstrumentRef::Plugin(Arc::new(surge)),
    );
    let comp = PluginRef::new(COMP_LV2, "ZamComp", "Damien Zammit", "/p/ZamComp.lv2");
    let strip = StripId::Track(TrackId(2));
    song.doc = mixer::add_plugin_effect(&song.doc, strip, 1, Arc::new(comp))
        .unwrap()
        .doc;
    let asked = loads(&opened(&mut m, song));
    assert_eq!(asked.len(), 2);
    let ticket = |key: &str| asked.iter().find(|l| l.1 == key).unwrap().0;
    let (surge, comp) = (ticket(SURGE_VST3), ticket(COMP_LV2));
    for ticket in [surge, comp] {
        m.handle(Input::PluginLoaded { ticket });
    }

    // They are asked for the states they have now; the copy waits for the answers.
    let effects = cmd(&mut m, Command::DuplicateTrack { track: 2 });
    assert!(loads(&effects).is_empty());
    let asks = copy_asks(&effects);
    let serial = asks[0].1;
    assert_eq!(asks, [(surge, serial), (comp, serial)]);
    assert_eq!(ids(&m), [1, 2]);
    assert!(copy_asks(&cmd(&mut m, Command::DuplicateTrack { track: 2 })).is_empty());
    assert!(said(&mut m, surge, serial, Some(&[9, 9])).is_empty());
    assert_eq!(ids(&m), [1, 2], "until every one has answered");

    // Surge XT has another state than the song: the copy has that one, in the song too. Of
    // the compressor, which the song has "as it starts", nothing tells whether it is still as
    // it started: the copy has what it said.
    let effects = said(&mut m, comp, serial, Some(&[7]));
    let asked = loads(&effects);
    let states: Vec<(String, PlugState)> =
        asked.iter().map(|l| (l.1.clone(), l.2.clone())).collect();
    assert_eq!(
        states,
        [
            (SURGE_VST3.to_owned(), PlugState::Bytes(Arc::from([9u8, 9]))),
            (COMP_LV2.to_owned(), PlugState::Bytes(Arc::from([7u8])))
        ]
    );
    let copy = m.view().song.tracks[2].id;
    assert_eq!(m.view().song.undo_count, 1);
    assert_eq!(m.view().song.play_target, Some(copy));
    let in_song = |m: &Model, place| m.play().song().doc.plugin(place).unwrap().state.clone();
    let copy_id = TrackId(copy);
    assert_eq!(&*in_song(&m, PluginPlace::Instrument(copy_id)), [9, 9]);
    let comp_place = PluginPlace::Insert(StripId::Track(copy_id), 1);
    assert_eq!(&*in_song(&m, comp_place), [7]);
    assert_eq!(
        &*in_song(&m, PluginPlace::Instrument(TrackId(2))),
        [1, 2, 3]
    );
    let state = |m: &Model| match &m.view().song.tracks[2].sound {
        SoundView::Plugin(p) => p.state,
        other => panic!("{other:?}"),
    };
    assert_eq!(state(&m), PluginState::Loading);
    for (ticket, ..) in &asked {
        m.handle(Input::PluginLoaded { ticket: *ticket });
    }
    assert_eq!(state(&m), PluginState::Ready);
    assert_eq!(m.plugin_tickets().len(), 4);

    // Undo lets go of them as a deleted track does; redo loads them as the song has them.
    cmd(&mut m, Command::Undo);
    let mut left: Vec<u64> = m.plugin_tickets().iter().map(|(_, t)| *t).collect();
    left.sort_unstable();
    assert_eq!(left, [surge, comp]);
    let effects = cmd(&mut m, Command::Redo);
    let states: Vec<PlugState> = loads(&effects).into_iter().map(|l| l.2).collect();
    assert_eq!(
        states,
        [
            PlugState::Bytes(Arc::from([9u8, 9])),
            PlugState::Bytes(Arc::from([7u8]))
        ]
    );
    assert_eq!(m.view().song.tracks[2].id, copy);
}

/// The asks for the states of a track to copy: ticket and the number of the duplicate.
fn copy_asks(effects: &[Effect]) -> Vec<(u64, u64)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::AskPlugin {
                ticket,
                ask: PluginAsk::State(StateAsk::Copy(serial)),
            } => Some((*ticket, *serial)),
            _ => None,
        })
        .collect()
}

fn said(m: &mut Model, ticket: u64, serial: u64, state: Option<&[u8]>) -> Vec<Effect> {
    let said = PluginSaid::State {
        ask: StateAsk::Copy(serial),
        state: state.map(Arc::from),
    };
    m.handle(Input::PluginSaid { ticket, said })
}

/// A song whose track 2 sounds with Surge XT (with the song's state 1 2 3), loaded. Returns
/// its ticket.
fn surge_loaded(m: &mut Model) -> u64 {
    let mut song = crate::play::tests::new_song_without_pack();
    let surge = PluginRef::new(SURGE_VST3, "Surge XT", "Surge Synth Team", "/p/Surge.vst3")
        .with_state(Arc::from([1u8, 2, 3]));
    let surge = InstrumentRef::Plugin(Arc::new(surge));
    with_track(&mut song, 2, "Surge XT", surge);
    let ticket = loads(&opened(m, song))[0].0;
    m.handle(Input::PluginLoaded { ticket });
    ticket
}

/// Answers every ask for a state in `effects` with `state`.
fn states_said(m: &mut Model, effects: &[Effect], state: &[u8]) -> Vec<Effect> {
    let mut out = Vec::new();
    for e in effects {
        if let Effect::AskPlugin {
            ticket,
            ask: PluginAsk::State(ask),
        } = e
        {
            let said = PluginSaid::State {
                ask: *ask,
                state: Some(Arc::from(state)),
            };
            out.extend(m.handle(Input::PluginSaid {
                ticket: *ticket,
                said,
            }));
        }
    }
    out
}

#[test]
fn a_plugin_still_as_it_started_is_as_it_starts_on_the_copy_too() {
    let mut m = rigged();
    let mut song = crate::play::tests::new_song_without_pack();
    let surge = PluginRef::new(SURGE_VST3, "Surge XT", "Surge Synth Team", "/p/Surge.vst3");
    with_track(
        &mut song,
        2,
        "Surge XT",
        InstrumentRef::Plugin(Arc::new(surge)),
    );
    let ticket = loads(&opened(&mut m, song))[0].0;
    m.handle(Input::PluginLoaded { ticket });
    // A visit to its window, with nothing changed: what it is as it starts is known now.
    let window = |said| Input::PluginSaid { ticket, said };
    let entered = m.handle(window(PluginSaid::WindowEntered));
    states_said(&mut m, &entered, &[5]);
    let left = m.handle(window(PluginSaid::WindowLeft));
    states_said(&mut m, &left, &[5]);
    assert_eq!(m.view().song.undo_count, 0);

    let serial = copy_asks(&cmd(&mut m, Command::DuplicateTrack { track: 2 }))[0].1;
    let states: Vec<PlugState> = (loads(&said(&mut m, ticket, serial, Some(&[5]))).into_iter())
        .map(|l| l.2)
        .collect();
    assert_eq!(states, [PlugState::Fresh]);
    let copy = TrackId(m.view().song.tracks[2].id);
    let doc = &m.play().song().doc;
    assert!(
        doc.plugin(PluginPlace::Instrument(copy))
            .unwrap()
            .state
            .is_empty()
    );
}

#[test]
fn a_plugin_with_no_answer_gives_the_copy_the_state_of_the_song() {
    let mut m = rigged();
    let surge = surge_loaded(&mut m);
    let effects = cmd(&mut m, Command::DuplicateTrack { track: 2 });
    let serial = copy_asks(&effects)[0].1;
    // Its process stopped meanwhile.
    let states: Vec<PlugState> = (loads(&said(&mut m, surge, serial, None)).into_iter())
        .map(|l| l.2)
        .collect();
    assert_eq!(states, [PlugState::Bytes(Arc::from([1u8, 2, 3]))]);
    assert_eq!(ids(&m).len(), 3);
}

#[test]
fn a_step_taken_back_while_the_states_are_asked_for_leaves_no_copy() {
    let mut m = rigged();
    let surge = surge_loaded(&mut m);
    let fader = Command::SetGain {
        strip: meloet_protocol::StripRef::Track { id: 1 },
        db: Some(-3.0),
        commit: true,
    };
    cmd(&mut m, fader);
    let serial = copy_asks(&cmd(&mut m, Command::DuplicateTrack { track: 2 }))[0].1;
    cmd(&mut m, Command::Undo);
    assert!(said(&mut m, surge, serial, Some(&[9])).is_empty());
    assert_eq!(ids(&m), [1, 2]);
    assert_eq!(
        m.view().song.redo_count,
        1,
        "the step undone can be done again"
    );
    // Another song while the states are asked for: no copy either.
    let serial = copy_asks(&cmd(&mut m, Command::DuplicateTrack { track: 2 }))[0].1;
    cmd(&mut m, Command::NewSong);
    assert!(said(&mut m, surge, serial, Some(&[9])).is_empty());
    assert_eq!(m.view().song.tracks.len(), 1);
    assert_eq!(m.view().song.undo_count, 0);
}

#[test]
fn a_plugin_that_does_not_sound_yet_gives_the_copy_the_state_of_the_song() {
    let mut m = rigged();
    let mut song = crate::play::tests::new_song_without_pack();
    let surge = PluginRef::new(
        SURGE_VST3,
        "Surge XT",
        "Surge Synth Team",
        "/p/Surge XT.vst3",
    )
    .with_state(Arc::from([4u8, 5]));
    with_track(
        &mut song,
        2,
        "Surge XT",
        InstrumentRef::Plugin(Arc::new(surge)),
    );
    opened(&mut m, song);
    let effects = cmd(&mut m, Command::DuplicateTrack { track: 2 });
    let states: Vec<PlugState> = loads(&effects).into_iter().map(|l| l.2).collect();
    assert_eq!(states, [PlugState::Bytes(Arc::from([4u8, 5]))]);
}

#[test]
fn the_copy_of_a_vocal_track_has_its_input_and_neither_its_solo_nor_its_monitor() {
    let m = &mut ready();
    cmd(m, Command::AddVocalTrack);
    let vocal = m.view().song.tracks[1].id;
    cmd(
        m,
        Command::SetVocalInput {
            track: vocal,
            input: meloet_protocol::VocalInput::Stereo12,
        },
    );
    cmd(m, Command::ToggleMonitor { track: vocal });
    cmd(m, Command::ToggleSolo { track: vocal });
    // Its ● off: the copy has it all the same, as the track that is selected (spec 3, 24-C).
    cmd(m, Command::ToggleVocalArm { track: vocal });

    cmd(m, Command::DuplicateTrack { track: vocal });
    let view = m.view();
    let (source, copy) = (&view.song.tracks[1], &view.song.tracks[2]);
    assert_eq!(names(m), ["ピアノ", "歌", "歌 2"]);
    assert_eq!(view.song.play_target, Some(copy.id));
    assert_eq!(
        view.song.keys_target,
        Some(1),
        "the keys stay with the keys track"
    );
    let kind = |t: &meloet_protocol::TrackView| match t.kind {
        TrackKindView::Vocal {
            input,
            armed,
            monitor,
            ..
        } => (input, armed, monitor),
        TrackKindView::Keys => panic!("a keys track"),
    };
    let stereo = meloet_protocol::VocalInput::Stereo12;
    assert_eq!(kind(source), (stereo, false, true));
    assert_eq!(kind(copy), (stereo, true, false));
    assert!(source.soloed && !copy.soloed);
}

#[test]
fn a_song_has_at_most_sixty_four_tracks() {
    let m = &mut ready();
    let mut song = crate::play::tests::new_song_without_pack();
    for id in 2..=MAX_TRACKS as u32 {
        with_track(
            &mut song,
            id,
            &format!("トラック {id}"),
            InstrumentRef::SINE,
        );
    }
    opened(m, song);
    let effects = cmd(m, Command::DuplicateTrack { track: 1 });
    assert_eq!(status(m), Some(StatusMessage::TooManyTracks));
    assert_eq!(m.view().song.tracks.len(), MAX_TRACKS);
    assert_eq!(m.view().song.undo_count, 0);
    assert!(ops(&effects).is_empty());
    // One less, and there is room.
    cmd(m, Command::DeleteTrack { track: 64 });
    cmd(m, Command::DuplicateTrack { track: 1 });
    assert_eq!(m.view().song.tracks.len(), MAX_TRACKS);
    assert_eq!(m.view().song.tracks[1].name, "シンセ 2");
}

#[test]
fn nothing_is_copied_while_recording() {
    let m = &mut ready();
    cmd(m, Command::SetCountIn { bars: 0 });
    cmd(m, Command::Record);
    started(m);
    cmd(m, Command::DuplicateTrack { track: 1 });
    assert_eq!(ids(m), [1]);
    assert_eq!(m.view().song.undo_count, 0);
}
