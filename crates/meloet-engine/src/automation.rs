//! Automation on the audio thread (docs/architecture.md 21-5 to 21-7).
//!
//! The control side puts the lanes of a track that are not resting into one [`AutoSet`] and
//! sends it, as a sequence is sent; the old one comes back on a retired ring. A lane holds
//! points of the place of the knob a person sees, 0..=1, and is read where the sound it
//! moves is heard: a parameter of the instrument and the mute as far ahead as the track's
//! delays add up to, a parameter of an insert as far ahead as the delays of that slot and
//! the ones after it, the fader, the pan and the sends not ahead at all.
//!
//! - Fader, pan, sends: every [`MIX_STEP`] frames the value at the end of the step, and the
//!   gain goes there in a straight line. A place moves no faster than the whole way in
//!   [`crate::RAMP_S`], so a jump is a short slope.
//! - An effect that comes with meloet: its stretch of sound is cut every [`EFFECT_STEP`]
//!   frames, and each piece starts with `set_param`.
//! - A plugin: points with their frame in the stretch ([`Effect::set_param_at`]): the value
//!   at the last frame, the bends of the line in between, at most [`PLUGIN_POINTS`] for one
//!   plugin. A value the plugin was sent last is not sent again.
//! - The mute: a step, read where the track's sequence is read.

use meloet_model::AutoTarget;
use meloet_model::effects::EffectKind;
use meloet_model::{Gain, Pan};

use crate::effect::Effect;
use crate::instrument::Instrument;

/// Frames between two values of a fader, a pan or a send.
pub const MIX_STEP: usize = 32;
/// Frames between two `set_param` of an effect that comes with meloet.
pub const EFFECT_STEP: usize = 64;
/// Points one plugin gets in one stretch, all its lanes together.
pub const PLUGIN_POINTS: usize = 128;
/// Lanes held at once on one track ([`crate::EngineCommand::HoldLane`]).
pub const HOLDS: usize = 16;

/// One lane, as the control side hands it over.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoLane {
    pub target: AutoTarget,
    /// The value stays at the point before until the next one.
    pub step: bool,
    /// Tick and place, ticks rising. A lane without points is left out.
    pub points: Vec<(u64, f32)>,
    /// For a target of an insert: the slot it is in now. Moves with the slot.
    pub slot: usize,
    /// For a target of an insert: the effect that comes with meloet whose table gives the
    /// values, `None` for a plugin. A lane is only played on an effect of its own kind
    /// (one that takes points, or one that does not).
    pub effect: Option<EffectKind>,
}

impl AutoLane {
    /// The place on the line at `tick`.
    pub fn place_at(&self, tick: f64) -> f32 {
        place_at(&self.points, self.step, tick)
    }
}

/// The place on the line through `points` at `tick`: straight between two points, or the
/// point before in a `step` lane; the first point's before it, the last's after.
pub fn place_at(points: &[(u64, f32)], step: bool, tick: f64) -> f32 {
    let after = points.partition_point(|p| p.0 as f64 <= tick);
    match (after.checked_sub(1).map(|i| points[i]), points.get(after)) {
        (None, Some(first)) => first.1,
        (Some(before), None) => before.1,
        (Some(before), Some(_)) if step => before.1,
        (Some((t0, v0)), Some((t1, v1))) => {
            let share = ((tick - t0 as f64) / (t1 - t0) as f64) as f32;
            v0 + (v1 - v0) * share.clamp(0.0, 1.0)
        }
        (None, None) => f32::NAN,
    }
}

/// Where the song is heard by the sound a lane moves.
pub(crate) trait AutoClock {
    /// The tick `ahead` frames after the next frame to render: the next frame's while
    /// stopped, round the loop when it goes past the loop's end.
    fn tick_ahead(&self, ahead: u64) -> f64;
    /// Frames from `ahead` frames after the next frame to the first frame at `tick` or
    /// past it; `None` when the loop goes round on the way, or the song does not move.
    fn frames_to(&self, tick: f64, ahead: u64) -> Option<u64>;
}

pub(crate) struct LaneRun {
    pub lane: AutoLane,
    /// The place a plugin was sent last; NaN: nothing, send again.
    sent: f32,
    /// Fader, pan, send: where the place has got to on its way; NaN: from the hand's value.
    now: f32,
    held: bool,
}

/// The lanes of one track that are played ([`crate::EngineCommand::SetAutomation`]).
pub struct AutoSet {
    pub(crate) lanes: Box<[LaneRun]>,
    /// Where the next stretch starts with the plugin points when not all of them fit.
    turn: usize,
}

impl AutoSet {
    pub fn new(lanes: Vec<AutoLane>) -> AutoSet {
        let lanes = (lanes.into_iter())
            .filter(|lane| !lane.points.is_empty())
            .map(|lane| LaneRun {
                lane,
                sent: f32::NAN,
                now: f32::NAN,
                held: false,
            })
            .collect();
        AutoSet { lanes, turn: 0 }
    }

    pub fn lanes(&self) -> impl Iterator<Item = &AutoLane> {
        self.lanes.iter().map(|run| &run.lane)
    }

    /// Takes over from the set before it: where the fader, pan and sends had got to, so a
    /// new set does not jump, and the lanes held.
    pub(crate) fn follow(&mut self, before: Option<&AutoSet>, holds: &[Option<AutoTarget>]) {
        for run in self.lanes.iter_mut() {
            run.held = holds.contains(&Some(run.lane.target));
            if let Some(old) = before.and_then(|b| b.find(run.lane.target)) {
                run.now = old.now;
            }
        }
    }

    fn find(&self, target: AutoTarget) -> Option<&LaneRun> {
        self.lanes.iter().find(|run| run.lane.target == target)
    }

    fn played(&mut self, target: AutoTarget) -> Option<&mut LaneRun> {
        (self.lanes.iter_mut()).find(|run| run.lane.target == target && !run.held)
    }

    pub(crate) fn hold(&mut self, target: AutoTarget, on: bool) {
        for run in self.lanes.iter_mut().filter(|r| r.lane.target == target) {
            run.held = on;
            run.now = f32::NAN;
            run.sent = f32::NAN;
        }
    }

    /// Everything is sent again and starts from the hand's values (a new stream).
    pub(crate) fn forget(&mut self) {
        for run in self.lanes.iter_mut() {
            run.sent = f32::NAN;
            run.now = f32::NAN;
        }
    }

    /// The slot `from` went to `to` and the ones between moved by one.
    pub(crate) fn move_slot(&mut self, from: usize, to: usize) {
        for run in self.lanes.iter_mut() {
            if !matches!(run.lane.target, AutoTarget::Insert { .. }) {
                continue;
            }
            let s = run.lane.slot;
            run.lane.slot = if s == from {
                to
            } else if from < to && (from + 1..=to).contains(&s) {
                s - 1
            } else if to < from && (to..from).contains(&s) {
                s + 1
            } else {
                s
            };
        }
    }

    pub(crate) fn has_mix(&self) -> bool {
        (self.lanes.iter()).any(|r| {
            !r.held
                && matches!(
                    r.lane.target,
                    AutoTarget::Volume | AutoTarget::Pan | AutoTarget::Send(_)
                )
        })
    }

    pub(crate) fn has_level(&self) -> bool {
        (self.lanes.iter())
            .any(|r| !r.held && matches!(r.lane.target, AutoTarget::Volume | AutoTarget::Pan))
    }

    pub(crate) fn has_send(&self, bus: usize) -> bool {
        (self.lanes.iter())
            .any(|r| !r.held && matches!(r.lane.target, AutoTarget::Send(b) if b.index() == bus))
    }

    /// Whether the mute lane mutes at `tick`; `None` without one.
    pub(crate) fn mute_at(&self, tick: f64) -> Option<bool> {
        let run = (self.lanes.iter()).find(|r| !r.held && r.lane.target == AutoTarget::Mute)?;
        Some(run.lane.place_at(tick) >= 0.5)
    }

    /// The tick of the next point of the mute lane after `tick` where it changes from what
    /// it is at `tick`.
    pub(crate) fn next_mute_change(&self, tick: f64) -> Option<u64> {
        let run = (self.lanes.iter()).find(|r| !r.held && r.lane.target == AutoTarget::Mute)?;
        let now = run.lane.place_at(tick) >= 0.5;
        let points = &run.lane.points;
        let from = points.partition_point(|p| p.0 as f64 <= tick);
        (points[from..].iter())
            .find(|p| (p.1 >= 0.5) != now)
            .map(|p| p.0)
    }

    /// The fader's gain (linear) and the pan (−1..=1) at the end of a step of `frames`
    /// that ends at `tick`, each the hand's own where no lane plays it.
    pub(crate) fn level(
        &mut self,
        tick: f64,
        frames: usize,
        whole_way: f32,
        hand: (f32, f32),
    ) -> (f32, f32) {
        let most = frames as f32 / whole_way;
        let gain = match self.played(AutoTarget::Volume) {
            Some(run) => {
                let from = || gain_place(hand.0);
                Gain::from_position(run.glide(tick, most, from)).linear()
            }
            None => hand.0,
        };
        let pan = match self.played(AutoTarget::Pan) {
            Some(run) => Pan::unit_at(run.glide(tick, most, || (hand.1 + 1.0) / 2.0)),
            None => hand.1,
        };
        (gain, pan)
    }

    /// A send's gain (linear) at the end of a step, as [`AutoSet::level`].
    pub(crate) fn send(
        &mut self,
        bus: usize,
        tick: f64,
        frames: usize,
        whole_way: f32,
        hand: f32,
    ) -> f32 {
        let most = frames as f32 / whole_way;
        let run = (self.lanes.iter_mut())
            .find(|r| !r.held && matches!(r.lane.target, AutoTarget::Send(b) if b.index() == bus));
        match run {
            Some(run) => Gain::from_position(run.glide(tick, most, || gain_place(hand))).linear(),
            None => hand,
        }
    }

    /// Whether a lane that is not held plays parameter `param` of the instrument.
    pub(crate) fn plays_instrument(&self, param: u32) -> bool {
        (self.lanes.iter()).any(|r| !r.held && r.lane.target == AutoTarget::Instrument { param })
    }

    /// The instrument's lanes send their values again (another instrument is on the seat).
    pub(crate) fn resend_instrument(&mut self) {
        for run in self.lanes.iter_mut() {
            if matches!(run.lane.target, AutoTarget::Instrument { .. }) {
                run.sent = f32::NAN;
            }
        }
    }

    /// Whether a lane plays a parameter of the instrument.
    pub(crate) fn has_instrument(&self) -> bool {
        (self.lanes.iter())
            .any(|r| !r.held && matches!(r.lane.target, AutoTarget::Instrument { .. }))
    }

    /// Sends the points of the instrument's lanes for a stretch of `frames`, read `ahead`.
    pub(crate) fn instrument_points(
        &mut self,
        clock: &dyn AutoClock,
        ahead: u64,
        frames: usize,
        instrument: &mut dyn Instrument,
    ) {
        let turn = self.turn;
        let mut sent = 0;
        let lanes = &mut self.lanes;
        let mine = |r: &LaneRun| !r.held && matches!(r.lane.target, AutoTarget::Instrument { .. });
        let count = lanes.iter().filter(|r| mine(r)).count();
        let mut spare = PLUGIN_POINTS.saturating_sub(count);
        for i in 0..lanes.len() {
            let run = &mut lanes[(turn + i) % lanes.len()];
            let AutoTarget::Instrument { param } = run.lane.target else {
                continue;
            };
            if run.held || sent >= PLUGIN_POINTS {
                continue;
            }
            sent += 1;
            run.points(clock, ahead, frames, &mut spare, |frame, place| {
                instrument.set_param_at(frame, param, place);
            });
        }
        if count > PLUGIN_POINTS {
            self.turn = (turn + PLUGIN_POINTS) % lanes.len();
        }
    }

    /// Whether a lane plays a parameter of what is in `slot`.
    pub(crate) fn has_slot(&self, slot: usize) -> bool {
        (self.lanes.iter()).any(|r| {
            !r.held && matches!(r.lane.target, AutoTarget::Insert { .. }) && r.lane.slot == slot
        })
    }

    /// Gives the effect in `slot` the values of its lanes at the frame `ahead` frames on.
    pub(crate) fn effect_values(
        &mut self,
        slot: usize,
        clock: &dyn AutoClock,
        ahead: u64,
        effect: &mut dyn Effect,
    ) {
        let tick = clock.tick_ahead(ahead);
        for run in self.lanes.iter_mut() {
            let (AutoTarget::Insert { param, .. }, Some(kind), false) =
                (run.lane.target, run.lane.effect, run.held)
            else {
                continue;
            };
            if run.lane.slot != slot {
                continue;
            }
            let place = run.lane.place_at(tick);
            if place != run.sent
                && let Some(spec) = kind.param(param)
            {
                effect.set_param(param, spec.value_at(place));
                run.sent = place;
            }
        }
    }

    /// Sends the points of the lanes of the plugin in `slot` for a stretch of `frames`.
    pub(crate) fn effect_points(
        &mut self,
        slot: usize,
        clock: &dyn AutoClock,
        ahead: u64,
        frames: usize,
        effect: &mut dyn Effect,
    ) {
        let mine = |r: &LaneRun| {
            !r.held
                && r.lane.effect.is_none()
                && r.lane.slot == slot
                && matches!(r.lane.target, AutoTarget::Insert { .. })
        };
        let lanes = &mut self.lanes;
        let count = lanes.iter().filter(|r| mine(r)).count();
        let mut spare = PLUGIN_POINTS.saturating_sub(count);
        let turn = self.turn;
        let mut sent = 0;
        for i in 0..lanes.len() {
            let run = &mut lanes[(turn + i) % lanes.len()];
            if !mine(run) || sent >= PLUGIN_POINTS {
                continue;
            }
            let AutoTarget::Insert { param, .. } = run.lane.target else {
                continue;
            };
            sent += 1;
            run.points(clock, ahead, frames, &mut spare, |frame, place| {
                effect.set_param_at(frame, param, place);
            });
        }
        if count > PLUGIN_POINTS {
            self.turn = (turn + PLUGIN_POINTS) % lanes.len();
        }
    }
}

/// The place of a fader or a send at a linear gain.
fn gain_place(linear: f32) -> f32 {
    if linear > 0.0 {
        Gain::from_db(20.0 * f64::from(linear).log10()).position()
    } else {
        0.0
    }
}

impl LaneRun {
    /// Moves the place towards the line at `tick` by `most` at most, and gives it.
    fn glide(&mut self, tick: f64, most: f32, from: impl FnOnce() -> f32) -> f32 {
        let target = self.lane.place_at(tick);
        if self.now.is_nan() {
            self.now = from();
        }
        self.now += (target - self.now).clamp(-most, most);
        self.now
    }

    /// The points of a stretch for a plugin: the bends inside it, while `spare` lasts, and
    /// the value at its last frame. Nothing the plugin was sent last.
    fn points(
        &mut self,
        clock: &dyn AutoClock,
        ahead: u64,
        frames: usize,
        spare: &mut usize,
        mut push: impl FnMut(u32, f32),
    ) {
        let last = frames.saturating_sub(1) as u64;
        let first_tick = clock.tick_ahead(ahead);
        let last_tick = clock.tick_ahead(ahead + last);
        let mut sent = self.sent;
        if last_tick > first_tick && clock.frames_to(last_tick, ahead).is_some() {
            let points = &self.lane.points;
            let from = points.partition_point(|p| p.0 as f64 <= first_tick);
            for &(tick, _) in points[from..]
                .iter()
                .take_while(|p| p.0 as f64 <= last_tick)
            {
                if *spare == 0 {
                    break;
                }
                let Some(frame) = clock.frames_to(tick as f64, ahead) else {
                    break;
                };
                let place = self.lane.place_at(tick as f64);
                if place != sent && frame < last {
                    push(frame as u32, place);
                    sent = place;
                    *spare -= 1;
                }
            }
        }
        let place = self.lane.place_at(last_tick);
        if place != sent {
            push(last as u32, place);
        }
        self.sent = place;
    }
}
