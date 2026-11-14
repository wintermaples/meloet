//! "Placeholder sound": a soft, plainly electronic tone. It does not try to be a piano.
//!
//! A sine with a little 2nd and 3rd harmonic, a few ms of attack, a decay to a quiet sustain and a
//! short release. Velocity changes loudness and brightness.

use std::f32::consts::TAU;

use crate::instrument::{Instrument, InstrumentEvent};

const VOICES: usize = 32;
const ATTACK_S: f32 = 0.004;
const DECAY_TAU_S: f32 = 0.25;
const SUSTAIN_LEVEL: f32 = 0.35;
/// -60 dB after about 200 ms, voice freed (-80 dB) after about 280 ms.
const RELEASE_TAU_S: f32 = 0.030;
/// Fade used when a sounding voice is taken for a new note.
const STEAL_S: f32 = 0.003;
const SILENT: f32 = 1.0e-4;
const VOICE_GAIN: f32 = 0.16;
const BEND_SEMITONES: f32 = 2.0;
const VIBRATO_HZ: f32 = 5.5;
const VIBRATO_CENTS: f32 = 30.0;
const CC_MODULATION: u8 = 1;
const CC_SUSTAIN: u8 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Release,
    Steal,
}

#[derive(Clone, Copy)]
struct Pending {
    note: u8,
    velocity: u8,
    held: bool,
    started: u64,
}

#[derive(Clone, Copy)]
struct Voice {
    stage: Stage,
    note: u8,
    /// The key is still down. A voice with the key up keeps sounding only while the pedal is down.
    held: bool,
    phase: f32,
    /// Cycles per sample, before bend and vibrato.
    inc: f32,
    env: f32,
    amp: f32,
    h2: f32,
    h3: f32,
    steal_step: f32,
    started: u64,
    /// The note waiting for this voice to finish its steal fade.
    pending: Option<Pending>,
}

impl Voice {
    const IDLE: Voice = Voice {
        stage: Stage::Idle,
        note: 0,
        held: false,
        phase: 0.0,
        inc: 0.0,
        env: 0.0,
        amp: 0.0,
        h2: 0.0,
        h3: 0.0,
        steal_step: 0.0,
        started: 0,
        pending: None,
    };

    fn sounding(&self) -> bool {
        matches!(self.stage, Stage::Attack | Stage::Decay)
    }

    fn start(&mut self, note: u8, velocity: u8, held: bool, started: u64, sample_rate: f32) {
        let v = f32::from(velocity.min(127)) / 127.0;
        let freq = 440.0 * ((f32::from(note) - 69.0) / 12.0).exp2();
        // Leave out a harmonic that would pass Nyquist even with the bend fully up.
        let fits = |harmonic: f32| freq * harmonic * 1.15 < sample_rate * 0.5;
        *self = Voice {
            stage: Stage::Attack,
            note,
            held,
            phase: 0.0,
            inc: freq / sample_rate,
            env: 0.0,
            amp: VOICE_GAIN * (0.15 * v + 0.85 * v * v),
            h2: if fits(2.0) { 0.06 + 0.24 * v } else { 0.0 },
            h3: if fits(3.0) { 0.01 + 0.11 * v * v } else { 0.0 },
            steal_step: 0.0,
            started,
            pending: None,
        };
    }
}

pub struct PlaceholderSynth {
    voices: [Voice; VOICES],
    sample_rate: f32,
    attack_step: f32,
    decay_coef: f32,
    release_coef: f32,
    steal_frames: f32,
    pedal: bool,
    bend_ratio: f32,
    vibrato_depth: f32,
    lfo_phase: f32,
    /// Pitch ratio (bend x vibrato) at the end of the last rendered block.
    pitch_now: f32,
    note_counter: u64,
}

impl Default for PlaceholderSynth {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaceholderSynth {
    /// Usable only after [`Instrument::prepare`].
    pub fn new() -> Self {
        let mut synth = PlaceholderSynth {
            voices: [Voice::IDLE; VOICES],
            sample_rate: 48_000.0,
            attack_step: 0.0,
            decay_coef: 0.0,
            release_coef: 0.0,
            steal_frames: 1.0,
            pedal: false,
            bend_ratio: 1.0,
            vibrato_depth: 0.0,
            lfo_phase: 0.0,
            pitch_now: 1.0,
            note_counter: 0,
        };
        synth.set_rate(48_000.0);
        synth
    }

    fn set_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.attack_step = 1.0 / (ATTACK_S * sample_rate);
        self.decay_coef = (-1.0 / (DECAY_TAU_S * sample_rate)).exp();
        self.release_coef = (-1.0 / (RELEASE_TAU_S * sample_rate)).exp();
        self.steal_frames = (STEAL_S * sample_rate).max(1.0);
    }

    fn note_on(&mut self, note: u8, velocity: u8) {
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.sounding() && v.note == note)
        {
            v.stage = Stage::Release;
            v.held = false;
        }
        self.note_counter += 1;
        let started = self.note_counter;
        if let Some(v) = self.voices.iter_mut().find(|v| v.stage == Stage::Idle) {
            v.start(note, velocity, true, started, self.sample_rate);
            return;
        }
        // No free voice: take the oldest, a releasing one if there is any.
        let rank = |v: &Voice| {
            let class = match v.stage {
                Stage::Release => 0,
                Stage::Steal => 2,
                _ => 1,
            };
            (class, v.started)
        };
        let steal_frames = self.steal_frames;
        if let Some(v) = self.voices.iter_mut().min_by_key(|v| rank(v)) {
            if v.stage != Stage::Steal {
                v.stage = Stage::Steal;
                v.steal_step = v.env / steal_frames;
            }
            v.pending = Some(Pending {
                note,
                velocity,
                held: true,
                started,
            });
        }
    }

    fn note_off(&mut self, note: u8) {
        let pedal = self.pedal;
        for v in self.voices.iter_mut() {
            if v.sounding() && v.note == note && v.held {
                v.held = false;
                if !pedal {
                    v.stage = Stage::Release;
                }
            }
            if let Some(p) = v.pending.as_mut().filter(|p| p.note == note) {
                p.held = false;
            }
        }
    }

    fn release_all(&mut self) {
        for v in self.voices.iter_mut() {
            v.held = false;
            v.pending = None;
            if v.sounding() {
                v.stage = Stage::Release;
            }
        }
    }
}

impl Instrument for PlaceholderSynth {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.set_rate(sample_rate);
        self.reset();
    }

    fn event(&mut self, ev: InstrumentEvent) {
        match ev {
            InstrumentEvent::NoteOn { note, velocity: 0 } => self.note_off(note),
            InstrumentEvent::NoteOn { note, velocity } => self.note_on(note, velocity),
            InstrumentEvent::NoteOff { note } => self.note_off(note),
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } => {
                self.pedal = value >= 64;
                if !self.pedal {
                    for v in self.voices.iter_mut().filter(|v| v.sounding() && !v.held) {
                        v.stage = Stage::Release;
                    }
                }
            }
            InstrumentEvent::Control {
                controller: CC_MODULATION,
                value,
            } => {
                let full = (VIBRATO_CENTS / 1200.0).exp2() - 1.0;
                self.vibrato_depth = full * f32::from(value.min(127)) / 127.0;
            }
            InstrumentEvent::Control { .. } => {}
            InstrumentEvent::PitchBend { value } => {
                let norm = (f32::from(value) / 8192.0).clamp(-1.0, 1.0);
                self.bend_ratio = (norm * BEND_SEMITONES / 12.0).exp2();
            }
            InstrumentEvent::AllNotesOff => self.release_all(),
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let (left, right) = (&mut left[..frames], &mut right[..frames]);
        left.fill(0.0);
        if frames == 0 {
            return;
        }

        // Bend and vibrato move in a straight line across the block, so neither steps.
        self.lfo_phase = (self.lfo_phase + VIBRATO_HZ * frames as f32 / self.sample_rate).fract();
        let pitch_start = self.pitch_now;
        let pitch_end = self.bend_ratio * (1.0 + self.vibrato_depth * (TAU * self.lfo_phase).sin());
        let pitch_step = (pitch_end - pitch_start) / frames as f32;
        self.pitch_now = pitch_end;

        let (attack_step, decay_coef, release_coef) =
            (self.attack_step, self.decay_coef, self.release_coef);
        let (pedal, sample_rate) = (self.pedal, self.sample_rate);

        for v in self.voices.iter_mut() {
            if v.stage == Stage::Idle {
                continue;
            }
            let mut pitch = pitch_start;
            for out in left.iter_mut() {
                pitch += pitch_step;
                match v.stage {
                    Stage::Attack => {
                        v.env += attack_step;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = Stage::Decay;
                        }
                    }
                    Stage::Decay => v.env = SUSTAIN_LEVEL + (v.env - SUSTAIN_LEVEL) * decay_coef,
                    Stage::Release => {
                        v.env *= release_coef;
                        if v.env < SILENT {
                            v.stage = Stage::Idle;
                            break;
                        }
                    }
                    Stage::Steal => {
                        v.env -= v.steal_step;
                        if v.env <= SILENT {
                            let Some(p) = v.pending else {
                                v.stage = Stage::Idle;
                                break;
                            };
                            v.start(p.note, p.velocity, p.held, p.started, sample_rate);
                            if !p.held && !pedal {
                                v.stage = Stage::Release;
                            }
                            continue;
                        }
                    }
                    Stage::Idle => break,
                }
                let (s, c) = (TAU * v.phase).sin_cos();
                // sin x + h2 sin 2x + h3 sin 3x, from one sin/cos pair.
                let wave = s * (1.0 + 2.0 * v.h2 * c + v.h3 * (3.0 - 4.0 * s * s));
                *out += wave * v.env * v.amp;
                v.phase += v.inc * pitch;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
            }
        }
        right.copy_from_slice(left);
    }

    fn reset(&mut self) {
        self.voices = [Voice::IDLE; VOICES];
        self.pedal = false;
        self.bend_ratio = 1.0;
        self.vibrato_depth = 0.0;
        self.lfo_phase = 0.0;
        self.pitch_now = 1.0;
    }
}
