//! A plugin slot where an instrument and an effect go (docs/architecture.md 20-2), with the
//! plugin's end of the shared memory on a thread of this process. The audio thread's side
//! allocates nothing, also while a slot stops.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::thread::JoinHandle;

use meloet_engine::{
    BlockTiming, Effect, EffectContext, Engine, EngineCommand, EngineConfig, EngineParts,
    Instrument, InstrumentEvent, MixStrip, PluginBlock, PluginEffect, PluginInstrument, PluginSlot,
};
use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{
    Bell, EngineEnd, FromPlugin, Geometry, Mapping, PluginEnd, ToPlugin, block_flags,
};

const GEOMETRY: Geometry = Geometry {
    max_frames: 256,
    channels: 2,
    slots: 2,
    to_plugin_events: 64,
    from_plugin_events: 16,
};
/// Long blocks, so that their deadline (most of a block) is far away on a busy PC.
const DEVICE_FRAMES: usize = 9600;

struct FakeHost {
    quit: Arc<AtomicBool>,
    /// The notes held, as the plugin's end heard them.
    held: Arc<AtomicU32>,
    resets: Arc<AtomicU32>,
    map: Arc<Mapping>,
    thread: Option<JoinHandle<()>>,
}

impl FakeHost {
    /// Slot 0 is an instrument: 0.1 on the left and 0.2 on the right for every key held.
    /// Slot 1 is an effect: the input times its parameter 3 (0.5 at first), a NaN when the
    /// parameter is 1. Both say a latency of 32 frames with their first block.
    fn start(map: &Arc<Mapping>) -> FakeHost {
        let quit = Arc::new(AtomicBool::new(false));
        let held = Arc::new(AtomicU32::new(0));
        let resets = Arc::new(AtomicU32::new(0));
        let thread = {
            let (map, quit, held, resets) =
                (map.clone(), quit.clone(), held.clone(), resets.clone());
            std::thread::spawn(move || {
                let bell = Bell::new(map.clone());
                let mut instrument = PluginEnd::new(map.clone(), 0).unwrap();
                let mut effect = PluginEnd::new(map, 1).unwrap();
                let (mut left, mut right) = ([0.0f32; 256], [0.0f32; 256]);
                let mut factor = 0.5f32;
                let mut first = [true; 2];
                while !quit.load(Relaxed) {
                    let seen = bell.count();
                    let mut worked = false;
                    if let Some(block) = instrument.take_request() {
                        worked = true;
                        if block.flags & block_flags::RESET != 0 {
                            held.store(0, Relaxed);
                            resets.fetch_add(1, Relaxed);
                        }
                        while let Some(event) = instrument.pop_event() {
                            match event {
                                ToPlugin::Midi {
                                    data: [0x90, _, 1..=127],
                                    ..
                                } => held.fetch_add(1, Relaxed),
                                ToPlugin::Midi {
                                    data: [0x80, ..], ..
                                } => held.fetch_sub(1, Relaxed),
                                _ => 0,
                            };
                        }
                        let keys = held.load(Relaxed) as f32;
                        left[..block.frames].fill(0.1 * keys);
                        right[..block.frames].fill(0.2 * keys);
                        instrument.write_output(0, &left[..block.frames]);
                        instrument.write_output(1, &right[..block.frames]);
                        if std::mem::take(&mut first[0]) {
                            instrument.push_event(FromPlugin::Latency { frames: 32 });
                        }
                        instrument.finish();
                    }
                    if let Some(block) = effect.take_request() {
                        worked = true;
                        while let Some(event) = effect.pop_event() {
                            if let ToPlugin::Param { id: 3, value, .. } = event {
                                factor = value as f32;
                            }
                        }
                        let factor = if factor == 1.0 { f32::NAN } else { factor };
                        for channel in 0..2 {
                            effect.read_input(channel, &mut left[..block.frames]);
                            left[..block.frames].iter_mut().for_each(|x| *x *= factor);
                            effect.write_output(channel, &left[..block.frames]);
                        }
                        if std::mem::take(&mut first[1]) {
                            effect.push_event(FromPlugin::Latency { frames: 32 });
                        }
                        effect.finish();
                    }
                    if !worked {
                        bell.wait(seen, 20_000_000);
                    }
                }
            })
        };
        FakeHost {
            quit,
            held,
            resets,
            map: map.clone(),
            thread: Some(thread),
        }
    }

    /// The host process is gone: nothing answers any more.
    fn stop(&mut self) {
        self.quit.store(true, Relaxed);
        Bell::new(self.map.clone()).ring();
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

impl Drop for FakeHost {
    fn drop(&mut self) {
        self.stop();
    }
}

fn slot(
    map: &Arc<Mapping>,
    slot: u32,
    block: &Arc<PluginBlock>,
) -> (PluginSlot, Arc<meloet_engine::PluginWatch>) {
    PluginSlot::new(EngineEnd::new(map.clone(), slot).unwrap(), block.clone(), 0)
}

#[test]
fn a_plugin_sits_where_an_instrument_and_an_effect_go() {
    let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
    let EngineParts {
        mut engine,
        mut handle,
        ..
    } = Engine::new(EngineConfig {
        sample_rate: 48_000,
        max_frames: 256,
        max_tracks: 2,
    });
    engine.prepare(48_000);
    let block = handle.plugin_block().clone();
    let (instrument, instrument_watch) = slot(&map, 0, &block);
    let (effect, effect_watch) = slot(&map, 1, &block);
    let mut host = FakeHost::start(&map);

    handle
        .swap_instrument(0, Box::new(PluginInstrument(instrument)))
        .ok()
        .unwrap();
    handle
        .set_effect(MixStrip::Track(0), 0, Some(Box::new(PluginEffect(effect))))
        .ok()
        .unwrap();
    handle
        .send(EngineCommand::SetLookahead {
            track: 0,
            frames: 64,
        })
        .ok()
        .unwrap();
    handle
        .send(EngineCommand::Audition {
            track: 0,
            keys: meloet_engine::Chord::new(&[60, 64]),
            velocity: 100,
        })
        .ok()
        .unwrap();
    let mut out = vec![0.0f32; DEVICE_FRAMES * 2];
    engine.process(&mut out, BlockTiming::default());
    // Two keys through the instrument (0.2 and 0.4), halved by the effect; the fades of the
    // first block are over at its end.
    let last = &out[out.len() - 2..];
    assert!(
        (last[0] - 0.1).abs() < 1e-6 && (last[1] - 0.2).abs() < 1e-6,
        "{last:?}"
    );
    assert_eq!(host.held.load(Relaxed), 2);
    assert_eq!(instrument_watch.latency(), 32);
    assert_eq!(effect_watch.latency(), 32);

    // A parameter through the effect's ring; then one that makes the plugin give NaN, which
    // becomes silence and not a NaN in the mix.
    handle
        .send(EngineCommand::SetEffectParam {
            strip: MixStrip::Track(0),
            slot: 0,
            param: 3,
            value: 0.25,
        })
        .ok()
        .unwrap();
    engine.process(&mut out, BlockTiming::default());
    let last = &out[out.len() - 2..];
    assert!(
        (last[0] - 0.05).abs() < 1e-6 && (last[1] - 0.1).abs() < 1e-6,
        "{last:?}"
    );
    handle
        .send(EngineCommand::SetEffectParam {
            strip: MixStrip::Track(0),
            slot: 0,
            param: 3,
            value: 1.0,
        })
        .ok()
        .unwrap();
    engine.process(&mut out, BlockTiming::default());
    assert!(out.iter().all(|x| x.is_finite()));
    assert_eq!(&out[out.len() - 2..], [0.0, 0.0]);
    handle
        .send(EngineCommand::SetEffectParam {
            strip: MixStrip::Track(0),
            slot: 0,
            param: 3,
            value: 0.5,
        })
        .ok()
        .unwrap();
    engine.process(&mut out, BlockTiming::default());
    assert!(!instrument_watch.stopped() && !effect_watch.stopped());
    assert_eq!(
        (instrument_watch.unanswered(), effect_watch.unanswered()),
        (0, 0)
    );

    // The host process goes. The engine goes on: the first stretch waits out its deadline,
    // and from the next one on both slots are stopped and cost no time.
    host.stop();
    let began = monotonic_ns();
    engine.process(&mut out, BlockTiming::default());
    engine.process(&mut out, BlockTiming::default());
    let took = monotonic_ns() - began;
    assert!(instrument_watch.stopped() && effect_watch.stopped());
    assert_eq!(
        (instrument_watch.unanswered(), effect_watch.unanswered()),
        (1, 1)
    );
    assert!(out.iter().all(|x| *x == 0.0));
    // One deadline (most of a block of 0.2 s, shared), not one for every stretch or slot.
    assert!(took < 390_000_000, "{took}");

    assert_eq!(common::audio_block_counts(), (0, 0));
    drop(engine);
    while handle.take_retired().is_some() || handle.take_retired_effect().is_some() {}
}

#[test]
fn a_stopped_effect_lets_the_sound_through_and_a_reset_reaches_the_plugin() {
    let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
    let block = Arc::new(PluginBlock::default());
    let (instrument, _) = slot(&map, 0, &block);
    let (effect, watch) = slot(&map, 1, &block);
    let (mut instrument, mut effect) = (PluginInstrument(instrument), PluginEffect(effect));
    let mut host = FakeHost::start(&map);
    let ctx = EffectContext { tempo_bpm: 120.0 };
    let patient = || block.set_deadline(monotonic_ns() + 5_000_000_000);
    let (mut left, mut right) = (vec![0.0f32; 600], vec![0.0f32; 600]);

    {
        let _audio = meloet_engine::rt::enter();
        // Longer than the slot's block: rendered in pieces.
        patient();
        instrument.event(InstrumentEvent::NoteOn {
            note: 60,
            velocity: 100,
        });
        instrument.event(InstrumentEvent::PitchBend { value: -8192 });
        instrument.event(InstrumentEvent::Control {
            controller: 64,
            value: 127,
        });
        instrument.render(&mut left, &mut right);
        assert!(left.iter().all(|x| *x == 0.1) && right.iter().all(|x| *x == 0.2));
        effect.process(&mut left, &mut right, &ctx);
        assert!(left.iter().all(|x| *x == 0.05) && right.iter().all(|x| *x == 0.1));

        patient();
        instrument.reset();
        instrument.render(&mut left[..64], &mut right[..64]);
        assert!(left[..64].iter().all(|x| *x == 0.0));
    }
    assert_eq!(host.resets.load(Relaxed), 1);

    host.stop();
    {
        let _audio = meloet_engine::rt::enter();
        left.fill(0.7);
        right.fill(-0.7);
        for round in 0..3 {
            block.set_deadline(monotonic_ns() + 3_000_000);
            effect.process(&mut left, &mut right, &ctx);
            assert!(
                left.iter().all(|x| *x == 0.7) && right.iter().all(|x| *x == -0.7),
                "round {round}"
            );
            instrument.render(&mut left[..10], &mut right[..10]);
            assert!(left[..10].iter().all(|x| *x == 0.0));
            left.fill(0.7);
            right.fill(-0.7);
        }
        effect.set_param(3, 0.25);
        effect.reset();
    }
    assert!(watch.stopped());
    assert_eq!(watch.unanswered(), 1);
    assert_eq!(common::audio_block_counts(), (0, 0));
}
