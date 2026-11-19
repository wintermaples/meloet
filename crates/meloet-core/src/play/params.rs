//! The parameters of the plugin the parameters panel shows (docs/architecture.md 3-1 the
//! 10th, 20-12, 20-13).
//!
//! The list is asked of the plugin when it is selected and kept here as a table with a
//! number. The screen fetches the table once and then gets only the values that change, at
//! most 30 times a second. A new list (the plugin was selected again, said its values
//! changed, was given another state or loaded again) is a table with a new number; the old
//! one stays until the new one has come.

use std::collections::HashMap;
use std::sync::Arc;

use meloet_model::PluginChange;
use meloet_plugin::wire::{ParamInfo, ParamNow, ParamShape, TextValue};
use meloet_protocol::{
    ParamEnd, ParamGroup, ParamKind, ParamRef, ParamRow, ParamTable, ParamValue, ParamValues,
    PluginSlotRef, StatusMessage,
};

use super::Play;
use super::mixer::Picked;
use crate::state::{Effect, PluginAsk, PluginSaid, StateAsk};

/// The least time between two `ParamValues` messages: 30 a second at most.
pub(super) const VALUES_EVERY_US: u64 = 33_334;

#[derive(Debug)]
struct Table {
    number: u32,
    /// In the plugin's order.
    rows: Vec<ParamInfo>,
    /// Where in `rows` the parameter of an id is.
    at: HashMap<u32, usize>,
    /// The last `ParamValues::seq` sent.
    seq: u64,
    /// The rows whose value changed since then.
    changed: Vec<usize>,
    sent_us: Option<u64>,
}

/// The parameter that is dragged in the panel: what the plugin hears is not in the song
/// yet. One at a time: when another one is moved, this drag is over.
#[derive(Debug)]
struct Dragged {
    ticket: u64,
    param: u32,
    /// Its value before the drag, which Esc puts back.
    was: Option<f64>,
}

#[derive(Debug, Default)]
pub(super) struct Panel {
    /// The plugin that is selected, by the ticket of its load.
    ticket: Option<u64>,
    table: Option<Table>,
    /// The list is to be asked for (again).
    stale: bool,
    /// The number of the last ask; the answer to an older one is dropped.
    asked: u64,
    tables: u32,
    dragged: Option<Dragged>,
    /// Drags that ended while their plugin was stopped: it comes back with the dragged
    /// value when it is loaded again, and is given its own then.
    owed: Vec<Dragged>,
    /// A step is being made of a change the plugin has already: the drags go on.
    own_step: bool,
}

fn param_kind(info: &ParamInfo) -> ParamKind {
    match info.shape {
        ParamShape::Continuous => ParamKind::Smooth,
        ParamShape::Stepped { steps } => ParamKind::Steps {
            steps,
            labels: info.labels.clone(),
        },
        ParamShape::Toggle => ParamKind::Toggle,
    }
}

/// The value as a plugin that gives no text for it is shown.
fn number(value: f64) -> String {
    format!("{value:.2}")
}

impl Play {
    /// The selected plugin when it sounds and the screen has its table `table`, with the
    /// row of `param`.
    fn table_row(&self, slot: PluginSlotRef, table: u32, param: u32) -> Option<(u64, &ParamInfo)> {
        let Some(Picked::Plugin(place, _)) = self.selected_pick() else {
            return None;
        };
        let ticket = self.panel.ticket?;
        let plug = self.plugs.of(ticket).filter(|plug| plug.sounds())?;
        let shown = self.panel.table.as_ref().filter(|t| t.number == table)?;
        let same = *place == Self::slot_place(slot) && self.spot_of(*place) == Some(plug.spot);
        let row = &shown.rows[*shown.at.get(&param)?];
        same.then_some((ticket, row))
    }

    /// `SetPluginParam`.
    pub fn set_plugin_param(
        &mut self,
        slot: PluginSlotRef,
        table: u32,
        param: u32,
        value: f64,
        commit: bool,
        effects: &mut Vec<Effect>,
    ) {
        let Some((ticket, row)) = self.table_row(slot, table, param) else {
            return;
        };
        if !value.is_finite() {
            return;
        }
        let (was, value) = (row.value, value.clamp(0.0, 1.0));
        let going =
            (self.panel.dragged.as_ref()).is_some_and(|d| (d.ticket, d.param) == (ticket, param));
        if !going {
            self.before_param(ticket, effects);
        }
        if commit {
            self.panel.dragged = None;
        } else if !going {
            self.panel.dragged = Some(Dragged { ticket, param, was });
        }
        let ask = PluginAsk::SetParam { param, value };
        effects.push(Effect::AskPlugin { ticket, ask });
        if commit {
            self.param_set(ticket, param, effects);
        }
    }

    /// Before the panel changes a parameter that it is not dragging already: a drag of
    /// another one is over, what the plugin's window changed until now is a step of its
    /// own, and a plugin the song has "as it starts" says what that is. So the state the
    /// step is made of holds this one change.
    fn before_param(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        self.cancel_plugin_drags(effects);
        self.cut_visit(ticket, effects);
        self.want_base(ticket, effects);
    }

    /// Whether a parameter of the plugin is being dragged in the panel.
    pub(super) fn dragging(&self, ticket: u64) -> bool {
        (self.panel.dragged.as_ref()).is_some_and(|d| d.ticket == ticket)
    }

    /// The panel set a parameter for good: the plugin's state is asked for, and the answer
    /// makes the step.
    fn param_set(&mut self, ticket: u64, param: u32, effects: &mut Vec<Effect>) {
        let puts = self.plugs.of(ticket).map_or(0, |plug| plug.puts);
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::State(StateAsk::Param { param, puts }),
        });
    }

    /// Esc during a drag: the plugin gets back the value it had.
    pub(super) fn cancel_plugin_drags(&mut self, effects: &mut Vec<Effect>) {
        if let Some(drag) = self.panel.dragged.take() {
            self.panel.owed.push(drag);
            self.pay_owed(effects);
        }
    }

    /// Gives every plugin that sounds the value it had before a drag that is over. A visit
    /// to its window that started under the drag starts anew from there.
    fn pay_owed(&mut self, effects: &mut Vec<Effect>) {
        let plugs = &self.plugs;
        let mut paid = Vec::new();
        self.panel.owed.retain(|drag| {
            let (ticket, param) = (drag.ticket, drag.param);
            match (plugs.of(ticket), drag.was) {
                (Some(plug), Some(value)) if plug.sounds() => {
                    let ask = PluginAsk::SetParam { param, value };
                    effects.push(Effect::AskPlugin { ticket, ask });
                    paid.push(ticket);
                    false
                }
                (Some(_), Some(_)) => true,
                _ => false,
            }
        });
        for ticket in paid {
            self.visit_anew(ticket, effects);
        }
    }

    /// A step that puts into the song what the plugin has already. The drags of the panel
    /// go on over it.
    pub(super) fn apply_own(&mut self, edit: meloet_model::Edit, effects: &mut Vec<Effect>) {
        self.panel.own_step = true;
        self.apply(edit, effects);
        self.panel.own_step = false;
    }

    /// The song changed: as for every preview, a drag is over and the plugin has its own
    /// value again.
    pub(super) fn song_changed_under_drags(&mut self, effects: &mut Vec<Effect>) {
        if !self.panel.own_step {
            self.cancel_plugin_drags(effects);
        }
    }

    /// `TypeParam` for a plugin: the plugin reads the text, and its answer sets the value.
    pub(super) fn type_plugin_param(
        &mut self,
        slot: PluginSlotRef,
        table: u32,
        param: u32,
        text: &str,
        effects: &mut Vec<Effect>,
    ) {
        if let Some((ticket, _)) = self.table_row(slot, table, param) {
            let text = text.to_owned();
            let ask = PluginAsk::ReadText { param, text };
            effects.push(Effect::AskPlugin { ticket, ask });
        }
    }

    /// The plugin read a typed text. Returns what the status line says of it.
    fn text_read(
        &mut self,
        ticket: u64,
        param: u32,
        read: Option<TextValue>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let Some(Picked::Plugin(place, _)) = self.selected_pick() else {
            return None;
        };
        let (slot, (table, _)) = (Self::slot_ref(*place), self.table_shown()?);
        let (_, row) = (self.table_row(slot, table, param)).filter(|(t, _)| *t == ticket)?;
        let Some(read) = read else {
            let kept = (row.text.clone())
                .or_else(|| row.value.map(number))
                .unwrap_or_default();
            let param = ParamRef::Plugin { slot, table, param };
            return Some(StatusMessage::ParamRejected { param, kept });
        };
        let clamped = read.clamped.then(|| {
            let end = |text: Option<String>, value: f64| text.unwrap_or_else(|| number(value));
            StatusMessage::ParamClamped {
                to: ParamEnd::Plugin {
                    name: row.name.clone(),
                    value: end(read.text, read.value),
                    range: format!("{}〜{}", end(read.low, 0.0), end(read.high, 1.0)),
                },
            }
        });
        self.before_param(ticket, effects);
        let value = read.value.clamp(0.0, 1.0);
        let ask = PluginAsk::SetParam { param, value };
        effects.push(Effect::AskPlugin { ticket, ask });
        self.param_set(ticket, param, effects);
        clamped
    }

    /// A parameter of a plugin has this value now. Kept for the next `ParamValues` when the
    /// panel shows the plugin; a parameter that is not in its list is passed over.
    fn param_now(&mut self, ticket: u64, now: ParamNow) {
        if self.panel.ticket != Some(ticket) {
            return;
        }
        let Some(table) = &mut self.panel.table else {
            return;
        };
        let Some(&at) = table.at.get(&now.id) else {
            return;
        };
        let row = &mut table.rows[at];
        if (row.value, &row.text) != (now.value, &now.text) {
            (row.value, row.text) = (now.value, now.text);
            if !table.changed.contains(&at) {
                table.changed.push(at);
            }
        }
    }

    /// The table of `ticket`, if the panel shows it, is to be read again.
    pub(super) fn table_stale(&mut self, ticket: u64) {
        if self.panel.ticket == Some(ticket) {
            self.panel.stale = true;
        }
    }

    /// The plugin listed its parameters: a table with a new number.
    fn params_listed(&mut self, ticket: u64, ask: u64, params: Vec<ParamInfo>) {
        let panel = &mut self.panel;
        if panel.ticket != Some(ticket) || ask != panel.asked {
            return;
        }
        panel.tables += 1;
        let at = (params.iter().enumerate())
            .map(|(at, p)| (p.id, at))
            .collect();
        panel.table = Some(Table {
            number: panel.tables,
            rows: params,
            at,
            seq: 0,
            changed: Vec::new(),
            sent_us: None,
        });
        let row = self.mixer_row();
        if row.is_some_and(|row| !self.has_param(row)) {
            self.select_param(None);
        }
    }

    pub(super) fn has_param(&self, param: u32) -> bool {
        (self.panel.table.as_ref()).is_some_and(|t| t.at.contains_key(&param))
    }

    pub(super) fn param_name(&self, param: u32) -> Option<&str> {
        let table = self.panel.table.as_ref()?;
        Some(&table.rows[*table.at.get(&param)?].name)
    }

    /// The number of the table the panel shows and how many parameters it has.
    pub(super) fn table_shown(&self) -> Option<(u32, u32)> {
        let table = self.panel.table.as_ref()?;
        Some((table.number, table.rows.len() as u32))
    }

    /// After every input: the table follows what is selected, a list that is wanted is asked
    /// for once its plugin sounds, and the values that changed are sent when it is time.
    pub fn sync_panel(&mut self, effects: &mut Vec<Effect>) {
        let selected = match self.selected_pick() {
            Some(Picked::Plugin(place, key)) => (self.spot_of(*place))
                .and_then(|spot| self.plugs.at(spot))
                .filter(|plug| plug.plugin.key == *key),
            _ => None,
        };
        let (ticket, sounds) = (
            selected.map(|p| p.ticket),
            selected.is_some_and(|p| p.sounds()),
        );
        let dragged = self.panel.dragged.as_ref().map(|drag| drag.ticket);
        if dragged.is_some_and(|ticket| !self.plugs.of(ticket).is_some_and(|plug| plug.sounds())) {
            self.cancel_plugin_drags(effects);
        }
        self.pay_owed(effects);
        let panel = &mut self.panel;
        if ticket != panel.ticket {
            panel.ticket = ticket;
            panel.table = None;
            panel.stale = ticket.is_some();
        }
        if let (Some(ticket), true, true) = (ticket, panel.stale, sounds) {
            panel.stale = false;
            panel.asked += 1;
            let ask = PluginAsk::Params { ask: panel.asked };
            effects.push(Effect::AskPlugin { ticket, ask });
        }
        let now_us = self.now_us;
        if let Some(table) = &mut panel.table
            && !table.changed.is_empty()
            && (table.sent_us).is_none_or(|sent| now_us >= sent + VALUES_EVERY_US)
        {
            table.seq += 1;
            table.sent_us = Some(now_us);
            let values = (table.changed.drain(..))
                .map(|at| {
                    let row = &table.rows[at];
                    ParamValue {
                        id: row.id,
                        value: row.value,
                        text: row.text.clone(),
                    }
                })
                .collect();
            effects.push(Effect::ParamValues(ParamValues {
                table: table.number,
                seq: table.seq,
                values,
            }));
        }
    }

    /// When an `Input::Tick` is wanted next: for values that wait to be sent, and for a
    /// window that is slow to open.
    pub fn wake_at_us(&self) -> Option<u64> {
        let values = (self.panel.table.as_ref())
            .filter(|table| !table.changed.is_empty())
            .and_then(|table| Some(table.sent_us? + VALUES_EVERY_US));
        [values, self.windows_wake_us()].into_iter().flatten().min()
    }

    /// The reply of `plugin_params`.
    pub fn plugin_params(&self, table: u32) -> Option<ParamTable> {
        let shown = self.panel.table.as_ref().filter(|t| t.number == table)?;
        let mut groups: Vec<ParamGroup> = Vec::new();
        for info in &shown.rows {
            let row = ParamRow {
                id: info.id,
                name: info.name.clone(),
                kind: param_kind(info),
                default: info.default,
                value: info.value,
                text: info.text.clone(),
            };
            match groups.iter_mut().find(|g| g.name == info.group) {
                Some(group) => group.params.push(row),
                None => groups.push(ParamGroup {
                    name: info.group.clone(),
                    params: vec![row],
                }),
            }
        }
        Some(ParamTable {
            table,
            seq: shown.seq,
            groups,
        })
    }

    /// Something the plugin loaded under `ticket` did by itself, or its answer to an ask.
    /// Returns what the status line says of it.
    pub fn plugin_said(
        &mut self,
        ticket: u64,
        said: PluginSaid,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        match said {
            PluginSaid::Editor(window) => self.plug_window_said(ticket, window),
            PluginSaid::WindowOpened {
                width,
                height,
                larger_than_screen,
            } => return self.window_opened(ticket, (width, height), larger_than_screen),
            PluginSaid::WindowFailed => self.window_failed(ticket),
            PluginSaid::WindowResized {
                width,
                height,
                larger_than_screen,
            } => self.window_resized(ticket, (width, height), larger_than_screen),
            PluginSaid::WindowClosed => self.window_closed(ticket, effects),
            PluginSaid::WindowEntered => self.window_entered(ticket, effects),
            PluginSaid::WindowLeft => self.window_left(ticket, effects),
            PluginSaid::Param(now) => self.param_now(ticket, now),
            PluginSaid::ParamsChanged => self.table_stale(ticket),
            PluginSaid::Params { ask, params } => self.params_listed(ticket, ask, params),
            PluginSaid::State { ask, state } => self.state_said(ticket, ask, state, effects),
            PluginSaid::TextRead { param, read } => {
                return self.text_read(ticket, param, read, effects);
            }
        }
        None
    }

    fn state_said(
        &mut self,
        ticket: u64,
        ask: StateAsk,
        state: Option<Arc<[u8]>>,
        effects: &mut Vec<Effect>,
    ) {
        match (ask, state) {
            (StateAsk::Base, Some(state)) => self.base_said(ticket, &state),
            (StateAsk::Param { param, puts }, Some(state)) => {
                if self.plugs.of(ticket).is_some_and(|plug| plug.puts == puts) {
                    self.visit_goes_on_from(ticket, &state);
                    self.plugin_changed(ticket, state, PluginChange::Param(param), effects);
                }
            }
            (StateAsk::Base | StateAsk::Param { .. }, None) => {}
            (StateAsk::Entered(_) | StateAsk::Left(_), state) => {
                self.visit_state(ticket, ask, state, effects);
            }
        }
    }
}

#[cfg(test)]
mod tests;
