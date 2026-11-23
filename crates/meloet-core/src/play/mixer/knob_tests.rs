//! The knobs of a sound that comes with meloet (round 12, E3) in the parameters panel.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{
    BuiltinInstrument, InstrumentRef, Song, SoundKnobs, Track, TrackColor, TrackId,
};
use meloet_protocol::{
    AutoTarget, Command, InstrumentKind, LaneHome, LaneName, SelectedParams, SoundKnob,
    SoundKnobsView, StatusMessage, StepName, StepWhat, TrackRef,
};

use super::Preview;
use crate::play::EngineOp;
use crate::play::tests::{cmd, ready, status};
use crate::state::{Effect, Input, Model};

const PIANO: u32 = 1;
const KIT: u32 = 2;

/// A grand piano on track 1 and a studio kit on track 2.
fn two_sounds(m: &mut Model) {
    let mut song = Song::new_default();
    Arc::make_mut(&mut song.doc.tracks[0]).instrument =
        InstrumentRef::Builtin(BuiltinInstrument::GrandPiano);
    song.doc.tracks.push(Arc::new(Track {
        kind: meloet_model::TrackKind::Keys,
        id: TrackId(KIT),
        name: "ドラム".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::Builtin(BuiltinInstrument::StudioKit),
        channel: Default::default(),
        sends: [meloet_model::Gain::SILENT; 2],
        muted: false,
        clips: Vec::new(),
        automation: Vec::new(),
        arp: Default::default(),
        knobs: Default::default(),
    }));
    cmd(m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/つまみ.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    });
}

fn knob(
    track: u32,
    instrument: InstrumentKind,
    knob: SoundKnob,
    value: f32,
    commit: bool,
) -> Command {
    Command::SetSoundKnob {
        track,
        instrument,
        knob,
        value,
        commit,
    }
}

fn piano(k: SoundKnob, value: f32, commit: bool) -> Command {
    knob(PIANO, InstrumentKind::GrandPiano, k, value, commit)
}

fn shown(m: &Model) -> Option<SoundKnobsView> {
    match m.view().params.selected {
        Some(SelectedParams::Sound(s)) => s.knobs,
        _ => None,
    }
}

fn knob_ops(effects: &[Effect]) -> Vec<(usize, [f32; 4])> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::Knobs { seat, knobs }) => Some((*seat, knobs.values())),
            _ => None,
        })
        .collect()
}

fn undo_count(m: &Model) -> u32 {
    m.view().song.undo_count
}

#[test]
fn the_panel_shows_the_knobs_of_a_sound_and_a_row_can_be_chosen() {
    let m = &mut ready();
    two_sounds(m);
    cmd(m, Command::SelectInstrument { track: PIANO });
    assert!(
        !matches!(status(m), Some(StatusMessage::NoParams { .. })),
        "a sound with knobs has parameters"
    );
    assert_eq!(
        shown(m),
        Some(SoundKnobsView {
            values: [50.0, 50.0, 50.0, 0.0],
            usable: [true; 4],
            row: None,
        })
    );
    cmd(m, Command::SelectParam { param: Some(3) });
    assert_eq!(shown(m).unwrap().row, Some(3));
    assert_eq!(
        status(m),
        Some(StatusMessage::SoundKnobSelected {
            loop_bars: None,
            track: TrackRef {
                number: 1,
                name: m.view().song.tracks[0].name.clone(),
            },
            knob: SoundKnob::Vibrato,
        })
    );
    cmd(m, Command::SelectParam { param: Some(4) });
    assert_eq!(shown(m).unwrap().row, None);

    cmd(m, Command::SelectInstrument { track: KIT });
    let kit = shown(m).unwrap();
    assert_eq!(kit.usable, [true, false, true, false], "a drum kit");
    cmd(m, Command::SelectParam { param: Some(1) });
    assert_eq!(shown(m).unwrap().row, None, "a row that is not used");
}

#[test]
fn a_drag_is_heard_and_is_one_step_when_let_go_and_esc_goes_back() {
    let m = &mut ready();
    two_sounds(m);
    cmd(m, Command::SelectInstrument { track: PIANO });
    let seat = m.play().track_seats()[0].unwrap();
    let before = m.view();
    for value in [40.0, 30.0, 25.0] {
        let effects = cmd(m, piano(SoundKnob::Brightness, value, false));
        assert_eq!(effects.len(), 1, "only the engine hears it: {effects:?}");
        assert_eq!(knob_ops(&effects), [(seat, [value, 50.0, 50.0, 0.0])]);
    }
    assert_eq!(
        m.play().mix_previews(),
        [Preview::Knob {
            track: TrackId(PIANO),
            knob: meloet_model::SoundKnob::Brightness,
            value: 25.0
        }]
    );
    assert_eq!(m.view().song, before.song, "the song is as it was");
    assert_eq!(shown(m).unwrap().values[0], 50.0);

    cmd(m, piano(SoundKnob::Brightness, 25.0, true));
    assert!(m.play().mix_previews().is_empty());
    assert_eq!(shown(m).unwrap().values[0], 25.0);
    assert_eq!(undo_count(m), 1);
    let rows = m.history_rows(1, 100);
    assert_eq!(
        (&rows[0].name, &rows[0].what),
        (
            &StepName::SetSoundKnob,
            &StepWhat::SoundKnob {
                track: TrackRef {
                    number: 1,
                    name: m.view().song.tracks[0].name.clone(),
                },
                knob: SoundKnob::Brightness,
                from: 50.0,
                to: 25.0,
            }
        )
    );

    let effects = cmd(m, Command::Undo);
    assert_eq!(knob_ops(&effects), [(seat, [50.0, 50.0, 50.0, 0.0])]);
    assert_eq!(shown(m).unwrap().values[0], 50.0);
    cmd(m, Command::Redo);
    assert_eq!(shown(m).unwrap().values[0], 25.0);

    cmd(m, piano(SoundKnob::Vibrato, 70.0, false));
    let effects = cmd(m, Command::CancelMixPreview);
    assert_eq!(knob_ops(&effects), [(seat, [25.0, 50.0, 50.0, 0.0])]);
    assert_eq!(undo_count(m), 1, "Esc is no step");

    cmd(m, piano(SoundKnob::Attack, 140.0, true));
    assert_eq!(
        shown(m).unwrap().values[1],
        100.0,
        "a value typed is put into range"
    );
    assert_eq!(undo_count(m), 2);
}

#[test]
fn a_command_for_another_sound_or_a_knob_it_does_not_use_does_nothing() {
    let m = &mut ready();
    two_sounds(m);
    let before = m.view();
    let effects = cmd(
        m,
        knob(
            PIANO,
            InstrumentKind::Strings,
            SoundKnob::Release,
            10.0,
            true,
        ),
    );
    assert!(knob_ops(&effects).is_empty());
    let kit = |k, value| knob(KIT, InstrumentKind::StudioKit, k, value, true);
    cmd(m, kit(SoundKnob::Attack, 10.0));
    cmd(m, kit(SoundKnob::Vibrato, 10.0));
    cmd(m, piano(SoundKnob::Release, f32::NAN, true));
    cmd(
        m,
        knob(
            9,
            InstrumentKind::GrandPiano,
            SoundKnob::Release,
            10.0,
            true,
        ),
    );
    assert_eq!(m.view().song, before.song);
    cmd(m, kit(SoundKnob::Release, 10.0));
    assert_eq!(undo_count(m), 1, "the kit's 「のび」 is used");
}

#[test]
fn a_lane_moves_a_knob_and_rests_when_the_knob_is_set_by_hand() {
    let m = &mut ready();
    two_sounds(m);
    let home = LaneHome::Track { track: PIANO };
    let target = AutoTarget::Instrument { param: 3 };
    cmd(m, Command::AddLane { home, target });
    let lane = &m.view().song.tracks[0].lanes[0];
    assert_eq!(
        lane.name,
        LaneName::Knob {
            knob: SoundKnob::Vibrato
        }
    );
    let lane_id = lane.lane_id;
    cmd(
        m,
        Command::AddAutoPoint {
            lane: lane_id,
            tick: 0,
            value: Some(0.2),
        },
    );
    let lane_id = m.view().song.tracks[0].lanes[0].lane_id;
    cmd(
        m,
        Command::AddAutoPoint {
            lane: lane_id,
            tick: 7680,
            value: Some(0.8),
        },
    );
    let steps = undo_count(m);
    let effects = cmd(m, piano(SoundKnob::Vibrato, 30.0, false));
    let held = effects.iter().any(|e| {
        matches!(
            e,
            Effect::Engine(EngineOp::HoldLane {
                on: true,
                target: meloet_model::AutoTarget::Instrument { param: 3 },
                ..
            })
        )
    });
    assert!(held, "the hand holds the lane: {effects:?}");
    cmd(m, piano(SoundKnob::Vibrato, 30.0, true));
    assert_eq!(
        undo_count(m),
        steps + 1,
        "the value and the rest are one step"
    );
    assert!(m.view().song.tracks[0].lanes[0].resting);
    assert_eq!(
        m.play().doc().tracks[0].knobs,
        SoundKnobs::default()
            .with(meloet_model::SoundKnob::Vibrato, 30.0)
            .unwrap()
    );
}

#[test]
fn a_copy_of_a_track_sounds_with_the_same_knobs() {
    let m = &mut ready();
    two_sounds(m);
    cmd(m, piano(SoundKnob::Brightness, 20.0, true));
    cmd(m, piano(SoundKnob::Vibrato, 65.0, true));
    let effects = cmd(m, Command::DuplicateTrack { track: PIANO });
    let seats = m.play().track_seats();
    let copy = seats[1].expect("the copy is next to the piano, on a seat");
    assert_ne!(Some(copy), seats[0]);
    assert_eq!(knob_ops(&effects), [(copy, [20.0, 50.0, 50.0, 65.0])]);
    let made = effects.iter().any(|e| {
        matches!(e, Effect::Engine(EngineOp::Instrument { seat, instrument })
            if *seat == copy
                && *instrument == InstrumentRef::Builtin(BuiltinInstrument::GrandPiano))
    });
    assert!(made, "the same sound on its seat: {effects:?}");
}
