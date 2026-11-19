//! The audio thread's rule for the parts of the player a SoundFont uses (envelopes, LFOs,
//! filter, classes, loop modes, controllers): nothing allocates or frees. One test only, so
//! nothing else runs in this binary while the counts are taken.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::{Instrument, InstrumentEvent};
use support::pack::Storm;
use support::sf2::{TempFile, rich};

#[test]
fn playing_a_soundfont_does_not_allocate() {
    let file = TempFile::new("rt", &rich().bytes());
    let mut sampler = file.sampler(48_000.0, 256);
    let mut storm = Storm::new(7);
    let (mut left, mut right) = (vec![0.0_f32; 256], vec![0.0_f32; 256]);
    assert_eq!(common::audio_block_counts(), (0, 0));

    let (mut most_voices, mut loudest) = (0, 0.0_f32);
    {
        let _block = meloet_engine::rt::enter();
        for block in 0..8000_usize {
            for _ in 0..block % 4 {
                sampler.event(storm.next());
            }
            if block % 40 == 0 {
                let controller = [7, 10, 11][block / 40 % 3];
                let value = 40 + (block % 88) as u8;
                sampler.event(InstrumentEvent::Control { controller, value });
            }
            if block % 2000 == 1999 {
                sampler.reset();
            }
            let frames = [256, 1, 64, 200, 33][block % 5];
            sampler.render(&mut left[..frames], &mut right[..frames]);
            most_voices = most_voices.max(sampler.active_voices());
            loudest = left[..frames].iter().fold(loudest, |m, x| m.max(x.abs()));
        }
    }
    assert_eq!(common::audio_block_counts(), (0, 0));
    assert!(most_voices > 16, "{most_voices}");
    assert!(loudest > 0.1, "{loudest}");
}
