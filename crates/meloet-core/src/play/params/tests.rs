use meloet_model::{PluginPlace, StepKind, TrackId};
use meloet_protocol::{
    AutoTarget, Band, Command, InsertRef, LaneName, PluginWindowKind, StepName, StepWhat, StripRef,
};

use super::*;
use crate::catalog::tests::COMP_VST3;
use crate::play::automation::NAMES_ASK;
use crate::play::tests::status;
use crate::play::windows::tests::{FRESH, KNOB, LEVEL, LOUD, MODE, Rig, state_of};
use crate::state::Input;
use crate::state::added_tests::loads;

/// The values a `ParamValues` message carries, as (id, value, text).
fn carried(values: &ParamValues) -> Vec<(u32, Option<f64>, Option<&str>)> {
    (values.values.iter())
        .map(|v| (v.id, v.value, v.text.as_deref()))
        .collect()
}

fn set(r: &mut Rig, slot: PluginSlotRef, param: u32, value: f64, commit: bool) {
    let table = r.selected().table.expect("a table");
    r.cmd(Command::SetPluginParam {
        slot,
        table,
        param,
        value,
        commit,
    });
}

#[test]
fn the_list_is_a_table_with_a_number_and_a_new_one_when_it_may_have_changed() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();

    // Selected: the list is asked for, and the panel has no table until it has come.
    let effects = r.cmd(Command::SelectInstrument { track });
    let ask = PluginAsk::Params { ask: 1 };
    assert!(effects.contains(&Effect::AskPlugin { ticket: surge, ask }));
    let selected = r.selected();
    assert_eq!((selected.table, selected.count), (None, 0));
    assert_eq!(r.m.plugin_params(1), None);
    r.settle();
    let selected = r.selected();
    assert_eq!((selected.table, selected.count), (Some(1), 3));
    assert_eq!(selected.window, PluginWindowKind::Opens);

    // The table: the plugin's order, its groups, the three kinds of row, values and texts.
    let table = r.m.plugin_params(1).expect("the table");
    assert_eq!((table.table, table.seq), (1, 0));
    let names: Vec<(Option<&str>, Vec<&str>)> = (table.groups.iter())
        .map(|g| {
            let rows = g.params.iter().map(|p| p.name.as_str()).collect();
            (g.name.as_deref(), rows)
        })
        .collect();
    assert_eq!(
        names,
        [(None, vec!["Level"]), (Some("Shape"), vec!["Mode", "Loud"])]
    );
    let level = &table.groups[0].params[0];
    assert_eq!((level.id, level.kind.clone()), (LEVEL, ParamKind::Smooth));
    assert_eq!((level.default, level.value), (0.5, Some(0.5)));
    assert_eq!(level.text.as_deref(), Some("50 %"));
    let labels = ["Soft", "Hard", "Odd"].map(str::to_owned).to_vec();
    let steps = ParamKind::Steps { steps: 3, labels };
    assert_eq!(table.groups[1].params[0].kind, steps);
    assert_eq!(table.groups[1].params[1].kind, ParamKind::Toggle);
    assert_eq!(r.m.plugin_params(2), None, "not the table the panel shows");

    // Selected again while it is selected: the same table.
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    assert_eq!(r.selected().table, Some(1));
    // Another plugin, and back: new tables. One that does not sound yet has none.
    let comp = r.effect(track, 0, COMP_VST3);
    r.settle();
    let insert = InsertRef {
        strip: StripRef::Track { id: track },
        slot: 0,
    };
    assert_eq!(r.selected().slot, PluginSlotRef::Insert { insert });
    assert_eq!(r.selected().table, Some(2));
    assert_eq!(r.m.plugin_params(1), None);
    r.cmd(Command::SelectInstrument { track });
    assert_eq!(r.selected().table, None);
    r.settle();
    assert_eq!(r.selected().table, Some(3));
    let _ = comp;

    // The plugin says its values changed: read again. The old table stays until then.
    r.host.plugins.insert(surge, [0.2, 1.0, 1.0]);
    let effects = r.said(surge, PluginSaid::ParamsChanged);
    let ask = PluginAsk::Params { ask: 4 };
    assert!(effects.contains(&Effect::AskPlugin { ticket: surge, ask }));
    assert_eq!(r.selected().table, Some(3));
    r.settle();
    assert_eq!(r.selected().table, Some(4));
    let table = r.m.plugin_params(4).unwrap();
    assert_eq!(table.groups[0].params[0].value, Some(0.2));
    assert_eq!(table.groups[1].params[0].text.as_deref(), Some("Odd"));

    // Stopped, the table stays (its rows are unusable); loaded again, it is read again.
    r.host.dead.insert(surge);
    r.go(Input::PluginStopped { ticket: surge });
    assert_eq!(r.selected().table, Some(4));
    set(&mut r, slot, LEVEL, 0.9, true);
    assert!(
        r.host.answers.is_empty(),
        "a stopped plugin is asked nothing"
    );
    r.cmd(Command::ReloadPlugin { slot });
    r.go(Input::PluginLoaded { ticket: surge });
    r.settle();
    assert_eq!(r.selected().table, Some(5));
    // Nothing selected: no table.
    r.cmd(Command::SelectInsert { insert: None });
    assert_eq!(r.m.view().params.selected, None);
    assert_eq!(r.m.plugin_params(5), None);
}

#[test]
fn parameters_that_only_take_a_keyboards_knobs_are_apart_and_not_counted() {
    let mut r = Rig::new();
    r.host.knobs = 4;
    let (_, track, _) = r.surge();
    r.cmd(Command::SelectInstrument { track });
    r.settle();

    let selected = r.selected();
    assert_eq!((selected.table, selected.count), (Some(1), 3));
    let table = r.m.plugin_params(1).expect("the table");
    let names: Vec<Vec<&str>> = (table.groups.iter())
        .map(|g| g.params.iter().map(|p| p.name.as_str()).collect())
        .collect();
    assert_eq!(names, [vec!["Level"], vec!["Mode", "Loud"]]);
    let knobs: Vec<(u32, &str)> = (table.keyboard_knobs.iter())
        .map(|p| (p.id, p.name.as_str()))
        .collect();
    assert_eq!(
        knobs,
        [
            (KNOB, "MIDI CC 0|0"),
            (KNOB + 1, "MIDI CC 0|1"),
            (KNOB + 2, "MIDI CC 0|2"),
            (KNOB + 3, "MIDI CC 0|3"),
        ]
    );

    // One found by searching can be chosen like any row.
    r.cmd(Command::SelectParam {
        param: Some(KNOB + 2),
    });
    assert_eq!(r.selected().row, Some(KNOB + 2));
}

#[test]
fn values_go_to_the_screen_only_as_they_change_and_thirty_times_a_second_at_most() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    r.pass(1_000_000);

    // A row dragged for a second, a move every millisecond, each answered by the plugin.
    for n in 1..=1000u32 {
        set(&mut r, slot, LEVEL, f64::from(n) / 1000.0, false);
        r.settle();
        r.pass(1_000);
    }
    r.pass(VALUES_EVERY_US);
    let sent = &r.host.values;
    assert!((25..=31).contains(&sent.len()), "{} messages", sent.len());
    for (n, pair) in sent.windows(2).enumerate() {
        let apart = pair[1].0 - pair[0].0;
        assert!(apart >= VALUES_EVERY_US, "message {n}: {apart} µs apart");
    }
    let seqs: Vec<u64> = sent.iter().map(|(_, v)| v.seq).collect();
    assert_eq!(seqs, (1..=sent.len() as u64).collect::<Vec<_>>());
    assert!(sent.iter().all(|(_, v)| v.table == 1));
    // The last value is never left out.
    let last = &sent.last().unwrap().1;
    assert_eq!(carried(last), [(LEVEL, Some(1.0), Some("100 %"))]);
    assert_eq!(r.m.wake_at_us(), None, "nothing waits");
    // The table fetched now has the values and the number of the last message.
    let table = r.m.plugin_params(1).unwrap();
    assert_eq!(table.seq, sent.len() as u64);
    assert_eq!(table.groups[0].params[0].value, Some(1.0));
    assert_eq!(r.undo_count(), 1, "a drag that is not let go is no step");

    // Turned in the plugin's window: the same way. A parameter the list does not have is
    // passed over, and a value that did not change is not sent.
    r.host.values.clear();
    let turned = r.host.turn(surge, MODE, 1.0);
    r.go(turned);
    let again = r.host.turn(surge, MODE, 1.0);
    r.go(again);
    let hidden = ParamNow {
        id: 77,
        value: Some(0.5),
        text: None,
    };
    r.said(surge, PluginSaid::Param(hidden));
    r.pass(VALUES_EVERY_US);
    let sent: Vec<_> = r.host.values.iter().map(|(_, v)| carried(v)).collect();
    assert_eq!(sent, [[(MODE, Some(1.0), Some("Odd"))]]);
    // Of a plugin the panel does not show, nothing is sent.
    r.host.values.clear();
    r.cmd(Command::SelectInsert { insert: None });
    let turned = r.host.turn(surge, LEVEL, 0.3);
    r.go(turned);
    r.pass(VALUES_EVERY_US);
    assert!(r.host.values.is_empty());
}

#[test]
fn a_row_dragged_is_heard_let_go_is_one_step_and_esc_puts_the_value_back() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    let place = PluginPlace::Instrument(TrackId(track));
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    let steps = r.undo_count();

    // Dragged: the plugin hears every move, the song has none of them.
    for value in [0.6, 0.7, 0.8] {
        set(&mut r, slot, LEVEL, value, false);
    }
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.8, 0.0, 0.0]);
    assert_eq!(r.undo_count(), steps);
    // Esc: the plugin has the value from before the drag.
    r.cmd(Command::CancelMixPreview);
    r.settle();
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert_eq!(r.undo_count(), steps);

    // Let go: one step, whatever the number of moves.
    for (value, commit) in [(0.6, false), (0.7, false), (0.8, true)] {
        set(&mut r, slot, LEVEL, value, commit);
    }
    r.settle();
    assert_eq!(r.undo_count(), steps + 1);
    let step = r.m.play().steps().last().cloned();
    assert_eq!(
        step,
        Some(StepKind::SetPluginParam {
            place,
            param: LEVEL
        })
    );
    let in_song = |r: &Rig| r.m.play().song().doc.plugin(place).unwrap().state.clone();
    assert_eq!(in_song(&r), state_of([0.8, 0.0, 0.0]));
    assert_eq!(r.host.states_put, 0, "the plugin has it already");
    // The value it has already: no step.
    set(&mut r, slot, LEVEL, 0.8, true);
    r.settle();
    assert_eq!(r.undo_count(), steps + 1);

    // Undo and redo put the whole state back, without loading the plugin anew, and the
    // screen gets a new table each time.
    let table = r.selected().table;
    let effects = r.cmd(Command::Undo);
    assert!(loads(&effects).is_empty());
    r.settle();
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert_ne!(r.selected().table, table);
    let table = r.selected().table.unwrap();
    assert_eq!(
        r.m.plugin_params(table).unwrap().groups[0].params[0].value,
        Some(0.5)
    );
    r.cmd(Command::Redo);
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.8, 0.0, 0.0]);

    // A command for a table the screen does not show any more, for a parameter the list
    // does not have, or for a plugin that is not the selected one, is dropped.
    let now = r.selected().table.unwrap();
    for (table, param) in [(now - 1, LEVEL), (now, 77)] {
        r.cmd(Command::SetPluginParam {
            slot,
            table,
            param,
            value: 0.1,
            commit: true,
        });
    }
    let other = PluginSlotRef::Instrument { track: track + 50 };
    set(&mut r, other, LEVEL, 0.1, true);
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.8, 0.0, 0.0]);
    assert_eq!(r.undo_count(), steps + 1);

    // The song changes under a drag: the drag is over, as every preview is.
    set(&mut r, slot, MODE, 1.0, false);
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.8, 1.0, 0.0]);
    r.cmd(Command::SetTempo { bpm: 97.0 });
    r.settle();
    assert_eq!(r.host.plugins[&surge], [0.8, 0.0, 0.0]);
}

#[test]
fn a_typed_value_is_read_by_the_plugin_and_a_row_is_chosen_by_the_plugins_number() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    r.cmd(Command::SelectInstrument { track });
    r.settle();
    let steps = r.undo_count();
    let typed = |r: &mut Rig, param: u32, text: &str| {
        let table = r.selected().table.unwrap();
        let param = ParamRef::Plugin { slot, table, param };
        let text = text.to_owned();
        r.cmd(Command::TypeParam { param, text });
        r.settle();
    };

    typed(&mut r, LEVEL, "30 %");
    assert_eq!(r.host.plugins[&surge], [0.3, 0.0, 0.0]);
    assert_eq!(r.undo_count(), steps + 1);
    assert!(!matches!(
        status(&r.m),
        Some(StatusMessage::ParamClamped { .. } | StatusMessage::ParamRejected { .. })
    ));

    // Past the end: put at the end, and said with the plugin's own texts.
    typed(&mut r, LEVEL, "140");
    assert_eq!(r.host.plugins[&surge], [1.0, 0.0, 0.0]);
    assert_eq!(r.undo_count(), steps + 2);
    let end = ParamEnd::Plugin {
        name: "Level".into(),
        value: "100 %".into(),
        range: "0 %〜100 %".into(),
    };
    assert_eq!(status(&r.m), Some(StatusMessage::ParamClamped { to: end }));

    // Not a value: nothing changes, and the text the row shows is said.
    typed(&mut r, LEVEL, "much");
    assert_eq!(r.host.plugins[&surge], [1.0, 0.0, 0.0]);
    assert_eq!(r.undo_count(), steps + 2);
    let table = r.selected().table.unwrap();
    let param = ParamRef::Plugin {
        slot,
        table,
        param: LEVEL,
    };
    let kept = "100 %".into();
    let rejected = StatusMessage::ParamRejected { param, kept };
    assert_eq!(status(&r.m), Some(rejected));
    // The next thing the person does puts the sentence away.
    r.cmd(Command::SelectParam { param: None });
    assert!(!matches!(
        status(&r.m),
        Some(StatusMessage::ParamRejected { .. })
    ));

    // A row is chosen by the plugin's own number for it; the status line names it while
    // the panel is open.
    r.cmd(Command::ToggleBand { band: Band::Params });
    r.cmd(Command::SelectParam { param: Some(MODE) });
    assert_eq!(r.selected().row, Some(MODE));
    assert_eq!(
        status(&r.m),
        Some(StatusMessage::PluginParamSelected {
            loop_bars: None,
            slot,
            plugin: "Surge XT".into(),
            param: "Mode".into(),
        })
    );
    r.cmd(Command::SelectParam { param: Some(77) });
    assert_eq!(r.selected().row, None);
    // The row stays over a new table that still has it.
    r.cmd(Command::SelectParam { param: Some(LOUD) });
    r.said(surge, PluginSaid::ParamsChanged);
    r.settle();
    assert_eq!(r.selected().row, Some(LOUD));
}

#[test]
fn a_value_the_plugin_does_not_tell_has_no_position() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    let effects = r.cmd(Command::SelectInstrument { track });
    let asked = effects.iter().find_map(|e| match e {
        Effect::AskPlugin {
            ask: PluginAsk::Params { ask },
            ..
        } => Some(*ask),
        _ => None,
    });
    r.host.answers.clear();
    let mut params = r.host.params(surge);
    (params[0].value, params[0].text) = (None, None);
    let ask = asked.expect("the list is asked for");
    r.said(surge, PluginSaid::Params { ask, params });
    let table = r.m.plugin_params(1).unwrap();
    let level = &table.groups[0].params[0];
    assert_eq!((level.value, level.text.clone()), (None, None));

    // Dragged and given up: there is no value to put back, so nothing is.
    set(&mut r, slot, LEVEL, 0.9, false);
    r.settle();
    r.cmd(Command::CancelMixPreview);
    r.settle();
    assert_eq!(r.host.plugins[&surge][0], 0.9);
    r.pass(VALUES_EVERY_US);
    let (_, last) = r.host.values.last().expect("the value came");
    assert_eq!(carried(last), [(LEVEL, Some(0.9), Some("90 %"))]);
}

fn lane_names(r: &Rig) -> Vec<Option<String>> {
    (r.m.view().song.tracks[0].lanes.iter())
        .map(|lane| match &lane.name {
            LaneName::Plugin { param, .. } => param.clone(),
            other => panic!("not a plugin's lane: {other:?}"),
        })
        .collect()
}

fn names_asks(effects: &[Effect]) -> Vec<u64> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::AskPlugin { ticket, ask } if *ask == (PluginAsk::Params { ask: NAMES_ASK }) => {
                Some(*ticket)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_plugin_the_panel_shows_is_watched_and_its_lanes_are_the_rows_written() {
    let mut r = Rig::new();
    let (surge, track, _) = r.surge();
    let comp = r.effect(track, 0, COMP_VST3);
    let insert = r.m.view().song.tracks[0].slots[0].as_ref().unwrap().id;
    r.cmd(Command::SelectInsert { insert: None });
    r.settle();
    r.host.watches.clear();
    let lane = |r: &mut Rig, target| {
        r.cmd(Command::AddLane {
            home: meloet_protocol::LaneHome::Track { track },
            target,
        })
    };

    // Nothing is watched while the panel shows no plugin, lanes or not.
    lane(&mut r, AutoTarget::Instrument { param: MODE });
    r.settle();
    assert!(r.host.watches.is_empty());

    r.cmd(Command::SelectInstrument { track });
    assert_eq!(r.host.watches, [(surge, true)]);
    assert_eq!(r.selected().automated, [] as [u32; 0], "no table yet");
    r.settle();
    assert_eq!(r.selected().automated, [MODE]);
    // In the order of the table, whatever the order of the lanes.
    lane(&mut r, AutoTarget::Instrument { param: LEVEL });
    lane(
        &mut r,
        AutoTarget::Insert {
            insert,
            param: LOUD,
        },
    );
    lane(&mut r, AutoTarget::Volume);
    assert_eq!(r.selected().automated, [LEVEL, MODE]);
    assert_eq!(r.host.watches, [(surge, true)], "told once");

    // What a lane gives the plugin goes to the screen as any value of it does.
    r.host.values.clear();
    let steps = r.undo_count();
    let now = ParamNow {
        id: LEVEL,
        value: Some(0.25),
        text: Some("25 %".into()),
    };
    r.said(surge, PluginSaid::Param(now));
    r.pass(VALUES_EVERY_US);
    let sent: Vec<_> = r.host.values.iter().map(|(_, v)| carried(v)).collect();
    assert_eq!(sent, [[(LEVEL, Some(0.25), Some("25 %"))]]);
    assert_eq!(r.undo_count(), steps, "a value a lane gave is no step");

    // Another plugin in the panel: the first is let go.
    let slot = InsertRef {
        strip: StripRef::Track { id: track },
        slot: 0,
    };
    r.cmd(Command::SelectInsert { insert: Some(slot) });
    r.settle();
    assert_eq!(r.host.watches[1..], [(surge, false), (comp, true)]);
    assert_eq!(r.selected().automated, [LOUD]);
    r.cmd(Command::SelectInsert { insert: None });
    assert_eq!(r.host.watches[3..], [(comp, false)]);
}

#[test]
fn the_names_of_the_lanes_parameters_come_whatever_the_panel_shows() {
    let mut r = Rig::new();
    let (surge, track, _) = r.surge();
    r.effect(track, 0, COMP_VST3);
    let insert = r.m.view().song.tracks[0].slots[0].as_ref().unwrap().id;
    r.cmd(Command::SelectInsert { insert: None });
    r.settle();
    let lane = |r: &mut Rig, target| {
        r.cmd(Command::AddLane {
            home: meloet_protocol::LaneHome::Track { track },
            target,
        })
    };

    // The panel shows nothing: the plugin is asked for its list, once.
    let effects = lane(&mut r, AutoTarget::Instrument { param: MODE });
    assert_eq!(names_asks(&effects), [surge]);
    assert_eq!(lane_names(&r), [None]);
    let effects = r.cmd(Command::SelectInsert { insert: None });
    assert!(names_asks(&effects).is_empty());
    r.settle();
    assert_eq!(lane_names(&r), [Some("Mode".to_owned())]);
    assert_eq!(r.selected_or_none(), None, "the list was not the panel's");

    // The panel shows the compressor: the lane of the instrument is named all the same,
    // and the lane of the compressor from the table the panel has.
    let slot = InsertRef {
        strip: StripRef::Track { id: track },
        slot: 0,
    };
    r.cmd(Command::SelectInsert { insert: Some(slot) });
    r.settle();
    let table = r.selected().table;
    let effects = lane(
        &mut r,
        AutoTarget::Insert {
            insert,
            param: LOUD,
        },
    );
    assert!(names_asks(&effects).is_empty());
    let effects = lane(&mut r, AutoTarget::Instrument { param: LEVEL });
    assert_eq!(names_asks(&effects), [surge]);
    r.settle();
    let names = ["Mode", "Loud", "Level"].map(|name| Some(name.to_owned()));
    assert_eq!(lane_names(&r), names);
    assert_eq!(r.selected().table, table, "the panel keeps its table");

    // A parameter the plugin does not list has no name, and is not asked for again.
    let effects = lane(&mut r, AutoTarget::Instrument { param: 77 });
    assert_eq!(names_asks(&effects), [surge]);
    r.settle();
    assert_eq!(lane_names(&r)[3], None);
    let effects = r.cmd(Command::SelectInsert { insert: None });
    assert!(names_asks(&effects).is_empty());

    // The rows of the history name the parameter too, also of a lane that is gone.
    let target = AutoTarget::Instrument { param: MODE };
    r.cmd(Command::DeleteLane {
        home: meloet_protocol::LaneHome::Track { track },
        target,
    });
    let rows = r.m.history_rows(1, 99);
    let named: Vec<(StepName, Option<String>)> = (rows.into_iter())
        .filter_map(|row| match row.what {
            StepWhat::Lane {
                target: LaneName::Plugin { param, .. },
                ..
            } => Some((row.name, param)),
            _ => None,
        })
        .collect();
    let name = |name: &str| Some(name.to_owned());
    assert_eq!(
        named,
        [
            (StepName::AddLane, name("Mode")),
            (StepName::AddLane, name("Loud")),
            (StepName::AddLane, name("Level")),
            (StepName::AddLane, None),
            (StepName::DeleteLane, name("Mode")),
        ]
    );
}
