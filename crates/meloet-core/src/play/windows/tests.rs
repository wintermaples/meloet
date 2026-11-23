use std::collections::{HashMap, HashSet, VecDeque};

use meloet_model::{PluginPlace, StepKind, TrackId};
use meloet_plugin::wire::{ParamInfo, ParamNow, ParamShape, TextValue};
use meloet_protocol::{
    Command, EffectRef, InsertRef, ParamValues, PluginSlotRef, PluginWindowKind, PluginWindowView,
    SelectedParams, ShelfMode, StripRef,
};

use super::*;
use crate::catalog::tests::{COMP_LV2, COMP_VST3, SURGE_LV2, SURGE_VST3};
use crate::play::PlugState;
use crate::play::tests::status;
use crate::plugins::hosts::HostError;
use crate::state::added_tests::{loads, plugin, rigged};
use crate::state::{Input, Model, PluginSaid};

pub(crate) const LEVEL: u32 = 0;
pub(crate) const MODE: u32 = 1;
pub(crate) const LOUD: u32 = 2;
/// The first id of the parameters that only take a keyboard's knobs (`FakeHost::knobs`).
pub(crate) const KNOB: u32 = 1000;

/// What a plugin of the fake host starts with: its level, its mode, its switch.
pub(crate) const FRESH: [f64; 3] = [0.5, 0.0, 0.0];

/// What the plugins' thread and the plugin processes do with the effects: every loaded
/// plugin is one with three parameters, which are its whole state.
#[derive(Default)]
pub(crate) struct FakeHost {
    /// By the ticket of the load.
    pub plugins: HashMap<u64, [f64; 3]>,
    /// The plugins whose process stopped.
    pub dead: HashSet<u64>,
    /// The answers that are on their way to the core thread, in order. A plugin does what
    /// it is asked right away; what it answers gets to the rules later.
    pub answers: VecDeque<Input>,
    /// What went to the screen, with the time it went at.
    pub values: Vec<(u64, ParamValues)>,
    /// How often a state was put back into a plugin.
    pub states_put: u32,
    pub now_us: u64,
    /// How many parameters that only take a keyboard's knobs every plugin lists besides
    /// its three, from id `KNOB`.
    pub knobs: u32,
    /// Every `PluginAsk::Watch`, in order.
    pub watches: Vec<(u64, bool)>,
}

pub(crate) fn state_of(values: [f64; 3]) -> Arc<[u8]> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn values_of(state: &[u8]) -> [f64; 3] {
    let mut values = FRESH;
    if state.len() == 24 {
        for (value, bytes) in values.iter_mut().zip(state.chunks(8)) {
            *value = f64::from_le_bytes(bytes.try_into().unwrap());
        }
    }
    values
}

fn text_of(param: u32, value: f64) -> String {
    match param {
        LEVEL => format!("{:.0} %", value * 100.0),
        MODE => ["Soft", "Hard", "Odd"][(value * 2.0).round() as usize].to_string(),
        _ => if value >= 0.5 { "On" } else { "Off" }.to_string(),
    }
}

/// The value as the plugin takes it: a step of the mode, on or off for the switch.
fn taken(param: u32, value: f64) -> f64 {
    match param {
        LEVEL => value,
        MODE => (value * 2.0).round() / 2.0,
        _ => value.round(),
    }
}

fn now_of(param: u32, value: f64) -> ParamNow {
    ParamNow {
        id: param,
        value: Some(value),
        text: Some(text_of(param, value)),
    }
}

impl FakeHost {
    pub fn take(&mut self, effects: &[Effect]) {
        for effect in effects {
            match effect {
                Effect::LoadPlugin { ticket, state, .. } => {
                    let values = match state {
                        PlugState::Fresh => FRESH,
                        PlugState::Bytes(state) => values_of(state),
                        PlugState::Of(other) => self.plugins.get(other).copied().unwrap_or(FRESH),
                    };
                    self.plugins.insert(*ticket, values);
                }
                Effect::ReloadPlugin { ticket } => {
                    self.dead.remove(ticket);
                }
                Effect::SetPluginState { ticket, state } => {
                    self.plugins.insert(*ticket, values_of(state));
                    self.states_put += 1;
                }
                Effect::AskPlugin { ticket, ask } => {
                    let said = self.asked(*ticket, ask.clone());
                    let ticket = *ticket;
                    (self.answers).extend(said.map(|said| Input::PluginSaid { ticket, said }));
                }
                Effect::ParamValues(values) => self.values.push((self.now_us, values.clone())),
                _ => {}
            }
        }
    }

    pub fn params(&self, ticket: u64) -> Vec<ParamInfo> {
        let values = self.plugins.get(&ticket).copied().unwrap_or(FRESH);
        let row = |id: u32, name: &str, group: Option<&str>, shape, labels: &[&str]| ParamInfo {
            id,
            name: name.into(),
            group: group.map(str::to_owned),
            default: FRESH[id as usize],
            shape,
            labels: labels.iter().map(|l| l.to_string()).collect(),
            automatable: true,
            midi_cc: false,
            value: Some(values[id as usize]),
            text: Some(text_of(id, values[id as usize])),
        };
        let steps = ParamShape::Stepped { steps: 3 };
        let knob = |at: u32| ParamInfo {
            id: KNOB + at,
            name: format!("MIDI CC 0|{at}"),
            group: None,
            default: 0.0,
            shape: ParamShape::Continuous,
            labels: Vec::new(),
            automatable: false,
            midi_cc: true,
            value: Some(0.0),
            text: None,
        };
        let mut list = vec![row(LEVEL, "Level", None, ParamShape::Continuous, &[])];
        // Between the others, as a plugin may list them anywhere.
        list.extend((0..self.knobs).map(knob));
        list.extend([
            row(MODE, "Mode", Some("Shape"), steps, &["Soft", "Hard", "Odd"]),
            row(LOUD, "Loud", Some("Shape"), ParamShape::Toggle, &[]),
        ]);
        list
    }

    /// Does what the plugin is asked. `None`: it has no answer.
    fn asked(&mut self, ticket: u64, ask: PluginAsk) -> Option<PluginSaid> {
        let alive = self.plugins.contains_key(&ticket) && !self.dead.contains(&ticket);
        Some(match ask {
            PluginAsk::State(ask) => PluginSaid::State {
                ask,
                state: alive.then(|| state_of(self.plugins[&ticket])),
            },
            PluginAsk::Watch { on } => {
                self.watches.push((ticket, on));
                return None;
            }
            PluginAsk::PutState { state } => PluginSaid::StatePut(
                (self.put(ticket, alive, &state)).map(|()| state_of(self.plugins[&ticket])),
            ),
            _ if !alive => return None,
            PluginAsk::OpenWindow { .. } | PluginAsk::CloseWindow => return None,
            PluginAsk::KeyNames => PluginSaid::KeyNames(None),
            PluginAsk::Params { ask } => PluginSaid::Params {
                ask,
                params: self.params(ticket),
            },
            PluginAsk::SetParam { param, value } => {
                let value = taken(param, value);
                self.plugins.get_mut(&ticket)?[param as usize] = value;
                PluginSaid::Param(now_of(param, value))
            }
            PluginAsk::ReadText { param, text } => {
                let number = text.trim_end_matches(" %").parse::<f64>().ok();
                let read = number.filter(|_| param == LEVEL).map(|percent| {
                    let value = (percent / 100.0).clamp(0.0, 1.0);
                    TextValue {
                        value,
                        clamped: !(0.0..=100.0).contains(&percent),
                        text: Some(text_of(LEVEL, value)),
                        low: Some(text_of(LEVEL, 0.0)),
                        high: Some(text_of(LEVEL, 1.0)),
                    }
                });
                PluginSaid::TextRead { param, read }
            }
            PluginAsk::Texts { ask, param, values } => PluginSaid::Texts {
                ask,
                texts: (values.iter()).map(|v| Some(text_of(param, *v))).collect(),
            },
        })
    }

    /// A whole state put in: one of 24 bytes is taken; one that starts with `KILL` stops the
    /// plugin's process, one with `SKIP` is passed over; any other is refused.
    fn put(&mut self, ticket: u64, alive: bool, state: &[u8]) -> Result<(), HostError> {
        if !alive {
            return Err(HostError::Gone);
        }
        if state.starts_with(b"KILL") {
            self.dead.insert(ticket);
            return Err(HostError::Gone);
        }
        if state.len() != 24 {
            return Err(HostError::Refused("not a state of the fake".into()));
        }
        if state.starts_with(b"SKIP") {
            return Ok(());
        }
        self.plugins.insert(ticket, values_of(state));
        self.states_put += 1;
        Ok(())
    }

    /// The person turns a parameter in the plugin's window.
    pub fn turn(&mut self, ticket: u64, param: u32, value: f64) -> Input {
        let value = taken(param, value);
        self.plugins.get_mut(&ticket).expect("loaded")[param as usize] = value;
        let said = PluginSaid::Param(now_of(param, value));
        Input::PluginSaid { ticket, said }
    }
}

/// A model with the fake host behind it.
pub(crate) struct Rig {
    pub m: Model,
    pub host: FakeHost,
}

impl Rig {
    pub fn new() -> Rig {
        Rig {
            m: rigged(),
            host: FakeHost::default(),
        }
    }

    pub fn go(&mut self, input: Input) -> Vec<Effect> {
        self.m.set_now(self.host.now_us, None);
        let effects = self.m.handle(input);
        self.host.take(&effects);
        effects
    }

    pub fn cmd(&mut self, command: Command) -> Vec<Effect> {
        self.go(Input::Command(command))
    }

    pub fn said(&mut self, ticket: u64, said: PluginSaid) -> Vec<Effect> {
        self.go(Input::PluginSaid { ticket, said })
    }

    /// Every answer gets to the rules, and the answers to what they ask then too.
    pub fn settle(&mut self) {
        while let Some(input) = self.host.answers.pop_front() {
            self.go(input);
        }
    }

    /// Time passes, and the core thread wakes when it asked to.
    pub fn pass(&mut self, us: u64) {
        let until = self.host.now_us + us;
        while let Some(at) = self.m.wake_at_us().filter(|at| *at <= until) {
            self.host.now_us = at.max(self.host.now_us);
            self.go(Input::Tick);
        }
        self.host.now_us = until;
    }

    /// The load asked for by `effects` is answered.
    pub fn loaded(&mut self, effects: &[Effect]) -> u64 {
        let loads = loads(effects);
        assert_eq!(loads.len(), 1, "{effects:?}");
        self.go(Input::PluginLoaded { ticket: loads[0].0 });
        loads[0].0
    }

    /// The first track sounds with Surge XT. Returns the ticket, the track and the slot.
    pub fn surge(&mut self) -> (u64, u32, PluginSlotRef) {
        let track = self.m.view().song.tracks[0].id;
        self.cmd(Command::OpenShelf {
            mode: ShelfMode::Replace { track },
        });
        let sound = Some(plugin(SURGE_VST3));
        let effects = self.cmd(Command::ShelfSelectSound { sound });
        let ticket = self.loaded(&effects);
        self.cmd(Command::ShelfConfirm);
        (ticket, track, PluginSlotRef::Instrument { track })
    }

    /// A plugin effect in a slot of the track's strip, loaded. Returns its ticket.
    pub fn effect(&mut self, track: u32, slot: u8, key: &str) -> u64 {
        let strip = StripRef::Track { id: track };
        self.cmd(Command::OpenShelf {
            mode: ShelfMode::InsertEffect { strip, slot },
        });
        self.cmd(Command::ShelfChooseEffect {
            effect: Some(EffectRef::Plugin { key: key.into() }),
        });
        let effects = self.cmd(Command::ShelfConfirm);
        self.loaded(&effects)
    }

    pub fn windows(&self) -> Vec<PluginWindowView> {
        self.m.view().windows.list
    }

    pub fn undo_count(&self) -> u32 {
        self.m.view().song.undo_count
    }

    pub fn selected_or_none(&self) -> Option<SelectedParams> {
        self.m.view().params.selected
    }

    pub fn selected(&self) -> meloet_protocol::SelectedPlugin {
        match self.m.view().params.selected {
            Some(SelectedParams::Plugin(plugin)) => plugin,
            other => panic!("no plugin is selected: {other:?}"),
        }
    }
}

fn asks(effects: &[Effect]) -> Vec<(u64, PluginAsk)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::AskPlugin { ticket, ask } => Some((*ticket, ask.clone())),
            _ => None,
        })
        .collect()
}

fn open_ask(ticket: u64, title: &str) -> (u64, PluginAsk) {
    let title = title.into();
    (ticket, PluginAsk::OpenWindow { title })
}

fn opened(width: u32, height: u32, larger_than_screen: bool) -> PluginSaid {
    PluginSaid::WindowOpened {
        width,
        height,
        larger_than_screen,
    }
}

fn row(slot: Option<PluginSlotRef>, name: &str, size: (u32, u32), large: bool) -> PluginWindowView {
    PluginWindowView {
        slot,
        name: name.into(),
        width: size.0,
        height: size.1,
        larger_than_screen: large,
    }
}

#[test]
fn a_window_is_asked_for_said_when_slow_listed_when_up_and_stays_with_its_plugin() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    let strip = StripRef::Track { id: track };

    // Selecting the instrument asks for its window; the panel stays as it is.
    let effects = r.cmd(Command::SelectInstrument { track });
    assert_eq!(asks(&effects)[0], open_ask(surge, "Surge XT"));
    assert!(r.windows().is_empty(), "not up yet");
    let front = r.m.view().workspace.layout.right_front;
    assert_ne!(front, Some(meloet_protocol::Band::Params));

    // Not up after 0.2 s: the status line says so, until it is.
    let opening = StatusMessage::PluginWindowOpening {
        name: "Surge XT".into(),
        slot: Some(slot),
    };
    assert_eq!(r.m.wake_at_us(), Some(SLOW_US));
    r.pass(SLOW_US - 1);
    assert_ne!(status(&r.m), Some(opening.clone()));
    r.pass(1);
    assert_eq!(status(&r.m), Some(opening));
    assert_eq!(r.m.wake_at_us(), None);
    r.said(surge, opened(912, 569, false));
    assert_eq!(
        status(&r.m),
        Some(StatusMessage::PluginWindowOpen {
            name: "Surge XT".into()
        })
    );
    assert_eq!(
        r.windows(),
        [row(Some(slot), "Surge XT", (912, 569), false)]
    );

    // A second one, larger than the screen: said with its size.
    let comp = r.effect(track, 0, COMP_LV2);
    let at = |slot| InsertRef { strip, slot };
    let effects = r.cmd(Command::SelectInsert {
        insert: Some(at(0)),
    });
    assert_eq!(asks(&effects)[0], open_ask(comp, "ZamComp"));
    r.pass(SLOW_US / 2);
    r.said(comp, opened(3000, 2000, true));
    let in_slot = |slot| Some(PluginSlotRef::Insert { insert: at(slot) });
    assert_eq!(
        status(&r.m),
        Some(StatusMessage::PluginWindowLarge {
            name: "ZamComp".into(),
            slot: in_slot(0),
            width: 3000,
            height: 2000,
        })
    );
    let zam = |slot| row(in_slot(slot), "ZamComp", (3000, 2000), true);
    let surge_row = row(Some(slot), "Surge XT", (912, 569), false);
    assert_eq!(r.windows(), [surge_row.clone(), zam(0)]);

    // The insert is moved: the window is where its plugin is.
    r.cmd(Command::MoveInsert {
        insert: at(0),
        to: 2,
    });
    assert_eq!(r.windows(), [surge_row.clone(), zam(2)]);
    r.cmd(Command::Undo);
    assert_eq!(r.windows(), [surge_row.clone(), zam(0)]);

    // Asked for again it comes to the front, and all of them do in the order of the list.
    let effects = r.cmd(Command::OpenPluginWindow { slot: in_slot(0) });
    assert_eq!(asks(&effects), [open_ask(comp, "ZamComp")]);
    assert_eq!(r.windows().len(), 2);
    assert_eq!(r.m.wake_at_us(), None, "nothing is being opened");
    let effects = r.cmd(Command::RaisePluginWindows);
    let both = [open_ask(surge, "Surge XT"), open_ask(comp, "ZamComp")];
    assert_eq!(asks(&effects), both);

    let resized = PluginSaid::WindowResized {
        width: 800,
        height: 600,
        larger_than_screen: false,
    };
    r.said(comp, resized);
    assert_eq!(
        r.windows()[1],
        row(in_slot(0), "ZamComp", (800, 600), false)
    );

    // Closed with its own button; open again, the effect is taken out; the other's process
    // stops. No news comes of the last two.
    r.said(comp, PluginSaid::WindowClosed);
    assert_eq!(r.windows(), std::slice::from_ref(&surge_row));
    r.cmd(Command::OpenPluginWindow { slot: in_slot(0) });
    r.said(comp, opened(800, 600, false));
    assert_eq!(r.windows().len(), 2);
    r.cmd(Command::RemoveEffect { insert: at(0) });
    assert_eq!(r.windows(), [surge_row]);
    r.go(Input::PluginStopped { ticket: surge });
    assert!(r.windows().is_empty());
    let effects = r.cmd(Command::OpenPluginWindow { slot: Some(slot) });
    assert!(asks(&effects).is_empty(), "a stopped plugin opens nothing");
}

#[test]
fn a_window_that_fails_one_meloet_does_not_open_and_the_one_of_a_tried_row() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();

    // It could not be made: said so, and not said to be opening any more.
    r.cmd(Command::OpenPluginWindow { slot: Some(slot) });
    r.said(surge, PluginSaid::WindowFailed);
    let failed = StatusMessage::PluginWindowFailed {
        name: "Surge XT".into(),
    };
    assert_eq!(status(&r.m), Some(failed.clone()));
    r.pass(SLOW_US * 2);
    assert!(!matches!(
        status(&r.m),
        Some(StatusMessage::PluginWindowOpening { .. })
    ));
    assert!(r.windows().is_empty());
    // A window that was not asked for is not said.
    r.cmd(Command::Back);
    r.said(surge, PluginSaid::WindowFailed);
    assert_ne!(status(&r.m), Some(failed));

    // What the plugin says of its window when it is loaded counts over what the scan found.
    r.cmd(Command::SelectInstrument { track });
    assert_eq!(r.selected().window, PluginWindowKind::Opens);
    r.said(surge, PluginSaid::Editor(PluginWindowKind::NotYet));
    assert_eq!(r.selected().window, PluginWindowKind::NotYet);
    let effects = r.cmd(Command::SelectInstrument { track });
    assert!(
        !asks(&effects)
            .iter()
            .any(|(_, ask)| matches!(ask, PluginAsk::OpenWindow { .. }))
    );
    let effects = r.cmd(Command::OpenPluginWindow { slot: Some(slot) });
    assert!(asks(&effects).is_empty());
    // One the scan found without a window meloet opens.
    let comp = r.effect(track, 1, COMP_VST3);
    let insert = InsertRef {
        strip: StripRef::Track { id: track },
        slot: 1,
    };
    let in_slot = Some(PluginSlotRef::Insert { insert });
    assert!(asks(&r.cmd(Command::OpenPluginWindow { slot: in_slot })).is_empty());
    let _ = comp;

    // The row the shelf auditions sits in no slot; with Enter its window is its track's.
    r.cmd(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    assert!(asks(&r.cmd(Command::OpenPluginWindow { slot: None })).is_empty());
    let sound = Some(plugin(SURGE_LV2));
    let effects = r.cmd(Command::ShelfSelectSound { sound });
    assert!(asks(&r.cmd(Command::OpenPluginWindow { slot: None })).is_empty());
    let tried = r.loaded(&effects);
    let effects = r.cmd(Command::OpenPluginWindow { slot: None });
    assert_eq!(asks(&effects), [open_ask(tried, "Surge XT")]);
    r.said(tried, opened(912, 569, false));
    assert_eq!(r.windows(), [row(None, "Surge XT", (912, 569), false)]);
    // What is changed in it is no step: it is in no song yet.
    let steps = r.undo_count();
    r.said(tried, PluginSaid::WindowEntered);
    let turned = r.host.turn(tried, LEVEL, 0.8);
    r.go(turned);
    r.said(tried, PluginSaid::WindowLeft);
    r.settle();
    assert_eq!(r.undo_count(), steps);
    r.cmd(Command::ShelfConfirm);
    let new = r.m.view().song.tracks.last().unwrap().id;
    let slot = Some(PluginSlotRef::Instrument { track: new });
    assert_eq!(r.windows(), [row(slot, "Surge XT", (912, 569), false)]);
    // The audition ends another way: the window goes with the plugin.
    r.cmd(Command::DeleteTrack { track: new });
    assert!(r.windows().is_empty());
}

fn last_step(r: &Rig) -> StepKind {
    r.m.play().steps().last().cloned().expect("a step")
}

#[test]
fn a_visit_to_a_window_is_one_step_when_the_plugin_changed_and_none_when_it_did_not() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    let place = PluginPlace::Instrument(TrackId(track));
    r.cmd(Command::OpenPluginWindow { slot: Some(slot) });
    r.said(surge, opened(912, 569, false));
    let steps = r.undo_count();

    // In and out with nothing changed: no step.
    let effects = r.said(surge, PluginSaid::WindowEntered);
    let entered = PluginAsk::State(StateAsk::Entered(1));
    assert_eq!(asks(&effects), [(surge, entered)]);
    r.settle();
    let effects = r.said(surge, PluginSaid::WindowLeft);
    assert_eq!(
        asks(&effects),
        [(surge, PluginAsk::State(StateAsk::Left(1)))]
    );
    r.settle();
    assert_eq!(r.undo_count(), steps);

    // Two knobs turned many times in one visit: one step, and nothing is put back into
    // the plugin, which has it all already.
    r.said(surge, PluginSaid::WindowEntered);
    for n in 1..=40 {
        let turned = r.host.turn(surge, LEVEL, 0.5 + 0.01 * f64::from(n));
        r.go(turned);
    }
    let turned = r.host.turn(surge, MODE, 1.0);
    r.go(turned);
    assert_eq!(r.undo_count(), steps, "not before the person is back");
    r.said(surge, PluginSaid::WindowLeft);
    r.settle();
    assert_eq!(r.undo_count(), steps + 1);
    assert_eq!(last_step(&r), StepKind::PluginWindow { place });
    assert_eq!(r.host.states_put, 0);
    let in_song = |r: &Rig| r.m.play().song().doc.plugin(place).unwrap().state.clone();
    assert_eq!(in_song(&r), state_of([0.9, 1.0, 0.0]));

    // Undo: as before the visit, which is "as it starts" for the song. The plugin is given
    // the state it said then, not loaded anew. Redo: as after the visit.
    let effects = r.cmd(Command::Undo);
    assert!(loads(&effects).is_empty());
    assert_eq!(r.host.plugins[&surge], FRESH);
    assert!(in_song(&r).is_empty());
    assert_eq!(r.undo_count(), steps);
    r.cmd(Command::Redo);
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 0.0]);
    assert_eq!(r.host.states_put, 2);
    assert_eq!(r.windows().len(), 1, "the window stays through both");

    // The window is closed with its button while the person is in it: the visit ends there.
    r.said(surge, PluginSaid::WindowEntered);
    let turned = r.host.turn(surge, LOUD, 1.0);
    r.go(turned);
    r.said(surge, PluginSaid::WindowClosed);
    r.settle();
    assert_eq!(r.undo_count(), steps + 2);
    assert_eq!(in_song(&r), state_of([0.9, 1.0, 1.0]));
    r.cmd(Command::Undo);
    assert_eq!(r.host.plugins[&surge], [0.9, 1.0, 0.0]);

    // The process stops under the visit: it cannot say what it had, and nothing is a step
    // while it is stopped.
    r.cmd(Command::OpenPluginWindow { slot: Some(slot) });
    r.said(surge, opened(912, 569, false));
    r.said(surge, PluginSaid::WindowEntered);
    r.settle();
    let turned = r.host.turn(surge, LEVEL, 0.1);
    r.go(turned);
    r.host.dead.insert(surge);
    let steps = r.undo_count();
    r.go(Input::PluginStopped { ticket: surge });
    r.settle();
    assert_eq!(r.undo_count(), steps);
    assert!(r.windows().is_empty());
    // Loaded again, it comes back with what was turned in the window: the visit's step.
    r.cmd(Command::ReloadPlugin { slot });
    r.go(Input::PluginLoaded { ticket: surge });
    r.settle();
    assert_eq!(r.undo_count(), steps + 1);
    assert_eq!(last_step(&r), StepKind::PluginWindow { place });
    assert_eq!(in_song(&r), state_of([0.1, 1.0, 0.0]));
}

#[test]
fn the_panel_moved_under_an_open_visit_makes_the_windows_step_first_then_its_own() {
    let mut r = Rig::new();
    let (surge, track, slot) = r.surge();
    let place = PluginPlace::Instrument(TrackId(track));
    r.cmd(Command::SelectInstrument { track });
    r.said(surge, opened(912, 569, false));
    r.settle();
    let table = r.selected().table.expect("its table");
    let steps = r.undo_count();

    r.said(surge, PluginSaid::WindowEntered);
    let turned = r.host.turn(surge, LEVEL, 0.7);
    r.go(turned);
    // The keyboard is still in the window when a row of the panel is dragged and let go.
    for (value, commit) in [(0.4, false), (1.0, false), (1.0, true)] {
        r.cmd(Command::SetPluginParam {
            slot,
            table,
            param: LOUD,
            value,
            commit,
        });
    }
    r.settle();
    let made: Vec<StepKind> = r.m.play().steps()[steps as usize..].to_vec();
    let own = StepKind::SetPluginParam { place, param: LOUD };
    assert_eq!(made, [StepKind::PluginWindow { place }, own]);
    // The visit goes on from there: more turned in the window is one more step.
    let turned = r.host.turn(surge, MODE, 0.5);
    r.go(turned);
    r.said(surge, PluginSaid::WindowLeft);
    r.settle();
    assert_eq!(r.undo_count(), steps + 3);
    for back in [[0.7, 0.0, 1.0], [0.7, 0.0, 0.0], FRESH] {
        r.cmd(Command::Undo);
        r.settle();
        assert_eq!(r.host.plugins[&surge], back);
    }
    // Out of the window, a drag makes its one step and no visit starts.
    r.cmd(Command::Redo);
    r.settle();
    let table = r.selected().table.expect("its table");
    r.cmd(Command::SetPluginParam {
        slot,
        table,
        param: MODE,
        value: 1.0,
        commit: true,
    });
    r.settle();
    assert_eq!(
        last_step(&r),
        StepKind::SetPluginParam { place, param: MODE }
    );
    assert_eq!(r.undo_count(), steps + 2);
}
