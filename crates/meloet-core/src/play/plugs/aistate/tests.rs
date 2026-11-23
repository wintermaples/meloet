use meloet_model::{PluginPlace, StepKind, TrackId};
use meloet_protocol::Command;

use super::*;
use crate::ai::{AiAnswer, AiAsk};
use crate::catalog::tests::SURGE_VST3;
use crate::play::windows::tests::{FRESH, LEVEL, LOUD, Rig, state_of};
use crate::state::added_tests::loads;
use crate::state::{Input, PluginSaid};

fn ask(r: &mut Rig, ask: AiAsk) -> AiAnswer {
    r.go(Input::Ai(ask));
    r.m.take_ai_answer().expect("an answer")
}

fn put(r: &mut Rig, place: PluginPlace, state: Arc<[u8]>) -> u64 {
    let AiAnswer::Done(done) = ask(r, AiAsk::SetPluginState { place, state }) else {
        panic!("the put is taken on")
    };
    assert!(done.pending && done.step.is_none());
    done.ticket.expect("a ticket")
}

fn outcome(r: &Rig, ticket: u64) -> AiOutcome {
    let outcomes = r.m.ai_song().outcomes;
    let (_, outcome) = (outcomes.iter().find(|(t, _)| *t == ticket)).expect("the ticket is kept");
    outcome.clone()
}

fn failed(r: &Rig, ticket: u64) -> String {
    match outcome(r, ticket) {
        AiOutcome::Failed(why) => why,
        other => panic!("{other:?}"),
    }
}

/// How the put of `ticket` ended, as the AI link learns it.
fn end(r: &mut Rig, ticket: u64) -> AiPutEnd {
    match ask(r, AiAsk::StatePut { ticket }) {
        AiAnswer::State(AiState::Put(put)) => {
            assert_eq!(put.undo_count, r.undo_count());
            put.end
        }
        other => panic!("{other:?}"),
    }
}

fn in_song(r: &Rig, place: PluginPlace) -> Arc<[u8]> {
    r.m.play().song().doc.plugin(place).unwrap().state.clone()
}

fn surge(r: &mut Rig) -> (u64, PluginPlace) {
    let (ticket, track, _) = r.surge();
    (ticket, PluginPlace::Instrument(TrackId(track)))
}

fn state_asks(effects: &[Effect]) -> usize {
    (effects.iter())
        .filter(|e| {
            matches!(
                e,
                Effect::AskPlugin {
                    ask: PluginAsk::State(StateAsk::Ai),
                    ..
                }
            )
        })
        .count()
}

#[test]
fn the_state_is_asked_of_the_plugin_and_given_once_while_it_is_new() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    r.host.plugins.insert(surge, [0.25, 0.5, 1.0]);

    let effects = r.go(Input::Ai(AiAsk::PluginState { place }));
    assert_eq!(
        r.m.take_ai_answer(),
        Some(AiAnswer::State(AiState::Waiting))
    );
    assert_eq!(state_asks(&effects), 1);
    // Asked again before the plugin answers: not asked of it twice.
    let effects = r.go(Input::Ai(AiAsk::PluginState { place }));
    assert_eq!(state_asks(&effects), 0);
    r.settle();
    let AiAnswer::State(AiState::Got(got)) = ask(&mut r, AiAsk::PluginState { place }) else {
        panic!("the state")
    };
    assert_eq!(got.bytes, state_of([0.25, 0.5, 1.0]));
    assert_eq!(&*got.plugin.key, SURGE_VST3);
    assert!(in_song(&r, place).is_empty(), "the song is not changed");

    // Taken: the next read asks the plugin anew. An answer nobody takes within a second
    // is not given; the plugin is asked again.
    let effects = r.go(Input::Ai(AiAsk::PluginState { place }));
    assert_eq!(state_asks(&effects), 1);
    r.settle();
    r.host.now_us += KEEP_US + 1;
    let effects = r.go(Input::Ai(AiAsk::PluginState { place }));
    assert_eq!(
        r.m.take_ai_answer(),
        Some(AiAnswer::State(AiState::Waiting))
    );
    assert_eq!(state_asks(&effects), 1);

    // No plugin there, or one that does not sound.
    let empty = PluginPlace::Insert(meloet_model::StripId::Master, 0);
    let refused = AiAnswer::Refused(AiRefusal::NoSuchPlugin);
    assert_eq!(ask(&mut r, AiAsk::PluginState { place: empty }), refused);
    r.host.dead.insert(surge);
    r.go(Input::PluginStopped { ticket: surge });
    let refused = AiAnswer::Refused(AiRefusal::PluginNotLoaded);
    assert_eq!(ask(&mut r, AiAsk::PluginState { place }), refused);
}

#[test]
fn a_state_put_in_is_one_step_and_undo_and_redo_put_the_states_back() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    let new = state_of([0.9, 1.0, 1.0]);
    let steps = r.undo_count();

    let ticket = put(&mut r, place, new.clone());
    assert_eq!(outcome(&r, ticket), AiOutcome::Pending);
    assert_eq!(
        ask(&mut r, AiAsk::StatePut { ticket }),
        AiAnswer::State(AiState::Waiting)
    );
    assert_eq!(r.undo_count(), steps, "not before the plugin has it");
    r.settle();
    assert_eq!(outcome(&r, ticket), AiOutcome::Done(None));
    assert_eq!(r.undo_count(), steps + 1);
    let AiPutEnd::Step(Some(row)) = end(&mut r, ticket) else {
        panic!("a step with its row")
    };
    assert_eq!(row.index, steps + 1);
    // An end nobody knows of.
    assert!(matches!(
        ask(&mut r, AiAsk::StatePut { ticket: 999 }),
        AiAnswer::Refused(AiRefusal::Cannot(_))
    ));
    assert_eq!(
        r.m.play().steps().last(),
        Some(&StepKind::PluginWindow { place })
    );
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 1.0]);
    assert_eq!(in_song(&r, place), new);

    // The same state again leaves the song as it is.
    let same = AiAnswer::Refused(AiRefusal::Unchanged);
    let again = AiAsk::SetPluginState {
        place,
        state: new.clone(),
    };
    assert_eq!(ask(&mut r, again), same);

    // Undo: as the plugin started, which it said before it was changed; not loaded anew.
    let effects = r.cmd(Command::Undo);
    assert!(loads(&effects).is_empty());
    r.settle();
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert!(in_song(&r, place).is_empty());
    r.cmd(Command::Redo);
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 1.0]);
    assert_eq!(in_song(&r, place), new);

    // A second state over the first: a second step, and undo goes back to the first.
    let second = state_of([0.1, 0.0, 0.0]);
    put(&mut r, place, second.clone());
    // Another put while one goes on is turned away.
    let busy = AiAnswer::Refused(AiRefusal::PluginBusy);
    let third = AiAsk::SetPluginState {
        place,
        state: state_of([0.2, 0.0, 0.0]),
    };
    assert_eq!(ask(&mut r, third), busy);
    r.settle();
    assert_eq!(r.undo_count(), steps + 2);
    assert_eq!(in_song(&r, place), second);
    r.cmd(Command::Undo);
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 1.0]);
}

#[test]
fn a_refused_state_makes_no_step_and_the_plugin_gets_the_songs_state_again() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    let first = state_of([0.75, 0.5, 0.0]);
    put(&mut r, place, first.clone());
    r.settle();
    let steps = r.undo_count();
    let put_before = r.host.states_put;

    let ticket = put(&mut r, place, Arc::from(&b"not a state"[..]));
    r.settle();
    assert!(
        failed(&r, ticket).contains("refused"),
        "{}",
        failed(&r, ticket)
    );
    assert!(matches!(end(&mut r, ticket), AiPutEnd::Refused(_)));
    assert_eq!(r.undo_count(), steps);
    assert_eq!(in_song(&r, place), first);
    assert_eq!(r.host.plugins[&surge], [0.75, 0.5, 0.0]);
    assert_eq!(
        r.host.states_put,
        put_before + 1,
        "the song's state is put back"
    );

    // Taken, but passed over: the plugin says the state it had, and that is no step, and no
    // failure either.
    let mut skip = b"SKIP".to_vec();
    skip.resize(24, 0);
    let ticket = put(&mut r, place, skip.into());
    r.settle();
    assert_eq!(outcome(&r, ticket), AiOutcome::Done(None));
    assert_eq!(end(&mut r, ticket), AiPutEnd::Same);
    assert_eq!(r.undo_count(), steps);
    assert_eq!(in_song(&r, place), first);

    // On a plugin the song has "as it starts": what it said it started with goes back.
    r.cmd(Command::Undo);
    r.settle();
    assert!(in_song(&r, place).is_empty());
    let ticket = put(&mut r, place, Arc::from(&b"broken"[..]));
    r.settle();
    assert!(matches!(outcome(&r, ticket), AiOutcome::Failed(_)));
    assert!(matches!(end(&mut r, ticket), AiPutEnd::Refused(_)));
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert!(in_song(&r, place).is_empty());
}

#[test]
fn a_state_that_stops_the_process_makes_no_step_and_the_plugin_is_loaded_again() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    let first = state_of([0.75, 0.5, 0.0]);
    put(&mut r, place, first.clone());
    r.settle();
    let steps = r.undo_count();

    let mut kill = b"KILL".to_vec();
    kill.resize(24, 0);
    let ticket = put(&mut r, place, kill.into());
    // The answer comes first, then the news that the process stopped.
    r.settle();
    assert!(
        failed(&r, ticket).contains("stopped"),
        "{}",
        failed(&r, ticket)
    );
    assert_eq!(end(&mut r, ticket), AiPutEnd::Stopped);
    let effects = r.go(Input::PluginStopped { ticket: surge });
    assert!(effects.contains(&Effect::ReloadPlugin { ticket: surge }));
    r.go(Input::PluginLoaded { ticket: surge });
    assert_eq!(r.undo_count(), steps);
    assert_eq!(in_song(&r, place), first);
    assert_eq!(r.host.plugins[&surge], [0.75, 0.5, 0.0], "as before");
    assert!(r.m.play().stopped_plugin().is_none());

    // The other way round: the process stopped before the answer came.
    r.host.now_us += REVIVE_US + 1;
    let ticket = put(&mut r, place, state_of([0.1, 0.0, 0.0]));
    r.host.answers.clear();
    r.host.dead.insert(surge);
    r.go(Input::PluginStopped { ticket: surge });
    let gone = PluginSaid::StatePut(Err(HostError::Gone));
    let effects = r.said(surge, gone);
    assert!(effects.contains(&Effect::ReloadPlugin { ticket: surge }));
    assert!(matches!(outcome(&r, ticket), AiOutcome::Failed(_)));
    assert_eq!(end(&mut r, ticket), AiPutEnd::Stopped);
    assert_eq!(in_song(&r, place), first);

    // Long after, a plugin that stops is not loaded again by itself.
    r.go(Input::PluginLoaded { ticket: surge });
    r.host.now_us += REVIVE_US + 1;
    r.host.dead.insert(surge);
    let effects = r.go(Input::PluginStopped { ticket: surge });
    assert!(!effects.contains(&Effect::ReloadPlugin { ticket: surge }));
}

#[test]
fn an_undo_while_the_state_goes_in_wins_and_makes_it_no_step() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    let steps = r.undo_count();
    put(&mut r, place, state_of([0.75, 0.5, 0.0]));
    r.settle();

    let ticket = put(&mut r, place, state_of([0.1, 0.0, 0.0]));
    r.cmd(Command::Undo);
    r.settle();
    assert!(
        failed(&r, ticket).contains("undo"),
        "{}",
        failed(&r, ticket)
    );
    assert_eq!(end(&mut r, ticket), AiPutEnd::Overtaken);
    assert_eq!(r.undo_count(), steps);
    assert!(in_song(&r, place).is_empty());
    assert_eq!(r.host.plugins[&surge], FRESH);
}

#[test]
fn a_put_waits_for_a_visit_to_be_its_own_step_and_the_panel_reads_the_plugin_again() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    let track = match place {
        PluginPlace::Instrument(t) => t.0,
        PluginPlace::Insert(..) => unreachable!(),
    };
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    let table = r.selected().table;
    r.said(surge, PluginSaid::WindowEntered);
    r.settle();
    let turned = r.host.turn(surge, LEVEL, 0.3);
    r.go(turned);

    let steps = r.m.play().steps().len();
    put(&mut r, place, state_of([0.9, 1.0, 1.0]));
    r.settle();
    let made: Vec<StepKind> = r.m.play().steps()[steps..].to_vec();
    let window = StepKind::PluginWindow { place };
    assert_eq!(made, [window, window]);
    assert_ne!(r.selected().table, table, "the panel's list is read again");

    // The visit goes on from the state put in: only what the window changes since is its.
    let turned = r.host.turn(surge, LOUD, 0.0);
    r.go(turned);
    r.said(surge, PluginSaid::WindowLeft);
    r.settle();
    assert_eq!(r.m.play().steps().len(), steps + 3);
    assert_eq!(in_song(&r, place), state_of([0.9, 1.0, 0.0]));
    r.cmd(Command::Undo);
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 1.0]);
    r.cmd(Command::Undo);
    assert_eq!(
        r.host.plugins[&surge],
        [0.3, 0.0, 0.0],
        "the window's step stays"
    );
}

#[test]
fn an_undo_while_a_state_stops_the_process_loads_the_plugin_anew_with_the_songs_state() {
    let mut r = Rig::new();
    let (surge, place) = surge(&mut r);
    put(&mut r, place, state_of([0.75, 0.5, 0.0]));
    r.settle();

    let mut kill = b"KILL".to_vec();
    kill.resize(24, 0);
    let ticket = put(&mut r, place, kill.into());
    // The undo's state goes to a process that is ending: it never takes it.
    r.cmd(Command::Undo);
    r.settle();
    assert!(failed(&r, ticket).contains("stopped"));
    assert_eq!(end(&mut r, ticket), AiPutEnd::Stopped);
    let effects = r.go(Input::PluginStopped { ticket: surge });
    assert!(!effects.contains(&Effect::ReloadPlugin { ticket: surge }));
    let loads = loads(&effects);
    assert_eq!(loads.len(), 1, "{effects:?}");
    assert_eq!(
        loads[0].2,
        crate::play::PlugState::Fresh,
        "as the song has it"
    );
    r.go(Input::PluginLoaded { ticket: loads[0].0 });
    assert!(in_song(&r, place).is_empty());
    assert_eq!(r.host.plugins[&loads[0].0], FRESH);
    assert!(r.m.play().stopped_plugin().is_none());
}

#[test]
fn a_put_on_its_way_is_not_carried_into_a_plugin_loaded_again() {
    let mut r = Rig::new();
    let (_, place) = surge(&mut r);
    put(&mut r, place, state_of([0.75, 0.5, 0.0]));
    r.settle();
    let first = in_song(&r, place);

    let ticket = put(&mut r, place, state_of([0.1, 0.0, 0.0]));
    // Another sample rate: every plugin is loaded again, this one with the song's state.
    let mut effects = Vec::new();
    r.m.play_mut().set_plug_rate(44_100, &mut effects);
    r.host.take(&effects);
    let again = loads(&effects);
    let ours = (again.iter())
        .find(|l| l.1 == SURGE_VST3)
        .expect("loaded again");
    assert_eq!(ours.2, crate::play::PlugState::Bytes(first.clone()));
    r.settle();
    assert!(failed(&r, ticket).contains("loaded again"));
    assert_eq!(end(&mut r, ticket), AiPutEnd::Overtaken);
    assert_eq!(in_song(&r, place), first);
}
