//! What the AI link does to parameters: sets of plugin parameters that end with a definite
//! outcome, texts of values, and parameters of built-in effects set as one step.

use meloet_model::effects::EffectKind;
use meloet_model::{PluginPlace, StepKind, StripId, TrackId};
use meloet_protocol::{AutoTarget, Command};

use super::*;
use crate::ai::{AiAnswer, AiAsk, AiEffect, AiParamNow};
use crate::play::windows::tests::{FRESH, LEVEL, LOUD, MODE, Rig, state_of};
use crate::state::Input;

fn ask(r: &mut Rig, ask: AiAsk) -> AiAnswer {
    r.go(Input::Ai(ask));
    r.m.take_ai_answer().expect("an answer")
}

/// Sets values of the plugin at `place`; returns the ticket of the set.
fn set(r: &mut Rig, place: PluginPlace, values: &[(u32, f64)]) -> u64 {
    let values = values.to_vec();
    match ask(r, AiAsk::SetPluginParams { place, values }) {
        AiAnswer::Done(done) => {
            assert!(done.pending);
            done.ticket.expect("a ticket")
        }
        other => panic!("{other:?}"),
    }
}

fn outcome(r: &mut Rig, ticket: u64) -> AiParams {
    match ask(r, AiAsk::ParamsSet { ticket }) {
        AiAnswer::Params(params) => params,
        other => panic!("{other:?}"),
    }
}

fn ended(r: &mut Rig, ticket: u64) -> AiParamsSet {
    match outcome(r, ticket) {
        AiParams::Set(set) => set,
        other => panic!("not ended: {other:?}"),
    }
}

fn now(id: u32, asked: f64, value: f64, text: &str) -> AiParamNow {
    AiParamNow {
        id,
        asked,
        value: Some(value),
        text: Some(text.into()),
    }
}

fn surge(r: &mut Rig) -> (u64, u32, PluginPlace) {
    let (surge, track, _) = r.surge();
    r.settle();
    (surge, track, PluginPlace::Instrument(TrackId(track)))
}

#[test]
fn several_values_are_one_step_and_the_set_ends_with_it() {
    let mut r = Rig::new();
    let (surge, _, place) = surge(&mut r);
    let steps = r.undo_count();

    let ticket = set(&mut r, place, &[(LEVEL, 0.25), (MODE, 0.6), (LOUD, 1.0)]);
    assert_eq!(
        outcome(&mut r, ticket),
        AiParams::Waiting,
        "until the plugin answers"
    );
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.25, 0.5, 1.0]);
    assert_eq!(r.undo_count(), steps + 1, "one step for the three");
    let set = ended(&mut r, ticket);
    assert!(matches!(set.end, AiSetEnd::Step(Some(_))), "{:?}", set.end);
    assert_eq!(set.undo_count, steps + 1);
    // What the plugin took: the mode lands on a step.
    assert_eq!(
        set.params,
        [
            now(LEVEL, 0.25, 0.25, "25 %"),
            now(MODE, 0.6, 0.5, "Hard"),
            now(LOUD, 1.0, 1.0, "On"),
        ]
    );
    let in_song = r.m.play().song().doc.plugin(place).unwrap().state.clone();
    assert_eq!(in_song, state_of([0.25, 0.5, 1.0]));
    assert_eq!(
        r.m.play().steps().last(),
        Some(&StepKind::SetPluginParam {
            place,
            param: LEVEL
        })
    );
    // Read again, the same.
    assert_eq!(ended(&mut r, ticket), set);

    // Undo puts the whole state back.
    r.cmd(Command::Undo);
    r.settle();
    assert_eq!(r.host.plugins[&surge], FRESH);
}

#[test]
fn values_the_plugin_has_already_are_no_step_and_say_so() {
    let mut r = Rig::new();
    let (_, _, place) = surge(&mut r);
    let ticket = set(&mut r, place, &[(LEVEL, 0.4)]);
    r.settle();
    let steps = r.undo_count();

    // The same value, and a mode inside the step it is on.
    let ticket2 = set(&mut r, place, &[(LEVEL, 0.4), (MODE, 0.1)]);
    r.settle();
    assert_eq!(r.undo_count(), steps);
    let set = ended(&mut r, ticket2);
    assert_eq!(set.end, AiSetEnd::Same);
    assert_eq!(set.params[1], now(MODE, 0.1, 0.0, "Soft"));
    assert_ne!(ticket, ticket2);

    // A ticket nobody gave is refused.
    assert_eq!(
        ask(&mut r, AiAsk::ParamsSet { ticket: 999 }),
        AiAnswer::Refused(AiRefusal::NoSuchSet)
    );
}

#[test]
fn a_lane_on_one_parameter_sets_none_of_them() {
    let mut r = Rig::new();
    let (surge, track, place) = surge(&mut r);
    r.cmd(Command::AddLane {
        home: meloet_protocol::LaneHome::Track { track },
        target: AutoTarget::Instrument { param: MODE },
    });
    r.settle();
    let steps = r.undo_count();
    let values = vec![(LEVEL, 0.9), (MODE, 1.0)];
    assert_eq!(
        ask(&mut r, AiAsk::SetPluginParams { place, values }),
        AiAnswer::Refused(AiRefusal::LaneDrives(MODE))
    );
    r.settle();
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert_eq!(r.undo_count(), steps);
    // The old ask says it as it did.
    let one = AiAsk::SetPluginParam {
        place,
        param: MODE,
        value: 1.0,
    };
    assert_eq!(ask(&mut r, one), AiAnswer::Refused(AiRefusal::HasLane));
}

#[test]
fn a_state_put_back_first_or_a_plugin_that_cannot_say_ends_the_set_without_a_step() {
    let mut r = Rig::new();
    let (surge, _, place) = surge(&mut r);
    set(&mut r, place, &[(LEVEL, 0.3)]);
    r.settle();
    let steps = r.undo_count();

    // Undo comes before the plugin's answer: the song's state goes back into the plugin.
    let ticket = set(&mut r, place, &[(LEVEL, 0.8)]);
    r.cmd(Command::Undo);
    r.settle();
    assert_eq!(ended(&mut r, ticket).end, AiSetEnd::Overtaken);
    assert_eq!(r.undo_count(), steps - 1);
    assert_eq!(r.host.plugins[&surge], FRESH);

    // The plugin's process stopped: it cannot say its state.
    r.host.dead.insert(surge);
    let ticket = set(&mut r, place, &[(LEVEL, 0.7)]);
    r.settle();
    assert_eq!(ended(&mut r, ticket).end, AiSetEnd::NoState);
    assert_eq!(r.undo_count(), steps - 1);
}

#[test]
fn texts_of_values_are_asked_once_and_given_once() {
    let mut r = Rig::new();
    let (_, _, place) = surge(&mut r);
    let steps = r.undo_count();
    let texts = |r: &mut Rig| {
        let values = vec![0.0, 0.5, 1.0];
        ask(
            r,
            AiAsk::ParamTexts {
                place,
                param: MODE,
                values,
            },
        )
    };
    assert_eq!(texts(&mut r), AiAnswer::Params(AiParams::Waiting));
    assert_eq!(texts(&mut r), AiAnswer::Params(AiParams::Waiting));
    let asked = r.host.answers.len();
    assert_eq!(asked, 1, "asked once");
    r.settle();
    let said = ["Soft", "Hard", "Odd"].map(|t| Some(t.to_owned())).to_vec();
    assert_eq!(texts(&mut r), AiAnswer::Params(AiParams::Texts(said)));
    // Taken: the next ask asks the plugin again.
    assert_eq!(texts(&mut r), AiAnswer::Params(AiParams::Waiting));
    // Nothing was set.
    assert_eq!(r.undo_count(), steps);
}

#[test]
fn values_of_a_built_in_effect_are_one_step() {
    let mut r = Rig::new();
    let track = r.m.view().song.tracks[0].id;
    let strip = StripId::Track(TrackId(track));
    let put = AiAsk::PutEffect {
        strip,
        slot: 1,
        effect: AiEffect::Builtin(EffectKind::Delay),
        replace: false,
    };
    assert!(matches!(ask(&mut r, put), AiAnswer::Done(_)));
    let steps = r.undo_count();
    let values = vec![(0, 4.0), (3, 50.0), (6, 1.0)];
    let write = AiAsk::SetEffectParams {
        strip,
        slot: 1,
        values,
    };
    let AiAnswer::Done(done) = ask(&mut r, write.clone()) else {
        panic!("set")
    };
    assert_eq!((done.undo_count, done.step.is_some()), (steps + 1, true));
    let insert = r.m.play().song().doc.insert(strip, 1).unwrap().clone();
    assert_eq!(
        (insert.value(0), insert.value(3), insert.value(6)),
        (Some(4.0), Some(50.0), Some(1.0))
    );
    // Again: no step, and no refusal.
    let AiAnswer::Done(again) = ask(&mut r, write) else {
        panic!("answered")
    };
    assert_eq!((again.undo_count, again.step), (steps + 1, None));
    // Out of range: brought into it.
    let values = vec![(3, 500.0)];
    ask(
        &mut r,
        AiAsk::SetEffectParams {
            strip,
            slot: 1,
            values,
        },
    );
    let insert = r.m.play().song().doc.insert(strip, 1).unwrap().clone();
    assert_eq!(insert.value(3), Some(95.0));
    // A parameter it does not have, or one a lane moves: nothing is set.
    let id = insert.id;
    let bad = AiAsk::SetEffectParams {
        strip,
        slot: 1,
        values: vec![(0, 1.0), (40, 1.0)],
    };
    assert_eq!(
        ask(&mut r, bad),
        AiAnswer::Refused(AiRefusal::NoSuchParam(40))
    );
    r.cmd(Command::AddLane {
        home: meloet_protocol::LaneHome::Track { track },
        target: AutoTarget::Insert {
            insert: id.0,
            param: 3,
        },
    });
    let steps = r.undo_count();
    let laned = AiAsk::SetEffectParams {
        strip,
        slot: 1,
        values: vec![(0, 1.0), (3, 10.0)],
    };
    assert_eq!(
        ask(&mut r, laned),
        AiAnswer::Refused(AiRefusal::LaneDrives(3))
    );
    assert_eq!(r.undo_count(), steps);
    let insert = r.m.play().song().doc.insert(strip, 1).unwrap();
    assert_eq!(insert.value(0), Some(4.0));
    // An empty slot.
    let empty = AiAsk::SetEffectParams {
        strip,
        slot: 3,
        values: vec![(0, 1.0)],
    };
    assert_eq!(ask(&mut r, empty), AiAnswer::Refused(AiRefusal::SlotEmpty));
}
