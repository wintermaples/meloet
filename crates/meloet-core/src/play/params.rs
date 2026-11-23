//! The parameters of the plugin the parameters panel shows (docs/architecture.md 3-1 the
//! 10th, 20-12, 20-13).
//!
//! The list is asked of the plugin when it is selected and kept here as a table with a
//! number. The screen fetches the table once and then gets only the values that change, at
//! most 30 times a second. A new list (the plugin was selected again, said its values
//! changed, was given another state or loaded again) is a table with a new number; the old
//! one stays until the new one has come.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use meloet_model::effects::EffectKind;
use meloet_model::{EditError, PluginChange, PluginPlace, StripId, mixer};
use meloet_plugin::wire::{ParamInfo, ParamNow, ParamShape, TextValue};
use meloet_protocol::{
    ParamEnd, ParamGroup, ParamKind, ParamRef, ParamRow, ParamTable, ParamValue, ParamValues,
    PluginSlotRef, StatusMessage,
};

use super::Play;
use super::automation::{LetGo, NAMES_ASK};
use super::mixer::Picked;
use crate::ai::{AiParamNow, AiParams, AiParamsSet, AiRefusal, AiSetEnd};
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
    /// The plugin whose process was told that the panel shows it (`PluginAsk::Watch`).
    watched: Option<u64>,
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
    /// Lists the AI link asked for, by the plugin's ticket, until it takes them.
    ai_lists: HashMap<u64, Vec<ParamInfo>>,
    /// The plugins asked for a list for the AI link, and when (`now_us`).
    ai_asked: Vec<(u64, u64)>,
    /// The sets of parameters the AI link asked for, the newest last; the last few that
    /// ended are kept for it to read.
    ai_sets: VecDeque<AiSet>,
    ai_sets_made: u64,
    /// Texts of values the AI link asked plugins for, until it takes them.
    ai_texts: Vec<AiTexts>,
    ai_texts_asked: u64,
}

/// Parameters of one plugin the AI link set, until the step of its state is made or not.
#[derive(Debug)]
struct AiSet {
    ticket: u64,
    /// The plugin, by the ticket of its load.
    plugin: u64,
    params: Vec<AiParamNow>,
    /// Which of `params` the plugin has answered for.
    answered: Vec<bool>,
    /// With the undo count then.
    end: Option<(AiSetEnd, u32)>,
}

#[derive(Debug)]
struct AiTexts {
    ask: u64,
    plugin: u64,
    param: u32,
    values: Vec<f64>,
    asked_us: u64,
    texts: Option<Vec<Option<String>>>,
}

/// The `ask` of a list of parameters for the AI link.
const AI_PARAMS_ASK: u64 = u64::MAX - 1;
/// How long a list asked for the AI link is waited for before it is asked again.
const AI_ASK_AGAIN_US: u64 = 2_000_000;
/// Sets of the AI link that ended and are kept for it to read.
const AI_SETS_KEPT: usize = 16;
/// Sets that have not ended are dropped beyond this many (a plugin that never answers).
const AI_SETS_MOST: usize = 64;

fn ai_param(info: ParamInfo) -> crate::ai::AiParam {
    let steps = match info.shape {
        ParamShape::Continuous => 0,
        ParamShape::Stepped { steps } => steps,
        ParamShape::Toggle => 2,
    };
    crate::ai::AiParam {
        id: info.id,
        name: info.name,
        group: info.group,
        value: info.value,
        default: info.default,
        text: info.text,
        steps,
        labels: info.labels,
        automatable: info.automatable,
        midi_cc: info.midi_cc,
    }
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
        let lane = self.ticket_lane(ticket, param);
        if !commit {
            if let Some(key) = lane {
                self.hand_moved(key, value as f32, effects);
            }
            return;
        }
        let let_go = match lane {
            Some(key) => self.hand_let_go(key, value as f32, false, effects),
            None => LetGo::Free,
        };
        match let_go {
            // The lane gives the plugin its values again.
            LetGo::Written | LetGo::Nothing => {}
            LetGo::Rest(key) => {
                self.rest_with_plugin_state(ticket, param, key);
                self.param_set(ticket, param, effects);
            }
            LetGo::Free => self.param_set(ticket, param, effects),
        }
    }

    /// The ticket of the plugin at `place`, when it is loaded and sounds.
    fn ai_ticket(&self, place: PluginPlace) -> Result<u64, AiRefusal> {
        self.doc().plugin(place).ok_or(AiRefusal::NoSuchPlugin)?;
        (self.plugin_tickets().into_iter())
            .find(|(p, _)| *p == place)
            .map(|(_, ticket)| ticket)
            .ok_or(AiRefusal::PluginNotLoaded)
    }

    /// The AI link reads the parameters of the plugin at `place`: the list the plugin gave,
    /// or the plugin is asked for one.
    pub(crate) fn ai_params(
        &mut self,
        place: PluginPlace,
        effects: &mut Vec<Effect>,
    ) -> Result<AiParams, AiRefusal> {
        let ticket = self.ai_ticket(place)?;
        if let Some(list) = self.panel.ai_lists.remove(&ticket) {
            return Ok(AiParams::Listed(list.into_iter().map(ai_param).collect()));
        }
        let now = self.now_us;
        let asked = &mut self.panel.ai_asked;
        asked.retain(|(_, at)| now.saturating_sub(*at) < AI_ASK_AGAIN_US);
        if !asked.iter().any(|(t, _)| *t == ticket) {
            asked.push((ticket, now));
            effects.push(Effect::AskPlugin {
                ticket,
                ask: PluginAsk::Params { ask: AI_PARAMS_ASK },
            });
        }
        Ok(AiParams::Waiting)
    }

    /// The AI link sets a parameter of the plugin at `place`, as the panel does when a value
    /// is typed: the step comes with the plugin's state. Not for a parameter a lane moves.
    /// Returns the ticket of the set.
    pub(crate) fn ai_set_plugin_param(
        &mut self,
        place: PluginPlace,
        param: u32,
        value: f64,
        effects: &mut Vec<Effect>,
    ) -> Result<u64, AiRefusal> {
        let ticket = self.ai_ticket(place)?;
        let key = self.ticket_lane(ticket, param);
        if key.is_some_and(|key| self.lane_at(key).is_some()) {
            return Err(AiRefusal::HasLane);
        }
        self.ai_set_plugin_params(place, &[(param, value)], effects)
    }

    /// The AI link sets parameters of the plugin at `place`, every one before the plugin is
    /// asked for its state once: one step. Nothing is set when a lane moves one of them.
    /// Returns the ticket of the set; [`Play::ai_params_set`] tells how it ended.
    pub(crate) fn ai_set_plugin_params(
        &mut self,
        place: PluginPlace,
        values: &[(u32, f64)],
        effects: &mut Vec<Effect>,
    ) -> Result<u64, AiRefusal> {
        let plugin = self.ai_ticket(place)?;
        let Some(&(first, _)) = values.first() else {
            return Err(AiRefusal::Unchanged);
        };
        if values.iter().any(|(_, value)| !value.is_finite()) {
            return Err(AiRefusal::Unchanged);
        }
        for &(param, _) in values {
            let key = self.ticket_lane(plugin, param);
            if key.is_some_and(|key| self.lane_at(key).is_some()) {
                return Err(AiRefusal::LaneDrives(param));
            }
        }
        self.before_param(plugin, effects);
        for &(param, value) in values {
            let value = value.clamp(0.0, 1.0);
            let ask = PluginAsk::SetParam { param, value };
            effects.push(Effect::AskPlugin {
                ticket: plugin,
                ask,
            });
        }
        self.panel.ai_sets_made += 1;
        let ticket = self.panel.ai_sets_made;
        let puts = self.plugs.of(plugin).map_or(0, |plug| plug.puts);
        let ask = StateAsk::Param {
            param: first,
            puts,
            ai: Some(ticket),
        };
        effects.push(Effect::AskPlugin {
            ticket: plugin,
            ask: PluginAsk::State(ask),
        });
        let params = (values.iter())
            .map(|&(id, asked)| AiParamNow {
                id,
                asked: asked.clamp(0.0, 1.0),
                value: None,
                text: None,
            })
            .collect();
        let sets = &mut self.panel.ai_sets;
        sets.push_back(AiSet {
            ticket,
            plugin,
            answered: vec![false; values.len()],
            params,
            end: None,
        });
        while sets.iter().filter(|s| s.end.is_some()).count() > AI_SETS_KEPT
            || sets.len() > AI_SETS_MOST
        {
            let at = sets.iter().position(|s| s.end.is_some()).unwrap_or(0);
            sets.remove(at);
        }
        Ok(ticket)
    }

    /// How the AI link's set of `ticket` ended, or `Waiting`.
    pub(crate) fn ai_params_set(&self, ticket: u64) -> Result<AiParams, AiRefusal> {
        let set = (self.panel.ai_sets.iter())
            .find(|s| s.ticket == ticket)
            .ok_or(AiRefusal::NoSuchSet)?;
        Ok(match &set.end {
            None => AiParams::Waiting,
            Some((end, undo_count)) => AiParams::Set(AiParamsSet {
                params: set.params.clone(),
                end: end.clone(),
                undo_count: *undo_count,
            }),
        })
    }

    fn ai_set_ended(&mut self, ticket: u64, end: AiSetEnd) {
        let undo_count = self.undo_count();
        if let Some(set) = self.panel.ai_sets.iter_mut().find(|s| s.ticket == ticket) {
            set.end = Some((end, undo_count));
        }
    }

    /// The AI link asks the plugin at `place` for its texts of some values of a parameter:
    /// the texts once it said them, else `Waiting`.
    pub(crate) fn ai_param_texts(
        &mut self,
        place: PluginPlace,
        param: u32,
        values: Vec<f64>,
        effects: &mut Vec<Effect>,
    ) -> Result<AiParams, AiRefusal> {
        let plugin = self.ai_ticket(place)?;
        let same = |t: &AiTexts| t.plugin == plugin && t.param == param && t.values == values;
        let texts = &mut self.panel.ai_texts;
        if let Some(at) = texts.iter().position(|t| same(t) && t.texts.is_some()) {
            let said = texts.remove(at).texts.unwrap_or_default();
            return Ok(AiParams::Texts(said));
        }
        let now = self.now_us;
        texts.retain(|t| now.saturating_sub(t.asked_us) < AI_ASK_AGAIN_US);
        if !texts.iter().any(same) {
            self.panel.ai_texts_asked += 1;
            let ask = self.panel.ai_texts_asked;
            let values_asked = values.clone();
            texts.push(AiTexts {
                ask,
                plugin,
                param,
                values,
                asked_us: now,
                texts: None,
            });
            effects.push(Effect::AskPlugin {
                ticket: plugin,
                ask: PluginAsk::Texts {
                    ask,
                    param,
                    values: values_asked,
                },
            });
        }
        Ok(AiParams::Waiting)
    }

    /// The AI link sets parameters of the built-in effect in a slot, each brought into its
    /// range, as one step. Nothing is set when a lane moves one of them. `false`: every value
    /// was so already.
    pub(crate) fn ai_set_effect_params(
        &mut self,
        strip: StripId,
        slot: usize,
        values: &[(u32, f32)],
        effects: &mut Vec<Effect>,
    ) -> Result<bool, AiRefusal> {
        let insert = self.doc().insert(strip, slot).ok_or(AiRefusal::SlotEmpty)?;
        let kind: EffectKind = insert.kind().ok_or(AiRefusal::NoSuchEffect)?;
        for &(param, value) in values {
            kind.param(param).ok_or(AiRefusal::NoSuchParam(param))?;
            if !value.is_finite() {
                return Err(AiRefusal::Unchanged);
            }
            if let Some((key, _)) = self.param_lane(strip, slot, kind, param, value)
                && self.lane_at(key).is_some()
            {
                return Err(AiRefusal::LaneDrives(param));
            }
        }
        let mut doc = self.doc().clone();
        let mut step = None;
        for &(param, value) in values {
            match mixer::set_param(&doc, strip, slot, kind, param, value) {
                Ok(edit) => {
                    doc = edit.doc;
                    step.get_or_insert(edit.step);
                }
                Err(EditError::Unchanged) => {}
                Err(_) => return Err(AiRefusal::NoSuchEffect),
            }
        }
        let Some(step) = step else {
            return Ok(false);
        };
        self.apply(meloet_model::Edit { doc, step }, effects);
        Ok(true)
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
        let ai = None;
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::State(StateAsk::Param { param, puts, ai }),
        });
    }

    /// Esc during a drag: the plugin gets back the value it had.
    pub(super) fn cancel_plugin_drags(&mut self, effects: &mut Vec<Effect>) {
        if let Some(drag) = self.panel.dragged.take() {
            if let Some(key) = self.ticket_lane(drag.ticket, drag.param) {
                self.drop_grip(key, effects);
            }
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
        // A typed value rests the lane, 「● 書く」 or not (docs/architecture.md 21-11).
        if let Some(key) = self.ticket_lane(ticket, param)
            && self.lane_at(key).is_some_and(|l| l.moves())
        {
            self.rest_with_plugin_state(ticket, param, key);
        }
        self.param_set(ticket, param, effects);
        clamped
    }

    /// A parameter of a plugin has this value now. Kept for the next `ParamValues` when the
    /// panel shows the plugin; a parameter that is not in its list is passed over.
    fn param_now(&mut self, ticket: u64, now: ParamNow) {
        // The answers of one plugin come in the order asked: the oldest set waiting for this
        // one takes it; a value said after that (moved in the window) goes to the newest.
        let sets = &mut self.panel.ai_sets;
        let open = |s: &AiSet, answered: bool| {
            s.plugin == ticket
                && s.end.is_none()
                && (s.params.iter().zip(&s.answered)).any(|(p, a)| p.id == now.id && *a == answered)
        };
        let at = (sets.iter().position(|s| open(s, false)))
            .or_else(|| sets.iter().rposition(|s| open(s, true)));
        if let Some(set) = at.map(|at| &mut sets[at]) {
            let rows = set.params.iter_mut().zip(set.answered.iter_mut());
            for (param, answered) in rows.filter(|(p, _)| p.id == now.id) {
                (param.value, param.text, *answered) = (now.value, now.text.clone(), true);
            }
        }
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

    /// A row of the table the panel shows, when it is the table of the plugin at `place`.
    pub(super) fn shown_param(&self, place: PluginPlace, param: u32) -> Option<&ParamInfo> {
        let table = self.panel.table.as_ref()?;
        let plug = self.plugs.of(self.panel.ticket?)?;
        (self.place_of(plug.spot) == Some(place)).then_some(())?;
        Some(&table.rows[*table.at.get(&param)?])
    }

    pub(super) fn has_param(&self, param: u32) -> bool {
        (self.panel.table.as_ref()).is_some_and(|t| t.at.contains_key(&param))
    }

    /// The key of the plugin whose table the panel shows.
    pub(super) fn shown_table_key(&self) -> Option<&Arc<str>> {
        self.panel.table.as_ref()?;
        Some(&self.plugs.of(self.panel.ticket?)?.plugin.key)
    }

    /// The parameters of the selected plugin at `place` that have a lane, in the order of
    /// the table.
    pub(super) fn automated_rows(&self, place: PluginPlace) -> Vec<u32> {
        let (
            Some(table),
            PluginPlace::Instrument(track) | PluginPlace::Insert(StripId::Track(track), _),
        ) = (self.panel.table.as_ref(), place)
        else {
            return Vec::new();
        };
        let lanes = self.doc().track(track).map(|t| &t.automation[..]);
        let mut rows: Vec<usize> = (lanes.unwrap_or_default().iter())
            .filter_map(|lane| match self.plugin_of_lane(track, lane.target)? {
                (at, param) if at == place => table.at.get(&param).copied(),
                _ => None,
            })
            .collect();
        rows.sort_unstable();
        rows.into_iter().map(|at| table.rows[at].id).collect()
    }

    /// The name of a parameter of the plugin `ticket`, when the panel lists that plugin.
    pub(super) fn panel_param_name(&self, ticket: u64, param: u32) -> Option<String> {
        let listed = self.panel.ticket == Some(ticket);
        (self.param_name(param).filter(|_| listed)).map(str::to_owned)
    }

    pub(super) fn param_name(&self, param: u32) -> Option<&str> {
        let table = self.panel.table.as_ref()?;
        Some(&table.rows[*table.at.get(&param)?].name)
    }

    /// The number of the table the panel shows and how many parameters it has.
    pub(super) fn table_shown(&self) -> Option<(u32, u32)> {
        let table = self.panel.table.as_ref()?;
        let count = table.rows.iter().filter(|row| !row.midi_cc).count();
        Some((table.number, count as u32))
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
        // Only the plugin the panel shows tells what the lanes do to its parameters.
        let watched = ticket.filter(|_| sounds);
        if watched != panel.watched {
            let watch = |ticket, on| Effect::AskPlugin {
                ticket,
                ask: PluginAsk::Watch { on },
            };
            effects.extend(panel.watched.map(|ticket| watch(ticket, false)));
            effects.extend(watched.map(|ticket| watch(ticket, true)));
            panel.watched = watched;
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
        let mut keyboard_knobs = Vec::new();
        for info in &shown.rows {
            let row = ParamRow {
                id: info.id,
                name: info.name.clone(),
                kind: param_kind(info),
                default: info.default,
                value: info.value,
                text: info.text.clone(),
            };
            if info.midi_cc {
                keyboard_knobs.push(row);
                continue;
            }
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
            keyboard_knobs,
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
            PluginSaid::ParamTouch { param, on, .. } => {
                self.plugin_touched(ticket, param, on, effects);
            }
            PluginSaid::ParamMoved {
                param,
                value,
                heard,
            } => self.plugin_edited(ticket, param, value, heard, effects),
            PluginSaid::WindowTouches { touches } => self.plugin_hands(ticket, touches),
            PluginSaid::ParamsChanged => self.table_stale(ticket),
            PluginSaid::Params {
                ask: NAMES_ASK,
                params,
            } => self.lane_names_listed(ticket, &params),
            PluginSaid::Params {
                ask: AI_PARAMS_ASK,
                params,
            } => {
                self.panel.ai_asked.retain(|(t, _)| *t != ticket);
                self.panel.ai_lists.insert(ticket, params);
            }
            PluginSaid::Params { ask, params } => self.params_listed(ticket, ask, params),
            PluginSaid::State {
                ask: StateAsk::Copy(serial),
                state,
            } => {
                let (track, states) = self.copy_state_said(ticket, serial, state)?;
                return self.make_copy(track, states, effects);
            }
            PluginSaid::State { ask, state } => self.state_said(ticket, ask, state, effects),
            PluginSaid::KeyNames(names) => self.key_names_said(ticket, names),
            PluginSaid::StatePut(put) => self.ai_state_put(ticket, put, effects),
            PluginSaid::TextRead { param, read } => {
                return self.text_read(ticket, param, read, effects);
            }
            PluginSaid::Texts { ask, texts } => {
                if let Some(asked) = self.panel.ai_texts.iter_mut().find(|t| t.ask == ask) {
                    asked.texts = Some(texts);
                }
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
            (StateAsk::Ai, state) => self.ai_state_said(ticket, state),
            (StateAsk::Param { param, puts, ai }, Some(state)) => {
                let (steps, plug) = (self.undo_count(), self.plugs.of(ticket));
                let placed = plug.is_some_and(|plug| self.place_of(plug.spot).is_some());
                let current = plug.is_some_and(|plug| plug.puts == puts);
                if current {
                    self.visit_goes_on_from(ticket, &state);
                    self.plugin_changed(ticket, state, PluginChange::Param(param), effects);
                }
                if let Some(ai) = ai {
                    let now = self.undo_count();
                    let end = match (placed, current) {
                        (false, _) => AiSetEnd::Gone,
                        (true, false) => AiSetEnd::Overtaken,
                        _ if now > steps => {
                            AiSetEnd::Step(self.history_rows(now, now).pop().map(Box::new))
                        }
                        _ => AiSetEnd::Same,
                    };
                    self.ai_set_ended(ai, end);
                }
            }
            (StateAsk::Param { ai: Some(ai), .. }, None) => {
                self.ai_set_ended(ai, AiSetEnd::NoState)
            }
            (StateAsk::Base | StateAsk::Param { .. }, None) | (StateAsk::Copy(_), _) => {}
            (StateAsk::Entered(_) | StateAsk::Left(_), state) => {
                self.visit_state(ticket, ask, state, effects);
            }
        }
    }
}

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod tests;
