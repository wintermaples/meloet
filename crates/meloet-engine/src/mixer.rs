//! What a strip of the mixer does to sound on the audio thread: the effects in its insert
//! slots, its fader and pan, and (for a track) its sends. docs/architecture.md 19-11.

use std::f32::consts::FRAC_PI_2;

use rtrb::Producer;

use crate::effect::{Effect, EffectContext};

/// The insert slots of a strip. Sound goes through them from the first to the last.
pub const INSERT_SLOTS: usize = 4;

/// How long a fader, a pan or a send takes to reach a new value, in a straight line.
pub const RAMP_S: f32 = 0.020;

/// A strip of the mixer, as the engine numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MixStrip {
    /// A track of the engine; the audition track too.
    Track(usize),
    /// 0 is return A, 1 is return B.
    Return(usize),
    Master,
}

/// What a pan (−1.0 all left, 0.0 the middle, 1.0 all right) multiplies left and right by.
///
/// A balance: in the middle both sides are untouched, and turning to one side takes the other
/// side down along a quarter of a cosine, to nothing at the end. A side never gets louder, so
/// a pan cannot push a strip over its fader, and a stereo instrument keeps its own left and
/// right (nothing is folded from one side into the other).
pub fn pan_gains(pan: f32) -> [f32; 2] {
    let down = |amount: f32| {
        if amount >= 1.0 {
            0.0
        } else {
            (amount * FRAC_PI_2).cos()
        }
    };
    if !pan.is_finite() {
        [1.0, 1.0]
    } else if pan > 0.0 {
        [down(pan), 1.0]
    } else {
        [1.0, down(-pan)]
    }
}

/// A value that goes to its target in a straight line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Ramp {
    now: f32,
    target: f32,
    step: f32,
    /// Frames until `now` is `target`.
    left: u32,
}

impl Ramp {
    pub fn at(value: f32) -> Ramp {
        Ramp {
            now: value,
            target: value,
            step: 0.0,
            left: 0,
        }
    }

    /// `frames` 0: at once.
    pub fn set(&mut self, target: f32, frames: u32) {
        self.target = target;
        if frames == 0 || target == self.now {
            self.snap();
        } else {
            self.step = (target - self.now) / frames as f32;
            self.left = frames;
        }
    }

    pub fn snap(&mut self) {
        self.now = self.target;
        self.left = 0;
    }

    pub fn steady(&self) -> bool {
        self.left == 0
    }

    /// Neither sounding nor about to.
    pub fn silent(&self) -> bool {
        self.now == 0.0 && self.target == 0.0
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.now = if self.left == 0 {
                self.target
            } else {
                self.now + self.step
            };
        }
        self.now
    }
}

/// One insert slot. When its effect changes, what it sounded like before (the effect that
/// was there, or the sound passing through untouched) fades into what it sounds like now.
///
/// A slot that is passed by lets the sound through untouched and keeps its effect running on
/// that sound, unheard: when the effect is heard again, its tail and its envelopes are where
/// they would have been, and nothing starts from silence.
#[derive(Default)]
struct Slot {
    effect: Option<Box<dyn Effect>>,
    /// The effect that was here before, until the fade is over and the retired ring takes it.
    old: Option<Box<dyn Effect>>,
    fade_left: u32,
    /// 「とばす」. Belongs to the slot: an effect put into it later comes in passed by.
    bypassed: bool,
    /// Taken out of the chain while it goes to another place of its strip.
    lifted: bool,
    /// Frames until the slot is all the way passed by, or all the way heard again.
    pass_left: u32,
}

impl Slot {
    fn passed_by(&self) -> bool {
        self.bypassed || self.lifted
    }

    fn holds(&self) -> bool {
        self.effect.is_some() || self.old.is_some()
    }

    /// Neither heard nor on its way to being heard.
    fn unheard(&self) -> bool {
        !self.holds() || self.passed_by() && self.pass_left == 0
    }

    /// `fade` 0: at once. A fade that is under way turns around where it is.
    fn pass(&mut self, bypassed: bool, lifted: bool, fade: u32) {
        let was = self.passed_by();
        self.bypassed = bypassed;
        self.lifted = lifted;
        if self.passed_by() != was {
            self.pass_left = if self.holds() {
                fade.saturating_sub(self.pass_left)
            } else {
                0
            };
        }
    }

    /// `fade` 0: at once.
    fn set(
        &mut self,
        effect: Option<Box<dyn Effect>>,
        fade: u32,
        retired: &mut Producer<Box<dyn Effect>>,
    ) {
        // Cannot fail: a command is taken only while every effect of a strip fits, and the
        // engine is the only producer.
        if let Some(before) = self.old.take() {
            let _ = retired.push(before);
        }
        self.old = std::mem::replace(&mut self.effect, effect);
        self.fade_left = fade;
        if fade == 0
            && let Some(old) = self.old.take()
        {
            let _ = retired.push(old);
        }
    }

    fn process(&mut self, io: &mut Io<'_>, retired: &mut Producer<Box<dyn Effect>>) {
        if !self.holds() {
            self.fade_left = 0;
            self.pass_left = 0;
            return;
        }
        let frames = io.left.len();
        let passed_by = self.passed_by();
        let around = passed_by || self.pass_left > 0;
        if around {
            io.dry_l[..frames].copy_from_slice(io.left);
            io.dry_r[..frames].copy_from_slice(io.right);
        }
        if self.fade_left > 0 {
            let n = io.left.len().min(self.fade_left as usize);
            let (before_l, before_r) = (&mut io.spare_l[..n], &mut io.spare_r[..n]);
            before_l.copy_from_slice(&io.left[..n]);
            before_r.copy_from_slice(&io.right[..n]);
            if let Some(old) = self.old.as_mut() {
                old.process(before_l, before_r, &io.ctx);
            }
            if let Some(effect) = self.effect.as_mut() {
                effect.process(io.left, io.right, &io.ctx);
            }
            for i in 0..n {
                let level = (self.fade_left - i as u32) as f32 / io.fade_frames as f32;
                io.left[i] += (before_l[i] - io.left[i]) * level;
                io.right[i] += (before_r[i] - io.right[i]) * level;
            }
            self.fade_left -= n as u32;
        } else if let Some(effect) = self.effect.as_mut() {
            effect.process(io.left, io.right, &io.ctx);
        }
        if around {
            let n = frames.min(self.pass_left as usize);
            for i in 0..n {
                let to_go = (self.pass_left - i as u32) as f32 / io.fade_frames as f32;
                let heard = if passed_by { to_go } else { 1.0 - to_go };
                io.left[i] = io.dry_l[i] + (io.left[i] - io.dry_l[i]) * heard;
                io.right[i] = io.dry_r[i] + (io.right[i] - io.dry_r[i]) * heard;
            }
            self.pass_left -= n as u32;
            if passed_by {
                io.left[n..].copy_from_slice(&io.dry_l[n..frames]);
                io.right[n..].copy_from_slice(&io.dry_r[n..frames]);
            }
        }
        if self.fade_left == 0
            && self.old.is_some()
            && !retired.is_full()
            && let Some(old) = self.old.take()
        {
            let _ = retired.push(old);
        }
    }
}

/// The sound a strip works on, in place, and what its slots need for it.
pub(crate) struct Io<'a> {
    pub left: &'a mut [f32],
    pub right: &'a mut [f32],
    /// As long as `left` at least; for the sound of a slot before its effect changed.
    pub spare_l: &'a mut [f32],
    pub spare_r: &'a mut [f32],
    /// As long as `left` at least; for the sound that goes past a slot.
    pub dry_l: &'a mut [f32],
    pub dry_r: &'a mut [f32],
    pub ctx: EffectContext,
    pub fade_frames: u32,
}

/// An effect on its way to another slot of its strip.
///
/// The same effect cannot sound at both places at once, so the two orders cannot be laid over
/// each other. Instead the effect fades out of the chain where it is, the slots change places
/// while it is not heard (the others keep their order, so nothing is heard of that), and it
/// fades in where it is now: two fades of [`crate::FADE_S`], each between two sounds that are
/// both whole. The effect runs all the while.
#[derive(Clone, Copy)]
struct Move {
    from: usize,
    to: usize,
    fade: u32,
}

impl Move {
    /// Where the slot that will be at `slot` is until the move lands.
    fn before(&self, slot: usize) -> usize {
        let (from, to) = (self.from, self.to);
        if slot == to {
            from
        } else if from < to && (from..to).contains(&slot) {
            slot + 1
        } else if to < from && (to + 1..=from).contains(&slot) {
            slot - 1
        } else {
            slot
        }
    }
}

fn rotate(slots: &mut [Slot], from: usize, to: usize) {
    if from < to {
        slots[from..=to].rotate_left(1);
    } else {
        slots[to..=from].rotate_right(1);
    }
}

pub(crate) struct Strip {
    slots: [Slot; INSERT_SLOTS],
    moving: Option<Move>,
    /// Fader × pan, per side.
    left: Ramp,
    right: Ramp,
    sends: [Ramp; 2],
}

impl Strip {
    pub fn new() -> Strip {
        Strip {
            slots: Default::default(),
            moving: None,
            left: Ramp::at(1.0),
            right: Ramp::at(1.0),
            sends: [Ramp::at(0.0); 2],
        }
    }

    /// The delays of the effects in the slots, added up. A slot that is passed by delays
    /// nothing.
    pub fn latency(&self) -> u32 {
        (self.slots.iter())
            .filter(|slot| !slot.bypassed)
            .filter_map(|slot| slot.effect.as_ref())
            .fold(0u32, |sum, effect| sum.saturating_add(effect.latency()))
    }

    /// `gain` is linear. `frames` 0: at once.
    pub fn set_level(&mut self, gain: f32, pan: f32, frames: u32) {
        if !gain.is_finite() {
            return;
        }
        let [l, r] = pan_gains(pan);
        let gain = gain.max(0.0);
        self.left.set(gain * l, frames);
        self.right.set(gain * r, frames);
    }

    pub fn set_sends(&mut self, sends: [f32; 2], frames: u32) {
        for (ramp, gain) in self.sends.iter_mut().zip(sends) {
            if gain.is_finite() {
                ramp.set(gain.max(0.0), frames);
            }
        }
    }

    /// The effect comes back on the ring when `slot` is not one.
    pub fn set_effect(
        &mut self,
        slot: usize,
        effect: Option<Box<dyn Effect>>,
        fade: u32,
        retired: &mut Producer<Box<dyn Effect>>,
    ) {
        self.land();
        match self.slots.get_mut(slot) {
            Some(slot) => slot.set(effect, fade, retired),
            None => {
                if let Some(effect) = effect {
                    let _ = retired.push(effect);
                }
            }
        }
    }

    /// The slot as the commands number it: a move that has not landed yet counts as done.
    fn slot_mut(&mut self, slot: usize) -> Option<&mut Slot> {
        let slot = self.moving.map_or(slot, |m| m.before(slot));
        self.slots.get_mut(slot)
    }

    pub fn set_param(&mut self, slot: usize, param: u32, value: f32) {
        if let Some(effect) = self.slot_mut(slot).and_then(|s| s.effect.as_mut()) {
            effect.set_param(param, value);
        }
    }

    /// 「とばす」 and 「もどす」. `fade` 0: at once.
    pub fn set_bypass(&mut self, slot: usize, bypassed: bool, fade: u32) {
        if let Some(slot) = self.slot_mut(slot) {
            slot.pass(bypassed, slot.lifted, fade);
        }
    }

    /// The slot `from` goes to `to`, and the slots between move by one. `fade` 0: at once.
    pub fn move_effect(&mut self, from: usize, to: usize, fade: u32) {
        self.land();
        if from >= INSERT_SLOTS || to >= INSERT_SLOTS || from == to {
            return;
        }
        // Passing only slots that are not heard changes nothing in the sound, and neither
        // does moving one that is not heard.
        let passes_heard = (from.min(to)..=from.max(to))
            .filter(|at| *at != from)
            .any(|at| !self.slots[at].unheard());
        if fade == 0 || self.slots[from].unheard() || !passes_heard {
            rotate(&mut self.slots, from, to);
            return;
        }
        let slot = &mut self.slots[from];
        slot.pass(slot.bypassed, true, fade);
        self.moving = Some(Move { from, to, fade });
    }

    /// The effect on its way is at its new place, and fades in there from as far out as it
    /// had got.
    fn land(&mut self) {
        if let Some(m) = self.moving.take() {
            rotate(&mut self.slots, m.from, m.to);
            let slot = &mut self.slots[m.to];
            slot.pass(slot.bypassed, false, m.fade);
        }
    }

    pub fn inserts(&mut self, io: &mut Io<'_>, retired: &mut Producer<Box<dyn Effect>>) {
        if self
            .moving
            .is_some_and(|m| self.slots[m.from].pass_left == 0)
        {
            self.land();
        }
        for slot in &mut self.slots {
            slot.process(io, retired);
        }
    }

    /// Fader and pan, in place. Returns the peaks, and whether every sample is a number.
    pub fn level(&mut self, left: &mut [f32], right: &mut [f32]) -> ([f32; 2], bool) {
        let mut peak = [0.0f32; 2];
        let mut finite = true;
        if self.left.steady() && self.right.steady() {
            let (gl, gr) = (self.left.now, self.right.now);
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                *l *= gl;
                *r *= gr;
                peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
                finite &= l.is_finite() & r.is_finite();
            }
        } else {
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                *l *= self.left.next();
                *r *= self.right.next();
                peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
                finite &= l.is_finite() & r.is_finite();
            }
        }
        (peak, finite)
    }

    /// Adds what goes to return `bus` of `left` and `right` (the strip's sound after its
    /// fader and pan) to `to`.
    pub fn send(&mut self, bus: usize, left: &[f32], right: &[f32], to: [&mut [f32]; 2]) {
        let ramp = &mut self.sends[bus];
        if ramp.silent() {
            return;
        }
        let [to_l, to_r] = to;
        for i in 0..left.len() {
            let gain = ramp.next();
            to_l[i] += left[i] * gain;
            to_r[i] += right[i] * gain;
        }
    }

    /// Fader, pan, sends and the slots are where they are going, without the way there. An
    /// old effect stays if the ring is full.
    pub fn end_fades(&mut self, retired: &mut Producer<Box<dyn Effect>>) {
        self.left.snap();
        self.right.snap();
        for send in &mut self.sends {
            send.snap();
        }
        self.land();
        for slot in &mut self.slots {
            slot.fade_left = 0;
            slot.pass_left = 0;
            if !retired.is_full()
                && let Some(old) = slot.old.take()
            {
                let _ = retired.push(old);
            }
        }
    }

    /// For a new stream. Not on the audio thread: an old effect is dropped here if the ring
    /// is full.
    pub fn prepare(
        &mut self,
        sample_rate: f32,
        max_frames: usize,
        retired: &mut Producer<Box<dyn Effect>>,
    ) {
        self.end_fades(retired);
        for slot in &mut self.slots {
            slot.old = None;
            if let Some(effect) = slot.effect.as_mut() {
                effect.prepare(sample_rate, max_frames);
            }
        }
    }

    /// Back to a new strip: its effects go to the ring.
    pub fn clear(&mut self, retired: &mut Producer<Box<dyn Effect>>) {
        for slot in &mut self.slots {
            for effect in [slot.effect.take(), slot.old.take()].into_iter().flatten() {
                let _ = retired.push(effect);
            }
            slot.fade_left = 0;
            slot.pass(false, false, 0);
        }
        self.moving = None;
        self.left = Ramp::at(1.0);
        self.right = Ramp::at(1.0);
        self.sends = [Ramp::at(0.0); 2];
    }
}
