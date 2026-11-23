use std::path::PathBuf;

use meloet_model::effects::{EffectKind as ModelEffect, compressor, reverb};
use meloet_model::mixer::INSERT_MIX;
use meloet_model::{Song, StepKind, StripId, TrackId, mixer, presets};
use meloet_protocol::{
    AutoTarget, Command, EffectKind, InsertRef, PresetRef, SelectedParams, StatusMessage, StripRef,
};

use crate::play::tests::{cmd, ready, status};
use crate::state::{Effect, Input, Model};

const T1: InsertRef = InsertRef {
    strip: StripRef::Track { id: 1 },
    slot: 0,
};

/// A song with `kind` in the first slot of track 1, opened, and the insert selected.
fn with_effect(m: &mut Model, kind: ModelEffect) -> u32 {
    let mut song = Song::new_default();
    let strip = StripId::Track(TrackId(1));
    song.doc = mixer::add_effect(&song.doc, strip, 0, kind).unwrap().doc;
    let insert = song.doc.insert(strip, 0).unwrap().id.0;
    cmd(m, Command::OpenSong);
    let path = PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song),
        from_previous: false,
    });
    cmd(m, Command::SelectInsert { insert: Some(T1) });
    insert
}

fn values(m: &Model) -> Vec<f32> {
    let strip = StripId::Track(TrackId(1));
    (m.play().song().doc.insert(strip, 0))
        .unwrap()
        .values()
        .to_vec()
}

fn shown(m: &Model) -> Option<PresetRef> {
    match m.view().params.selected {
        Some(SelectedParams::Builtin(sel)) => sel.preset,
        other => panic!("{other:?}"),
    }
}

fn factory(id: &str) -> PresetRef {
    PresetRef::Factory { id: id.into() }
}

fn user(name: &str) -> PresetRef {
    PresetRef::User { name: name.into() }
}

fn apply(m: &mut Model, preset: PresetRef) -> Vec<Effect> {
    cmd(m, Command::ApplyPreset { insert: T1, preset })
}

fn saved(effects: &[Effect]) -> Option<crate::presets::UserPresets> {
    effects.iter().find_map(|e| match e {
        Effect::SavePresets(p) => Some((**p).clone()),
        _ => None,
    })
}

#[test]
fn a_factory_preset_sets_every_value_in_one_step_and_undo_takes_it_back() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Compressor);
    let before = values(m);
    assert_eq!(shown(m), None);
    let steps = m.play().undo_count();

    apply(m, factory("vocalEven"));
    let want = presets::find(ModelEffect::Compressor, "vocalEven")
        .unwrap()
        .values_of(ModelEffect::Compressor);
    assert_eq!(values(m), want);
    assert_eq!(m.play().undo_count(), steps + 1);
    assert_eq!(
        m.play().steps().last(),
        Some(&StepKind::ApplyPreset {
            strip: StripId::Track(TrackId(1)),
            slot: 0,
            kind: ModelEffect::Compressor
        })
    );
    assert_eq!(shown(m), Some(factory("vocalEven")));

    // The same again: no step.
    apply(m, factory("vocalEven"));
    assert_eq!(m.play().undo_count(), steps + 1);

    // One value moved by hand: the name goes.
    cmd(
        m,
        Command::SetParam {
            insert: T1,
            effect: EffectKind::Compressor,
            param: compressor::RATIO,
            value: 7.0,
            commit: true,
        },
    );
    assert_eq!(shown(m), None);

    cmd(m, Command::Undo);
    assert_eq!(shown(m), Some(factory("vocalEven")));
    cmd(m, Command::Undo);
    assert_eq!(values(m), before);
    assert_eq!(shown(m), None);
    cmd(m, Command::Redo);
    assert_eq!(values(m), want);
}

#[test]
fn a_preset_of_another_effect_an_unknown_id_or_an_empty_slot_does_nothing() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Compressor);
    let steps = m.play().undo_count();
    for preset in [factory("hall"), factory("nothing"), user("nobody")] {
        apply(m, preset);
    }
    let empty = InsertRef { slot: 3, ..T1 };
    cmd(
        m,
        Command::ApplyPreset {
            insert: empty,
            preset: factory("vocalEven"),
        },
    );
    assert_eq!(m.play().undo_count(), steps);
}

#[test]
fn the_mix_of_a_reverb_stays_where_the_strip_put_it() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Reverb);
    assert_eq!(values(m)[reverb::MIX as usize], INSERT_MIX);
    apply(m, factory("hall"));
    let now = values(m);
    assert_eq!(now[reverb::MIX as usize], INSERT_MIX);
    assert_eq!(now[reverb::DECAY as usize], 3.2);
    assert_eq!(shown(m), Some(factory("hall")));
}

#[test]
fn a_parameter_a_lane_moves_stays_and_the_status_line_says_how_many() {
    let m = &mut ready();
    let insert = with_effect(m, ModelEffect::Compressor);
    let target = AutoTarget::Insert {
        insert,
        param: compressor::THRESHOLD,
    };
    cmd(m, Command::AddLane { track: 1, target });
    let lane = (m.view().song.tracks[0].lanes.iter())
        .find(|l| l.target == target)
        .unwrap()
        .lane_id;
    cmd(
        m,
        Command::AddAutoPoint {
            lane,
            tick: 0,
            value: Some(0.5),
        },
    );
    let threshold = values(m)[compressor::THRESHOLD as usize];
    let steps = m.play().undo_count();

    apply(m, factory("drumPunch"));
    assert_eq!(m.play().undo_count(), steps + 1);
    assert_eq!(status(m), Some(StatusMessage::PresetLanesKept { count: 1 }));
    let now = values(m);
    assert_eq!(now[compressor::THRESHOLD as usize], threshold);
    assert_eq!(now[compressor::RATIO as usize], 4.0);
    assert_eq!(now[compressor::ATTACK as usize], 20.0);
    assert_eq!(shown(m), None, "the threshold is not the preset's");

    // Moved by hand, the lane rests: the preset sets it now.
    cmd(
        m,
        Command::SetParam {
            insert: T1,
            effect: EffectKind::Compressor,
            param: compressor::THRESHOLD,
            value: -40.0,
            commit: true,
        },
    );
    let lanes = m.view().song.tracks[0].lanes.clone();
    assert!(lanes.iter().any(|l| l.target == target && l.resting));
    apply(m, factory("drumPunch"));
    assert_eq!(values(m)[compressor::THRESHOLD as usize], -15.0);
    assert_eq!(shown(m), Some(factory("drumPunch")));
}

#[test]
fn not_while_counting_in() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Limiter);
    let before = values(m);
    cmd(m, Command::Record);
    apply(m, factory("finish"));
    assert_eq!(values(m), before);
}

#[test]
fn keeping_writing_over_and_removing_a_preset_of_the_pc() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Gate);
    let steps = m.play().undo_count();
    assert_eq!(m.view().presets.user, []);

    let kept = saved(&cmd(
        m,
        Command::SavePreset {
            insert: T1,
            name: "  静かに  ".into(),
        },
    ))
    .expect("written");
    assert_eq!(kept.of(ModelEffect::Gate)[0].name, "静かに");
    assert_eq!(m.play().undo_count(), steps, "no step");
    let view = m.view().presets;
    assert_eq!(view.user.len(), 1);
    assert_eq!(
        (view.user[0].effect, &view.user[0].names[..]),
        (EffectKind::Gate, &["静かに".to_owned()][..])
    );
    assert_eq!(shown(m), Some(user("静かに")));

    // A name of nothing is not kept.
    let none = cmd(
        m,
        Command::SavePreset {
            insert: T1,
            name: "   ".into(),
        },
    );
    assert_eq!(saved(&none), None);

    // Another value, kept under the same name: written over, and said.
    cmd(
        m,
        Command::SetParam {
            insert: T1,
            effect: EffectKind::Gate,
            param: 0,
            value: -30.0,
            commit: true,
        },
    );
    assert_eq!(shown(m), None);
    let over = cmd(
        m,
        Command::SavePreset {
            insert: T1,
            name: "静かに".into(),
        },
    );
    assert!(saved(&over).is_some());
    assert_eq!(
        status(m),
        Some(StatusMessage::PresetOverwritten {
            name: "静かに".into()
        })
    );
    assert_eq!(shown(m), Some(user("静かに")));

    // Back to the start, then the preset again.
    cmd(m, Command::Undo);
    assert_eq!(shown(m), None);
    apply(m, user("静かに"));
    assert_eq!(values(m)[0], -30.0);

    let gone = cmd(
        m,
        Command::DeletePreset {
            effect: EffectKind::Gate,
            name: "静かに".into(),
        },
    );
    assert_eq!(saved(&gone).map(|p| p.of(ModelEffect::Gate).len()), Some(0));
    assert_eq!(m.view().presets.user, []);
    assert_eq!(shown(m), None);
    let again = cmd(
        m,
        Command::DeletePreset {
            effect: EffectKind::Gate,
            name: "静かに".into(),
        },
    );
    assert_eq!(saved(&again), None);
}

#[test]
fn past_a_hundred_nothing_is_kept_and_the_status_line_says_so() {
    let m = &mut ready();
    with_effect(m, ModelEffect::Chorus);
    for n in 0..meloet_protocol::PRESETS_PER_EFFECT {
        let name = format!("p{n}");
        cmd(m, Command::SavePreset { insert: T1, name });
    }
    let more = cmd(
        m,
        Command::SavePreset {
            insert: T1,
            name: "one more".into(),
        },
    );
    assert_eq!(saved(&more), None);
    assert_eq!(
        status(m),
        Some(StatusMessage::TooManyPresets {
            effect: EffectKind::Chorus
        })
    );
}
