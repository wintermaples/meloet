//! Plays a [`SampleBank`]: picks regions for keys and pedal, and mixes their voices.

use std::sync::Arc;

use meloet_engine::{Instrument, InstrumentEvent, SongPlace};

use super::bank::{Region, SampleBank, SampleData, Trigger};
use super::shape::Shaped;

/// Loudness, loop decay and bend are worked out once per this many frames and move in a
/// straight line in between.
pub(super) const CHUNK: usize = 32;
/// Fade of a voice that gives its place to a new note.
const STEAL_S: f32 = 0.003;
/// Fade of a voice stopped by its choke group (the length SFZ players use for `off_mode=fast`).
const CHOKE_S: f32 = 0.006;
/// A release shorter than this would click.
const MIN_RELEASE_S: f32 = 0.002;
/// -80 dB: where an envelope stops (docs/architecture.md 19-2).
const SILENT: f32 = 1.0e-4;
/// The release time is the time to fall this far.
const RELEASE_FLOOR: f32 = 1.0e-3;
const SMOOTH_S: f32 = 0.005;
const BEND_SEMITONES: f32 = 2.0;
const CC_SUSTAIN: u8 = 64;
const CC_VOLUME: u8 = 7;
const CC_PAN: u8 = 10;
const CC_EXPRESSION: u8 = 11;
/// What General MIDI has the volume controller at before anything moves it.
const DEFAULT_VOLUME: u8 = 100;
/// Volume, expression and pan reach a new value in about this time.
const MIX_SMOOTH_S: f32 = 0.02;
/// A voice with a shape is never further from its file's pitch than this.
const MAX_SHAPED_SEMITONES: f32 = 120.0;
/// The groups of release sounds that take turns which a press of the song keeps its turns
/// for; a key with more takes turns as they come in the others.
const RELEASE_GROUPS: usize = 4;
const NO_GROUP: u32 = u32::MAX;

/// For each group of release sounds that take turns (or [`NO_GROUP`]), the turn its place in
/// the song gives the release of a press.
type ReleaseTurns = [(u32, u32); RELEASE_GROUPS];

/// Where the regions that take turns take theirs from.
#[derive(Clone, Copy)]
enum Turn<'a> {
    /// The counters, which go on from wherever they are: what someone plays now.
    Counted,
    /// A press of the song: the earlier presses of its key in the region's velocities.
    Press(SongPlace<'a>),
    /// A move of the song's pedal: the earlier moves the same way.
    Nth(u32),
    /// The release of a press: what the press's place gave each group, counted for the others.
    Release(ReleaseTurns),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hold {
    /// The key is down.
    Key,
    /// The key is up and the pedal keeps the note.
    Pedal,
    /// Nothing holds the voice: it is in its release, or it never listens to its key.
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Attack,
    Full,
    Release,
}

#[derive(Clone, Copy)]
struct Voice {
    active: bool,
    region: u32,
    key: u8,
    hold: Hold,
    /// The note-on, note-off or pedal move that started the voice.
    event: u64,
    /// Start order, to find the oldest.
    born: u64,
    /// In frames of the file.
    pos: f64,
    step: f64,
    /// Share of the pitch bend the voice follows.
    follow: f32,
    gain_left: f32,
    gain_right: f32,
    stage: Stage,
    env: f32,
    attack_step: f32,
    release_coef: f32,
    /// What the loop's own decay has left.
    decay: f32,
    decay_coef: f32,
    /// 1, or on its way down to 0 when the voice was stolen or choked.
    fade: f32,
    fade_step: f32,
    /// Loudness at the end of the last chunk.
    amp: f32,
}

impl Voice {
    const FREE: Voice = Voice {
        active: false,
        region: 0,
        key: 0,
        hold: Hold::None,
        event: 0,
        born: 0,
        pos: 0.0,
        step: 1.0,
        follow: 0.0,
        gain_left: 0.0,
        gain_right: 0.0,
        stage: Stage::Full,
        env: 0.0,
        attack_step: 0.0,
        release_coef: 0.0,
        decay: 1.0,
        decay_coef: 1.0,
        fade: 1.0,
        fade_step: 0.0,
        amp: 0.0,
    };

    fn fading(&self) -> bool {
        self.fade_step > 0.0
    }

    fn fade_out(&mut self, step: f32) {
        if !self.fading() {
            self.fade_step = step;
        }
    }

    fn release(&mut self) {
        self.hold = Hold::None;
        self.stage = Stage::Release;
    }

    /// Moves loudness on by `frames`; false when the voice has ended.
    fn advance(&mut self, frames: usize, in_loop: bool) -> bool {
        let n = frames as f32;
        let mut alive = true;
        match self.stage {
            Stage::Attack => {
                self.env += self.attack_step * n;
                if self.env >= 1.0 {
                    self.env = 1.0;
                    self.stage = Stage::Full;
                }
            }
            Stage::Full => {}
            Stage::Release => {
                self.env *= self.release_coef.powi(frames as i32);
                alive &= self.env > SILENT;
            }
        }
        if in_loop && self.decay_coef < 1.0 {
            self.decay *= self.decay_coef.powi(frames as i32);
            alive &= self.decay > SILENT;
        }
        if self.fading() {
            self.fade -= self.fade_step * n;
            alive &= self.fade > 0.0;
        }
        self.amp = if alive {
            self.env * self.decay * self.fade
        } else {
            0.0
        };
        alive
    }
}

/// Catmull-Rom through four neighbouring frames, `t` in 0..1 between the middle two.
#[inline]
pub(super) fn interpolate(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (y2 - y0);
    let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
    ((c3 * t + c2) * t + c1) * t + y1
}

/// Adds one chunk of one voice to the buffers, loudness going from `amp` by `amp_step` a frame
/// (the pair).
/// Returns false when a file without a loop has run out.
fn mix(
    v: &mut Voice,
    sample: &SampleData,
    looped: Option<(usize, usize)>,
    step: f64,
    (mut amp, amp_step): (f32, f32),
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    let frames = left.len();
    let data = &sample.data[..];
    let stereo = sample.channels == 2;
    let limit = looped.map_or(sample.frames, |(_, end)| end);
    let first = v.pos as usize;
    let last = (v.pos + step * (frames - 1) as f64) as usize;
    let mut pos = v.pos;

    if first >= 1 && last + 3 < limit {
        // Nothing to look out for in this chunk: the four frames are always there (one frame to
        // spare, as adding the step up need not land where multiplying it did).
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let i = pos as usize;
            let t = (pos - i as f64) as f32;
            let (x_left, x_right) = if stereo {
                let w = &data[2 * (i - 1)..2 * (i + 3)];
                (
                    interpolate(w[0].into(), w[2].into(), w[4].into(), w[6].into(), t),
                    interpolate(w[1].into(), w[3].into(), w[5].into(), w[7].into(), t),
                )
            } else {
                let w = &data[i - 1..i + 3];
                let x = interpolate(w[0].into(), w[1].into(), w[2].into(), w[3].into(), t);
                (x, x)
            };
            *l += x_left * amp * v.gain_left;
            *r += x_right * amp * v.gain_right;
            amp += amp_step;
            pos += step;
        }
        v.pos = pos;
        return true;
    }

    // Near the start, the end or the loop point: frames past the loop's end are the frames after
    // its start, and outside the file is silence.
    let tap = |frame: isize, channel: usize| -> f32 {
        let mut frame = frame;
        if let Some((start, end)) = looped
            && frame >= end as isize
        {
            frame = start as isize + (frame - end as isize) % (end - start) as isize;
        }
        if frame < 0 || frame as usize >= sample.frames {
            0.0
        } else if stereo {
            data[2 * frame as usize + channel].into()
        } else {
            data[frame as usize].into()
        }
    };
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let i = pos as isize;
        let t = (pos - i as f64) as f32;
        let x_left = interpolate(tap(i - 1, 0), tap(i, 0), tap(i + 1, 0), tap(i + 2, 0), t);
        let x_right = if stereo {
            interpolate(tap(i - 1, 1), tap(i, 1), tap(i + 1, 1), tap(i + 2, 1), t)
        } else {
            x_left
        };
        *l += x_left * amp * v.gain_left;
        *r += x_right * amp * v.gain_right;
        amp += amp_step;
        pos += step;
        match looped {
            Some((start, end)) if pos >= end as f64 => {
                pos = start as f64 + (pos - end as f64) % (end - start) as f64;
            }
            None if pos >= sample.frames as f64 => {
                v.pos = pos;
                return false;
            }
            _ => {}
        }
    }
    v.pos = pos;
    true
}

/// Everything that changes while playing; the bank is only read.
struct State {
    sample_rate: f32,
    /// Twice the polyphony: a voice that gives its place fades in its own slot while the new
    /// note starts in another, so no note waits for a fade.
    voices: Vec<Voice>,
    /// The same places as `voices`, for the voices of regions with a shape; empty when the
    /// bank has none.
    shaped: Vec<Shaped>,
    polyphony: usize,
    sustain_pedal: bool,
    /// One counter for each set of regions that take turns.
    turns: Vec<u32>,
    /// The event that last moved each counter, so one key moves it once.
    turn_moved: Vec<u64>,
    key_down: [bool; 128],
    /// The key went up while the pedal was down; its release sound waits for the pedal.
    key_waiting: [bool; 128],
    release_turns: [ReleaseTurns; 128],
    key_velocity: [u8; 128],
    key_on_frame: [u64; 128],
    pedal: bool,
    bend_target: f32,
    bend: f32,
    /// The bend in semitones at each chunk of the block being rendered.
    bend_curve: Vec<f32>,
    /// Controllers 7 and 11, 0..=127. Only voices with a shape listen to them and to pan.
    volume_cc: u8,
    expression_cc: u8,
    volume_target: f32,
    volume: f32,
    /// -1..1 from controller 10.
    pan_target: f32,
    pan: f32,
    /// (volume, pan) at each chunk of the block being rendered.
    mix_curve: Vec<(f32, f32)>,
    frame: u64,
    events: u64,
    births: u64,
}

impl State {
    fn clear(&mut self) {
        self.voices.fill(Voice::FREE);
        self.turns.fill(0);
        self.turn_moved.fill(0);
        self.key_down = [false; 128];
        self.key_waiting = [false; 128];
        self.release_turns = [[(NO_GROUP, 0); RELEASE_GROUPS]; 128];
        self.pedal = false;
        self.bend_target = 0.0;
        self.bend = 0.0;
        self.volume_cc = DEFAULT_VOLUME;
        self.expression_cc = 127;
        self.set_volume();
        self.volume = self.volume_target;
        self.pan_target = 0.0;
        self.pan = 0.0;
    }

    /// Each controller is a loudness of (value / 127)^2, as the velocity is.
    fn set_volume(&mut self) {
        let share = f32::from(self.volume_cc) * f32::from(self.expression_cc) / (127.0 * 127.0);
        self.volume_target = share * share;
    }

    /// `held_s`: how long the key was down, for a release sound.
    fn trigger(
        &mut self,
        bank: &SampleBank,
        trigger: Trigger,
        key: u8,
        velocity: u8,
        held_s: f32,
        turn: Turn<'_>,
    ) {
        self.events += 1;
        let by_key = matches!(trigger, Trigger::Attack | Trigger::Release);
        let candidates = bank.candidates(trigger, key);
        let fits = |r: &Region| !by_key || (r.vel_lo..=r.vel_hi).contains(&velocity);
        for &index in candidates {
            let region = &bank.regions[index as usize];
            if !fits(region) {
                continue;
            }
            let its_turn = region.round_robin.as_ref().is_none_or(|rr| {
                let counted = self.turns[rr.group as usize];
                let turn = match turn {
                    Turn::Counted => counted,
                    Turn::Press(place) => place.earlier(region.vel_lo, region.vel_hi),
                    Turn::Nth(n) => n,
                    Turn::Release(turns) => (turns.iter())
                        .find(|&&(group, _)| group == rr.group)
                        .map_or(counted, |&(_, turn)| turn),
                };
                turn % rr.count == rr.index
            });
            if its_turn {
                // A pedal sound has no key: it plays as recorded.
                let (key, velocity) = if by_key {
                    (key, velocity)
                } else {
                    (region.root, 127)
                };
                self.start(bank, index, trigger, key, velocity, held_s);
            }
        }
        for &index in candidates {
            let region = &bank.regions[index as usize];
            if let Some(rr) = region.round_robin.as_ref().filter(|_| fits(region)) {
                let group = rr.group as usize;
                if self.turn_moved[group] != self.events {
                    self.turn_moved[group] = self.events;
                    self.turns[group] = self.turns[group].wrapping_add(1);
                }
            }
        }
    }

    fn start(
        &mut self,
        bank: &SampleBank,
        index: u32,
        trigger: Trigger,
        key: u8,
        velocity: u8,
        held_s: f32,
    ) {
        let region = &bank.regions[index as usize];
        let sample = &bank.samples[region.sample as usize];
        let event = self.events;

        if region.choke_group != 0 {
            let step = 1.0 / (CHOKE_S * self.sample_rate).max(1.0);
            for v in self.voices.iter_mut().filter(|v| v.active) {
                if v.event != event
                    && bank.regions[v.region as usize].choked_by & region.choke_group != 0
                {
                    v.fade_out(step);
                }
            }
        }

        let sounding = |v: &&mut Voice| v.active && !v.fading();
        if self.voices.iter_mut().filter(sounding).count() >= self.polyphony {
            // The oldest voice nobody holds goes first; if every voice is held, the oldest.
            let oldest = self
                .voices
                .iter_mut()
                .filter(sounding)
                .filter(|v| v.event != event)
                .min_by_key(|v| (v.hold != Hold::None, v.born));
            match oldest {
                Some(v) => v.fade_out(1.0 / (STEAL_S * self.sample_rate).max(1.0)),
                None => return,
            }
        }
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(free) => free,
            // More than a polyphony of notes inside one steal fade: take the fade nearest its end.
            None => {
                let nearest = self
                    .voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.fading())
                    .min_by(|a, b| a.1.fade.total_cmp(&b.1.fade));
                match nearest {
                    Some((slot, _)) => slot,
                    None => return,
                }
            }
        };

        let mut semitones =
            (f32::from(key) - f32::from(region.root)) * region.keytrack + region.tune_semitones;
        if region.shape.is_some() {
            semitones = semitones.clamp(-MAX_SHAPED_SEMITONES, MAX_SHAPED_SEMITONES);
        }
        let step = f64::from(semitones / 12.0).exp2() * f64::from(sample.rate)
            / f64::from(self.sample_rate);
        // SFZ's amp_veltrack: (velocity / 127)^2 at 100 %, no change at 0 %, turned round below 0.
        let curve = (f32::from(velocity) / 127.0).powi(2);
        let by_velocity = if region.veltrack >= 0.0 {
            1.0 - region.veltrack * (1.0 - curve)
        } else {
            -region.veltrack * (1.0 - curve)
        };
        // SFZ's rt_decay: a release sound is this much quieter for every second the key was down.
        let by_time = 10.0_f32.powf(-region.rt_decay_db_per_s * held_s / 20.0);
        let gain = region.gain * by_velocity * by_time;
        let free_running = trigger != Trigger::Attack || region.one_shot;
        let attack_frames = region.attack_s * self.sample_rate;
        let release_frames = region.release_s.max(MIN_RELEASE_S) * self.sample_rate;
        let env = if attack_frames >= 1.0 { 0.0 } else { 1.0 };
        self.births += 1;
        self.voices[slot] = Voice {
            active: true,
            region: index,
            key,
            hold: if free_running { Hold::None } else { Hold::Key },
            event,
            born: self.births,
            pos: 0.0,
            step,
            follow: region.keytrack,
            gain_left: gain * region.pan_left,
            gain_right: gain * region.pan_right,
            stage: if env == 0.0 {
                Stage::Attack
            } else {
                Stage::Full
            },
            env,
            attack_step: 1.0 / attack_frames.max(1.0),
            release_coef: (RELEASE_FLOOR.ln() / release_frames).exp(),
            decay: 1.0,
            decay_coef: 10.0_f32.powf(-region.loop_decay_db_per_s / 20.0 / self.sample_rate),
            fade: 1.0,
            fade_step: 0.0,
            amp: env,
        };
        if let Some(shape) = region.shape.as_deref() {
            self.voices[slot].follow = 1.0;
            self.shaped[slot] = Shaped::start(shape, key, self.sample_rate);
        }
    }

    fn note_on(&mut self, bank: &SampleBank, key: u8, velocity: u8, place: Option<SongPlace<'_>>) {
        let k = usize::from(key);
        self.key_down[k] = true;
        self.key_velocity[k] = velocity;
        self.key_on_frame[k] = self.frame;
        let mut release = [(NO_GROUP, 0); RELEASE_GROUPS];
        if let Some(place) = place {
            let mut kept = 0;
            for &index in bank.candidates(Trigger::Release, key) {
                let region = &bank.regions[index as usize];
                let Some(rr) = region.round_robin.as_ref() else {
                    continue;
                };
                let fits = (region.vel_lo..=region.vel_hi).contains(&velocity);
                if fits && kept < RELEASE_GROUPS && release[..kept].iter().all(|g| g.0 != rr.group)
                {
                    release[kept] = (rr.group, place.earlier(region.vel_lo, region.vel_hi));
                    kept += 1;
                }
            }
        }
        self.release_turns[k] = release;
        let turn = place.map_or(Turn::Counted, Turn::Press);
        self.trigger(bank, Trigger::Attack, key, velocity, 0.0, turn);
    }

    fn note_off(&mut self, bank: &SampleBank, key: u8) {
        let k = usize::from(key);
        if !self.key_down[k] {
            return;
        }
        self.key_down[k] = false;
        let kept = self.pedal && self.sustain_pedal;
        for v in self.voices.iter_mut().filter(|v| v.active && v.key == key) {
            if v.hold == Hold::Key {
                if kept {
                    v.hold = Hold::Pedal;
                } else {
                    v.release();
                }
            }
        }
        if kept {
            self.key_waiting[k] = true;
        } else {
            self.release_sound(bank, key);
        }
    }

    /// The sound of a key coming up (hammer, damper), quieter the longer the note has rung.
    fn release_sound(&mut self, bank: &SampleBank, key: u8) {
        let k = usize::from(key);
        let held_s = (self.frame - self.key_on_frame[k]) as f32 / self.sample_rate;
        self.trigger(
            bank,
            Trigger::Release,
            key,
            self.key_velocity[k],
            held_s,
            Turn::Release(self.release_turns[k]),
        );
    }

    /// `turn`: `None` moves the pedal without the sound of it moving.
    fn set_pedal(&mut self, bank: &SampleBank, down: bool, turn: Option<Turn<'_>>) {
        if down == self.pedal {
            return;
        }
        self.pedal = down;
        if down {
            if let Some(turn) = turn {
                self.trigger(bank, Trigger::PedalDown, 0, 127, 0.0, turn);
            }
            return;
        }
        for v in self.voices.iter_mut().filter(|v| v.active) {
            if v.hold == Hold::Pedal {
                v.release();
            }
        }
        for key in 0..128_u8 {
            let k = usize::from(key);
            if self.key_waiting[k] {
                self.key_waiting[k] = false;
                if !self.key_down[k] {
                    self.release_sound(bank, key);
                }
            }
        }
        if let Some(turn) = turn {
            self.trigger(bank, Trigger::PedalUp, 0, 127, 0.0, turn);
        }
    }

    fn release_all(&mut self) {
        for v in self.voices.iter_mut().filter(|v| v.active) {
            if v.hold != Hold::None {
                v.release();
            }
        }
        self.key_down = [false; 128];
        self.key_waiting = [false; 128];
    }

    fn render(&mut self, bank: &SampleBank, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
        let frames = left.len();
        let chunks = frames.div_ceil(CHUNK).min(self.bend_curve.len());
        let share = 1.0 - (-(CHUNK as f32) / (SMOOTH_S * self.sample_rate)).exp();
        for bend in &mut self.bend_curve[..chunks] {
            self.bend += (self.bend_target - self.bend) * share;
            if (self.bend_target - self.bend).abs() < 1.0e-4 {
                self.bend = self.bend_target;
            }
            *bend = self.bend;
        }

        if bank.shaped {
            let share = 1.0 - (-(CHUNK as f32) / (MIX_SMOOTH_S * self.sample_rate)).exp();
            for mix in &mut self.mix_curve[..chunks] {
                self.volume += (self.volume_target - self.volume) * share;
                if (self.volume_target - self.volume).abs() < 1.0e-5 {
                    self.volume = self.volume_target;
                }
                self.pan += (self.pan_target - self.pan) * share;
                if (self.pan_target - self.pan).abs() < 1.0e-4 {
                    self.pan = self.pan_target;
                }
                *mix = (self.volume, self.pan);
            }
        }

        for (slot, v) in self.voices.iter_mut().enumerate().filter(|(_, v)| v.active) {
            let region = &bank.regions[v.region as usize];
            let sample = &bank.samples[region.sample as usize];
            let chunked = left.chunks_mut(CHUNK).zip(right.chunks_mut(CHUNK));
            if let Some(shape) = region.shape.as_deref() {
                let x = &mut self.shaped[slot];
                let curves = self.bend_curve[..chunks]
                    .iter()
                    .zip(&self.mix_curve[..chunks]);
                for ((l, r), (&bend, &(volume, pan))) in chunked.zip(curves) {
                    if v.stage == Stage::Release && !x.released {
                        x.release();
                    }
                    let moved = x.advance(shape, l.len());
                    let mut alive = moved.alive;
                    if v.fading() {
                        v.fade -= v.fade_step * l.len() as f32;
                        alive &= v.fade > 0.0;
                    }
                    let cents = bend * 100.0 + moved.pitch_cents;
                    let step = if cents == 0.0 {
                        v.step
                    } else {
                        v.step * f64::from(cents / 1200.0).exp2()
                    };
                    x.set_pan((shape.pan + pan).clamp(-1.0, 1.0));
                    // With a shape `gain_left` is the gain of both sides; the shape has the pan.
                    let amp = if alive {
                        moved.level * v.fade * volume * v.gain_left
                    } else {
                        0.0
                    };
                    let to = (amp * x.pan_left, amp * x.pan_right);
                    if x.fresh {
                        x.fresh = false;
                        if x.starts_full() {
                            (x.amp_left, x.amp_right) = to;
                        }
                    }
                    let looped = region
                        .looped
                        .filter(|_| !(shape.loop_until_release && x.released));
                    let rate = self.sample_rate;
                    let more = x.mix(shape, sample, looped, &mut v.pos, step, to, rate, l, r);
                    if !(alive && more) {
                        v.active = false;
                        break;
                    }
                }
                continue;
            }
            for ((l, r), &bend) in chunked.zip(&self.bend_curve[..chunks]) {
                let step = if bend == 0.0 || v.follow == 0.0 {
                    v.step
                } else {
                    v.step * f64::from(bend * v.follow / 12.0).exp2()
                };
                let from = v.amp;
                let in_loop = region
                    .looped
                    .is_some_and(|(start, _)| v.pos >= start as f64);
                let alive = v.advance(l.len(), in_loop);
                let amp_step = (v.amp - from) / l.len() as f32;
                let more = mix(v, sample, region.looped, step, (from, amp_step), l, r);
                if !(alive && more) {
                    v.active = false;
                    break;
                }
            }
        }
        self.frame += frames as u64;
    }
}

/// A sampled instrument on one track.
pub struct Sampler {
    bank: Arc<SampleBank>,
    state: State,
}

impl Sampler {
    pub fn new(bank: Arc<SampleBank>) -> Sampler {
        let state = State {
            sample_rate: 48_000.0,
            voices: vec![Voice::FREE; bank.polyphony() * 2],
            shaped: if bank.shaped {
                vec![Shaped::IDLE; bank.polyphony() * 2]
            } else {
                Vec::new()
            },
            polyphony: bank.polyphony(),
            sustain_pedal: bank.sustain_pedal(),
            turns: vec![0; bank.round_robin_groups],
            turn_moved: vec![0; bank.round_robin_groups],
            key_down: [false; 128],
            key_waiting: [false; 128],
            release_turns: [[(NO_GROUP, 0); RELEASE_GROUPS]; 128],
            key_velocity: [0; 128],
            key_on_frame: [0; 128],
            pedal: false,
            bend_target: 0.0,
            bend: 0.0,
            bend_curve: vec![0.0; 1],
            volume_cc: DEFAULT_VOLUME,
            expression_cc: 127,
            volume_target: 1.0,
            volume: 1.0,
            pan_target: 0.0,
            pan: 0.0,
            mix_curve: vec![(1.0, 0.0); 1],
            frame: 0,
            events: 0,
            births: 0,
        };
        Sampler { bank, state }
    }

    pub fn bank(&self) -> &Arc<SampleBank> {
        &self.bank
    }

    /// Voices making sound now, the ones fading out for a newer note included.
    pub fn active_voices(&self) -> usize {
        self.state.voices.iter().filter(|v| v.active).count()
    }
}

impl Instrument for Sampler {
    fn prepare(&mut self, sample_rate: f32, max_frames: usize) {
        self.state.sample_rate = sample_rate;
        self.state.bend_curve = vec![0.0; max_frames.div_ceil(CHUNK).max(1)];
        self.state.mix_curve = vec![(1.0, 0.0); self.state.bend_curve.len()];
        self.state.clear();
    }

    fn event(&mut self, ev: InstrumentEvent) {
        let (bank, state) = (&*self.bank, &mut self.state);
        match ev {
            InstrumentEvent::NoteOn { note, velocity: 0 } => state.note_off(bank, note & 127),
            InstrumentEvent::NoteOn { note, velocity } => {
                state.note_on(bank, note & 127, velocity.min(127), None);
            }
            InstrumentEvent::NoteOff { note } => state.note_off(bank, note & 127),
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } => state.set_pedal(bank, value >= 64, Some(Turn::Counted)),
            InstrumentEvent::Control { controller, value } => {
                let value = value.min(127);
                match controller {
                    CC_VOLUME => state.volume_cc = value,
                    CC_EXPRESSION => state.expression_cc = value,
                    CC_PAN => state.pan_target = ((f32::from(value) - 64.0) / 63.0).max(-1.0),
                    _ => {}
                }
                state.set_volume();
            }
            InstrumentEvent::PitchBend { value } => {
                let value = f32::from(value.clamp(-8192, 8191));
                state.bend_target = value / 8192.0 * BEND_SEMITONES;
            }
            InstrumentEvent::AllNotesOff => state.release_all(),
        }
    }

    fn song_note_on(&mut self, note: u8, velocity: u8, place: SongPlace<'_>) {
        let velocity = velocity.min(127);
        if velocity > 0 {
            (self.state).note_on(&self.bank, note & 127, velocity, Some(place));
        }
    }

    fn song_pedal(&mut self, value: u8, place: SongPlace<'_>) {
        let turn = (!place.chased()).then_some(Turn::Nth(place.earlier_moves()));
        (self.state).set_pedal(&self.bank, value >= 64, turn);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.state.render(&self.bank, left, right);
    }

    fn reset(&mut self) {
        self.state.clear();
    }
}
