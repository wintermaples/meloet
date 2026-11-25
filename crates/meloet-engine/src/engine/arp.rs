//! The arpeggio of the track the keyboards play (round 12, E2).
//!
//! The keys held on a track whose arpeggio is on do not sound: they make the pattern, and
//! the pattern's notes sound one at a time, each at the very frame of its step.
//!
//! - While the song moves, the steps are on the song's grid, counted from the head of each
//!   bar ([`Engine::arp_grid_from`]): a note's place in the song is its step's tick.
//! - While it stands still, the steps start with the first key and go on at the tempo of
//!   the place the song is at, counted in frames.
//! - A note ends after [`ARP_GATE_PERCENT`] of a step, or where the next step comes first.
//! - When the clock under it changes (play starts or stops, the loop goes round, the place
//!   jumps, another tempo map), the note that sounds ends there and the next step is found
//!   anew.
//!
//! Every note made here is written down as played ([`Engine::arp_send`]): in a recording at
//! its tick, else as what can be kept afterwards. The keys that made it are not.

use meloet_model::{
    ARP_GATE_PERCENT, ARP_KEYS_MAX, ARP_OCTAVES_MAX, Arp, ArpShape, TICKS_PER_QUARTER,
};

use super::{
    BlockTiming, Engine, InstrumentEvent, MidiIn, RECORDED_HEADROOM, RecentAt, Recorded,
    TICKS_PER_S_PER_BPM, Transport, TransportState,
};
use std::sync::atomic::Ordering::Relaxed;

const SEQ_MAX: usize = ARP_KEYS_MAX * ARP_OCTAVES_MAX as usize;

/// One step, in ticks.
pub(super) fn step_ticks(set: &Arp) -> i64 {
    let step = 4 * TICKS_PER_QUARTER as i64 / i64::from(set.rate.division());
    if set.triplet { step * 2 / 3 } else { step }
}

fn gate_ticks(step: i64) -> i64 {
    step * i64::from(ARP_GATE_PERCENT) / 100
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub note: u8,
    pub velocity: u8,
}

/// The keys of the pattern and the order they are played in. Nothing here allocates.
pub(super) struct Pattern {
    /// By note, lowest first.
    keys: [Key; ARP_KEYS_MAX],
    len: usize,
    /// Every note of one way up: the keys in each octave, by note, each note once.
    seq: [Key; SEQ_MAX],
    seq_len: usize,
    /// Steps played since the pattern started.
    step: u32,
    /// Where in `seq` the last step was (「ばらばら」 does not say a note twice in a row).
    last: usize,
    rng: u32,
}

impl Pattern {
    pub fn new() -> Pattern {
        Pattern {
            keys: [Key::default(); ARP_KEYS_MAX],
            len: 0,
            seq: [Key::default(); SEQ_MAX],
            seq_len: 0,
            step: 0,
            last: usize::MAX,
            rng: 0x9E37_79B9,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The pattern starts from its first note again when the next key comes.
    pub fn clear(&mut self) {
        self.len = 0;
        self.seq_len = 0;
        self.step = 0;
        self.last = usize::MAX;
    }

    /// A key joins. One that is in already takes the new velocity; one past
    /// [`ARP_KEYS_MAX`] is left out.
    pub fn press(&mut self, note: u8, velocity: u8, set: &Arp) {
        let keys = &mut self.keys[..self.len];
        let at = keys.partition_point(|k| k.note < note);
        if keys.get(at).is_some_and(|k| k.note == note) {
            keys[at].velocity = velocity;
        } else if self.len < ARP_KEYS_MAX {
            self.keys.copy_within(at..self.len, at + 1);
            self.keys[at] = Key { note, velocity };
            self.len += 1;
        } else {
            return;
        }
        self.rebuild(set);
    }

    pub fn release(&mut self, note: u8, set: &Arp) {
        let keys = &self.keys[..self.len];
        let at = keys.partition_point(|k| k.note < note);
        if keys.get(at).is_some_and(|k| k.note == note) {
            self.keys.copy_within(at + 1..self.len, at);
            self.len -= 1;
            self.rebuild(set);
        }
    }

    /// Only the keys of `down` (one bit a note) stay.
    pub fn retain(&mut self, down: u128, set: &Arp) {
        let mut kept = 0;
        for i in 0..self.len {
            if down & (1 << self.keys[i].note) != 0 {
                self.keys[kept] = self.keys[i];
                kept += 1;
            }
        }
        self.len = kept;
        self.rebuild(set);
        if kept == 0 {
            self.clear();
        }
    }

    /// The way up for the keys and `set.octaves`. Notes past 127 are left out.
    pub fn rebuild(&mut self, set: &Arp) {
        self.seq_len = 0;
        for octave in 0..set.octaves.clamp(1, ARP_OCTAVES_MAX) {
            for key in &self.keys[..self.len] {
                let note = u16::from(key.note) + 12 * u16::from(octave);
                if note > 127 {
                    continue;
                }
                let note = note as u8;
                let seq = &self.seq[..self.seq_len];
                let at = seq.partition_point(|k| k.note < note);
                if seq.get(at).is_some_and(|k| k.note == note) {
                    continue;
                }
                self.seq.copy_within(at..self.seq_len, at + 1);
                self.seq[at] = Key {
                    note,
                    velocity: key.velocity,
                };
                self.seq_len += 1;
            }
        }
    }

    /// The note of the next step.
    pub fn next(&mut self, set: &Arp) -> Option<Key> {
        let m = self.seq_len;
        if m == 0 {
            return None;
        }
        let step = self.step as usize;
        let at = match set.shape {
            ArpShape::Up => step % m,
            ArpShape::Down => m - 1 - step % m,
            ArpShape::UpDown if m == 1 => 0,
            ArpShape::UpDown => {
                let round = 2 * m - 2;
                let p = step % round;
                if p < m { p } else { round - p }
            }
            ArpShape::Random => {
                // xorshift32
                let mut x = self.rng;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                self.rng = x;
                let x = x as usize;
                if m > 1 && self.last < m {
                    let other = x % (m - 1);
                    if other >= self.last { other + 1 } else { other }
                } else {
                    x % m
                }
            }
        };
        self.step = self.step.wrapping_add(1);
        self.last = at;
        Some(self.seq[at])
    }
}

/// What the steps are counted on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Clock {
    /// The song stands still: frames since the engine was made.
    Free,
    /// The song moves: frames from the transport's anchor, which holds while
    /// `Transport::serial` is this.
    Grid(u64),
}

#[derive(Clone, Copy)]
struct Sounding {
    note: u8,
    clock: Clock,
    /// Where it ends: on `Clock::Free`, the frame (with its fraction); on `Clock::Grid`,
    /// the tick and its frame.
    off_free: f64,
    off_tick: f64,
    off_frame: u64,
}

pub(super) struct ArpPlayer {
    pattern: Pattern,
    /// The keys that are down on the keyboards, one bit a note, whatever the arpeggio does.
    down: u128,
    /// What the next step was counted on; `None`: it is still to be found.
    clock: Option<Clock>,
    /// The length of a step changed: the next step on the grid is found anew.
    regrid: bool,
    next_free: f64,
    next_tick: i64,
    next_frame: u64,
    sounding: Option<Sounding>,
}

impl ArpPlayer {
    pub fn new() -> ArpPlayer {
        ArpPlayer {
            pattern: Pattern::new(),
            down: 0,
            clock: None,
            regrid: false,
            next_free: 0.0,
            next_tick: 0,
            next_frame: 0,
            sounding: None,
        }
    }

    /// Forgets the pattern and the note that sounds, without a word to the instrument.
    fn forget(&mut self) {
        self.pattern.clear();
        self.sounding = None;
        self.clock = None;
    }
}

/// The first whole frame at `frames` or past it; the same guard as `TempoMap::frame_of`.
fn free_frame(frames: f64) -> u64 {
    (frames - 1e-6).ceil().max(0.0) as u64
}

/// The head of the bar `tick` is in, and the head of the next one.
fn bar_of(t: &Transport, tick: i64) -> (i64, i64) {
    if t.state == TransportState::CountIn {
        let c = &t.count_in;
        let per_bar = (c.ticks_per_click * c.clicks_per_bar).max(1);
        let head = c.bar_tick + (tick - c.bar_tick).div_euclid(per_bar) * per_bar;
        (head, head + per_bar)
    } else {
        let at = t.map.meter_at(tick.max(0) as u64);
        (at.bar_tick as i64, t.map.tick_of_bar(at.bar + 1) as i64)
    }
}

/// The last step at `tick` or before it. Steps are counted from the head of each bar.
fn grid_floor(t: &Transport, tick: f64, step: i64) -> i64 {
    let tick = tick.floor() as i64;
    let (head, _) = bar_of(t, tick);
    head + (tick - head).div_euclid(step) * step
}

/// The step after the step `at`: a step's length on, or the head of the next bar when that
/// comes first.
fn grid_next(t: &Transport, at: i64, step: i64) -> i64 {
    let (_, next_bar) = bar_of(t, at);
    (at + step).min(next_bar)
}

impl Engine {
    /// The arpeggio of the track the keyboards play.
    fn arp_set(&self) -> Arp {
        self.play_target
            .map_or_else(Arp::default, |t| self.tracks[t].arp)
    }

    /// A keyboard event, before it sounds. True: the arpeggio took it, and it neither
    /// sounds nor is written down.
    pub(super) fn arp_key(&mut self, midi: MidiIn, timing: BlockTiming) -> bool {
        let before = self.arp.down;
        match midi.event {
            InstrumentEvent::NoteOn { note, velocity } if note <= 127 && velocity > 0 => {
                self.arp.down |= 1 << note;
            }
            InstrumentEvent::NoteOn { note, .. } | InstrumentEvent::NoteOff { note }
                if note <= 127 =>
            {
                self.arp.down &= !(1 << note);
            }
            _ => {}
        }
        let set = self.arp_set();
        if !set.on {
            return false;
        }
        let arp = &mut self.arp;
        match midi.event {
            InstrumentEvent::NoteOn { note, velocity } if velocity > 0 => {
                if note <= 127 {
                    // Held on after every key was let go: a new key starts a new chord.
                    if set.hold && before == 0 {
                        arp.pattern.clear();
                    }
                    arp.pattern.press(note, velocity.min(127), &set);
                }
                true
            }
            InstrumentEvent::NoteOn { note, .. } | InstrumentEvent::NoteOff { note } => {
                if !set.hold && note <= 127 {
                    arp.pattern.release(note, &set);
                    if arp.pattern.is_empty() {
                        arp.pattern.clear();
                        arp.clock = None;
                    }
                }
                true
            }
            InstrumentEvent::Control {
                controller: 121 | 123,
                ..
            }
            | InstrumentEvent::AllNotesOff => {
                self.arp_let_go(timing);
                false
            }
            _ => false,
        }
    }

    /// `EngineCommand::SetArp`.
    pub(super) fn set_arp(&mut self, track: usize, set: Arp, timing: BlockTiming) {
        let Some(slot) = self.tracks.get_mut(track) else {
            return;
        };
        let old = std::mem::replace(&mut slot.arp, set);
        if self.play_target != Some(track) || old == set {
            return;
        }
        match (old.on, set.on) {
            (true, false) => self.arp_let_go(timing),
            (false, true) => {
                // The keys held sound by themselves and would never be let go: their
                // releases are the arpeggio's from now on.
                for note in 0..128u8 {
                    if self.arp.down & (1 << note) != 0 {
                        let event = InstrumentEvent::NoteOff { note };
                        if let Some(instrument) = self.tracks[track].instrument.as_mut() {
                            instrument.event(event);
                        }
                        let t_us = timing.callback_us;
                        self.stamp_played(MidiIn { event, t_us }, timing, u64::MAX);
                    }
                }
                self.hand.keys = 0;
            }
            (true, true) => {
                if old.hold && !set.hold {
                    let down = self.arp.down;
                    self.arp.pattern.retain(down, &set);
                    if self.arp.pattern.is_empty() {
                        self.arp_let_go(timing);
                    }
                }
                self.arp.pattern.rebuild(&set);
                self.arp.regrid |= step_ticks(&old) != step_ticks(&set);
            }
            (false, false) => {}
        }
    }

    /// The note that sounds ends now and the pattern is empty: the arpeggio is silent until
    /// the next key.
    pub(super) fn arp_let_go(&mut self, timing: BlockTiming) {
        self.arp_cut(0, timing);
        self.arp.forget();
    }

    /// The keys play another track, or the track's instrument is gone: nothing is left of
    /// the pattern. The instrument is not told (it got `AllNotesOff`, or is no longer there).
    pub(super) fn arp_forget(&mut self) {
        self.arp.forget();
    }

    /// No stream: nothing sounds, and what the keyboards hold is not known.
    pub(super) fn arp_reset(&mut self) {
        self.arp.forget();
        self.arp.down = 0;
    }

    /// `EngineCommand::AllNotesOff`: the keyboards hold nothing.
    pub(super) fn arp_all_off(&mut self, timing: BlockTiming) {
        self.arp_let_go(timing);
        self.arp.down = 0;
    }

    fn arp_cut(&mut self, frame: usize, timing: BlockTiming) {
        if let Some(s) = self.arp.sounding.take() {
            let tick = self.transport.tick();
            self.arp_send(
                InstrumentEvent::NoteOff { note: s.note },
                frame,
                tick,
                timing,
            );
        }
    }

    /// A note of the arpeggio starts or ends at `frame` of this block: the instrument is
    /// told, and it is written down as played, at `tick` while the song moves.
    fn arp_send(&mut self, event: InstrumentEvent, frame: usize, tick: f64, timing: BlockTiming) {
        if let Some(instrument) = self
            .play_target
            .and_then(|t| self.tracks[t].instrument.as_mut())
        {
            instrument.event(event);
        }
        let t_us = self.dac_us(frame, timing).round() as u64;
        let t = &self.transport;
        let at = match t.state {
            TransportState::Recording => {
                let lap = t.lap;
                self.push_recorded(Recorded::Event {
                    event,
                    t_us,
                    tick,
                    lap,
                });
                return;
            }
            // Before the recording starts.
            TransportState::CountIn => return,
            TransportState::Playing => Some(RecentAt {
                tick,
                lap: t.run_lap,
            }),
            TransportState::Stopped => None,
        };
        let run = t.run;
        if self.recorded.slots() <= RECORDED_HEADROOM {
            self.meters.record_overflows.fetch_add(1, Relaxed);
            return;
        }
        self.push_recorded(Recorded::Recent {
            event,
            t_us,
            run,
            at,
        });
    }

    /// The first step of the grid that is still to come: its frame is the next frame to
    /// render, or after it.
    fn arp_grid_from(&self, step: i64) -> i64 {
        let t = &self.transport;
        let at = grid_floor(t, t.tick(), step);
        // By frames, as the sequences are sent: a step due at this very frame is still to
        // come, whatever the last bits of the position say.
        let to_come = if t.frames == 0 {
            at as f64 >= t.anchor_tick
        } else {
            t.frame_of(at as f64) >= t.frames
        };
        if to_come { at } else { grid_next(t, at, step) }
    }

    /// What the arpeggio has to do at `frame` of this block. Call it after
    /// [`Engine::step_at`]: the transport is where this frame plays.
    pub(super) fn arp_at(&mut self, frame: usize, timing: BlockTiming) {
        if self.arp.sounding.is_none() && self.arp.pattern.is_empty() {
            return;
        }
        let now = self.block_frame + frame as u64;
        let clock = if self.transport.state.is_moving() {
            Clock::Grid(self.transport.serial)
        } else {
            Clock::Free
        };
        // Its end was counted on a clock that is gone.
        if self.arp.sounding.is_some_and(|s| s.clock != clock) {
            self.arp_cut(frame, timing);
        }
        let set = self.arp_set();
        let step = step_ticks(&set);
        let regrid = std::mem::take(&mut self.arp.regrid) && clock != Clock::Free;
        if self.arp.pattern.is_empty() {
            self.arp.clock = None;
        } else if self.arp.clock != Some(clock) || regrid {
            self.arp.clock = Some(clock);
            match clock {
                Clock::Free => self.arp.next_free = now as f64,
                Clock::Grid(_) => {
                    let tick = self.arp_grid_from(step);
                    self.arp.next_tick = tick;
                    self.arp.next_frame = self.transport.frame_of(tick as f64);
                }
            }
        }

        if let Some(s) = self.arp.sounding {
            let due = match clock {
                Clock::Free => free_frame(s.off_free) <= now,
                Clock::Grid(_) => s.off_frame <= self.transport.frames,
            };
            if due {
                self.arp.sounding = None;
                let off = InstrumentEvent::NoteOff { note: s.note };
                self.arp_send(off, frame, s.off_tick, timing);
            }
        }

        if self.arp.pattern.is_empty() {
            return;
        }
        let due = match clock {
            Clock::Free => free_frame(self.arp.next_free) <= now,
            Clock::Grid(_) => self.arp.next_frame <= self.transport.frames,
        };
        if !due {
            return;
        }
        let tick = self.arp.next_tick as f64;
        if let Some(s) = self.arp.sounding.take() {
            let off = InstrumentEvent::NoteOff { note: s.note };
            self.arp_send(off, frame, tick, timing);
        }
        let Some(key) = self.arp.pattern.next(&set) else {
            return;
        };
        let on = InstrumentEvent::NoteOn {
            note: key.note,
            velocity: key.velocity,
        };
        self.arp_send(on, frame, tick, timing);
        let t = &self.transport;
        let mut sounding = Sounding {
            note: key.note,
            clock,
            off_free: 0.0,
            off_tick: 0.0,
            off_frame: 0,
        };
        match clock {
            Clock::Free => {
                let ticks_per_s = t.map.tempo_at(t.tick()) * TICKS_PER_S_PER_BPM;
                let frames = step as f64 / ticks_per_s * t.sample_rate;
                let at = self.arp.next_free;
                sounding.off_free = at + frames * f64::from(ARP_GATE_PERCENT) / 100.0;
                self.arp.next_free = at + frames;
            }
            Clock::Grid(_) => {
                let off = self.arp.next_tick + gate_ticks(step);
                sounding.off_tick = off as f64;
                sounding.off_frame = t.frame_of(off as f64);
                let next = grid_next(t, self.arp.next_tick, step);
                self.arp.next_tick = next;
                self.arp.next_frame = t.frame_of(next as f64);
            }
        }
        self.arp.sounding = Some(sounding);
    }

    /// Frames from `frame` of this block to where [`Engine::arp_at`] next has work
    /// (`u64::MAX`: none in sight). Call it after `arp_at` for the same frame.
    pub(super) fn arp_frames_left(&self, frame: usize) -> u64 {
        let now = self.block_frame + frame as u64;
        let t = &self.transport;
        let mut left = u64::MAX;
        if let Some(s) = self.arp.sounding {
            left = left.min(match s.clock {
                Clock::Free => free_frame(s.off_free).saturating_sub(now),
                Clock::Grid(_) => s.off_frame.saturating_sub(t.frames),
            });
        }
        if !self.arp.pattern.is_empty() {
            left = left.min(match self.arp.clock {
                Some(Clock::Free) => free_frame(self.arp.next_free).saturating_sub(now),
                Some(Clock::Grid(_)) => self.arp.next_frame.saturating_sub(t.frames),
                None => u64::MAX,
            });
        }
        left
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meloet_model::ArpRate;

    fn set(shape: ArpShape, octaves: u8) -> Arp {
        Arp {
            on: true,
            shape,
            octaves,
            ..Arp::default()
        }
    }

    fn notes(p: &mut Pattern, set: &Arp, n: usize) -> Vec<u8> {
        (0..n).map(|_| p.next(set).unwrap().note).collect()
    }

    #[test]
    fn a_step_is_a_whole_number_of_ticks_at_every_rate() {
        let mut steps = Vec::new();
        for rate in ArpRate::ALL {
            for triplet in [false, true] {
                let set = Arp {
                    rate,
                    triplet,
                    ..Arp::default()
                };
                let step = step_ticks(&set);
                assert_eq!(gate_ticks(step) * 100, step * 80);
                steps.push(step);
            }
        }
        assert_eq!(steps, [1920, 1280, 960, 640, 480, 320, 240, 160]);
    }

    #[test]
    fn the_shapes_go_through_the_keys_in_their_order() {
        let up = set(ArpShape::Up, 1);
        let mut p = Pattern::new();
        for note in [64, 60, 67] {
            p.press(note, 100, &up);
        }
        assert_eq!(notes(&mut p, &up, 7), [60, 64, 67, 60, 64, 67, 60]);

        let down = set(ArpShape::Down, 1);
        p.clear();
        for note in [64, 60, 67] {
            p.press(note, 100, &down);
        }
        assert_eq!(notes(&mut p, &down, 7), [67, 64, 60, 67, 64, 60, 67]);

        let both = set(ArpShape::UpDown, 1);
        p.clear();
        for note in [64, 60, 67] {
            p.press(note, 100, &both);
        }
        // The top and the bottom are not said twice.
        assert_eq!(
            notes(&mut p, &both, 9),
            [60, 64, 67, 64, 60, 64, 67, 64, 60]
        );
    }

    #[test]
    fn one_key_repeats_in_every_shape() {
        for shape in ArpShape::ALL {
            let s = set(shape, 1);
            let mut p = Pattern::new();
            p.press(48, 90, &s);
            assert_eq!(notes(&mut p, &s, 4), [48; 4], "{shape:?}");
        }
        // Two keys, up and down: one after the other.
        let s = set(ArpShape::UpDown, 1);
        let mut p = Pattern::new();
        p.press(48, 90, &s);
        p.press(50, 90, &s);
        assert_eq!(notes(&mut p, &s, 5), [48, 50, 48, 50, 48]);
    }

    #[test]
    fn octaves_go_up_by_note_with_each_note_once_and_none_past_127() {
        let s = set(ArpShape::Up, 3);
        let mut p = Pattern::new();
        // An octave apart: the second octave of the low key is the high key.
        p.press(60, 100, &s);
        p.press(72, 50, &s);
        assert_eq!(notes(&mut p, &s, 5), [60, 72, 84, 96, 60]);
        // A wide chord still goes up by note.
        p.clear();
        p.press(40, 100, &s);
        p.press(59, 100, &s);
        assert_eq!(notes(&mut p, &s, 6), [40, 52, 59, 64, 71, 83]);
        // The top of the keyboard.
        p.clear();
        p.press(120, 100, &s);
        p.press(110, 100, &s);
        assert_eq!(notes(&mut p, &s, 4), [110, 120, 122, 110]);
        // The velocity is the key's.
        p.clear();
        p.press(60, 33, &s);
        assert_eq!(p.next(&s).unwrap().velocity, 33);
        assert_eq!(
            p.next(&s).unwrap(),
            Key {
                note: 72,
                velocity: 33
            }
        );
    }

    #[test]
    fn sixteen_keys_are_played_and_the_seventeenth_is_left_out() {
        let s = set(ArpShape::Up, 1);
        let mut p = Pattern::new();
        for note in 0..17u8 {
            p.press(40 + note, 100, &s);
        }
        let got = notes(&mut p, &s, 17);
        assert_eq!(got[..16], (40..56).collect::<Vec<u8>>()[..]);
        assert_eq!(got[16], 40);
        // Letting go of the one left out changes nothing; of one that is in, takes it out.
        p.release(56, &s);
        p.release(40, &s);
        assert_eq!(notes(&mut p, &s, 2), [43, 44]);
        // Three octaves of sixteen keys fit.
        let wide = set(ArpShape::Up, 3);
        p.rebuild(&wide);
        assert_eq!(p.seq_len, 15 + 12 + 12);
    }

    #[test]
    fn keys_that_come_and_go_keep_the_count_of_steps() {
        let s = set(ArpShape::Up, 1);
        let mut p = Pattern::new();
        p.press(60, 100, &s);
        p.press(64, 100, &s);
        assert_eq!(notes(&mut p, &s, 3), [60, 64, 60]);
        p.press(67, 100, &s);
        // Step 3 of three keys.
        assert_eq!(notes(&mut p, &s, 3), [60, 64, 67]);
        p.release(60, &s);
        assert_eq!(notes(&mut p, &s, 2), [64, 67]);
        p.release(64, &s);
        p.release(67, &s);
        assert!(p.is_empty());
        assert_eq!(p.next(&s), None);
    }

    #[test]
    fn random_says_every_key_and_never_one_twice_in_a_row() {
        let s = set(ArpShape::Random, 2);
        let mut p = Pattern::new();
        for note in [60, 64, 67] {
            p.press(note, 100, &s);
        }
        let got = notes(&mut p, &s, 600);
        assert!(got.windows(2).all(|w| w[0] != w[1]));
        for note in [60, 64, 67, 72, 76, 79] {
            let count = got.iter().filter(|&&n| n == note).count();
            assert!((50..150).contains(&count), "{note}: {count}");
        }
    }

    #[test]
    fn retain_keeps_only_the_keys_that_are_down() {
        let s = set(ArpShape::Up, 1);
        let mut p = Pattern::new();
        for note in [60, 64, 67] {
            p.press(note, 100, &s);
        }
        p.retain(1 << 64, &s);
        assert_eq!(notes(&mut p, &s, 2), [64, 64]);
        p.retain(0, &s);
        assert!(p.is_empty());
    }
}
