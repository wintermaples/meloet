//! The sample phrases and the player that sounds one by itself. One test takes the counts of the
//! audio thread's rule, so every test here renders inside an audio block.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_model::InstrumentGroup;
use meloet_sound::phrases::{EIGHTH, PHRASE_S, PhrasePlayer, VELOCITY, for_group};
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
fn a_phrase_on_a_real_instrument_sounds_and_does_not_allocate() {
    let phrase = for_group(InstrumentGroup::Synth);
    let mut player = PhrasePlayer::new(Box::new(PolySynth::lead()), phrase);
    player.prepare(48_000.0, 256);
    let (left, _) = play(&mut player, 6 * 48_000, &[256, 64, 200]);
    let peak = |x: &[f32]| x.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
    assert!(peak(&left[..4 * 48_000]) > 0.05);
    assert!(peak(&left[5 * 48_000..]) < 0.001, "it has died away");
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}
