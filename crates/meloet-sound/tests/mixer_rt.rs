//! The audio thread's rule with the whole mixer on the real engine: sixteen tracks with four
//! effects each, the returns, the master, effects going in and out, parameters and faders
//! dragged. One test only, so nothing else runs in this binary while the counts are taken.

#[path = "../../meloet-engine/tests/common/mod.rs"]
mod common;
mod support;

use meloet_engine::{EngineCommand, INSERT_SLOTS, MixStrip};
use meloet_model::effects::EffectKind;
use meloet_sound::effects::create;
use support::mix::{Song, TRACKS, gain};

#[test]
fn the_whole_mixer_never_touches_the_allocator() {
    use EffectKind as K;
    let mut song = Song::new(&[K::Eq4, K::Compressor, K::Chorus, K::Delay], true);
    for bus in 0..2 {
        for (slot, kind) in [K::Eq4, K::Compressor, K::Chorus].into_iter().enumerate() {
            song.effect(MixStrip::Return(bus), slot + 1, Some(create(kind)));
        }
    }
    for (slot, kind) in [K::Eq4, K::Compressor, K::Reverb].into_iter().enumerate() {
        // The limiter moves to the last slot: nothing after it may pass its ceiling.
        song.effect(MixStrip::Master, slot, Some(create(kind)));
    }
    song.effect(MixStrip::Master, 3, Some(create(K::Limiter)));
    let ceiling = gain(-0.3);

    let strips: Vec<MixStrip> = (0..TRACKS)
        .map(MixStrip::Track)
        .chain([MixStrip::Return(0), MixStrip::Return(1), MixStrip::Master])
        .collect();
    let mut seed = 99u32;
    let mut next = move |n: usize| {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as usize % n
    };
    // What sits in each slot, to give parameters that the effect has.
    let mut kinds = vec![[None; INSERT_SLOTS]; strips.len()];
    let (mut loudest, mut blocks) = (0.0f32, 0);
    for round in 0..1500usize {
        let at = next(strips.len());
        let strip = strips[at];
        let slot = next(INSERT_SLOTS);
        let last_of_master = strip == MixStrip::Master && slot == 3;
        match next(10) {
            // A drag: a new value every block.
            0..=2 => song.send(EngineCommand::SetStrip {
                strip,
                gain: gain(next(67) as f32 - 60.0),
                pan: next(201) as f32 / 100.0 - 1.0,
            }),
            3 => song.send(EngineCommand::SetSends {
                track: next(TRACKS),
                sends: [gain(next(67) as f32 - 60.0), gain(next(67) as f32 - 60.0)],
            }),
            4..=6 => {
                let kind: EffectKind = kinds[at][slot].unwrap_or(K::Reverb);
                let params = kind.params();
                let param = next(params.len());
                let spec = &params[param];
                let value = match next(6) {
                    0 => f32::NAN,
                    n => spec.min + (spec.max - spec.min) * (n - 1) as f32 / 4.0,
                };
                if !last_of_master {
                    song.send(EngineCommand::SetEffectParam {
                        strip,
                        slot,
                        param: param as u32,
                        value,
                    });
                }
            }
            7 if !last_of_master => {
                let kind = K::ALL[next(K::ALL.len())];
                kinds[at][slot] = Some(kind);
                song.effect(strip, slot, Some(create(kind)));
            }
            8 if !last_of_master && round % 3 == 0 => {
                kinds[at][slot] = None;
                song.effect(strip, slot, None);
            }
            _ => {}
        }
        if round % 50 == 25 {
            // A tempo map with a ramp and a meter change, made here and swapped in by the
            // engine while the song plays (the delay follows the tempo it is told).
            use meloet_model::{Meter, MeterChange, Tempo, TempoMap, TempoPoint, Timeline};
            let tempo = |centi: u32| Tempo::from_centi(centi).unwrap();
            let timeline = Timeline {
                tempo: tempo(9_000 + (round as u32 % 7) * 1_000),
                meter: Meter::FOUR_FOUR,
                head_ramp: true,
                tempo_points: vec![TempoPoint {
                    tick: 4_000,
                    tempo: tempo(15_000),
                    ramp: false,
                }],
                meter_changes: vec![MeterChange {
                    bar: 1,
                    meter: Meter::new(6, 8).unwrap(),
                }],
            };
            let map = std::sync::Arc::new(TempoMap::new(&timeline));
            assert!(song.parts.handle.set_tempo_map(map).is_ok());
        }
        let frames = [256, 64, 1024, 333, 2048, 1][round % 6];
        let out = song.block(frames);
        assert!(out.iter().all(|s| s.is_finite()), "round {round}");
        loudest = out.iter().fold(loudest, |m, s| m.max(s.abs()));
        blocks += 1;
        // The control side's job, outside the audio block: effects that came back are dropped.
        song.parts.handle.drop_retired();
    }
    assert_eq!(blocks, 1500);
    assert!(loudest > 0.01, "the run made sound: {loudest}");
    assert!(loudest <= ceiling * 1.0001, "over the ceiling: {loudest}");
    assert_eq!(common::audio_block_counts(), (0, 0));
}
