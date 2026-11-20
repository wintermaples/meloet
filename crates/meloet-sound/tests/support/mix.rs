//! A song of sixteen computed instruments on the real engine, with or without a full mixer.

use meloet_engine::{
    BlockTiming, Effect, Engine, EngineCommand, EngineConfig, EngineParts, Instrument,
    InstrumentEvent, MixStrip, Sequence, SequenceEvent,
};
use meloet_model::TICKS_PER_QUARTER as Q;
use meloet_model::effects::EffectKind;
use meloet_sound::effects::create;
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};

pub const TRACKS: usize = 16;

fn instrument(track: usize) -> Box<dyn Instrument> {
    match track % 5 {
        0 => Box::new(PolySynth::pad()),
        1 => Box::new(PolySynth::lead()),
        2 => Box::new(PolySynth::bass()),
        3 => Box::new(ElectroKit::new()),
        _ => Box::new(SineSynth::new()),
    }
}

const BARS: u64 = 4;

/// A chord of three keys on every beat, held for most of it, for four bars.
fn sequence(track: usize) -> Sequence {
    let mut events = Vec::new();
    for beat in 0..BARS * 4 {
        let root = 36 + ((track as u64 * 5 + beat * 7) % 36) as u8;
        let keys = [root, root + 4, root + 7];
        for note in keys {
            events.push(SequenceEvent {
                tick: beat * Q,
                event: InstrumentEvent::NoteOn {
                    note,
                    velocity: 100,
                },
            });
        }
        for note in keys {
            events.push(SequenceEvent {
                tick: beat * Q + Q * 5 / 6,
                event: InstrumentEvent::NoteOff { note },
            });
        }
    }
    Sequence::new(events)
}

pub fn gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub struct Song {
    pub parts: EngineParts,
    out: Vec<f32>,
}

impl Song {
    /// Sixteen tracks at −12 dB, each playing its chords round and round. `inserts`: the effects of every track,
    /// from the first slot. `mixer`: the tracks send −15 dB to return A (a reverb) and −20 dB
    /// to return B (a delay), and the master has a limiter.
    pub fn new(inserts: &[EffectKind], mixer: bool) -> Song {
        let mut song = Song {
            parts: Engine::new(EngineConfig::default()),
            out: vec![0.0; 2048 * 2],
        };
        for track in 0..TRACKS {
            let handle = &mut song.parts.handle;
            assert!(handle.swap_instrument(track, instrument(track)).is_ok());
            assert!(handle.swap_sequence(track, sequence(track)).is_ok());
            song.send(EngineCommand::SetStrip {
                strip: MixStrip::Track(track),
                gain: gain(-12.0),
                pan: (track as f32 - 7.5) / 10.0,
            });
            for (slot, kind) in inserts.iter().enumerate() {
                song.effect(MixStrip::Track(track), slot, Some(create(*kind)));
            }
            if mixer {
                song.send(EngineCommand::SetSends {
                    track,
                    sends: [gain(-15.0), gain(-20.0)],
                });
            }
            song.block(64);
        }
        if mixer {
            song.effect(MixStrip::Return(0), 0, Some(create(EffectKind::Reverb)));
            song.effect(MixStrip::Return(1), 0, Some(create(EffectKind::Delay)));
            song.effect(MixStrip::Master, 0, Some(create(EffectKind::Limiter)));
        }
        song.send(EngineCommand::SetLoop {
            start: 0,
            end: BARS * 4 * Q,
            on: true,
        });
        song.send(EngineCommand::Play { from: 0 });
        song
    }

    pub fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    pub fn effect(&mut self, strip: MixStrip, slot: usize, effect: Option<Box<dyn Effect>>) {
        assert!(self.parts.handle.set_effect(strip, slot, effect).is_ok());
    }

    /// One device block; returns it, interleaved.
    pub fn block(&mut self, frames: usize) -> &[f32] {
        let out = &mut self.out[..frames * 2];
        self.parts.engine.process(out, BlockTiming::default());
        out
    }
}
