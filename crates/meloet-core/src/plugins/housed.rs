//! A loaded plugin as the engine holds it, and the word that it was handed back.
//!
//! The engine hands an instrument or an effect back on its retired rings when something
//! else takes its place, and the core thread drops it there. Only then may the plugin's
//! process be told to let it go (docs/architecture.md 20-10): the drop says so over a
//! channel the core thread reads.

use std::sync::mpsc::Sender;

use meloet_engine::{
    Effect, EffectContext, Instrument, InstrumentEvent, PluginEffect, PluginInstrument, PluginSlot,
};

use super::hosts::SlotId;

/// Which load of which plugin: a plugin that stopped and was loaded again keeps its id, and
/// the slot it had before is handed back after the new one is in the engine.
pub(crate) type Which = (SlotId, u32);

/// Tells that the engine's end of a plugin is gone, when it is.
struct Gone {
    which: Which,
    tell: Sender<Which>,
}

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = self.tell.send(self.which);
    }
}

pub(crate) struct HousedInstrument {
    // Before `gone`: the slot is dropped first.
    inner: PluginInstrument,
    _gone: Gone,
}

impl HousedInstrument {
    pub fn new(slot: PluginSlot, which: Which, tell: Sender<Which>) -> HousedInstrument {
        HousedInstrument {
            inner: PluginInstrument(slot),
            _gone: Gone { which, tell },
        }
    }
}

impl Instrument for HousedInstrument {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize) {
        self.inner.prepare(sample_rate, max_frames);
    }

    fn event(&mut self, ev: InstrumentEvent) {
        self.inner.event(ev);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.inner.render(left, right);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn ask(&mut self, frames: usize) {
        self.inner.ask(frames);
    }

    fn latency(&self) -> u32 {
        self.inner.latency()
    }

    fn set_param_at(&mut self, frame: u32, id: u32, value: f32) {
        self.inner.set_param_at(frame, id, value);
    }
}

pub(crate) struct HousedEffect {
    inner: PluginEffect,
    _gone: Gone,
}

impl HousedEffect {
    pub fn new(slot: PluginSlot, which: Which, tell: Sender<Which>) -> HousedEffect {
        HousedEffect {
            inner: PluginEffect(slot),
            _gone: Gone { which, tell },
        }
    }
}

impl Effect for HousedEffect {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize) {
        self.inner.prepare(sample_rate, max_frames);
    }

    fn set_param(&mut self, index: u32, value: f32) {
        self.inner.set_param(index, value);
    }

    fn takes_points(&self) -> bool {
        self.inner.takes_points()
    }

    fn set_param_at(&mut self, frame: u32, index: u32, value: f32) {
        self.inner.set_param_at(frame, index, value);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], ctx: &EffectContext) {
        self.inner.process(left, right, ctx);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn latency(&self) -> u32 {
        self.inner.latency()
    }
}
