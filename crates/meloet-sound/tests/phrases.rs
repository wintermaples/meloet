//! The sample phrases and the player that sounds one by itself. One test takes the counts of the
//! audio thread's rule, so every test here renders inside an audio block.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_model::InstrumentGroup;
use meloet_sound::phrases::{EIGHTH, GUITAR_CHORDS, PHRASE_S, PhrasePlayer, VELOCITY, for_group};
use meloet_sound::synth::PolySynth;

/// A 1.0 on the frame a key goes down, the number of keys held on every other frame.
#[derive(Default)]
struct Clicks {
    held: i32,
    pressed: bool,
}

impl Instrument for Clicks {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {
        *self = Clicks::default();
    }
    fn event(&mut self, ev: InstrumentEvent) {
        match ev {
            InstrumentEvent::NoteOn { velocity, .. } => {
                assert_eq!(velocity, VELOCITY);
                self.held += 1;
                self.pressed = true;
            }
            InstrumentEvent::NoteOff { .. } => self.held -= 1,
            _ => panic!("a phrase has notes only"),
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(self.held as f32);
        if std::mem::take(&mut self.pressed) {
            left[0] = 1.0;
        }
    }
    fn reset(&mut self) {
        *self = Clicks::default();
    }
}

fn play(player: &mut PhrasePlayer, frames: usize, blocks: &[usize]) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (vec![0.0; frames], vec![9.0; frames]);
    let _block = meloet_engine::rt::enter();
    let (mut at, mut turn) = (0, 0);
    while at < frames {
        let n = blocks[turn % blocks.len()].min(frames - at);
        player.render(&mut left[at..at + n], &mut right[at..at + n]);
        at += n;
        turn += 1;
    }
    (left, right)
}

#[test]
fn every_note_of_every_phrase_starts_on_its_own_frame() {
    const RATE: usize = 44_100;
    for group in InstrumentGroup::ALL {
        let phrase = for_group(group);
        assert!(!phrase.is_empty());
        let mut starts: Vec<usize> = phrase
            .iter()
            .map(|&(start, length, key)| {
                // The last note may ring a little into the tail.
                assert!((0.0..PHRASE_S).contains(&start), "{group:?}");
                assert!(start + length <= PHRASE_S + EIGHTH, "{group:?}");
                assert!(key <= 127);
                (start * RATE as f64).round() as usize
            })
            .collect();
        starts.sort_unstable();
        starts.dedup();

        let mut player = PhrasePlayer::new(Box::new(Clicks::default()), phrase);
        player.prepare(RATE as f32, 1024);
        let (left, right) = play(&mut player, 5 * RATE, &[1024, 1, 333, 64, 1000]);
        let clicks: Vec<usize> = (0..left.len()).filter(|&i| left[i] == 1.0).collect();
        assert_eq!(clicks, starts, "{group:?}");
        assert_eq!(
            right[4 * RATE + RATE / 2],
            0.0,
            "{group:?}: every key was let go"
        );

        // Keys from outside do not reach the instrument, and a reset ends the phrase.
        player.prepare(RATE as f32, 1024);
        player.event(InstrumentEvent::NoteOn {
            note: 1,
            velocity: 1,
        });
        play(&mut player, 100, &[64]);
        player.reset();
        let (left, right) = play(&mut player, RATE, &[256]);
        assert!(left.iter().chain(&right).all(|&x| x == 0.0), "{group:?}");

        // Prepared again, it starts again.
        player.prepare(RATE as f32, 256);
        assert_eq!(play(&mut player, 16, &[256]).0[0], 1.0, "{group:?}");
    }
}

#[test]
fn a_key_is_let_go_before_it_is_pressed_again() {
    for group in InstrumentGroup::ALL {
        let mut phrase = for_group(group);
        phrase.sort_by(|a, b| (a.2, a.0).partial_cmp(&(b.2, b.0)).unwrap());
        for w in phrase.windows(2) {
            if w[0].2 == w[1].2 {
                assert!(w[0].0 + w[0].1 <= w[1].0 + 1e-9, "{group:?}: {w:?}");
            }
        }
    }
}

#[test]
fn the_guitar_strums_c_am_f_g_on_every_eighth() {
    let phrase = for_group(InstrumentGroup::Guitar);
    // 16 strokes in two bars, each a little spread over its strings.
    let mut strokes: Vec<Vec<(f64, f64, u8)>> = Vec::new();
    for &note in &phrase {
        let stroke = (note.0 / EIGHTH).floor() as usize;
        assert!(note.0 - stroke as f64 * EIGHTH < 0.05, "{note:?}");
        if strokes.len() <= stroke {
            strokes.resize(stroke + 1, Vec::new());
        }
        strokes[stroke].push(note);
    }
    assert_eq!(strokes.len(), 16);
    for (i, stroke) in strokes.iter().enumerate() {
        let chord = GUITAR_CHORDS[i / 4];
        assert!(stroke.len() >= 4, "stroke {i}");
        assert!(stroke.iter().all(|n| chord.contains(&n.2)), "stroke {i}");
        assert!(
            stroke.windows(2).all(|w| w[1].0 > w[0].0),
            "one string after another"
        );
        let keys: Vec<u8> = stroke.iter().map(|n| n.2).collect();
        let down = i % 2 == 0;
        if down {
            assert_eq!(keys, chord, "down from the lowest string");
        } else {
            assert!(
                keys.windows(2).all(|w| w[1] < w[0]),
                "up from the highest string"
            );
            let longest_down = strokes[i - 1].iter().map(|n| n.1).fold(0.0, f64::max);
            assert!(
                stroke.iter().all(|n| n.1 < longest_down),
                "the up stroke is shorter"
            );
        }
    }
    let end = phrase.iter().map(|n| n.0 + n.1).fold(0.0, f64::max);
    assert!(end <= PHRASE_S, "two bars");
}

#[test]
fn the_wind_line_starts_on_c4_stays_up_to_c5_and_holds_its_last_note() {
    let phrase = for_group(InstrumentGroup::Wind);
    assert_eq!(phrase[0].0, 0.0);
    assert_eq!(phrase[0].2, 60);
    assert!(phrase.iter().all(|n| (60..=72).contains(&n.2)));
    assert!(
        phrase.windows(2).all(|w| w[0].0 + w[0].1 <= w[1].0),
        "one note at a time"
    );
    let last = phrase.last().unwrap();
    assert!(last.1 >= 4.0 * EIGHTH, "the last note is held two beats");
    assert!(
        (last.0 + last.1 - PHRASE_S).abs() < 1e-9,
        "to the end of the two bars"
    );
}

#[test]
fn a_phrase_on_a_real_instrument_sounds_and_does_not_allocate() {
    for group in [
        InstrumentGroup::Synth,
        InstrumentGroup::Guitar,
        InstrumentGroup::Wind,
    ] {
        let phrase = for_group(group);
        let mut player = PhrasePlayer::new(Box::new(PolySynth::lead()), phrase);
        player.prepare(48_000.0, 256);
        let (left, _) = play(&mut player, 6 * 48_000, &[256, 64, 200]);
        let peak = |x: &[f32]| x.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        assert!(peak(&left[..4 * 48_000]) > 0.05, "{group:?}");
        assert!(
            peak(&left[5 * 48_000..]) < 0.001,
            "{group:?}: it has died away"
        );
    }
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}
