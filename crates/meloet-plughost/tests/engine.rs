//! The engine with real host processes in its tracks. These are tests of the engine
//! (`meloet_engine::PluginSlot`); they live here because only this crate's tests are handed
//! the path of the `meloet-plughost` binary.
//!
//! - A host process killed, crashed or frozen from outside while it plays: its slots stop at
//!   the second block, and the engine and the other host process go on. With the fake
//!   plugins, so this runs everywhere.
//! - `measure_…`: block times with Surge XT on 16 tracks and a compressor on each, for
//!   docs/measurements. Ignored, and does nothing unless `MELOET_MEASURE` is set.

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{FAKE, GEOMETRY, Host};
use meloet_engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineParts, InstrumentEvent,
    MixStrip, PLUGIN_WAIT_SHARE, PlaceholderSynth, PluginEffect, PluginInstrument, PluginSlot,
    PluginWatch, Sequence, SequenceEvent,
};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::Geometry;
use meloet_plugin::ipc::futex::monotonic_ns;

const FRAMES: usize = 256;
const BLOCK_NS: u64 = 5_333_333;

fn engine(tracks: usize) -> EngineParts {
    let mut parts = Engine::new(EngineConfig {
        sample_rate: 48_000,
        max_frames: FRAMES,
        max_tracks: tracks,
    });
    parts.engine.prepare(48_000);
    parts
}

/// Loads a plugin into a slot of the host and wraps the engine's end of it.
fn slot(
    host: &mut Host,
    parts: &EngineParts,
    slot: u32,
    key: &str,
    bundle: &str,
) -> (PluginSlot, Arc<PluginWatch>) {
    let key: PluginKey = key.parse().unwrap();
    let (end, loaded) = host.load_plugin(slot, &key, bundle).unwrap();
    PluginSlot::new(end, parts.handle.plugin_block().clone(), loaded.latency)
}

/// As [`slot`], with the plugin run for some blocks first, with all the time it wants: a
/// plugin's first blocks can take many times longer than its later ones (Surge XT reads its
/// patch in them), and the engine would count that as not answering.
fn warm_slot(
    host: &mut Host,
    parts: &EngineParts,
    slot: u32,
    key: &str,
    bundle: &str,
) -> (PluginSlot, Arc<PluginWatch>) {
    let key: PluginKey = key.parse().unwrap();
    let (mut end, loaded) = host.load_plugin(slot, &key, bundle).unwrap();
    for n in 0..50 {
        let events = match n {
            0 => vec![common::note_on(60)],
            40 => vec![common::note_off(60)],
            _ => vec![],
        };
        common::block(&mut end, &[0.0; FRAMES], &events);
    }
    PluginSlot::new(end, parts.handle.plugin_block().clone(), loaded.latency)
}

/// Sleeps until `at` on the monotonic clock.
fn sleep_until(at: u64) {
    let now = monotonic_ns();
    if at > now {
        std::thread::sleep(Duration::from_nanos(at - now));
    }
}

/// One block on the clock; how long the engine took for it, and its loudest sample.
fn block(parts: &mut EngineParts, out: &mut [f32], at: &mut u64) -> (u64, f32) {
    sleep_until(*at);
    *at += BLOCK_NS;
    let began = monotonic_ns();
    parts.engine.process(out, BlockTiming::default());
    let took = monotonic_ns() - began;
    (took, out.iter().fold(0.0, |m, x| m.max(x.abs())))
}

/// Track 0: a sine of host A. Track 1: this process's own synth through a pass-through of
/// host A. Track 2: a sine of host B. `signal` is sent to host A while all three sound.
fn a_host_process_stopped_from_outside(signal: i32) {
    let env = [(FAKE, "1"), ("MELOET_PLUGHOST_NO_DUMP", "1")];
    let mut a = Host::start_with(GEOMETRY, &env);
    let mut b = Host::start_with(GEOMETRY, &env);
    let mut parts = engine(4);
    let fake = |name: &str| format!("lv2:urn:meloet:fake:{name}");
    let (sine_a, watch_sine_a) = slot(&mut a, &parts, 0, &fake("sine"), "/nowhere/fake.lv2");
    let (pass_a, watch_pass_a) = slot(&mut a, &parts, 1, &fake("pass"), "/nowhere/fake.lv2");
    let (sine_b, watch_b) = slot(&mut b, &parts, 0, &fake("sine"), "/nowhere/fake.lv2");
    let handle = &mut parts.handle;
    assert!(
        handle
            .swap_instrument(0, Box::new(PluginInstrument(sine_a)))
            .is_ok()
    );
    assert!(
        handle
            .swap_instrument(1, Box::new(PlaceholderSynth::new()))
            .is_ok()
    );
    assert!(
        handle
            .swap_instrument(2, Box::new(PluginInstrument(sine_b)))
            .is_ok()
    );
    let pass = Some(Box::new(PluginEffect(pass_a)) as Box<dyn meloet_engine::Effect>);
    assert!(handle.set_effect(MixStrip::Track(1), 0, pass).is_ok());
    // One key held on each track, by its sequence.
    for track in 0..3 {
        let note = SequenceEvent {
            tick: 0,
            event: InstrumentEvent::NoteOn {
                note: 57 + 4 * track as u8,
                velocity: 100,
            },
        };
        assert!(
            handle
                .swap_sequence(track, Sequence::new(vec![note]))
                .is_ok()
        );
    }
    assert!(handle.send(EngineCommand::Play { from: 0 }).is_ok());

    let mut out = [0.0f32; FRAMES * 2];
    let mut at = monotonic_ns();
    for _ in 0..100 {
        let (_, peak) = block(&mut parts, &mut out, &mut at);
        assert!(peak <= 1.0);
    }
    let watches = [&watch_sine_a, &watch_pass_a];
    let unanswered = |watches: &[&Arc<PluginWatch>]| -> Vec<u64> {
        watches.iter().map(|w| w.unanswered()).collect()
    };
    // A busy PC may have cost a block already; what counts is what happens from here.
    let before = unanswered(&watches);
    let before_b = watch_b.unanswered();
    assert!(watches.iter().all(|w| !w.stopped()) && !watch_b.stopped());

    // SAFETY: a plain call on the child this test started.
    assert_eq!(unsafe { libc::kill(a.child.0.id() as i32, signal) }, 0);
    if signal == libc::SIGSEGV {
        // Rust's own handler (it looks for stack overflows) takes the first one that comes
        // from outside, puts the default back and returns; the second one is the crash.
        // Two of the same signal sent at once would arrive as one.
        std::thread::sleep(Duration::from_millis(3));
        // SAFETY: as above.
        unsafe { libc::kill(a.child.0.id() as i32, signal) };
        at = monotonic_ns();
    }
    let killed = Instant::now();
    // The first block that goes without an answer, then the block after it: the process is
    // given up on. A slot's own watch says so when the slot next runs, so one that ran in that
    // block before the process was given up on says it a block later; the control side takes
    // any one of a process's slots for all of them.
    let mut first_missed = None;
    let mut times = Vec::new();
    for n in 0..8 {
        let (took, _) = block(&mut parts, &mut out, &mut at);
        times.push(took);
        let missed = unanswered(&watches) != before;
        match first_missed {
            None if missed => {
                first_missed = Some(n);
                assert!(watches.iter().all(|w| !w.stopped()), "stopped at once");
            }
            Some(first) if n == first + 1 => {
                assert!(watches.iter().any(|w| w.stopped()), "block {n}");
                break;
            }
            _ => {}
        }
    }
    let first = first_missed.expect("the slots of the dead process never missed a block");
    assert!(first <= 2, "noticed only at block {first}");
    let noticed = killed.elapsed();
    assert!(noticed < Duration::from_millis(60), "{noticed:?}");
    // A block is waited for once, to the deadline all slots share: the slot that is first to
    // go without an answer waits twice (the block it missed, and once more in the next), the
    // other once. Which is first depends on where in a block the process went: before the
    // instrument's answer, or between that and the effect's.
    let after = unanswered(&watches);
    let missed: Vec<u64> = (before.iter().zip(&after)).map(|(b, a)| a - b).collect();
    assert!(missed == [2, 1] || missed == [1, 2], "{missed:?}");
    let (took, _) = block(&mut parts, &mut out, &mut at);
    assert!(watches.iter().all(|w| w.stopped()));
    assert_eq!(unanswered(&watches), after);
    assert!(took < BLOCK_NS / 2, "a stopped slot was waited for: {took}");
    let share_ns = (BLOCK_NS as f64 * PLUGIN_WAIT_SHARE) as u64;
    assert!(times[first] < share_ns + BLOCK_NS, "{times:?}");

    // The engine and the other host process go on: the sine of B and the synth of track 1
    // (now past the stopped effect) still sound, and the stopped slots cost no time.
    let mut loudest = 0.0f32;
    let began = Instant::now();
    for _ in 0..50 {
        let (_, peak) = block(&mut parts, &mut out, &mut at);
        loudest = loudest.max(peak);
    }
    assert!(began.elapsed() < Duration::from_millis(50 * 6 + 200));
    assert!(loudest > 0.3, "{loudest}");
    assert!(!watch_b.stopped());
    assert!(watch_b.unanswered() - before_b <= 2);
    assert_eq!(unanswered(&watches), after);

    // The engine lets go of its ends before the mappings go.
    drop(parts);
    // SAFETY: as above. A frozen process must be woken to die.
    unsafe { libc::kill(a.child.0.id() as i32, libc::SIGCONT) };
}

#[test]
fn a_host_process_killed_from_outside_stops_its_slots_at_the_second_block() {
    a_host_process_stopped_from_outside(libc::SIGKILL);
}

#[test]
fn a_host_process_crashed_from_outside_stops_its_slots_at_the_second_block() {
    a_host_process_stopped_from_outside(libc::SIGSEGV);
}

#[test]
fn a_host_process_frozen_from_outside_stops_its_slots_at_the_second_block() {
    a_host_process_stopped_from_outside(libc::SIGSTOP);
}

const SURGE: (&str, &str) = (
    "vst3:ABCDEF019182FAEB566D624153675854",
    "/usr/lib/vst3/Surge XT.vst3",
);
const LSP_COMP: (&str, &str) = (
    "vst3:647370206B31732020202020756E7363",
    "/usr/lib/vst3/lsp-plugins.vst3",
);
const ZAMCOMP: (&str, &str) = (
    "vst3:2046504473616C63504D435A00000000",
    "/usr/lib/vst3/ZamComp.vst3",
);

fn percentile(sorted: &[u64], p: f64) -> f64 {
    let at = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[at] as f64 / 1000.0
}

/// One run: 16 tracks, Surge XT on each (all in one host process), `effect` on each (all in
/// another), every track playing chords round a loop. Prints one line of JSON.
fn measure(name: &str, effect: (&str, &str), engine_fifo: bool, host_rt: &str, staggered: bool) {
    const TRACKS: usize = 16;
    const BLOCKS: usize = 4000;
    let geometry = Geometry {
        max_frames: FRAMES as u32,
        channels: 2,
        slots: TRACKS as u32,
        to_plugin_events: 256,
        from_plugin_events: 64,
    };
    let env = [("MELOET_PLUGHOST_RT", host_rt)];
    let mut synths = Host::start_with(geometry, &env);
    let mut effects = Host::start_with(geometry, &env);
    let mut parts = engine(TRACKS);
    let mut watches = Vec::new();
    for track in 0..TRACKS {
        let (synth, synth_watch) = warm_slot(&mut synths, &parts, track as u32, SURGE.0, SURGE.1);
        let (comp, comp_watch) = warm_slot(&mut effects, &parts, track as u32, effect.0, effect.1);
        let handle = &mut parts.handle;
        assert!(
            handle
                .swap_instrument(track, Box::new(PluginInstrument(synth)))
                .is_ok()
        );
        let comp = Some(Box::new(PluginEffect(comp)) as Box<dyn meloet_engine::Effect>);
        assert!(handle.set_effect(MixStrip::Track(track), 0, comp).is_ok());
        // A chord of three keys on every beat, let go half a beat later. `staggered`: each
        // track a few ticks after the one before, as hands would play.
        let shift = if staggered { track as u64 * 7 } else { 0 };
        let mut events = Vec::new();
        for beat in 0..4u64 {
            for key in [48, 55, 64] {
                let note = key + (track % 12) as u8;
                events.push(SequenceEvent {
                    tick: beat * 480 + shift,
                    event: InstrumentEvent::NoteOn { note, velocity: 90 },
                });
                events.push(SequenceEvent {
                    tick: beat * 480 + 240 + shift,
                    event: InstrumentEvent::NoteOff { note },
                });
            }
        }
        assert!(handle.swap_sequence(track, Sequence::new(events)).is_ok());
        watches.push(synth_watch);
        watches.push(comp_watch);
        // The fades of the swaps end, and the commands fit their ring.
        let mut out = [0.0f32; FRAMES * 2];
        parts.engine.process(&mut out, BlockTiming::default());
        while parts.handle.take_retired().is_some() || parts.handle.take_retired_effect().is_some()
        {
        }
    }
    let handle = &mut parts.handle;
    let looped = EngineCommand::SetLoop {
        start: 0,
        end: 1920,
        on: true,
    };
    assert!(handle.send(looped).is_ok());
    assert!(handle.send(EngineCommand::Play { from: 0 }).is_ok());
    let _ = Chord::new(&[]);

    if engine_fifo {
        let param = libc::sched_param { sched_priority: 30 };
        // SAFETY: a plain call; 0 is the calling thread.
        let set = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) };
        assert_eq!(set, 0, "no SCHED_FIFO for the measuring thread");
    }
    let mut out = [0.0f32; FRAMES * 2];
    let mut at = monotonic_ns();
    let (mut times, mut waits) = (Vec::new(), Vec::new());
    let (mut loudest, mut over, mut unanswered_at_start) = (0.0f32, 0, 0);
    // The block (counted from the first one measured) at which a slot was first stopped.
    let mut first_stop = None;
    let waited = |watches: &[Arc<PluginWatch>]| watches.iter().map(|w| w.waited_ns()).sum::<u64>();
    let missed = |watches: &[Arc<PluginWatch>]| watches.iter().map(|w| w.unanswered()).sum::<u64>();
    for n in 0..BLOCKS + 200 {
        let before = waited(&watches);
        let (took, peak) = block(&mut parts, &mut out, &mut at);
        if n == 200 {
            // The first blocks (pages touched for the first time) are left out.
            times.clear();
            waits.clear();
            over = 0;
            unanswered_at_start = missed(&watches);
        }
        times.push(took);
        waits.push(waited(&watches) - before);
        if first_stop.is_none() && watches.iter().any(|w| w.stopped()) {
            first_stop = Some(n as i64 - 200);
        }
        over += usize::from(took > BLOCK_NS);
        loudest = loudest.max(peak);
        // A block that ran over must not make the next ones late too.
        at = at.max(monotonic_ns());
    }
    if engine_fifo {
        let param = libc::sched_param { sched_priority: 0 };
        // SAFETY: as above.
        unsafe { libc::sched_setscheduler(0, libc::SCHED_OTHER, &param) };
    }
    let stopped = watches.iter().filter(|w| w.stopped()).count();
    // Synths and effects take turns in `watches`.
    let synths_stopped = watches.iter().step_by(2).filter(|w| w.stopped()).count();
    let first_stop = first_stop.map_or("null".to_string(), |n| n.to_string());
    times.sort_unstable();
    waits.sort_unstable();
    println!(
        "{{\"run\":\"{name}\",\"engineFifo\":{engine_fifo},\"hostRt\":\"{host_rt}\",\
         \"staggered\":{staggered},\"waitShare\":{PLUGIN_WAIT_SHARE},\"blocks\":{},\
         \"blockUs\":{{\"p50\":{:.0},\"p99\":{:.0},\"p999\":{:.0},\"max\":{:.0}}},\
         \"waitedUs\":{{\"p50\":{:.0},\"p99\":{:.0},\"p999\":{:.0},\"max\":{:.0}}},\
         \"overOneBlock\":{over},\"unanswered\":{},\"stoppedSlots\":{stopped},\"stoppedSynths\":{synths_stopped},\
         \"firstStopBlock\":{first_stop},\"peak\":{loudest:.3}}}",
        times.len(),
        percentile(&times, 0.5),
        percentile(&times, 0.99),
        percentile(&times, 0.999),
        percentile(&times, 1.0),
        percentile(&waits, 0.5),
        percentile(&waits, 0.99),
        percentile(&waits, 0.999),
        percentile(&waits, 1.0),
        missed(&watches) - unanswered_at_start,
    );
    drop(parts);
}

#[test]
#[ignore = "a measurement: needs Surge XT, LSP and Zam plugins, a quiet PC, and MELOET_MEASURE=1"]
fn measure_sixteen_tracks_of_surge_xt_with_a_compressor_each() {
    if std::env::var_os("MELOET_MEASURE").is_none() {
        eprintln!("MELOET_MEASURE is not set; nothing measured");
        return;
    }
    for (effect_name, effect) in [("lsp", LSP_COMP), ("zam", ZAMCOMP)] {
        for staggered in [false, true] {
            for (mode, engine_fifo, host_rt) in [
                ("fifo", true, ""),
                ("engine-only", true, "off"),
                ("other", false, "off"),
            ] {
                let name = format!("{effect_name}-{mode}");
                measure(&name, effect, engine_fifo, host_rt, staggered);
            }
        }
    }
}
