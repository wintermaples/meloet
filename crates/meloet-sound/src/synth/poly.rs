//! One subtractive synth behind the lead, the bass and the pad: a few saw or pulse oscillators
//! into a low-pass filter, with one envelope for loudness and one for the filter. A patch is a
//! table of numbers.

use std::f32::consts::TAU;

use meloet_engine::knobs::{VoiceKnobs, vibrato_ratio};
use meloet_engine::{Instrument, InstrumentEvent, SongPlace};

use super::dsp::{Rng, Svf, decay_coef, pulse, saw, svf_g};

const MAX_VOICES: usize = 16;
const MAX_OSCS: usize = 4;
/// Bend, modulation and the filter cutoff are worked out once per this many frames and move in a
/// straight line in between.
const CHUNK: usize = 32;
/// Fade used when a sounding voice is taken for a new note.
const STEAL_S: f32 = 0.003;
const SILENT: f32 = 1.0e-4;
const SMOOTH_S: f32 = 0.005;
const BEND_SEMITONES: f32 = 2.0;
const VIBRATO_HZ: f32 = 5.5;
const CC_MODULATION: u8 = 1;
const CC_SUSTAIN: u8 = 64;

#[derive(Clone, Copy)]
enum Wave {
    Saw,
    /// The share of the cycle the pulse is high.
    Pulse(f32),
}

#[derive(Clone, Copy)]
struct Osc {
    wave: Wave,
    /// Away from the played key, in semitones.
    semitones: f32,
    level: f32,
    /// -1 left, 1 right. Used only by a stereo patch.
    pan: f32,
}

struct Patch {
    voices: usize,
    oscs: &'static [Osc],
    /// Left and right get their own filter, so panned oscillators stay apart.
    stereo: bool,
    /// Each note starts its oscillators somewhere else in their cycle, so the peaks of a chord
    /// do not line up. Off for a sound whose attack should be the same every time.
    random_phase: bool,
    /// Filter cutoff at C4 and full velocity, with the filter envelope at rest.
    cutoff_hz: f32,
    /// How far the cutoff follows the key: 1 keeps the same tone colour on every key.
    key_track: f32,
    /// How far a soft key closes the filter, in octaves at velocity 0.
    velocity_octaves: f32,
    /// How far the filter opens at the start of a note, in octaves at full velocity.
    filter_env_octaves: f32,
    filter_decay_s: f32,
    resonance_q: f32,
    attack_s: f32,
    decay_s: f32,
    sustain: f32,
    release_s: f32,
    vibrato_cents: f32,
    /// How far the modulation wheel opens the filter.
    mod_cutoff_octaves: f32,
    gain: f32,
}

const LEAD: Patch = Patch {
    voices: 16,
    oscs: &[
        Osc {
            wave: Wave::Saw,
            semitones: -0.05,
            level: 0.6,
            pan: 0.0,
        },
        Osc {
            wave: Wave::Pulse(0.5),
            semitones: 0.05,
            level: 0.4,
            pan: 0.0,
        },
    ],
    stereo: false,
    random_phase: true,
    cutoff_hz: 2600.0,
    key_track: 0.5,
    velocity_octaves: 1.0,
    filter_env_octaves: 2.0,
    filter_decay_s: 0.16,
    resonance_q: 1.3,
    attack_s: 0.003,
    decay_s: 0.30,
    sustain: 0.65,
    release_s: 0.045,
    vibrato_cents: 30.0,
    mod_cutoff_octaves: 0.0,
    gain: LEAD_GAIN,
};

const BASS: Patch = Patch {
    voices: 8,
    oscs: &[
        Osc {
            wave: Wave::Saw,
            semitones: 0.0,
            level: 0.55,
            pan: 0.0,
        },
        Osc {
            wave: Wave::Saw,
            semitones: 0.09,
            level: 0.30,
            pan: 0.0,
        },
        Osc {
            wave: Wave::Pulse(0.5),
            semitones: -12.0,
            level: 0.50,
            pan: 0.0,
        },
    ],
    stereo: false,
    random_phase: false,
    cutoff_hz: 520.0,
    key_track: 0.3,
    velocity_octaves: 1.0,
    filter_env_octaves: 3.2,
    filter_decay_s: 0.13,
    resonance_q: 1.6,
    attack_s: 0.002,
    decay_s: 0.25,
    sustain: 0.75,
    release_s: 0.035,
    vibrato_cents: 0.0,
    mod_cutoff_octaves: 2.5,
    gain: BASS_GAIN,
};

const PAD: Patch = Patch {
    voices: 16,
    oscs: &[
        Osc {
            wave: Wave::Saw,
            semitones: -0.11,
            level: 0.5,
            pan: -0.8,
        },
        Osc {
            wave: Wave::Saw,
            semitones: -0.04,
            level: 0.5,
            pan: 0.5,
        },
        Osc {
            wave: Wave::Saw,
            semitones: 0.05,
            level: 0.5,
            pan: -0.5,
        },
        Osc {
            wave: Wave::Saw,
            semitones: 0.12,
            level: 0.5,
            pan: 0.8,
        },
    ],
    stereo: true,
    random_phase: true,
    cutoff_hz: 1100.0,
    key_track: 0.4,
    velocity_octaves: 1.0,
    filter_env_octaves: 0.0,
    filter_decay_s: 1.0,
    resonance_q: 0.7,
    attack_s: 0.45,
    decay_s: 1.5,
    sustain: 0.8,
    release_s: 0.5,
    vibrato_cents: 0.0,
    mod_cutoff_octaves: 2.0,
    gain: PAD_GAIN,
};

// Set by measurement so that C4 at velocity 100 peaks near -12 dBFS (tests/synth_voices.rs).
const LEAD_GAIN: f32 = 0.213;
const BASS_GAIN: f32 = 0.194;
const PAD_GAIN: f32 = 0.301;

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
    /// Where the waves start, for a press of the song.
    seed: Option<u32>,
}

/// What a voice needs from the synth to start a note.
#[derive(Clone, Copy)]
struct StartCtx {
    patch: &'static Patch,
    sample_rate: f32,
    /// The modulation wheel's share of the cutoff, as a ratio.
    cutoff_mod: f32,
    /// What the knobs make of the attack and the release now.
    attack_step: f32,
    release_coef: f32,
}

#[derive(Clone, Copy)]
struct Voice {
    stage: Stage,
    note: u8,
    /// The key is still down. A voice with the key up keeps sounding only while the pedal is down.
    held: bool,
    phase: [f32; MAX_OSCS],
    /// Cycles per sample, before bend and vibrato.
    inc: [f32; MAX_OSCS],
    env: f32,
    amp: f32,
    steal_step: f32,
    /// Cutoff with the filter envelope at rest and the wheel down.
    cutoff_hz: f32,
    /// Filter envelope: 1 at the start of the note, falling to 0.
    filter_env: f32,
    filter_env_octaves: f32,
    /// The filter coefficient now; it moves to its target across a chunk.
    g: f32,
    filter: [Svf; 2],
    started: u64,
    /// The note waiting for this voice to finish its steal fade.
    pending: Option<Pending>,
    attack_step: f32,
    release_coef: f32,
    /// Frames since the voice started.
    age: u64,
    /// Pitch ratio of the knob's vibrato at the end of the last chunk.
    vib: f32,
}

impl Voice {
    const IDLE: Voice = Voice {
        stage: Stage::Idle,
        note: 0,
        held: false,
        phase: [0.0; MAX_OSCS],
        inc: [0.0; MAX_OSCS],
        env: 0.0,
        amp: 0.0,
        steal_step: 0.0,
        cutoff_hz: 1000.0,
        filter_env: 0.0,
        filter_env_octaves: 0.0,
        g: 0.1,
        filter: [Svf::ZERO; 2],
        started: 0,
        pending: None,
        attack_step: 0.0,
        release_coef: 0.0,
        age: 0,
        vib: 1.0,
    };

    fn sounding(&self) -> bool {
        matches!(self.stage, Stage::Attack | Stage::Decay)
    }

    fn release(&mut self, coef: f32) {
        self.stage = Stage::Release;
        self.release_coef = coef;
    }

    fn start(&mut self, p: Pending, ctx: &StartCtx, rng: &mut Rng) {
        let patch = ctx.patch;
        let mut own = p.seed.map(Rng::new);
        let rng = own.as_mut().unwrap_or(rng);
        let v = f32::from(p.velocity.min(127)) / 127.0;
        let key = f32::from(p.note);
        let freq = 440.0 * ((key - 69.0) / 12.0).exp2();
        let mut phase = [0.5; MAX_OSCS];
        let mut inc = [0.0; MAX_OSCS];
        for (i, osc) in patch.oscs.iter().enumerate() {
            inc[i] = freq * (osc.semitones / 12.0).exp2() / ctx.sample_rate;
            if patch.random_phase {
                phase[i] = rng.unit();
            }
        }
        let cutoff_hz = patch.cutoff_hz
            * (patch.key_track * (key - 60.0) / 12.0 + patch.velocity_octaves * (v - 1.0)).exp2();
        let filter_env_octaves = patch.filter_env_octaves * (0.4 + 0.6 * v);
        *self = Voice {
            stage: Stage::Attack,
            note: p.note,
            held: p.held,
            phase,
            inc,
            env: 0.0,
            amp: patch.gain * v * (0.4 + 0.6 * v),
            steal_step: 0.0,
            cutoff_hz,
            filter_env: 1.0,
            filter_env_octaves,
            g: svf_g(
                cutoff_hz * ctx.cutoff_mod * filter_env_octaves.exp2(),
                ctx.sample_rate,
            ),
            filter: [Svf::ZERO; 2],
            started: p.started,
            pending: None,
            attack_step: ctx.attack_step,
            release_coef: ctx.release_coef,
            age: 0,
            vib: 1.0,
        };
    }
}

/// The synth lead, the synth bass or the warm pad, by the patch it was made with.
pub struct PolySynth {
    patch: &'static Patch,
    voices: [Voice; MAX_VOICES],
    /// Left and right gain of each oscillator.
    osc_gain: [[f32; 2]; MAX_OSCS],
    sample_rate: f32,
    attack_step: f32,
    decay_coef: f32,
    release_coef: f32,
    filter_decay_coef: f32,
    steal_frames: f32,
    pedal: bool,
    bend_target: f32,
    bend_now: f32,
    mod_target: f32,
    mod_now: f32,
    lfo_phase: f32,
    /// Pitch ratio (bend x vibrato) at the end of the last chunk.
    pitch_now: f32,
    note_counter: u64,
    rng: Rng,
    knobs: VoiceKnobs,
}

impl PolySynth {
    /// Bright, for short lines. Usable only after [`Instrument::prepare`], like the others.
    pub fn lead() -> PolySynth {
        PolySynth::new(&LEAD)
    }

    /// Fat around C2, with an edge at the start of each note.
    pub fn bass() -> PolySynth {
        PolySynth::new(&BASS)
    }

    /// Slow to come and slow to go, wide.
    pub fn pad() -> PolySynth {
        PolySynth::new(&PAD)
    }

    fn new(patch: &'static Patch) -> PolySynth {
        debug_assert!(patch.voices <= MAX_VOICES && patch.oscs.len() <= MAX_OSCS);
        let mut osc_gain = [[0.0; 2]; MAX_OSCS];
        for (gain, osc) in osc_gain.iter_mut().zip(patch.oscs) {
            *gain = if patch.stereo {
                [
                    osc.level * (0.5 * (1.0 - osc.pan)).sqrt(),
                    osc.level * (0.5 * (1.0 + osc.pan)).sqrt(),
                ]
            } else {
                [osc.level; 2]
            };
        }
        let mut synth = PolySynth {
            patch,
            voices: [Voice::IDLE; MAX_VOICES],
            osc_gain,
            sample_rate: 48_000.0,
            attack_step: 0.0,
            decay_coef: 0.0,
            release_coef: 0.0,
            filter_decay_coef: 0.0,
            steal_frames: 1.0,
            pedal: false,
            bend_target: 1.0,
            bend_now: 1.0,
            mod_target: 0.0,
            mod_now: 0.0,
            lfo_phase: 0.0,
            pitch_now: 1.0,
            note_counter: 0,
            rng: Rng::new(0x6D65_6C6F),
            knobs: VoiceKnobs::new(),
        };
        synth.set_rate(48_000.0);
        synth
    }

    fn set_rate(&mut self, sample_rate: f32) {
        let p = self.patch;
        self.sample_rate = sample_rate;
        self.attack_step = 1.0 / (p.attack_s * sample_rate);
        self.decay_coef = decay_coef(p.decay_s, sample_rate);
        self.release_coef = decay_coef(p.release_s, sample_rate);
        self.filter_decay_coef = decay_coef(p.filter_decay_s, sample_rate);
        self.steal_frames = (STEAL_S * sample_rate).max(1.0);
    }

    fn start_ctx(&self) -> StartCtx {
        let (p, k) = (self.patch, &self.knobs);
        StartCtx {
            patch: p,
            sample_rate: self.sample_rate,
            cutoff_mod: (p.mod_cutoff_octaves * self.mod_now).exp2(),
            attack_step: if k.attack_as_is() {
                self.attack_step
            } else {
                1.0 / (k.attack_s(p.attack_s) * self.sample_rate).max(1.0)
            },
            release_coef: if k.release_as_is() {
                self.release_coef
            } else {
                decay_coef(p.release_s * k.release_scale(), self.sample_rate)
            },
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, seed: Option<u32>) {
        let ctx = self.start_ctx();
        let voices = &mut self.voices[..self.patch.voices];
        for v in voices.iter_mut().filter(|v| v.sounding() && v.note == note) {
            v.release(ctx.release_coef);
            v.held = false;
        }
        self.note_counter += 1;
        let pending = Pending {
            note,
            velocity,
            held: true,
            started: self.note_counter,
            seed,
        };
        if let Some(v) = voices.iter_mut().find(|v| v.stage == Stage::Idle) {
            v.start(pending, &ctx, &mut self.rng);
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
        if let Some(v) = voices.iter_mut().min_by_key(|v| rank(v)) {
            if v.stage != Stage::Steal {
                v.stage = Stage::Steal;
                v.steal_step = v.env / steal_frames;
            }
            v.pending = Some(pending);
        }
    }

    fn note_off(&mut self, note: u8) {
        let pedal = self.pedal;
        let coef = self.start_ctx().release_coef;
        for v in self.voices.iter_mut() {
            if v.sounding() && v.note == note && v.held {
                v.held = false;
                if !pedal {
                    v.release(coef);
                }
            }
            if let Some(p) = v.pending.as_mut().filter(|p| p.note == note) {
                p.held = false;
            }
        }
    }

    fn release_all(&mut self) {
        let coef = self.start_ctx().release_coef;
        for v in self.voices.iter_mut() {
            v.held = false;
            v.pending = None;
            if v.sounding() {
                v.release(coef);
            }
        }
    }

    /// Adds up to [`CHUNK`] frames of every voice to the two slices.
    fn render_chunk(&mut self, left: &mut [f32], right: &mut [f32]) {
        let patch = self.patch;
        let frames = left.len();
        let n = frames as f32;
        let sample_rate = self.sample_rate;

        let smooth = 1.0 - (-n / (SMOOTH_S * sample_rate)).exp();
        self.bend_now = approach(self.bend_now, self.bend_target, smooth);
        self.mod_now = approach(self.mod_now, self.mod_target, smooth);
        self.lfo_phase = (self.lfo_phase + VIBRATO_HZ * n / sample_rate).fract();
        let vibrato = ((patch.vibrato_cents / 1200.0).exp2() - 1.0) * self.mod_now;
        let pitch_start = self.pitch_now;
        let pitch_end = self.bend_now * (1.0 + vibrato * (TAU * self.lfo_phase).sin());
        let pitch_step = (pitch_end - pitch_start) / n;
        self.pitch_now = pitch_end;

        let ctx = self.start_ctx();
        let filter_env_fall = self.filter_decay_coef.powi(frames as i32);
        let k = 1.0 / patch.resonance_q;
        let decay_coef = self.decay_coef;
        let (pedal, sustain) = (self.pedal, patch.sustain);
        let osc_gain = &self.osc_gain[..patch.oscs.len()];
        let vibrato = self.knobs.vibrato_on();
        let cents = self.knobs.advance_vibrato(frames);

        for v in self.voices[..patch.voices].iter_mut() {
            if v.stage == Stage::Idle {
                continue;
            }
            v.filter_env *= filter_env_fall;
            let cutoff =
                v.cutoff_hz * ctx.cutoff_mod * (v.filter_env_octaves * v.filter_env).exp2();
            let mut g_step = (svf_g(cutoff, sample_rate) - v.g) / n;
            let mut pitch = pitch_start;
            let (mut vib, mut vib_step) = (1.0, 0.0);
            if vibrato {
                let end = vibrato_ratio(cents, v.age + frames as u64, sample_rate, 1.0);
                (vib, vib_step) = (v.vib, (end - v.vib) / n);
            }
            v.age += frames as u64;
            for (out_l, out_r) in left.iter_mut().zip(right.iter_mut()) {
                pitch += pitch_step;
                vib += vib_step;
                match v.stage {
                    Stage::Attack => {
                        v.env += v.attack_step;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = Stage::Decay;
                        }
                    }
                    Stage::Decay => v.env = sustain + (v.env - sustain) * decay_coef,
                    Stage::Release => {
                        v.env *= v.release_coef;
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
                            v.start(p, &ctx, &mut self.rng);
                            if !p.held && !pedal {
                                v.stage = Stage::Release;
                            }
                            g_step = 0.0;
                            (vib, vib_step) = (1.0, 0.0);
                            continue;
                        }
                    }
                    Stage::Idle => break,
                }

                let (mut l, mut r) = (0.0, 0.0);
                for (i, osc) in patch.oscs.iter().enumerate() {
                    let t = v.phase[i];
                    let dt = (v.inc[i] * (pitch * vib)).min(0.45);
                    let s = match osc.wave {
                        Wave::Saw => saw(t, dt),
                        Wave::Pulse(width) => pulse(t, dt, width),
                    };
                    l += s * osc_gain[i][0];
                    r += s * osc_gain[i][1];
                    let next = t + dt;
                    v.phase[i] = if next >= 1.0 { next - 1.0 } else { next };
                }
                v.g += g_step;
                l = v.filter[0].tick(l, v.g, k).0;
                r = if patch.stereo {
                    v.filter[1].tick(r, v.g, k).0
                } else {
                    l
                };
                let gain = v.env * v.amp;
                *out_l += l * gain;
                *out_r += r * gain;
            }
            v.vib = if vibrato { vib } else { 1.0 };
        }
    }
}

/// One step of a smoothed value towards its target. Lands on the target instead of creeping
/// towards it for ever in ever smaller numbers.
fn approach(now: f32, target: f32, share: f32) -> f32 {
    let diff = target - now;
    if diff.abs() < 1.0e-6 {
        target
    } else {
        now + diff * share
    }
}

impl Instrument for PolySynth {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.set_rate(sample_rate);
        self.knobs.prepare(sample_rate);
        self.reset();
    }

    fn event(&mut self, ev: InstrumentEvent) {
        match ev {
            InstrumentEvent::NoteOn { note, velocity: 0 } => self.note_off(note),
            InstrumentEvent::NoteOn { note, velocity } => self.note_on(note, velocity, None),
            InstrumentEvent::NoteOff { note } => self.note_off(note),
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } => {
                self.pedal = value >= 64;
                if !self.pedal {
                    let coef = self.start_ctx().release_coef;
                    for v in self.voices.iter_mut().filter(|v| v.sounding() && !v.held) {
                        v.release(coef);
                    }
                }
            }
            InstrumentEvent::Control {
                controller: CC_MODULATION,
                value,
            } => self.mod_target = f32::from(value.min(127)) / 127.0,
            InstrumentEvent::Control { .. } => {}
            InstrumentEvent::PitchBend { value } => {
                let norm = (f32::from(value) / 8192.0).clamp(-1.0, 1.0);
                self.bend_target = (norm * BEND_SEMITONES / 12.0).exp2();
            }
            InstrumentEvent::AllNotesOff => self.release_all(),
        }
    }

    fn song_note_on(&mut self, note: u8, velocity: u8, place: SongPlace<'_>) {
        if velocity > 0 {
            self.note_on(note, velocity, Some(place.seed()));
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let (left, right) = (&mut left[..frames], &mut right[..frames]);
        left.fill(0.0);
        right.fill(0.0);
        self.knobs.rendering();
        for (l, r) in left.chunks_mut(CHUNK).zip(right.chunks_mut(CHUNK)) {
            self.render_chunk(l, r);
        }
    }

    fn reset(&mut self) {
        self.voices = [Voice::IDLE; MAX_VOICES];
        self.pedal = false;
        self.bend_target = 1.0;
        self.bend_now = 1.0;
        self.mod_target = 0.0;
        self.mod_now = 0.0;
        self.lfo_phase = 0.0;
        self.pitch_now = 1.0;
        self.knobs.settle();
    }

    fn set_knob(&mut self, knob: u32, place: f32) {
        self.knobs.set(knob, place);
    }
}
