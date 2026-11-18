//! The mixer as the engine was given it, and the one place where effects are made.

use std::collections::BTreeMap;

use meloet_engine::{Effect, EngineCommand, MixStrip};
use meloet_model::{Gain, Insert, Inserts, Pan, Sends};
use meloet_sound::effects;

use crate::play::{EngineOp, StripSeat};

#[derive(Debug, Clone, PartialEq, Default)]
struct Strip {
    level: Option<(Gain, Pan)>,
    sends: Option<Sends>,
    inserts: Inserts,
}

/// What the engine was told about each strip, to tell an engine made anew.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct MixBook {
    strips: BTreeMap<StripSeat, Strip>,
}

impl MixBook {
    /// Takes in an op about the mixer. Returns false for any other op.
    pub fn note(&mut self, op: &EngineOp) -> bool {
        match op {
            EngineOp::Strip { strip, gain, pan } => {
                self.strips.entry(*strip).or_default().level = Some((*gain, *pan));
            }
            EngineOp::Sends { seat, sends } => {
                let strip = self.strips.entry(StripSeat::Track(*seat)).or_default();
                strip.sends = Some(*sends);
            }
            EngineOp::Insert {
                strip,
                slot,
                insert,
            } => {
                if let Some(place) = self
                    .strips
                    .entry(*strip)
                    .or_default()
                    .inserts
                    .get_mut(*slot)
                {
                    place.clone_from(insert);
                }
            }
            EngineOp::Param {
                strip,
                slot,
                param,
                value,
            } => {
                let strip = self.strips.get_mut(strip);
                if let Some(insert) = strip.and_then(|s| s.inserts.get_mut(*slot)?.as_mut()) {
                    insert.set(*param, *value);
                }
            }
            // The engine empties the strip with the seat. Not a mixer op: the seats see it.
            EngineOp::ClearSeat { seat } => {
                self.strips.remove(&StripSeat::Track(*seat));
                return false;
            }
            _ => return false,
        }
        true
    }

    /// The ops that bring an empty engine to this.
    pub fn ops(&self) -> Vec<EngineOp> {
        let mut ops = Vec::new();
        for (&strip, s) in &self.strips {
            if let Some((gain, pan)) = s.level {
                ops.push(EngineOp::Strip { strip, gain, pan });
            }
            if let (StripSeat::Track(seat), Some(sends)) = (strip, s.sends) {
                ops.push(EngineOp::Sends { seat, sends });
            }
            for (slot, insert) in s.inserts.iter().enumerate() {
                if insert.is_some() {
                    ops.push(EngineOp::Insert {
                        strip,
                        slot,
                        insert: insert.clone(),
                    });
                }
            }
        }
        ops
    }
}

pub(crate) fn mix_strip(strip: StripSeat) -> MixStrip {
    match strip {
        StripSeat::Track(seat) => MixStrip::Track(seat),
        StripSeat::Return(bus) => MixStrip::Return(bus.index()),
        StripSeat::Master => MixStrip::Master,
    }
}

/// The effect of an insert with its values. Call it outside the audio thread; the engine's
/// handle prepares what it gets.
pub(crate) fn make(insert: &Insert) -> Box<dyn Effect> {
    let mut effect = effects::create(insert.kind());
    for (param, value) in insert.values().iter().enumerate() {
        effect.set_param(param as u32, *value);
    }
    effect
}

/// The command of a mixer op that carries no effect. `None` for any other op.
pub(crate) fn command(op: &EngineOp) -> Option<EngineCommand> {
    Some(match *op {
        EngineOp::Strip { strip, gain, pan } => EngineCommand::SetStrip {
            strip: mix_strip(strip),
            gain: gain.linear(),
            pan: f32::from(pan.get()) / 100.0,
        },
        EngineOp::Sends { seat, sends } => EngineCommand::SetSends {
            track: seat,
            sends: sends.map(Gain::linear),
        },
        EngineOp::Param {
            strip,
            slot,
            param,
            value,
        } => EngineCommand::SetEffectParam {
            strip: mix_strip(strip),
            slot,
            param,
            value,
        },
        _ => return None,
    })
}
