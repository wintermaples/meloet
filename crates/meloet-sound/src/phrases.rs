//! The sample phrases of the shelf (spec 3 「サンプル」 and 「サンプルの ▶（84）」): one fixed
//! phrase for each group of instruments, tempo 120, C major, two bars, every note at one
//! velocity. [`PhrasePlayer`]
//! sounds a phrase on an instrument by itself, on the audio thread.

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_model::InstrumentGroup;

/// One eighth note at tempo 120, in seconds.
pub const EIGHTH: f64 = 0.25;
/// Two bars at tempo 120.
pub const PHRASE_S: f64 = 4.0;
/// What the last notes are given to die away in, after the two bars.
pub const TAIL_S: f64 = 2.0;
pub const VELOCITY: u8 = 100;

/// (start in seconds, length in seconds, key)
pub type Phrase = Vec<(f64, f64, u8)>;

pub fn eighths(keys: &[u8], length: f64) -> Phrase {
    keys.iter()
        .enumerate()
        .map(|(i, &key)| (i as f64 * EIGHTH, length, key))
        .collect()
}

/// C, Am, F and G for two beats each; over the G the right hand climbs.
pub fn keys_phrase() -> Phrase {
    let chords: [(u8, [u8; 3]); 4] = [
        (48, [60, 64, 67]),
        (45, [60, 64, 69]),
        (41, [60, 65, 69]),
        (43, [59, 62, 67]),
    ];
    let mut phrase = Phrase::new();
    for (i, (bass, hand)) in chords.into_iter().enumerate() {
        let start = i as f64;
        phrase.push((start, 0.95, bass));
        // The last chord is short in the right hand: it goes on into the line.
        let length = if i == 3 { 0.24 } else { 0.95 };
        phrase.extend(hand.map(|key| (start, length, key)));
    }
    for (i, key) in [71, 74, 79].into_iter().enumerate() {
        let last = i == 2;
        let length = if last { 0.45 } else { 0.24 };
        phrase.push((3.0 + (i + 1) as f64 * EIGHTH, length, key));
    }
    phrase
}

pub fn bass_line() -> Phrase {
    eighths(
        &[
            36, 36, 43, 36, 48, 36, 43, 48, 36, 36, 43, 48, 36, 43, 48, 43,
        ],
        0.21,
    )
}

pub fn pad_chords() -> Phrase {
    let c = [48, 55, 60, 64, 67].map(|key| (0.0, 1.95, key));
    let f = [53, 57, 60, 65, 69].map(|key| (2.0, 1.95, key));
    c.into_iter().chain(f).collect()
}

pub fn drum_beat() -> Phrase {
    let hit = |eighth: f64, key: u8| (eighth * EIGHTH, 0.05, key);
    let mut beat = vec![hit(0.0, 49)];
    // Bar 1 and the first half of bar 2: hi-hat on every eighth, kick on 1 and 3, snare on 2 and 4.
    for eighth in 0..12 {
        let open = eighth == 7;
        beat.push(hit(f64::from(eighth), if open { 46 } else { 42 }));
    }
    for eighth in [0.0, 3.0, 4.0, 8.0, 11.0] {
        beat.push(hit(eighth, 36));
    }
    for eighth in [2.0, 6.0, 10.0] {
        beat.push(hit(eighth, 38));
    }
    // The last two beats: sixteenths down the toms.
    for (i, key) in [50, 50, 47, 47, 45, 45, 45, 45].into_iter().enumerate() {
        beat.push(hit(12.0 + i as f64 * 0.5, key));
    }
    beat.push(hit(12.0, 36));
    beat.push(hit(14.0, 36));
    beat
}

pub fn lead_line() -> Phrase {
    let mut line = eighths(
        &[72, 76, 79, 76, 81, 79, 76, 74, 72, 74, 76, 79, 84, 79, 76],
        0.2,
    );
    line.push((15.0 * EIGHTH, 0.45, 72));
    line
}

/// The strings of a guitar chord, lowest first, as a guitarist holds C, Am, F and G.
pub const GUITAR_CHORDS: [&[u8]; 4] = [
    &[48, 52, 55, 60, 64],
    &[45, 52, 57, 60, 64],
    &[41, 48, 53, 57, 60, 65],
    &[43, 47, 50, 55, 59, 67],
];
/// How far apart the strings of one strum sound.
pub const STRUM_GAP: f64 = 0.008;

/// C, Am, F and G for two beats each, strummed on every eighth (spec 3 「サンプルの ▶（84）」):
/// down from the lowest string on the beat, up from the highest string between the beats, the
/// up strokes over the upper strings only and shorter.
pub fn guitar_strum() -> Phrase {
    let mut phrase = Phrase::new();
    for (i, chord) in GUITAR_CHORDS.into_iter().enumerate() {
        for stroke in 0..4 {
            let start = i as f64 + f64::from(stroke) * EIGHTH;
            let (strings, length): (Vec<u8>, f64) = if stroke % 2 == 0 {
                (chord.to_vec(), 0.2)
            } else {
                (chord.iter().rev().take(4).copied().collect(), 0.14)
            };
            for (n, key) in strings.into_iter().enumerate() {
                phrase.push((start + n as f64 * STRUM_GAP, length, key));
            }
        }
    }
    phrase
}

/// A short line from C4 that ends on a long C5 (spec 3 「サンプルの ▶（84）」). It stays
/// between C4 and C5, where the trumpet, the alto sax and the flute all sound.
pub fn wind_line() -> Phrase {
    // (beat, beats, key)
    let line: [(f64, f64, u8); 10] = [
        (0.0, 1.0, 60),
        (1.0, 0.5, 64),
        (1.5, 0.5, 67),
        (2.0, 1.0, 69),
        (3.0, 1.0, 67),
        (4.0, 0.5, 65),
        (4.5, 0.5, 64),
        (5.0, 0.5, 62),
        (5.5, 0.5, 67),
        (6.0, 2.0, 72),
    ];
    let beat = 2.0 * EIGHTH;
    line.into_iter()
        .enumerate()
        .map(|(i, (at, beats, key))| {
            let last = i == line.len() - 1;
            // A breath between the notes; the last one is held to the end of the two bars.
            let length = if last {
                beats * beat
            } else {
                beats * beat - 0.04
            };
            (at * beat, length, key)
        })
        .collect()
}

/// The phrase the ▶ of the shelf plays for an instrument of `group`. The organs and the
/// mallets are in the piano and keys group, and play its phrase.
pub fn for_group(group: InstrumentGroup) -> Phrase {
    match group {
        InstrumentGroup::PianoKeys => keys_phrase(),
        InstrumentGroup::Guitar => guitar_strum(),
        InstrumentGroup::Wind => wind_line(),
        InstrumentGroup::Bass => bass_line(),
        InstrumentGroup::Drums => drum_beat(),
        InstrumentGroup::PadStrings => pad_chords(),
        InstrumentGroup::Synth => lead_line(),
    }
}

/// An instrument that plays a phrase on its own from the first frame it renders, every note at
/// [`VELOCITY`], on a clock of its own: the frames it has rendered. Events from outside do not
/// reach the instrument inside. After the phrase it goes on rendering, so the last notes die
/// away as the instrument lets them.
pub struct PhrasePlayer {
    inner: Box<dyn Instrument>,
    phrase: Phrase,
    /// (frame, event) in the order they are sent; filled by `prepare`.
    events: Vec<(u64, InstrumentEvent)>,
    next: usize,
    frame: u64,
}

impl PhrasePlayer {
    pub fn new(inner: Box<dyn Instrument>, phrase: Phrase) -> PhrasePlayer {
        PhrasePlayer {
            inner,
            phrase,
            events: Vec::new(),
            next: 0,
            frame: 0,
        }
    }
}

impl Instrument for PhrasePlayer {
    /// Puts the phrase back at its start.
    fn prepare(&mut self, sample_rate: f32, max_frames: usize) {
        self.inner.prepare(sample_rate, max_frames);
        let frame = |seconds: f64| (seconds * f64::from(sample_rate)).round() as u64;
        self.events.clear();
        for &(start, length, note) in &self.phrase {
            let velocity = VELOCITY;
            let on = InstrumentEvent::NoteOn { note, velocity };
            self.events.push((frame(start), on));
            self.events
                .push((frame(start + length), InstrumentEvent::NoteOff { note }));
        }
        // At the same frame a key is let go before it is pressed again.
        self.events
            .sort_by_key(|(at, ev)| (*at, matches!(ev, InstrumentEvent::NoteOn { .. })));
        self.next = 0;
        self.frame = 0;
    }

    fn event(&mut self, _ev: InstrumentEvent) {}

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mut from = 0;
        while from < left.len() {
            while let Some(&(at, event)) = self.events.get(self.next)
                && at <= self.frame
            {
                self.inner.event(event);
                self.next += 1;
            }
            // Up to the next event, so that every note starts on its own frame.
            let room = left.len() - from;
            let n = match self.events.get(self.next) {
                Some(&(at, _)) => room.min((at - self.frame) as usize),
                None => room,
            };
            self.inner
                .render(&mut left[from..from + n], &mut right[from..from + n]);
            self.frame += n as u64;
            from += n;
        }
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.next = self.events.len();
    }
}
