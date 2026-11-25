//! What the song plays: per-track event lists ([`Sequence`]), the per-track cursor that walks
//! them on the audio thread ([`Lane`]), and the metronome click.

use std::f32::consts::TAU;

use crate::instrument::{Instrument, InstrumentEvent, SongPlace};

const CC_MODULATION: u8 = 1;
const CC_SUSTAIN: u8 = 64;
/// The sustain pedal is down from this value on, as instruments read it.
const PEDAL_DOWN: u8 = 64;
/// Stands for the pedal in the seeds, beside the keys 0..=127.
const PEDAL_KEY: u8 = 128;

const MOVED_SUSTAIN: u8 = 1;
const MOVED_MODULATION: u8 = 2;
const MOVED_BEND: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SequenceEvent {
    /// Song position in ticks ([`meloet_model::TICKS_PER_QUARTER`]).
    pub tick: u64,
    pub event: InstrumentEvent,
}

fn rank(event: InstrumentEvent) -> u8 {
    match event {
        InstrumentEvent::NoteOff { .. } | InstrumentEvent::NoteOn { velocity: 0, .. } => 0,
        InstrumentEvent::NoteOn { .. } => 2,
        _ => 1,
    }
}

/// The presses of a key between two tallies.
const TALLY_EVERY: usize = 64;

/// One track's events in song order. Built outside the audio thread.
#[derive(Debug, Default)]
pub struct Sequence {
    events: Box<[SequenceEvent]>,
    /// For each event; for a press, its seed and its number among the presses of its key; for
    /// a move of the sustain pedal, its seed and the earlier moves the same way.
    presses: Box<[(u32, u32)]>,
    /// The velocities of the presses, key after key, each key's in song order.
    velocities: Box<[u8]>,
    /// For each key, from its [`TALLY_EVERY`]th press on and every [`TALLY_EVERY`] presses
    /// after: the cumulative counts by velocity of its presses before that one.
    tallies: Box<[[u32; 128]]>,
    /// For each key: where its velocities and its tallies start (empty: no press at all).
    keys: Box<[(u32, u32)]>,
}

impl Sequence {
    /// Sorts by position and, at the same position, releases before controls before presses.
    /// `AllNotesOff` is dropped: the engine itself lets go of what a sequence leaves sounding.
    pub fn new(mut events: Vec<SequenceEvent>) -> Sequence {
        events.retain(|e| e.event != InstrumentEvent::AllNotesOff);
        events.sort_by_key(|e| (e.tick, rank(e.event)));
        let press = |e: &SequenceEvent| match e.event {
            InstrumentEvent::NoteOn { note, velocity } if velocity > 0 => {
                Some((usize::from(note & 127), velocity.min(127)))
            }
            _ => None,
        };

        let mut count = [0usize; 128];
        for (key, _) in events.iter().filter_map(press) {
            count[key] += 1;
        }
        let mut keys = vec![(0u32, 0u32); 128];
        let (mut velocity_at, mut tally_at) = (0usize, 0usize);
        for (key, n) in count.iter().enumerate() {
            keys[key] = (velocity_at as u32, tally_at as u32);
            velocity_at += n;
            tally_at += n.saturating_sub(1) / TALLY_EVERY;
        }

        let mut velocities = vec![0u8; velocity_at];
        let mut tallies = vec![[0u32; 128]; tally_at];
        let mut presses = vec![(0u32, 0u32); events.len()];
        let mut seen = [0usize; 128];
        let mut by_velocity = vec![[0u32; 128]; 128];
        let mut same_tick = [(u64::MAX, 0u32); 128];
        let (mut pedal_down, mut pedal_moves) = (false, [0u32; 2]);
        for (i, e) in events.iter().enumerate() {
            if let InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } = e.event
            {
                let down = value >= PEDAL_DOWN;
                let way = usize::from(down);
                presses[i] = (seed(e.tick, PEDAL_KEY, 0), pedal_moves[way]);
                if down != pedal_down {
                    pedal_down = down;
                    pedal_moves[way] += 1;
                }
                continue;
            }
            let Some((key, velocity)) = press(e) else {
                continue;
            };
            let ordinal = seen[key];
            seen[key] += 1;
            let (first_velocity, first_tally) = keys[key];
            if ordinal > 0 && ordinal % TALLY_EVERY == 0 {
                let tally = &mut tallies[first_tally as usize + ordinal / TALLY_EVERY - 1];
                let mut sum = 0;
                for (t, n) in tally.iter_mut().zip(by_velocity[key]) {
                    sum += n;
                    *t = sum;
                }
            }
            by_velocity[key][usize::from(velocity)] += 1;
            velocities[first_velocity as usize + ordinal] = velocity;
            let (tick, n) = &mut same_tick[key];
            if *tick == e.tick {
                *n += 1;
            } else {
                (*tick, *n) = (e.tick, 0);
            }
            presses[i] = (seed(e.tick, key as u8, *n), ordinal as u32);
        }
        Sequence {
            events: events.into_boxed_slice(),
            presses: presses.into_boxed_slice(),
            velocities: velocities.into_boxed_slice(),
            tallies: tallies.into_boxed_slice(),
            keys: keys.into_boxed_slice(),
        }
    }

    pub fn events(&self) -> &[SequenceEvent] {
        &self.events
    }

    /// The place of the event at `index`, a press of `note`.
    fn place(&self, index: usize, note: u8) -> SongPlace<'_> {
        let (seed, ordinal) = self.presses[index];
        let (first_velocity, first_tally) = self.keys[usize::from(note & 127)];
        let (first_velocity, ordinal) = (first_velocity as usize, ordinal as usize);
        let block = ordinal / TALLY_EVERY;
        let tally = (block > 0).then(|| &self.tallies[first_tally as usize + block - 1]);
        let earlier =
            &self.velocities[first_velocity + block * TALLY_EVERY..first_velocity + ordinal];
        SongPlace::tallied(seed, tally, earlier)
    }

    /// The place of the event at `index`, a move of the sustain pedal.
    fn pedal_place(&self, index: usize, chased: bool) -> SongPlace<'static> {
        let (seed, moves) = self.presses[index];
        SongPlace::pedal(seed, moves, chased)
    }
}

/// The same for the same tick, key and order there; far apart for neighbours.
fn seed(tick: u64, key: u8, order: u32) -> u32 {
    let mut x = tick
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(key) << 32 | u64::from(order));
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)) as u32
}

/// A set of track indexes (the tracks that do not play their sequence).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrackSet {
    bits: [u64; 4],
}

impl TrackSet {
    /// Tracks 0 to 255; others are ignored.
    pub const CAPACITY: usize = 256;

    pub fn insert(&mut self, track: usize) {
        if track < Self::CAPACITY {
            self.bits[track / 64] |= 1 << (track % 64);
        }
    }

    pub fn remove(&mut self, track: usize) {
        if track < Self::CAPACITY {
            self.bits[track / 64] &= !(1 << (track % 64));
        }
    }

    pub fn contains(&self, track: usize) -> bool {
        track < Self::CAPACITY && self.bits[track / 64] & 1 << (track % 64) != 0
    }
}

impl FromIterator<usize> for TrackSet {
    fn from_iter<I: IntoIterator<Item = usize>>(iter: I) -> Self {
        let mut set = TrackSet::default();
        iter.into_iter().for_each(|t| set.insert(t));
        set
    }
}

/// A track's place in its sequence and what the sequence has left switched on.
#[derive(Default)]
pub(crate) struct Lane {
    sequence: Option<Box<Sequence>>,
    /// Counts the sequences that were put here.
    version: u32,
    cursor: usize,
    /// Notes the sequence pressed and has not released, one bit per key.
    sounding: [u64; 2],
    moved: u8,
}

impl Lane {
    fn send(
        &mut self,
        event: InstrumentEvent,
        instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        match event {
            InstrumentEvent::NoteOn { note, velocity } if velocity > 0 => {
                self.sounding[usize::from(note >> 6) & 1] |= 1 << (note & 63);
            }
            InstrumentEvent::NoteOn { note, .. } | InstrumentEvent::NoteOff { note } => {
                self.sounding[usize::from(note >> 6) & 1] &= !(1 << (note & 63));
            }
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                ..
            } => self.moved |= MOVED_SUSTAIN,
            InstrumentEvent::Control {
                controller: CC_MODULATION,
                ..
            } => self.moved |= MOVED_MODULATION,
            InstrumentEvent::PitchBend { .. } => self.moved |= MOVED_BEND,
            _ => {}
        }
        if let Some(instrument) = instrument {
            instrument.event(event);
        }
    }

    /// Releases the notes the sequence holds and puts back to 0 the pedal, wheel and bend it
    /// moved. Keyboard notes on the same instrument are left alone.
    pub fn silence(&mut self, instrument: Option<&mut (dyn Instrument + 'static)>) {
        self.silence_with(instrument, true);
    }

    /// The same at the loop's end, going back to its start: a pedal the song holds is let go
    /// with no sound of a pedal let go (playing on, the pedal would not move there).
    pub fn silence_at_loop(&mut self, instrument: Option<&mut (dyn Instrument + 'static)>) {
        self.silence_with(instrument, false);
    }

    fn silence_with(
        &mut self,
        mut instrument: Option<&mut (dyn Instrument + 'static)>,
        pedal_sound: bool,
    ) {
        for (half, bits) in self.sounding.iter_mut().enumerate() {
            while *bits != 0 {
                let low = bits.trailing_zeros();
                *bits &= *bits - 1;
                if let Some(i) = instrument.as_deref_mut() {
                    i.event(InstrumentEvent::NoteOff {
                        note: (half as u32 * 64 + low) as u8,
                    });
                }
            }
        }
        if let Some(i) = instrument {
            if self.moved & MOVED_SUSTAIN != 0 {
                if pedal_sound {
                    i.event(InstrumentEvent::Control {
                        controller: CC_SUSTAIN,
                        value: 0,
                    });
                } else {
                    i.song_pedal(0, SongPlace::pedal(0, 0, true));
                }
            }
            if self.moved & MOVED_MODULATION != 0 {
                i.event(InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value: 0,
                });
            }
            if self.moved & MOVED_BEND != 0 {
                i.event(InstrumentEvent::PitchBend { value: 0 });
            }
        }
        self.moved = 0;
    }

    /// Forgets what is sounding without sending anything (the instrument was replaced or reset).
    pub fn forget(&mut self) {
        self.sounding = [0; 2];
        self.moved = 0;
    }

    /// Puts the cursor at the first event that is not `before` the position (`before` is given
    /// an event's tick, and holds for the events up to some tick only) and applies the last
    /// pedal, wheel and bend before it.
    pub fn seek(
        &mut self,
        before: impl Fn(u64) -> bool,
        instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        let Some(sequence) = self.sequence.as_deref() else {
            self.cursor = 0;
            return;
        };
        let events = sequence.events();
        let cursor = events.partition_point(|e| before(e.tick));
        self.cursor = cursor;
        let (mut sustain, mut modulation, mut bend) = (None, None, None);
        for (index, e) in events[..cursor].iter().enumerate().rev() {
            match e.event {
                InstrumentEvent::Control {
                    controller: CC_SUSTAIN,
                    value,
                } if sustain.is_none() => sustain = Some((value, index)),
                InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value,
                } if modulation.is_none() => modulation = Some(value),
                InstrumentEvent::PitchBend { value } if bend.is_none() => bend = Some(value),
                _ => {}
            }
            if sustain.is_some() && modulation.is_some() && bend.is_some() {
                break;
            }
        }
        let mut instrument = instrument;
        if let Some((value, index)) = sustain.filter(|&(v, _)| v != 0) {
            self.moved |= MOVED_SUSTAIN;
            if let Some(i) = instrument.as_deref_mut() {
                i.song_pedal(value, sequence.pedal_place(index, true));
            }
        }
        let chased = [
            modulation
                .filter(|&v| v != 0)
                .map(|value| InstrumentEvent::Control {
                    controller: CC_MODULATION,
                    value,
                }),
            bend.filter(|&v| v != 0)
                .map(|value| InstrumentEvent::PitchBend { value }),
        ];
        for event in chased.into_iter().flatten() {
            self.send(event, instrument.as_deref_mut());
        }
    }

    /// Sends the next event not yet sent. A press goes with its place in the song.
    pub fn fire_next(&mut self, instrument: Option<&mut (dyn Instrument + 'static)>) {
        let Some(sequence) = self.sequence.as_deref() else {
            return;
        };
        let Some(&e) = sequence.events().get(self.cursor) else {
            return;
        };
        match e.event {
            InstrumentEvent::NoteOn { note, velocity } if velocity > 0 => {
                self.sounding[usize::from(note >> 6) & 1] |= 1 << (note & 63);
                if let Some(instrument) = instrument {
                    instrument.song_note_on(note, velocity, sequence.place(self.cursor, note));
                }
            }
            InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } => {
                self.moved |= MOVED_SUSTAIN;
                if let Some(instrument) = instrument {
                    instrument.song_pedal(value, sequence.pedal_place(self.cursor, false));
                }
            }
            event => self.send(event, instrument),
        }
        self.cursor += 1;
    }

    /// Sends every event before `tick` that has not been sent yet.
    pub fn fire_before(
        &mut self,
        tick: u64,
        mut instrument: Option<&mut (dyn Instrument + 'static)>,
    ) {
        while self.next_tick().is_some_and(|next| next < tick) {
            self.fire_next(instrument.as_deref_mut());
        }
    }

    /// Position of the next event not yet sent, in ticks.
    pub fn next_tick(&self) -> Option<u64> {
        self.sequence
            .as_deref()
            .and_then(|s| s.events().get(self.cursor))
            .map(|e| e.tick)
    }

    /// Changes whenever [`Lane::next_tick`] may give another answer.
    pub fn place(&self) -> (usize, u32) {
        (self.cursor, self.version)
    }

    /// The sequence that was here comes back.
    pub fn replace_sequence(&mut self, sequence: Option<Box<Sequence>>) -> Option<Box<Sequence>> {
        self.version = self.version.wrapping_add(1);
        std::mem::replace(&mut self.sequence, sequence)
    }
}

/// What the metronome's clicks sound like (`EngineCommand::SetClick`). Each is made by
/// calculation, of cosines that all start at their top on the beat's frame and only die away,
/// so the first sample of a click is its loudest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClickSound {
    /// A short tone: the click as it always was.
    #[default]
    Beep,
    /// A bright, very short knock.
    Tick,
    /// Two square-like tones at 540 and 800 Hz, after the drum machines' cowbell.
    Cowbell,
}

/// One cosine of a click: its pitch against the click's, its share of the first sample, and
/// how fast it dies away.
struct Partial {
    ratio: f32,
    share: f32,
    decay_s: f32,
}

/// The beat's pitch, the bar head's (higher), and the cosines.
struct Recipe {
    beat_hz: f32,
    bar_hz: f32,
    partials: &'static [Partial],
}

const fn partial(ratio: f32, share: f32, decay_s: f32) -> Partial {
    Partial {
        ratio,
        share,
        decay_s,
    }
}

impl ClickSound {
    fn recipe(self) -> &'static Recipe {
        const BEEP: Recipe = Recipe {
            beat_hz: 1046.5,
            bar_hz: 1760.0,
            partials: &[partial(1.0, 1.0, 0.008)],
        };
        const TICK: Recipe = Recipe {
            beat_hz: 2500.0,
            bar_hz: 3150.0,
            partials: &[
                partial(1.0, 0.55, 0.004),
                partial(1.59, 0.3, 0.0025),
                partial(2.31, 0.15, 0.0015),
            ],
        };
        // 540 Hz and 800 Hz with their third and fifth harmonics; the bar head a fourth up.
        const COWBELL: Recipe = Recipe {
            beat_hz: 540.0,
            bar_hz: 720.0,
            partials: &[
                partial(1.0, 0.25, 0.010),
                partial(800.0 / 540.0, 0.35, 0.010),
                partial(3.0, 0.12, 0.006),
                partial(3.0 * 800.0 / 540.0, 0.16, 0.006),
                partial(5.0, 0.05, 0.004),
                partial(5.0 * 800.0 / 540.0, 0.07, 0.004),
            ],
        };
        match self {
            ClickSound::Beep => &BEEP,
            ClickSound::Tick => &TICK,
            ClickSound::Cowbell => &COWBELL,
        }
    }
}

#[derive(Clone, Copy)]
struct Tone {
    inc: f32,
    phase: f32,
    level: f32,
    decay: f32,
}

const MAX_PARTIALS: usize = 6;
/// Clicks that wait for their first frame: those of 16 beats while the master's inserts
/// delay the song.
const WAITING: usize = 16;

/// A click that starts later.
#[derive(Clone, Copy)]
struct Waiting {
    /// Frames until its first frame.
    frames: usize,
    bar_head: bool,
    sample_rate: f32,
    sound: ClickSound,
    gain: f32,
}

/// A short decaying sound, started exactly on a beat. One at a time: a new click ends the
/// one before.
pub(crate) struct Click {
    frame: usize,
    total: usize,
    tones: [Tone; MAX_PARTIALS],
    used: usize,
    waiting: [Option<Waiting>; WAITING],
}

impl Click {
    const LENGTH_S: f32 = 0.035;
    /// The first sample at 0 dB. The test tone peaks around 0.2 to 0.3; the click stays a
    /// little under.
    const BAR_LEVEL: f32 = 0.17;
    const BEAT_LEVEL: f32 = 0.13;
    /// The loudest a click may be made: +6 dB.
    const MAX_GAIN: f32 = 2.0;
    const SILENT: Tone = Tone {
        inc: 0.0,
        phase: 0.0,
        level: 0.0,
        decay: 0.0,
    };

    pub fn idle() -> Self {
        Click {
            frame: 0,
            total: 0,
            tones: [Self::SILENT; MAX_PARTIALS],
            used: 0,
            waiting: [None; WAITING],
        }
    }

    /// [`Click::start`] `delay` frames after the next frame [`Click::add`] is given. More
    /// than [`WAITING`] clicks waiting at once: the newest is dropped.
    pub fn start_after(
        &mut self,
        delay: usize,
        bar_head: bool,
        sample_rate: f32,
        sound: ClickSound,
        gain: f32,
    ) {
        if delay == 0 {
            self.start(bar_head, sample_rate, sound, gain);
        } else if let Some(free) = self.waiting.iter_mut().find(|w| w.is_none()) {
            *free = Some(Waiting {
                frames: delay,
                bar_head,
                sample_rate,
                sound,
                gain,
            });
        }
    }

    /// The clicks that wait are not heard.
    pub fn drop_waiting(&mut self) {
        self.waiting = [None; WAITING];
    }

    /// The clicks that wait start `by` frames later (earlier when below 0), as the song does
    /// when the master's inserts delay it by another amount; one whose start would be past
    /// is not heard, as the song there is not.
    pub fn delay_waiting(&mut self, by: i64) {
        for slot in self.waiting.iter_mut() {
            *slot = slot.and_then(|w| {
                let frames = usize::try_from(w.frames as i64 + by).ok()?;
                Some(Waiting { frames, ..w })
            });
        }
    }

    /// `gain` scales the whole click (1.0: as made). The bar head is higher and louder than
    /// the other beats with every sound.
    fn start(&mut self, bar_head: bool, sample_rate: f32, sound: ClickSound, gain: f32) {
        let recipe = sound.recipe();
        let (hz, level) = if bar_head {
            (recipe.bar_hz, Self::BAR_LEVEL)
        } else {
            (recipe.beat_hz, Self::BEAT_LEVEL)
        };
        let gain = if gain.is_finite() {
            gain.clamp(0.0, Self::MAX_GAIN)
        } else {
            1.0
        };
        let mut tones = [Self::SILENT; MAX_PARTIALS];
        for (tone, p) in tones.iter_mut().zip(recipe.partials) {
            *tone = Tone {
                inc: hz * p.ratio / sample_rate,
                phase: 0.0,
                level: level * gain * p.share,
                decay: (-1.0 / (p.decay_s * sample_rate)).exp(),
            };
        }
        self.frame = 0;
        self.total = (Self::LENGTH_S * sample_rate) as usize;
        self.tones = tones;
        self.used = recipe.partials.len().min(MAX_PARTIALS);
    }

    /// Adds to `out` (the same on the left and the right). The first frame after `start` is
    /// the loudest; a click that waits starts on its frame.
    pub fn add(&mut self, mut out: &mut [f32]) {
        loop {
            let next = (self.waiting.iter().enumerate())
                .filter_map(|(i, w)| w.map(|w| (i, w.frames)))
                .min_by_key(|&(_, frames)| frames)
                .filter(|&(_, frames)| frames < out.len());
            let Some((i, frames)) = next else {
                break;
            };
            let (before, rest) = std::mem::take(&mut out).split_at_mut(frames);
            self.sound(before);
            self.pass(frames);
            if let Some(w) = self.waiting[i].take() {
                self.start(w.bar_head, w.sample_rate, w.sound, w.gain);
            }
            out = rest;
        }
        self.pass(out.len());
        self.sound(out);
    }

    fn pass(&mut self, frames: usize) {
        for w in self.waiting.iter_mut().flatten() {
            w.frames -= frames;
        }
    }

    fn sound(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            if self.frame >= self.total {
                return;
            }
            let fade = ((self.total - self.frame) as f32 / 48.0).min(1.0);
            let mut sum = 0.0;
            for tone in &mut self.tones[..self.used] {
                sum += tone.level * (TAU * tone.phase).cos();
                tone.phase = (tone.phase + tone.inc).fract();
                tone.level *= tone.decay;
            }
            *o += sum * fade;
            self.frame += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(tick: u64, note: u8, velocity: u8) -> SequenceEvent {
        SequenceEvent {
            tick,
            event: InstrumentEvent::NoteOn { note, velocity },
        }
    }

    /// Two keys, hundreds of presses each at velocities that wander, some at the same tick,
    /// with releases and a pedal in between: every press is told the count a walk from the
    /// head gives, for any range of velocities.
    #[test]
    fn a_press_knows_the_earlier_presses_of_its_key_by_velocity() {
        let mut state = 12_345_u32;
        let mut next = |below: u32| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) % below
        };
        let mut events = Vec::new();
        for i in 0..700_u64 {
            let note = if next(3) == 0 { 38 } else { 42 };
            let tick = i / 2 * 240;
            events.push(on(tick, note, 1 + next(127) as u8));
            events.push(SequenceEvent {
                tick: tick + 100,
                event: InstrumentEvent::NoteOff { note },
            });
            if i % 50 == 0 {
                events.push(on(tick + 120, note, 0));
                events.push(SequenceEvent {
                    tick,
                    event: InstrumentEvent::Control {
                        controller: CC_SUSTAIN,
                        value: 127,
                    },
                });
            }
        }
        let sequence = Sequence::new(events);
        let ranges = [
            (1, 127),
            (1, 63),
            (64, 127),
            (30, 30),
            (100, 90),
            (0, 0),
            (126, 127),
        ];
        let mut heard: Vec<(u8, u8, u64)> = Vec::new();
        let mut seeds = std::collections::HashSet::new();
        for (index, e) in sequence.events().iter().enumerate() {
            let InstrumentEvent::NoteOn { note, velocity } = e.event else {
                continue;
            };
            if velocity == 0 {
                continue;
            }
            let place = sequence.place(index, note);
            for (lo, hi) in ranges {
                let walked = (heard.iter())
                    .filter(|&&(n, v, _)| n == note && lo.max(1) <= v && v <= hi)
                    .count() as u32;
                assert_eq!(place.earlier(lo, hi), walked, "press {index}, {lo}..={hi}");
            }
            let order = heard
                .iter()
                .filter(|h| h.0 == note && h.2 == e.tick)
                .count();
            assert_eq!(place.seed(), seed(e.tick, note, order as u32));
            assert!(seeds.insert(place.seed()), "press {index}: the seed is new");
            heard.push((note, velocity, e.tick));
        }
        assert_eq!(heard.len(), 700);
    }

    fn pedal(tick: u64, value: u8) -> SequenceEvent {
        SequenceEvent {
            tick,
            event: InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            },
        }
    }

    /// What reaches an instrument through `song_pedal`.
    #[derive(Default)]
    struct Pedals(Vec<(u8, u32, bool)>);

    impl Instrument for Pedals {
        fn prepare(&mut self, _: f32, _: usize) {}
        fn event(&mut self, _: InstrumentEvent) {}
        fn song_pedal(&mut self, value: u8, place: SongPlace<'_>) {
            self.0.push((value, place.earlier_moves(), place.chased()));
        }
        fn render(&mut self, _: &mut [f32], _: &mut [f32]) {}
        fn reset(&mut self) {}
    }

    /// A pedal recorded from a keyboard sends many values: only the ones that cross 64 are
    /// moves. Each move is told the earlier moves the same way; playing from the middle, the
    /// pedal where the song left it comes as chased.
    #[test]
    fn a_pedal_move_knows_the_earlier_moves_the_same_way() {
        let values = [127, 100, 30, 0, 90, 64, 63, 127];
        let events = (values.iter().enumerate())
            .map(|(i, &v)| pedal(i as u64 * 10, v))
            .collect();
        let mut lane = Lane::default();
        lane.replace_sequence(Some(Box::new(Sequence::new(events))));
        let mut heard = Pedals::default();
        lane.seek(|_| false, Some(&mut heard));
        lane.fire_before(u64::MAX, Some(&mut heard));
        let moves: Vec<u32> = heard.0.iter().map(|h| h.1).collect();
        assert_eq!(moves, [0, 1, 0, 1, 1, 2, 1, 2]);
        assert!(heard.0.iter().all(|h| !h.2));

        let mut heard = Pedals::default();
        lane.seek(|tick| tick < 45, Some(&mut heard));
        assert_eq!(heard.0, [(90, 1, true)]);
        lane.fire_before(55, Some(&mut heard));
        assert_eq!(heard.0[1], (64, 2, false));
        let mut heard = Pedals::default();
        lane.seek(|tick| tick < 35, Some(&mut heard));
        assert!(
            heard.0.is_empty(),
            "a pedal the song left at 0 is not chased"
        );
    }

    /// The seed is the note's own: other notes before it, or none, do not change it.
    #[test]
    fn a_press_keeps_its_seed_when_other_notes_come_and_go() {
        let seed_of = |events: Vec<SequenceEvent>, tick: u64| {
            let sequence = Sequence::new(events);
            let index = (sequence.events().iter())
                .position(|e| e.tick == tick && matches!(e.event, InstrumentEvent::NoteOn { .. }))
                .unwrap();
            sequence.place(index, 60).seed()
        };
        let alone = seed_of(vec![on(1920, 60, 90)], 1920);
        let after = seed_of(vec![on(0, 60, 90), on(960, 62, 50), on(1920, 60, 90)], 1920);
        assert_eq!(alone, after);
        assert_ne!(alone, seed_of(vec![on(1921, 60, 90)], 1921));
    }

    /// What reaches an instrument of the pedal, both ways.
    #[derive(Default)]
    struct PedalWays {
        events: Vec<u8>,
        song: Vec<(u8, bool)>,
    }

    impl Instrument for PedalWays {
        fn prepare(&mut self, _: f32, _: usize) {}
        fn event(&mut self, e: InstrumentEvent) {
            if let InstrumentEvent::Control {
                controller: CC_SUSTAIN,
                value,
            } = e
            {
                self.events.push(value);
            }
        }
        fn song_pedal(&mut self, value: u8, place: SongPlace<'_>) {
            self.song.push((value, place.chased()));
        }
        fn render(&mut self, _: &mut [f32], _: &mut [f32]) {}
        fn reset(&mut self) {}
    }

    /// A pedal the song holds over the loop's end is let go there with no sound (as chased),
    /// and when playing stops with a pedal let go as before.
    #[test]
    fn the_loop_lets_go_of_the_pedal_quietly_and_a_stop_as_before() {
        let events = vec![pedal(0, 127), on(10, 60, 100)];
        for at_loop in [true, false] {
            let mut lane = Lane::default();
            lane.replace_sequence(Some(Box::new(Sequence::new(events.clone()))));
            let mut heard = PedalWays::default();
            lane.seek(|_| false, Some(&mut heard));
            lane.fire_before(u64::MAX, Some(&mut heard));
            assert_eq!(heard.song, [(127, false)]);
            if at_loop {
                lane.silence_at_loop(Some(&mut heard));
                assert_eq!(heard.song, [(127, false), (0, true)]);
                assert!(heard.events.is_empty());
            } else {
                lane.silence(Some(&mut heard));
                assert_eq!(heard.song, [(127, false)]);
                assert_eq!(heard.events, [0]);
            }
        }
    }
}
