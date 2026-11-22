//! The audio thread's rule for the sample player: keys, pedal, bend and rendering never allocate
//! or free. One test only, so nothing else runs in this binary while the counts are taken.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::{Instrument, SongPlace};
use support::pack::{Storm, everything};

const EARLIER: [u8; 37] = [
    5, 127, 64, 90, 1, 33, 100, 100, 7, 64, 65, 120, 2, 80, 80, 80, 3, 99, 127, 50, 60, 70, 8, 9,
    110, 4, 64, 63, 62, 61, 12, 127, 126, 1, 40, 41, 42,
];

#[test]
fn playing_does_not_allocate() {
    let pack = everything("rt");
    let mut sampler = pack.sampler(48_000.0, 256);
    let mut storm = Storm::new(7);
    let (mut left, mut right) = (vec![0.0_f32; 256], vec![0.0_f32; 256]);
    assert_eq!(common::audio_block_counts(), (0, 0));

    let (mut most_voices, mut loudest) = (0, 0.0_f32);
    {
        let _block = meloet_engine::rt::enter();
        for block in 0..6000_usize {
            for _ in 0..block % 4 {
                sampler.event(storm.next());
            }
            if block % 3 == 0 {
                // A press of the song on the keys whose samples take turns.
                let earlier = &EARLIER[..block % EARLIER.len()];
                let place = SongPlace::new(block as u32, earlier);
                sampler.song_note_on(96 + (block % 2) as u8, 1 + (block % 127) as u8, place);
            }
            if block % 1500 == 1499 {
                sampler.reset();
            }
            let frames = [256, 1, 64, 200, 33][block % 5];
            sampler.render(&mut left[..frames], &mut right[..frames]);
            most_voices = most_voices.max(sampler.active_voices());
            loudest = left[..frames].iter().fold(loudest, |m, x| m.max(x.abs()));
        }
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
    // The storm did reach the voice limit and the stealing behind it.
    assert!(most_voices > 8, "{most_voices}");
    assert!(loudest > 0.1, "{loudest}");
}
