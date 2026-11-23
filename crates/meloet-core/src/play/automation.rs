//! Automation lanes (spec 3 Y16; docs/architecture.md 21-5, 21-8 to 21-11): the commands on
//! them, what of them is no step (folded, 「● 書く」, the hands on them, the selected points),
//! and what a hand on what a lane moves does to it.
//!
//! A hand on a fader, a pan, a send or a parameter whose lane moves it:
//! - with 「● 書く」 on while the song plays, writes: each value it sounds, at the place that
//!   was heard as it came (21-9), and when it is let go what it wrote is one step
//!   (`WriteAutomation`) and the value the hand put stays where it was;
//! - otherwise puts the lane to rest: the value and the rest are one step.
//!
//! While a hand is on it the lane is held, so that the engine does not move what the hand
//! moves ([`Play::hold_lane`]).

use std::collections::HashMap;
use std::sync::Arc;

use meloet_model::automation::{self as auto, AutoPoint, AutoTarget as ModelTarget, LaneId};
use meloet_model::edit::{Edit, EditError};
use meloet_model::effects::{EffectKind as ModelEffect, ParamShape};
use meloet_model::{Bus, Gain, InstrumentRef, Pan, PluginPlace, StepKind, StripId, TrackId};
use meloet_protocol::{
    AutoSelection, AutoTarget, AutomationView, LaneHome, LaneName, LanePoint, LanePoints, LaneRef,
    LaneView, SendBus, StatusMessage, TransportState, WritingLane,
};

use super::mixer::effect_kind;
use super::{Heard, Play, index_ranges};
use crate::state::{Effect, PluginAsk};

/// The `ask` of a list of parameters that is asked for the names of the lanes' parameters,
/// not for the parameters panel (whose asks count up from 1).
pub(super) const NAMES_ASK: u64 = u64::MAX;

/// What the plugins call the parameters that lanes move. Only those are kept (Surge XT
/// lists 2854), by the plugin's key: every plugin of one kind calls them the same. The
/// names come from the table of the parameters panel when it shows a plugin of that kind,
/// else one plugin of the kind is asked for its list, once. They are kept for the rows of
/// the history that tell of lanes that are gone, and from song to song.
#[derive(Debug, Default)]
pub(super) struct LaneNames {
    /// By key, then by the plugin's own number. `None`: the plugin lists no such parameter.
    known: HashMap<Arc<str>, HashMap<u32, Option<String>>>,
    /// The plugins asked for their lists, by ticket, until they answer or stop.
    asked: Vec<u64>,
}

impl LaneNames {
    fn knows(&self, key: &str, param: u32) -> bool {
        (self.known.get(key)).is_some_and(|names| names.contains_key(&param))
    }

    pub(super) fn get(&self, key: &str, param: u32) -> Option<String> {
        self.known.get(key)?.get(&param)?.clone()
    }

    fn learn(&mut self, key: &Arc<str>, param: u32, name: Option<String>) {
        (self.known.entry(key.clone()).or_default()).insert(param, name);
    }
}

/// A lane as core names it: its track and what it moves.
pub(super) type LaneKey = (TrackId, ModelTarget);

/// What one hand writes: the values it sounded, in each pass of the loop.
#[derive(Debug, Clone, Default)]
struct Writing {
    /// By lap, the oldest first.
    passes: Vec<(u32, Vec<AutoPoint>)>,
}

impl Writing {
    fn add(&mut self, lap: u32, tick: u64, value: f32) {
        let point = AutoPoint { tick, value };
        match self.passes.last_mut() {
            Some((l, points)) if *l == lap => points.push(point),
            _ => self.passes.push((lap, vec![point])),
        }
    }

    fn first_tick(&self) -> Option<u64> {
        self.passes.first()?.1.first().map(|p| p.tick)
    }

    /// Whether it went on into a later pass of the loop.
    fn looped(&self) -> bool {
        self.passes.len() > 1
    }
}

#[derive(Debug, Clone)]
enum Hand {
    /// The lane rests once the hand is let go.
    Rest,
    Write(Writing),
    /// It wrote, and the song stopped: what is left of it goes nowhere.
    Spent,
}

/// A hand on a control of the mixer or the parameters panel.
#[derive(Debug)]
struct Grip {
    lane: LaneKey,
    hand: Hand,
}

/// A parameter moved by hand in a plugin's own window (docs/architecture.md 21-10).
#[derive(Debug)]
struct Touch {
    ticket: u64,
    param: u32,
    lane: LaneKey,
    hand: Hand,
    /// The plugin says the hand is on it.
    down: bool,
}

/// Selected points: of the lane whose points are `lane` now, by their places, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PointSel {
    lane: LaneId,
    points: Vec<u32>,
}

#[derive(Debug, Default)]
pub(super) struct Auto {
    /// 「● 書く」 is on. Not saved; off when the song stops, and for a lane at rest.
    armed: Vec<LaneKey>,
    grips: Vec<Grip>,
    touches: Vec<Touch>,
    /// Values set in the parameters panel whose step comes when the plugin says its state:
    /// the lane rests with it.
    rest_with_state: Vec<(u64, u32, LaneKey)>,
    selected: Option<PointSel>,
    /// The song played or recorded at the last look.
    was_moving: bool,
    notice: Option<StatusMessage>,
    pub(super) names: LaneNames,
}

/// What a hand let go of its control does with the song.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum LetGo {
    /// No lane moves the control: as before.
    Free,
    /// The value goes into the song with the lane at rest, as one step.
    Rest(LaneKey),
    /// The written points are a step, made already; the value the hand put stays.
    Written,
    /// Nothing goes into the song: the song stopped while the hand wrote.
    Nothing,
}

pub(super) fn model_target(target: AutoTarget) -> ModelTarget {
    match target {
        AutoTarget::Volume => ModelTarget::Volume,
        AutoTarget::Pan => ModelTarget::Pan,
        AutoTarget::Send { bus } => ModelTarget::Send(match bus {
            SendBus::A => Bus::A,
            SendBus::B => Bus::B,
        }),
        AutoTarget::Mute => ModelTarget::Mute,
        AutoTarget::Instrument { param } => ModelTarget::Instrument { param },
        AutoTarget::Insert { insert, param } => ModelTarget::Insert {
            insert: meloet_model::InsertId(insert),
            param,
        },
    }
}

pub(super) fn protocol_target(target: ModelTarget) -> AutoTarget {
    match target {
        ModelTarget::Volume => AutoTarget::Volume,
        ModelTarget::Pan => AutoTarget::Pan,
        ModelTarget::Send(bus) => AutoTarget::Send {
            bus: match bus {
                Bus::A => SendBus::A,
                Bus::B => SendBus::B,
            },
        },
        ModelTarget::Mute => AutoTarget::Mute,
        ModelTarget::Instrument { param } => AutoTarget::Instrument { param },
        ModelTarget::Insert { insert, param } => AutoTarget::Insert {
            insert: insert.0,
            param,
        },
    }
}

fn lane_ref((track, target): LaneKey) -> LaneRef {
    LaneRef {
        home: LaneHome::Track { track: track.0 },
        target: protocol_target(target),
    }
}

impl Play {
    // ---- what the commands and the hands name ----

    pub(super) fn lane_at(&self, (track, target): LaneKey) -> Option<&auto::Lane> {
        self.doc().track(track)?.lane(target).map(|l| &**l)
    }

    fn can_write(&self) -> bool {
        matches!(
            self.state,
            TransportState::Playing | TransportState::Recording
        )
    }

    fn armed(&self, key: LaneKey) -> bool {
        self.auto.armed.contains(&key)
    }

    /// Where a value moved now goes: the pass and the place that is heard.
    fn heard_place(heard: Option<Heard>) -> Option<(u32, u64)> {
        let h = heard?;
        Some((h.lap, h.moving_tick()?))
    }

    /// The place of the knob where the hand put the value: what a lane with no points, or
    /// at rest, leaves sounding. For a plugin, what the parameters panel shows of it, or the
    /// middle when it shows another.
    pub(super) fn hand_place(&self, (track, target): LaneKey) -> f32 {
        let Some(t) = self.doc().track(track) else {
            return 0.0;
        };
        match target {
            ModelTarget::Volume => t.channel.gain.position(),
            ModelTarget::Pan => t.channel.pan.position(),
            ModelTarget::Send(bus) => t.sends[bus.index()].position(),
            ModelTarget::Mute => f32::from(u8::from(t.muted)),
            ModelTarget::Insert { insert, param } => {
                let held = t.channel.inserts.iter().flatten().find(|i| i.id == insert);
                let spec = held.and_then(|i| Some((i.kind()?.param(param)?, i.value(param)?)));
                match spec {
                    Some((spec, value)) => spec.position(value),
                    None => self.plugin_place(track, target).unwrap_or(0.5),
                }
            }
            ModelTarget::Instrument { .. } => self.plugin_place(track, target).unwrap_or(0.5),
        }
    }

    fn plugin_place(&self, track: TrackId, target: ModelTarget) -> Option<f32> {
        let (place, param) = self.plugin_of_lane(track, target)?;
        Some(self.shown_param(place, param)?.value? as f32)
    }

    /// Where the plugin a lane moves sits, and its own number for the parameter.
    pub(super) fn plugin_of_lane(
        &self,
        track: TrackId,
        target: ModelTarget,
    ) -> Option<(PluginPlace, u32)> {
        match target {
            ModelTarget::Instrument { param } => Some((PluginPlace::Instrument(track), param)),
            ModelTarget::Insert { insert, param } => {
                let t = self.doc().track(track)?;
                let slot = t.channel.inserts.iter().position(|i| {
                    i.as_ref()
                        .is_some_and(|i| i.id == insert && i.plugin_ref().is_some())
                })?;
                Some((PluginPlace::Insert(StripId::Track(track), slot), param))
            }
            _ => None,
        }
    }

    /// The lane a parameter of a plugin of the song would have.
    pub(super) fn plugin_lane(&self, place: PluginPlace, param: u32) -> Option<LaneKey> {
        match place {
            PluginPlace::Instrument(track) => Some((track, ModelTarget::Instrument { param })),
            PluginPlace::Insert(StripId::Track(track), slot) => {
                let insert = self.doc().insert(StripId::Track(track), slot)?.id;
                Some((track, ModelTarget::Insert { insert, param }))
            }
            PluginPlace::Insert(..) => None,
        }
    }

    pub(super) fn ticket_lane(&self, ticket: u64, param: u32) -> Option<LaneKey> {
        let plug = self.plugs.of(ticket)?;
        self.plugin_lane(self.place_of(plug.spot)?, param)
    }

    /// The lane an effect parameter of meloet in a slot of a track would have, and the
    /// place of `value` on its knob.
    pub(super) fn param_lane(
        &self,
        strip: StripId,
        slot: usize,
        kind: ModelEffect,
        param: u32,
        value: f32,
    ) -> Option<(LaneKey, f32)> {
        let StripId::Track(track) = strip else {
            return None;
        };
        let insert = self.doc().insert(strip, slot)?.id;
        let key = (track, ModelTarget::Insert { insert, param });
        Some((key, kind.param(param)?.position(value)))
    }

    /// Lanes of the fader, pan and sends of a track.
    pub(super) fn gain_lane(strip: StripId, gain: Gain) -> Option<(LaneKey, f32)> {
        match strip {
            StripId::Track(t) => Some(((t, ModelTarget::Volume), gain.position())),
            _ => None,
        }
    }

    pub(super) fn pan_lane(strip: StripId, pan: Pan) -> Option<(LaneKey, f32)> {
        match strip {
            StripId::Track(t) => Some(((t, ModelTarget::Pan), pan.position())),
            _ => None,
        }
    }

    pub(super) fn send_lane(track: TrackId, bus: Bus, gain: Gain) -> Option<(LaneKey, f32)> {
        Some(((track, ModelTarget::Send(bus)), gain.position()))
    }

    /// What a lane moves, as its header and the status line name it.
    pub(super) fn lane_name(&self, (track, target): LaneKey) -> Option<LaneName> {
        let t = self.doc().track(track)?;
        Some(match target {
            ModelTarget::Volume => LaneName::Volume,
            ModelTarget::Pan => LaneName::Pan,
            ModelTarget::Send(bus) => LaneName::Send {
                bus: match bus {
                    Bus::A => SendBus::A,
                    Bus::B => SendBus::B,
                },
            },
            ModelTarget::Mute => LaneName::Mute,
            ModelTarget::Instrument { param } => match &t.instrument {
                InstrumentRef::Plugin(p) => LaneName::Plugin {
                    plugin: p.name.to_string(),
                    param: self.plugin_param_name(PluginPlace::Instrument(track), param),
                },
                _ => return None,
            },
            ModelTarget::Insert { insert, param } => {
                let slot = t
                    .channel
                    .inserts
                    .iter()
                    .position(|i| i.as_ref().is_some_and(|i| i.id == insert))?;
                let held = t.channel.inserts[slot].as_ref()?;
                match (held.kind(), held.plugin_ref()) {
                    (Some(kind), _) => LaneName::Builtin {
                        effect: effect_kind(kind),
                        param,
                    },
                    (None, Some(p)) => LaneName::Plugin {
                        plugin: p.name.to_string(),
                        param: self.plugin_param_name(
                            PluginPlace::Insert(StripId::Track(track), slot),
                            param,
                        ),
                    },
                    (None, None) => return None,
                }
            }
        })
    }

    /// The name of a parameter a lane moves. The names are gathered at the end of an input;
    /// for a lane made within it, the table of the panel is looked at.
    fn plugin_param_name(&self, place: PluginPlace, param: u32) -> Option<String> {
        let known = (self.doc().plugin(place)).and_then(|p| self.auto.names.get(&p.key, param));
        known.or_else(|| self.shown_param(place, param).map(|row| row.name.clone()))
    }

    /// The lanes of the song that move a parameter of a plugin: the plugin's key and its
    /// own number for the parameter.
    fn plugin_lanes(&self) -> impl Iterator<Item = (&Arc<str>, u32)> {
        self.doc().tracks.iter().flat_map(move |t| {
            t.automation.iter().filter_map(move |l| {
                let (place, param) = self.plugin_of_lane(t.id, l.target)?;
                Some((&self.doc().plugin(place)?.key, param))
            })
        })
    }

    /// Gathers the names of the parameters that have lanes and no name yet.
    fn sync_lane_names(&mut self, effects: &mut Vec<Effect>) {
        let plugs = &self.plugs;
        (self.auto.names.asked).retain(|&t| plugs.of(t).is_some_and(|plug| plug.sounds()));
        let names = &self.auto.names;
        let mut wanted = (self.plugin_lanes())
            .filter(|(key, param)| !names.knows(key, *param))
            .peekable();
        if wanted.peek().is_none() {
            return;
        }
        let shown = self.shown_table_key();
        let mut learned = Vec::new();
        let mut ask: Vec<u64> = Vec::new();
        for (key, param) in wanted {
            let listed = (shown.filter(|shown| *shown == key)).and_then(|_| self.param_name(param));
            if let Some(name) = listed {
                learned.push((key.clone(), param, name.to_owned()));
                continue;
            }
            let of_kind = |t: &u64| plugs.of(*t).is_some_and(|plug| plug.plugin.key == *key);
            if names.asked.iter().chain(&ask).any(of_kind) {
                continue;
            }
            let sounding = (plugs.list.iter()).find(|p| p.plugin.key == *key && p.sounds());
            ask.extend(sounding.map(|plug| plug.ticket));
        }
        for (key, param, name) in learned {
            self.auto.names.learn(&key, param, Some(name));
        }
        for ticket in ask {
            self.auto.names.asked.push(ticket);
            effects.push(Effect::AskPlugin {
                ticket,
                ask: PluginAsk::Params { ask: NAMES_ASK },
            });
        }
    }

    /// A plugin listed its parameters for the names of the lanes' parameters.
    pub(super) fn lane_names_listed(
        &mut self,
        ticket: u64,
        params: &[meloet_plugin::wire::ParamInfo],
    ) {
        self.auto.names.asked.retain(|&asked| asked != ticket);
        let Some(of) = self.plugs.of(ticket).map(|plug| plug.plugin.key.clone()) else {
            return;
        };
        let wanted: Vec<u32> = (self.plugin_lanes())
            .filter(|(key, param)| **key == of && !self.auto.names.knows(key, *param))
            .map(|(_, param)| param)
            .collect();
        for param in wanted {
            let name = params
                .iter()
                .find(|p| p.id == param)
                .map(|p| p.name.clone());
            self.auto.names.learn(&of, param, name);
        }
    }

    fn rested_notice(&mut self, key: LaneKey) {
        if let (Some(track), Some(target)) = (self.track_ref(key.0), self.lane_name(key)) {
            self.auto.notice = Some(StatusMessage::AutomationRested { track, target });
        }
    }

    /// What the status line is to say of the lanes, once.
    pub fn take_auto_notice(&mut self) -> Option<StatusMessage> {
        self.auto.notice.take()
    }

    // ---- commands ----

    /// `AddLane`. A lane the track has already is unfolded, which is no step.
    pub fn add_lane(&mut self, track: u32, target: AutoTarget, effects: &mut Vec<Effect>) {
        let (track, target) = (TrackId(track), model_target(target));
        let Some(t) = self.doc().track(track) else {
            return;
        };
        if t.lane(target).is_some() {
            self.song
                .session
                .folded_lanes
                .retain(|k| *k != (track, target));
            return;
        }
        let step = self.steps_of(track, target);
        if let Ok((e, _)) = auto::add_lane(self.doc(), track, target, step) {
            self.apply(e, effects);
        }
    }

    /// Whether the value of `target` comes in steps (a stepped value, a switch, the mute).
    fn steps_of(&self, track: TrackId, target: ModelTarget) -> bool {
        match target {
            ModelTarget::Volume | ModelTarget::Pan | ModelTarget::Send(_) => false,
            ModelTarget::Mute => true,
            ModelTarget::Insert { insert, param } => {
                let t = self.doc().track(track);
                let held =
                    t.and_then(|t| t.channel.inserts.iter().flatten().find(|i| i.id == insert));
                match held.and_then(|i| i.kind()).and_then(|k| k.param(param)) {
                    Some(spec) => !matches!(spec.shape, ParamShape::Continuous),
                    None => self.plugin_steps(track, target),
                }
            }
            ModelTarget::Instrument { .. } => self.plugin_steps(track, target),
        }
    }

    fn plugin_steps(&self, track: TrackId, target: ModelTarget) -> bool {
        use meloet_plugin::wire::ParamShape as Shape;
        let row = (self.plugin_of_lane(track, target)).and_then(|(p, n)| self.shown_param(p, n));
        row.is_some_and(|row| !matches!(row.shape, Shape::Continuous))
    }

    /// `DeleteLane`. A lane that moved its target leaves it at the value of the line at the
    /// play line, so that the sound does not jump (docs/history/design-notes.ja.md B-92). A plugin keeps what
    /// the lane last gave it.
    pub fn delete_lane(&mut self, track: u32, target: AutoTarget, effects: &mut Vec<Effect>) {
        let key = (TrackId(track), model_target(target));
        let Ok(mut e) = auto::delete_lane(self.doc(), key.0, key.1) else {
            return;
        };
        let at = self.play_line();
        if let Some(value) = (self.lane_at(key))
            .filter(|l| l.moves())
            .and_then(|l| l.value_at(at))
        {
            settle(&mut e, key, value);
        }
        self.apply(e, effects);
    }

    /// `FoldLane`. Not a step.
    pub fn fold_lane(&mut self, track: u32, target: AutoTarget, folded: bool) {
        let key = (TrackId(track), model_target(target));
        let session = &mut self.song.session;
        session.folded_lanes.retain(|k| *k != key);
        if folded && self.lane_at(key).is_some() {
            self.song.session.folded_lanes.push(key);
        }
    }

    /// `AddAutoPoint`: on the grid of the piano roll, counted from the head of the meter's
    /// stretch as quantize counts it.
    pub fn add_auto_point(
        &mut self,
        lane: u32,
        tick: u64,
        value: Option<f32>,
        effects: &mut Vec<Effect>,
    ) {
        let Some((t, l)) = self.doc().lane(LaneId(lane)) else {
            return;
        };
        let flat = self.hand_place((t.id, l.target));
        let tick = self.on_grid(tick);
        if let Ok((e, id, at)) = auto::add_point(self.doc(), LaneId(lane), tick, value, flat) {
            self.apply_points(e, id, vec![at as u32], effects);
        }
    }

    fn on_grid(&self, tick: u64) -> u64 {
        let tick = tick.min(meloet_model::MAX_TICK);
        let map = self.map();
        let (start, meter, next) = map.meter_stretch(tick);
        let step = self.song.session.roll.grid.ticks(meter).max(1);
        let n = (tick - start + step / 2) / step;
        let at = start.saturating_add(n.saturating_mul(step));
        next.map_or(at, |next| at.min(next))
            .min(meloet_model::MAX_TICK)
    }

    pub fn move_auto_points(
        &mut self,
        lane: u32,
        points: &[u32],
        d_tick: i64,
        d_value: f32,
        effects: &mut Vec<Effect>,
    ) {
        let picked: Vec<usize> = points.iter().map(|&i| i as usize).collect();
        if let Ok((e, id, at)) =
            auto::move_points(self.doc(), LaneId(lane), &picked, d_tick, d_value)
        {
            let at = at.into_iter().map(|i| i as u32).collect();
            self.apply_points(e, id, at, effects);
        }
    }

    pub fn delete_auto_points(&mut self, lane: u32, points: &[u32], effects: &mut Vec<Effect>) {
        let picked: Vec<usize> = points.iter().map(|&i| i as usize).collect();
        if let Ok((e, id)) = auto::delete_points(self.doc(), LaneId(lane), &picked) {
            self.apply_points(e, id, Vec::new(), effects);
        }
    }

    /// A step on points, which leaves `selected` of the lane's new points selected.
    fn apply_points(
        &mut self,
        e: Edit,
        lane: LaneId,
        selected: Vec<u32>,
        effects: &mut Vec<Effect>,
    ) {
        self.apply(e, effects);
        self.auto.selected = (!selected.is_empty()).then_some(PointSel {
            lane,
            points: selected,
        });
        let now = self.sel();
        if let Some(last) = self.sel_undo.last_mut() {
            last.1 = now;
        }
    }

    /// `SelectAutoPoints`. Not a step.
    pub fn select_auto_points(&mut self, lane: u32, points: Vec<u32>) {
        let Some((_, l)) = self.doc().lane(LaneId(lane)) else {
            return;
        };
        let len = l.points.len() as u32;
        let mut points: Vec<u32> = points.into_iter().filter(|&i| i < len).collect();
        points.sort_unstable();
        points.dedup();
        self.auto.selected = (!points.is_empty()).then_some(PointSel {
            lane: LaneId(lane),
            points,
        });
    }

    pub(super) fn point_sel(&self) -> Option<PointSel> {
        self.auto.selected.clone()
    }

    pub(super) fn put_point_sel(&mut self, sel: Option<PointSel>) {
        self.auto.selected = sel;
    }

    /// `SetLaneWrite`. Nothing for a lane at rest, or one that is not there.
    pub fn set_lane_write(&mut self, track: u32, target: AutoTarget, on: bool) {
        let key = (TrackId(track), model_target(target));
        self.auto.armed.retain(|k| *k != key);
        if on && self.lane_at(key).is_some_and(|l| !l.resting) {
            self.auto.armed.push(key);
        }
    }

    /// `RestoreLane` 「もどす」. Returns what the status line says.
    pub fn restore_lane(
        &mut self,
        track: u32,
        target: AutoTarget,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let key = (TrackId(track), model_target(target));
        let e = auto::restore_lane(self.doc(), key.0, key.1).ok()?;
        self.apply(e, effects);
        Some(StatusMessage::AutomationRestored {
            track: self.track_ref(key.0)?,
            target: self.lane_name(key)?,
        })
    }

    // ---- hands ----

    /// A control a lane may move was moved, and only the sound changes (`commit: false`).
    /// `place`: where the knob is now.
    pub(super) fn hand_moved(&mut self, key: LaneKey, place: f32, effects: &mut Vec<Effect>) {
        let heard = Self::heard_place(self.heard);
        if let Some(grip) = self.auto.grips.iter_mut().find(|g| g.lane == key) {
            if let (Hand::Write(w), Some((lap, tick))) = (&mut grip.hand, heard) {
                w.add(lap, tick, place);
            }
            return;
        }
        let Some(lane) = self.lane_at(key).filter(|l| !l.resting) else {
            return;
        };
        let hand = if self.armed(key) && self.can_write() {
            let mut w = Writing::default();
            if let Some((lap, tick)) = heard {
                w.add(lap, tick, place);
            }
            Hand::Write(w)
        } else if lane.moves() {
            Hand::Rest
        } else {
            return;
        };
        self.auto.grips.push(Grip { lane: key, hand });
        self.hold_lane(key.0, key.1, true, effects);
    }

    /// The control is let go, or a value is set with no drag before it (`commit: true`).
    /// `typed`: the value was typed, which rests the lane even while 「● 書く」 is on.
    pub(super) fn hand_let_go(
        &mut self,
        key: LaneKey,
        place: f32,
        typed: bool,
        effects: &mut Vec<Effect>,
    ) -> LetGo {
        let heard = Self::heard_place(self.heard);
        if let Some(at) = self.auto.grips.iter().position(|g| g.lane == key) {
            let grip = self.auto.grips.remove(at);
            self.hold_lane(key.0, key.1, false, effects);
            return match grip.hand {
                Hand::Rest if self.lane_at(key).is_some_and(|l| !l.resting) => LetGo::Rest(key),
                Hand::Rest => LetGo::Free,
                Hand::Write(mut w) => {
                    if let Some((lap, tick)) = heard {
                        w.add(lap, tick, place);
                    }
                    self.write_down(key, &w, effects);
                    LetGo::Written
                }
                Hand::Spent => LetGo::Nothing,
            };
        }
        let Some(lane) = self.lane_at(key).filter(|l| !l.resting) else {
            return LetGo::Free;
        };
        if !typed && lane.step && self.armed(key) && self.can_write() {
            if let Some((_, tick)) = heard
                && let Ok((e, _)) = auto::write_press(self.doc(), key.0, key.1, tick, place)
            {
                self.apply(e, effects);
            }
            return LetGo::Written;
        }
        if lane.moves() {
            LetGo::Rest(key)
        } else {
            LetGo::Free
        }
    }

    /// Esc during a drag: the hands on the mixer and the panel are let go with no step, and
    /// their lanes move again.
    pub(super) fn cancel_hands(&mut self, effects: &mut Vec<Effect>) {
        for grip in std::mem::take(&mut self.auto.grips) {
            self.hold_lane(grip.lane.0, grip.lane.1, false, effects);
        }
    }

    /// A hand whose drag was cut short: let go with no step.
    pub(super) fn drop_grip(&mut self, key: LaneKey, effects: &mut Vec<Effect>) {
        let before = self.auto.grips.len();
        self.auto.grips.retain(|g| g.lane != key);
        if self.auto.grips.len() != before {
            self.hold_lane(key.0, key.1, false, effects);
        }
    }

    /// The step of what a hand wrote.
    fn write_down(&mut self, key: LaneKey, w: &Writing, effects: &mut Vec<Effect>) {
        let passes: Vec<Vec<AutoPoint>> = w.passes.iter().map(|(_, p)| p.clone()).collect();
        let flat = self.hand_place(key);
        if let Ok((e, _)) = auto::write(self.doc(), key.0, key.1, &passes, flat) {
            self.apply(e, effects);
        }
    }

    /// Puts `edit` into the song with the lanes of `rests` at rest, as one step; with no
    /// edit (the value was the song's already), the rest alone. The status line says so.
    pub(super) fn apply_resting(
        &mut self,
        edit: Option<Edit>,
        rests: &[LaneKey],
        own: bool,
        effects: &mut Vec<Effect>,
    ) {
        let edit = match edit {
            Some(mut e) => {
                for &(track, target) in rests {
                    auto::set_resting(&mut e.doc, track, target, true);
                }
                Some(e)
            }
            None => {
                let mut doc = self.doc().clone();
                let rested: Vec<&LaneKey> = (rests.iter())
                    .filter(|&&(track, target)| auto::set_resting(&mut doc, track, target, true))
                    .collect();
                rested.first().map(|&&(track, target)| Edit {
                    doc,
                    step: StepKind::RestLane { track, target },
                })
            }
        };
        let Some(e) = edit else {
            return;
        };
        if own {
            self.apply_own(e, effects);
        } else {
            self.apply(e, effects);
        }
        if let Some(&key) = rests.first() {
            self.rested_notice(key);
        }
    }

    /// A mixer value was let go with its lane to rest: the value and the rest are one step.
    pub(super) fn rest_with(
        &mut self,
        key: LaneKey,
        edit: Result<Edit, EditError>,
        effects: &mut Vec<Effect>,
    ) {
        self.apply_resting(edit.ok(), &[key], false, effects);
    }

    /// The parameters panel set a plugin's parameter whose lane is to rest: the rest comes
    /// with the step the plugin's state makes.
    pub(super) fn rest_with_plugin_state(&mut self, ticket: u64, param: u32, key: LaneKey) {
        self.auto
            .rest_with_state
            .retain(|(t, p, _)| (*t, *p) != (ticket, param));
        self.auto.rest_with_state.push((ticket, param, key));
    }

    /// The lanes that rest with the step of a plugin's state for parameter `param`.
    pub(super) fn take_param_rests(&mut self, ticket: u64, param: u32) -> Vec<LaneKey> {
        let mut rests = Vec::new();
        self.auto.rest_with_state.retain(|&(t, p, key)| {
            let ours = (t, p) == (ticket, param);
            if ours {
                rests.push(key);
            }
            !ours
        });
        rests
    }

    /// The lanes held from the plugin's window, which rest with the step of the visit.
    pub(super) fn take_window_rests(
        &mut self,
        ticket: u64,
        effects: &mut Vec<Effect>,
    ) -> Vec<LaneKey> {
        let mut rests = Vec::new();
        self.auto.touches.retain(|t| {
            let ours = t.ticket == ticket && matches!(t.hand, Hand::Rest);
            if ours {
                rests.push(t.lane);
            }
            !ours
        });
        for &(track, target) in &rests {
            self.hold_lane(track, target, false, effects);
        }
        rests
    }

    /// The mute of a track was pressed. Returns whether a lane took it (the mute itself
    /// stays as it is).
    pub(super) fn mute_pressed(&mut self, track: TrackId, effects: &mut Vec<Effect>) -> bool {
        let key = (track, ModelTarget::Mute);
        let Some(lane) = self.lane_at(key).filter(|l| !l.resting) else {
            return false;
        };
        if !(self.armed(key) && self.can_write()) {
            return false;
        }
        let Some((_, tick)) = Self::heard_place(self.heard) else {
            return true;
        };
        let muted = lane.value_at(tick).unwrap_or(self.hand_place(key)) >= 0.5;
        let value = if muted { 0.0 } else { 1.0 };
        if let Ok((e, _)) = auto::write_press(self.doc(), track, ModelTarget::Mute, tick, value) {
            self.apply(e, effects);
        }
        true
    }

    // ---- the plugin's own window (docs/architecture.md 21-10) ----

    /// The plugin process says a hand took (`on`) or let go a parameter in the plugin's own
    /// window: VST3 `beginEdit` and `endEdit`, LV2 `ui:touch`, or the time it gives a write
    /// with no touch. Its time is not needed: what is written is placed by the times of the
    /// values (`plugin_edited`).
    ///
    /// With 「● 書く」 on while the song plays, the hand writes from here until it lets go,
    /// which is one step at once (`WriteAutomation`), apart from the window's own step.
    /// Otherwise a lane that moves the parameter is held at once and rests with the step
    /// made when the person leaves the window.
    pub(super) fn plugin_touched(
        &mut self,
        ticket: u64,
        param: u32,
        on: bool,
        effects: &mut Vec<Effect>,
    ) {
        let at = (self.auto.touches.iter())
            .position(|t| (t.ticket, t.param) == (ticket, param) && t.down);
        if on {
            if at.is_none()
                && let Some(at) = self.touch(ticket, param, effects)
                && matches!(self.auto.touches[at].hand, Hand::Write(_))
            {
                self.visit_wrote(ticket);
            }
            return;
        }
        let Some(at) = at else {
            return;
        };
        let key = self.auto.touches[at].lane;
        match &self.auto.touches[at].hand {
            Hand::Rest => self.auto.touches[at].down = false,
            Hand::Write(w) => {
                let w = w.clone();
                self.auto.touches.remove(at);
                self.hold_lane(key.0, key.1, false, effects);
                self.write_down(key, &w, effects);
            }
            Hand::Spent => {
                self.auto.touches.remove(at);
                self.hold_lane(key.0, key.1, false, effects);
            }
        }
    }

    fn touch(&mut self, ticket: u64, param: u32, effects: &mut Vec<Effect>) -> Option<usize> {
        let key = self.ticket_lane(ticket, param)?;
        let lane = self.lane_at(key).filter(|l| !l.resting)?;
        let hand = if self.armed(key) && self.can_write() {
            Hand::Write(Writing::default())
        } else if lane.moves() {
            // Held from the window already, still waiting for the visit's step.
            if self.auto.touches.iter().any(|t| t.lane == key) {
                return None;
            }
            Hand::Rest
        } else {
            return None;
        };
        self.auto.touches.push(Touch {
            ticket,
            param,
            lane: key,
            hand,
            down: true,
        });
        self.hold_lane(key.0, key.1, true, effects);
        Some(self.auto.touches.len() - 1)
    }

    /// The plugin process says the hand in the window moved a parameter to `value`
    /// (0..=1), at the time `heard` stands for (as for [`Play::plugin_touched`]). A move
    /// with no touch said before it starts one.
    pub(super) fn plugin_edited(
        &mut self,
        ticket: u64,
        param: u32,
        value: f64,
        heard: Option<Heard>,
        effects: &mut Vec<Effect>,
    ) {
        let found = (self.auto.touches.iter())
            .position(|t| (t.ticket, t.param) == (ticket, param) && t.down);
        let Some(at) = found.or_else(|| self.touch(ticket, param, effects)) else {
            return;
        };
        if let (Hand::Write(w), Some((lap, tick))) =
            (&mut self.auto.touches[at].hand, Self::heard_place(heard))
        {
            w.add(lap, tick, value.clamp(0.0, 1.0) as f32);
        }
    }

    /// The plugin process says how many times a hand took a parameter or read a preset in
    /// the plugin's window since the person came into it. Said as the person leaves the
    /// window, before the plugin's state for the visit comes back: a visit whose hands all
    /// wrote into lanes is no step, even when the state's bytes differ (a lane moved the
    /// plugin).
    pub(super) fn plugin_hands(&mut self, ticket: u64, count: u32) {
        self.visit_hands(ticket, count);
    }

    // ---- keeping up ----

    /// After every input: 「● 書く」 goes off when the song stops, and what a hand wrote
    /// until then is a step; the rest of its moves goes nowhere (docs/history/design-notes.ja.md B-92).
    pub fn sync_automation(&mut self, effects: &mut Vec<Effect>) {
        self.sync_lane_names(effects);
        let moving = self.can_write();
        if self.auto.was_moving && !moving {
            self.auto.armed.clear();
            let mut written = Vec::new();
            let hands = (self.auto.grips.iter_mut().map(|g| (g.lane, &mut g.hand)))
                .chain(self.auto.touches.iter_mut().map(|t| (t.lane, &mut t.hand)));
            for (key, hand) in hands {
                if let Hand::Write(w) = hand {
                    written.push((key, std::mem::take(w)));
                    *hand = Hand::Spent;
                }
            }
            for (key, w) in written {
                self.write_down(key, &w, effects);
            }
        }
        self.auto.was_moving = moving;
    }

    /// The song changed: what names a lane that is gone, or at rest, lets go of it.
    pub(super) fn drop_gone_lanes(&mut self, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        let there = |(track, target): LaneKey| doc.track(track).and_then(|t| t.lane(target));
        let moving = |key: LaneKey| there(key).is_some_and(|l| !l.resting);
        self.auto.armed.retain(|&k| moving(k));
        let selected = self.auto.selected.take();
        self.auto.selected = selected.filter(|s| doc.lane(s.lane).is_some());
        self.song
            .session
            .folded_lanes
            .retain(|&k| there(k).is_some());
        self.auto
            .rest_with_state
            .retain(|&(_, _, k)| there(k).is_some());
        let mut gone = Vec::new();
        self.auto.grips.retain(|g| {
            let keep = moving(g.lane);
            if !keep {
                gone.push(g.lane);
            }
            keep
        });
        self.auto.touches.retain(|t| {
            let keep = moving(t.lane);
            if !keep {
                gone.push(t.lane);
            }
            keep
        });
        for (track, target) in gone {
            self.hold_lane(track, target, false, effects);
        }
    }

    /// The plugins whose window holds lanes went away: those lanes move again.
    pub(super) fn drop_gone_touches(&mut self, effects: &mut Vec<Effect>) {
        let plugs = &self.plugs;
        let mut gone = Vec::new();
        self.auto.touches.retain(|t| {
            let keep = plugs.of(t.ticket).is_some_and(|p| p.sounds());
            if !keep {
                gone.push(t.lane);
            }
            keep
        });
        self.auto
            .rest_with_state
            .retain(|(t, ..)| plugs.of(*t).is_some());
        for (track, target) in gone {
            self.hold_lane(track, target, false, effects);
        }
    }

    /// Another song: nothing of the lanes of the last one stays.
    pub(super) fn forget_automation(&mut self, effects: &mut Vec<Effect>) {
        for key in (self.auto.grips.iter().map(|g| g.lane))
            .chain(self.auto.touches.iter().map(|t| t.lane))
            .collect::<Vec<_>>()
        {
            self.hold_lane(key.0, key.1, false, effects);
        }
        self.auto = Auto {
            names: std::mem::take(&mut self.auto.names),
            ..Auto::default()
        };
    }

    // ---- views ----

    pub fn lanes_of(&self, track: TrackId) -> Vec<LaneView> {
        let Some(t) = self.doc().track(track) else {
            return Vec::new();
        };
        (t.automation.iter())
            .filter_map(|l| {
                Some(LaneView {
                    target: protocol_target(l.target),
                    name: self.lane_name((track, l.target))?,
                    lane_id: l.id.0,
                    points: u32::try_from(l.points.len()).unwrap_or(u32::MAX),
                    step: l.step,
                    resting: l.resting,
                })
            })
            .collect()
    }

    pub fn automation_view(&self) -> AutomationView {
        let a = &self.auto;
        let writing = (a.grips.iter().map(|g| (g.lane, &g.hand)))
            .chain(a.touches.iter().map(|t| (t.lane, &t.hand)))
            .filter_map(|(key, hand)| {
                let Hand::Write(w) = hand else {
                    return None;
                };
                let from_tick = match (w.looped(), self.doc().loop_range) {
                    (true, Some(range)) => range.start(),
                    _ => w.first_tick()?,
                };
                Some(WritingLane {
                    lane: lane_ref(key),
                    from_tick,
                })
            })
            .collect();
        AutomationView {
            folded: self
                .song
                .session
                .folded_lanes
                .iter()
                .map(|&k| lane_ref(k))
                .collect(),
            armed: a.armed.iter().map(|&k| lane_ref(k)).collect(),
            writing,
            held: (a.touches.iter())
                .filter(|t| matches!(t.hand, Hand::Rest))
                .map(|t| lane_ref(t.lane))
                .collect(),
            selected: a.selected.as_ref().map(|s| AutoSelection {
                lane_id: s.lane.0,
                points: index_ranges(&s.points),
            }),
        }
    }

    /// The points of a lane, binary (`meloet_protocol::LanePoints`); `None` for an id that
    /// is no lane's now.
    pub fn lane(&self, lane_id: u32) -> Option<Vec<u8>> {
        let (_, l) = self.doc().lane(LaneId(lane_id))?;
        let points = LanePoints {
            id: lane_id,
            step: l.step,
            points: (l.points.iter())
                .map(|p| LanePoint {
                    tick: p.tick,
                    value: p.value,
                })
                .collect(),
        };
        Some(points.encode())
    }

    /// While 「● 書く」 is on and the song plays (spec 6).
    pub fn writing_status(&self) -> Option<StatusMessage> {
        if !self.can_write() {
            return None;
        }
        let &key = self.auto.armed.first()?;
        Some(StatusMessage::AutomationWriting {
            track: self.track_ref(key.0)?,
            target: self.lane_name(key)?,
        })
    }

    /// Said once as the mixer opens (mock F6-MixerAuto, docs/history/design-notes.ja.md A-19 #4): the tracks
    /// whose fader a lane moves.
    pub fn automation_moving(&self) -> Option<StatusMessage> {
        let tracks: Vec<_> = (self.doc().tracks.iter())
            .filter(|t| t.lane(ModelTarget::Volume).is_some_and(|l| l.moves()))
            .filter_map(|t| self.track_ref(t.id))
            .collect();
        // round12 stub: M5 (the master's lane)
        (!tracks.is_empty()).then_some(StatusMessage::AutomationMoving {
            tracks,
            master: false,
        })
    }
}

/// Leaves what a deleted lane moved at `place`, for the targets the song holds a value of.
fn settle(e: &mut Edit, (track, target): LaneKey, place: f32) {
    let Some(slot) = e.doc.tracks.iter_mut().find(|t| t.id == track) else {
        return;
    };
    let t = std::sync::Arc::make_mut(slot);
    match target {
        ModelTarget::Volume => t.channel.gain = Gain::from_position(place),
        ModelTarget::Pan => t.channel.pan = Pan::new((Pan::unit_at(place) * 100.0).round() as i32),
        ModelTarget::Send(bus) => t.sends[bus.index()] = Gain::from_position(place),
        ModelTarget::Mute => t.muted = place >= 0.5,
        ModelTarget::Insert { insert, param } => {
            let held = t
                .channel
                .inserts
                .iter_mut()
                .flatten()
                .find(|i| i.id == insert);
            if let Some(i) = held
                && let Some(spec) = i.kind().and_then(|kind| kind.param(param))
            {
                i.set(param, spec.value_at(place));
            }
        }
        ModelTarget::Instrument { .. } => {}
    }
}

#[cfg(test)]
mod tests;
