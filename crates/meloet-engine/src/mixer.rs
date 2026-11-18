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
#[derive(Default)]
struct Slot {
    effect: Option<Box<dyn Effect>>,
    /// The effect that was here before, until the fade is over and the retired ring takes it.
    old: Option<Box<dyn Effect>>,
    fade_left: u32,
}

impl Slot {
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
    pub ctx: EffectContext,
    pub fade_frames: u32,
}

pub(crate) struct Strip {
    slots: [Slot; INSERT_SLOTS],
    /// Fader × pan, per side.
    left: Ramp,
    right: Ramp,
    sends: [Ramp; 2],
}

impl Strip {
    pub fn new() -> Strip {
        Strip {
            slots: Default::default(),
            left: Ramp::at(1.0),
            right: Ramp::at(1.0),
            sends: [Ramp::at(0.0); 2],
        }
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
        match self.slots.get_mut(slot) {
            Some(slot) => slot.set(effect, fade, retired),
            None => {
                if let Some(effect) = effect {
                    let _ = retired.push(effect);
                }
            }
        }
    }

    pub fn set_param(&mut self, slot: usize, param: u32, value: f32) {
        if let Some(effect) = self.slots.get_mut(slot).and_then(|s| s.effect.as_mut()) {
            effect.set_param(param, value);
        }
    }

    pub fn inserts(&mut self, io: &mut Io<'_>, retired: &mut Producer<Box<dyn Effect>>) {
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
        for slot in &mut self.slots {
            slot.fade_left = 0;
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
        }
        self.left = Ramp::at(1.0);
        self.right = Ramp::at(1.0);
        self.sends = [Ramp::at(0.0); 2];
    }
}
