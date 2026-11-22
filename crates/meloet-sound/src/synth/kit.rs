//! The electro kit: twelve drum-machine sounds made from sines, pulse waves and noise.
//!
//! Every sound is a one-shot: a key-up does nothing, the sound runs to its own end. Each sound
//! has two voices, so a new hit starts at once while the one before it fades.

use std::f32::consts::TAU;

use meloet_engine::{Instrument, InstrumentEvent, SongPlace};
use meloet_model::drums::DrumPiece;

use super::dsp::{Rng, Svf, decay_coef, pulse, svf_g};

const SLOTS: usize = 2;
const VOICES: usize = DrumPiece::COUNT * SLOTS;
const SILENT: f32 = 1.0e-4;
/// Fade of a voice that a waiting hit needs.
const STEAL_S: f32 = 0.002;
/// Fade of the hit before, when the same drum is hit again.
const RETRIGGER_S: f32 = 0.005;
/// Fade of an open hi-hat that a closed or pedal hi-hat stops.
const CHOKE_S: f32 = 0.012;
/// Six pulse waves at these pitches, none a multiple of another, make the metal of the hi-hats
/// and cymbals (the pitches of the TR-808's cymbal oscillators).
const METAL_HZ: [f32; 6] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0];

#[derive(Clone, Copy)]
enum Shape {
    /// Kick and toms: a sine that falls in pitch, plus a click.
    /// env 0 loudness, 1 click, 2 pitch.
    Tone {
        base_inc: f32,
        sweep: f32,
        drive: f32,
        click: f32,
    },
    /// Two sines and high-passed noise. env 0 sines, 1 noise, 2 pitch.
    Snare { noise: f32 },
    /// Two short high sines and a click. env 0 sines, 1 click.
    SideStick,
    /// Band-passed noise in three quick bursts and a tail. env 0.
    Clap { gap: u32 },
    /// Hi-hats and cymbals. env 0 the long ring, 1 the short hit, 2 the rise at the start.
    Metal {
        noise: f32,
        hit: f32,
        rise_step: f32,
    },
}

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    shape: Shape,
    /// Counts hits, to tell the older of two voices.
    order: u64,
    age: u32,
    gain: [f32; 2],
    fade: f32,
    /// 0 while the voice is not fading out.
    fade_step: f32,
    /// The velocity of the hit that waits for this voice to finish fading, and its seed.
    pending: Option<(u8, Option<u32>)>,
    phase: [f32; 6],
    inc: [f32; 6],
    env: [f32; 3],
    coef: [f32; 3],
    g: f32,
    k: f32,
    filter: Svf,
    rng: Rng,
}

impl Voice {
    const IDLE: Voice = Voice {
        active: false,
        shape: Shape::SideStick,
        order: 0,
        age: 0,
        gain: [0.0; 2],
        fade: 1.0,
        fade_step: 0.0,
        pending: None,
        phase: [0.0; 6],
        inc: [0.0; 6],
        env: [0.0; 3],
        coef: [0.0; 3],
        g: 0.1,
        k: 1.0,
        filter: Svf::ZERO,
        rng: Rng::new(1),
    };

    /// `seed`: a hit of the song, whose noise and metal start from it.
    fn start(
        &mut self,
        piece: DrumPiece,
        (velocity, seed): (u8, Option<u32>),
        order: u64,
        sample_rate: f32,
    ) {
        let v = f32::from(velocity.min(127)) / 127.0;
        let hz = |f: f32| f / sample_rate;
        let dc = |tau_s: f32| decay_coef(tau_s, sample_rate);
        let (level, pan) = level_and_pan(piece);
        let amp = level * v * (0.4 + 0.6 * v);
        *self = Voice {
            active: true,
            order,
            gain: [amp * (1.0 - pan).sqrt(), amp * (1.0 + pan).sqrt()],
            env: [1.0; 3],
            rng: seed.map_or(self.rng, Rng::new),
            ..Voice::IDLE
        };

        let mut tone = |f: f32, sweep: f32, ring_s: f32, fall_s: f32, drive: f32, click: f32| {
            self.coef = [dc(ring_s), dc(0.0015), dc(fall_s)];
            self.shape = Shape::Tone {
                base_inc: hz(f),
                sweep,
                drive,
                click,
            };
        };
        match piece {
            DrumPiece::Kick => tone(50.0, 2.2 + 1.3 * v, 0.20, 0.030, 1.1 + 0.5 * v, 0.25 * v),
            DrumPiece::LowTom => tone(85.0, 0.5 + 0.3 * v, 0.22, 0.05, 1.0, 0.08 * v),
            DrumPiece::MidTom => tone(120.0, 0.5 + 0.3 * v, 0.19, 0.05, 1.0, 0.08 * v),
            DrumPiece::HighTom => tone(165.0, 0.5 + 0.3 * v, 0.16, 0.05, 1.0, 0.08 * v),
            DrumPiece::Snare => {
                self.inc[0] = hz(185.0);
                self.inc[1] = hz(330.0);
                self.coef = [dc(0.07), dc(0.10 + 0.04 * v), dc(0.012)];
                self.g = svf_g(1400.0, sample_rate);
                self.shape = Shape::Snare {
                    noise: 0.6 + 0.4 * v,
                };
            }
            DrumPiece::SideStick => {
                self.inc[0] = hz(455.0);
                self.inc[1] = hz(1667.0);
                self.coef = [dc(0.010), dc(0.0012), 0.0];
                self.g = svf_g(3000.0, sample_rate);
                self.k = 0.7;
                self.shape = Shape::SideStick;
            }
            DrumPiece::Clap => {
                self.coef = [dc(0.006), dc(0.07 + 0.03 * v), 0.0];
                self.g = svf_g(1100.0, sample_rate);
                self.k = 0.6;
                self.shape = Shape::Clap {
                    gap: (0.011 * sample_rate) as u32,
                };
            }
            DrumPiece::ClosedHiHat => self.metal(sample_rate, 1.0, 7000.0, 0.035, 0.15, v),
            DrumPiece::PedalHiHat => self.metal(sample_rate, 1.0, 6000.0, 0.050, 0.15, v),
            DrumPiece::OpenHiHat => self.metal(sample_rate, 1.0, 6500.0, 0.26 + 0.08 * v, 0.15, v),
            DrumPiece::Crash => self.metal(sample_rate, 1.21, 3500.0, 0.75 + 0.25 * v, 0.5, v),
            DrumPiece::Ride => self.metal(sample_rate, 1.53, 5200.0, 0.55, 0.1, v),
        }
        match (piece, &mut self.shape) {
            // The foot closes the hi-hat more softly than a stick hits it.
            (DrumPiece::PedalHiHat, Shape::Metal { rise_step, .. }) => {
                self.env[2] = 0.0;
                *rise_step = 1.0 / (0.003 * sample_rate);
            }
            (DrumPiece::Crash, Shape::Metal { hit, .. }) => {
                self.coef[1] = dc(0.06);
                *hit = 0.35;
            }
            (DrumPiece::Ride, Shape::Metal { hit, .. }) => {
                self.coef[1] = dc(0.025);
                *hit = 0.5;
            }
            _ => {}
        }
    }

    fn metal(&mut self, sample_rate: f32, tune: f32, cut_hz: f32, ring_s: f32, noise: f32, v: f32) {
        for (inc, f) in self.inc.iter_mut().zip(METAL_HZ) {
            *inc = f * tune / sample_rate;
        }
        for phase in self.phase.iter_mut() {
            *phase = self.rng.unit();
        }
        self.coef = [decay_coef(ring_s, sample_rate), 0.0, 0.0];
        // A harder hit lets more of the lower metal through.
        self.g = svf_g(cut_hz * (1.25 - 0.25 * v), sample_rate);
        self.shape = Shape::Metal {
            noise,
            hit: 0.0,
            rise_step: 1.0,
        };
    }

    #[inline]
    fn turn(&mut self, i: usize, inc: f32) -> f32 {
        let t = self.phase[i];
        let next = t + inc;
        self.phase[i] = if next >= 1.0 { next - 1.0 } else { next };
        t
    }

    /// The next sample, before the voice's gain and fade.
    #[inline]
    fn tick(&mut self) -> f32 {
        let out = match self.shape {
            Shape::Tone {
                base_inc,
                sweep,
                drive,
                click,
            } => {
                let t = self.turn(0, base_inc * (1.0 + sweep * self.env[2]));
                soft_clip((TAU * t).sin() * drive) * self.env[0]
                    + self.rng.bipolar() * self.env[1] * click
            }
            Shape::Snare { noise } => {
                let bend = 1.0 + 0.6 * self.env[2];
                let (a, b) = (
                    self.turn(0, self.inc[0] * bend),
                    self.turn(1, self.inc[1] * bend),
                );
                let high = self.filter.tick(self.rng.bipolar(), self.g, self.k).2;
                ((TAU * a).sin() + 0.6 * (TAU * b).sin()) * 0.55 * self.env[0]
                    + high * noise * self.env[1]
            }
            Shape::SideStick => {
                let (a, b) = (self.turn(0, self.inc[0]), self.turn(1, self.inc[1]));
                let band = self.filter.tick(self.rng.bipolar(), self.g, self.k).1;
                (0.6 * (TAU * a).sin() + 0.8 * (TAU * b).sin()) * self.env[0] + band * self.env[1]
            }
            Shape::Clap { gap } => {
                if self.age == gap || self.age == 2 * gap {
                    self.env[0] = 1.0;
                } else if self.age == 3 * gap {
                    self.env[0] = 1.0;
                    self.coef[0] = self.coef[1];
                }
                self.filter.tick(self.rng.bipolar(), self.g, self.k).1 * self.env[0]
            }
            Shape::Metal {
                noise,
                hit,
                rise_step,
            } => {
                let mut sum = 0.0;
                for i in 0..METAL_HZ.len() {
                    let inc = self.inc[i];
                    sum += pulse(self.turn(i, inc), inc, 0.5);
                }
                let raw = sum * (1.0 / 6.0) + self.rng.bipolar() * noise;
                let high = self.filter.tick(raw, self.g, self.k).2;
                self.env[2] = (self.env[2] + rise_step).min(1.0);
                high * self.env[2] * ((1.0 - hit) * self.env[0] + hit * self.env[1])
            }
        };
        self.env[0] *= self.coef[0];
        self.env[1] *= self.coef[1];
        self.env[2] *= match self.shape {
            Shape::Metal { .. } => 1.0,
            _ => self.coef[2],
        };
        self.age += 1;
        out
    }

    fn fade_out(&mut self, seconds: f32, sample_rate: f32) {
        let step = 1.0 / (seconds * sample_rate).max(1.0);
        if self.active && step > self.fade_step {
            self.fade_step = step;
        }
    }
}

/// Rounds the top off instead of cutting it: 1 at 1.5 and above.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let x = x.clamp(-1.5, 1.5);
    x - x * x * x * (4.0 / 27.0)
}

/// Loudness set by measurement (tests/synth_voices.rs prints the peaks): the kick at velocity 100
/// peaks near -12 dBFS and the others sit around it as they would in a mix. Pan: -1 left, 1 right.
fn level_and_pan(piece: DrumPiece) -> (f32, f32) {
    match piece {
        DrumPiece::Kick => (KICK_LEVEL, 0.0),
        DrumPiece::Snare => (0.21, 0.0),
        DrumPiece::SideStick => (0.119, 0.0),
        DrumPiece::Clap => (0.46, 0.0),
        DrumPiece::ClosedHiHat => (0.51, 0.25),
        DrumPiece::PedalHiHat => (0.45, 0.25),
        DrumPiece::OpenHiHat => (0.46, 0.25),
        DrumPiece::LowTom => (0.293, 0.3),
        DrumPiece::MidTom => (0.333, 0.0),
        DrumPiece::HighTom => (0.29, -0.3),
        DrumPiece::Crash => (0.276, -0.35),
        DrumPiece::Ride => (0.27, 0.35),
    }
}

const KICK_LEVEL: f32 = 0.337;

pub struct ElectroKit {
    voices: [Voice; VOICES],
    sample_rate: f32,
    hits: u64,
}

impl Default for ElectroKit {
    fn default() -> Self {
        Self::new()
    }
}

impl ElectroKit {
    /// Usable only after [`Instrument::prepare`].
    pub fn new() -> ElectroKit {
        let mut voices = [Voice::IDLE; VOICES];
        for (i, v) in voices.iter_mut().enumerate() {
            v.rng = Rng::new(0x6B69_7400 + i as u32);
        }
        ElectroKit {
            voices,
            sample_rate: 48_000.0,
            hits: 0,
        }
    }

    fn slots(&mut self, piece: DrumPiece) -> &mut [Voice] {
        let first = piece.row() * SLOTS;
        &mut self.voices[first..first + SLOTS]
    }

    fn hit(&mut self, piece: DrumPiece, velocity: u8, seed: Option<u32>) {
        let sample_rate = self.sample_rate;
        if piece.stops_open_hi_hat() {
            for v in self.slots(DrumPiece::OpenHiHat) {
                v.fade_out(CHOKE_S, sample_rate);
            }
        }
        self.hits += 1;
        let order = self.hits;
        // A cymbal hit again rings on under the new hit; a drum hit again starts over.
        let rings_on = matches!(
            piece,
            DrumPiece::OpenHiHat | DrumPiece::Crash | DrumPiece::Ride
        );
        let slots = self.slots(piece);
        if !rings_on {
            for v in slots.iter_mut() {
                v.fade_out(RETRIGGER_S, sample_rate);
            }
        }
        if let Some(v) = slots.iter_mut().find(|v| !v.active) {
            v.start(piece, (velocity, seed), order, sample_rate);
            return;
        }
        // Both voices are busy: the hit waits a moment for the one that is already fading, or
        // for the older.
        if let Some(v) = slots
            .iter_mut()
            .min_by_key(|v| (v.fade_step == 0.0, v.order))
        {
            v.fade_out(STEAL_S, sample_rate);
            v.pending = Some((velocity, seed));
            v.order = order;
        }
    }
}

impl Instrument for ElectroKit {
    fn prepare(&mut self, sample_rate: f32, _max_frames: usize) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    fn event(&mut self, ev: InstrumentEvent) {
        if let InstrumentEvent::NoteOn { note, velocity } = ev
            && velocity > 0
            && let Some(piece) = DrumPiece::from_note(note)
        {
            self.hit(piece, velocity, None);
        }
    }

    fn song_note_on(&mut self, note: u8, velocity: u8, place: SongPlace<'_>) {
        if velocity > 0
            && let Some(piece) = DrumPiece::from_note(note)
        {
            self.hit(piece, velocity, Some(place.seed()));
        }
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let frames = left.len().min(right.len());
        let (left, right) = (&mut left[..frames], &mut right[..frames]);
        left.fill(0.0);
        right.fill(0.0);
        let sample_rate = self.sample_rate;
        for (index, v) in self.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            let piece = DrumPiece::ROWS[index / SLOTS];
            for (out_l, out_r) in left.iter_mut().zip(right.iter_mut()) {
                if v.fade_step > 0.0 {
                    v.fade -= v.fade_step;
                    if v.fade <= 0.0 {
                        let Some(pending) = v.pending else {
                            v.active = false;
                            break;
                        };
                        v.start(piece, pending, v.order, sample_rate);
                    }
                }
                let s = v.tick() * v.fade;
                *out_l += s * v.gain[0];
                *out_r += s * v.gain[1];
                if v.env[0] + v.env[1] < SILENT {
                    v.active = v.pending.is_some();
                    if let Some(pending) = v.pending {
                        v.start(piece, pending, v.order, sample_rate);
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            *v = Voice {
                rng: v.rng,
                ..Voice::IDLE
            };
        }
    }
}
