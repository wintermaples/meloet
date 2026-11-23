//! The audio thread's rule for the knobs of the sounds that come with meloet: moving them by
//! hand and by lane while playing never allocates or frees. One test only, so nothing else
//! runs in this binary while the counts are taken.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::{Instrument, SongPlace};
use meloet_sound::knobs::Knobbed;
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};
use support::pack::{Storm, everything};

#[test]
fn moving_the_knobs_does_not_allocate() {
    let pack = everything("knobs-rt");
    let mut all: Vec<Box<dyn Instrument>> = vec![
        Box::new(Knobbed::new(PolySynth::lead())),
        Box::new(Knobbed::new(PolySynth::bass())),
        Box::new(Knobbed::new(PolySynth::pad())),
        Box::new(Knobbed::new(SineSynth::new())),
        Box::new(Knobbed::new(ElectroKit::new())),
        Box::new(Knobbed::new(pack.sampler(48_000.0, 256))),
    ];
    for inst in all.iter_mut() {
        inst.prepare(48_000.0, 256);
    }
    let (mut left, mut right) = (vec![0.0_f32; 256], vec![0.0_f32; 256]);
    let mut storm = Storm::new(3);
    let before = common::audio_block_counts();
    let mut loudest = 0.0_f32;
    {
        let _block = meloet_engine::rt::enter();
        for block in 0..3000_usize {
            for inst in all.iter_mut() {
                for _ in 0..block % 3 {
                    inst.event(storm.next());
                }
                if block % 4 == 0 {
                    inst.song_note_on(60 + (block % 12) as u8, 100, SongPlace::new(7, &[]));
                }
                let knob = (block % 5) as u32;
                let place = (block % 101) as f32 / 100.0;
                inst.set_knob(knob, if block % 97 == 0 { f32::NAN } else { place });
                for point in 0..(block % 40) as u32 {
                    inst.set_param_at(point * 6, point % 4, (point as f32 * 0.13).fract());
                }
                if block % 1000 == 999 {
                    inst.reset();
                }
                let frames = [256, 1, 64, 200, 33][block % 5];
                inst.render(&mut left[..frames], &mut right[..frames]);
                loudest = left[..frames].iter().fold(loudest, |m, x| m.max(x.abs()));
            }
        }
    }
    assert_eq!(common::audio_block_counts(), before);
    assert!(loudest > 0.05, "{loudest}");
}
