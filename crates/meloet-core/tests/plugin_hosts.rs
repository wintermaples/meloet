//! The plugin processes with the real `meloet-plughost`, its fake plugins and a real engine
//! (no device: the test calls the engine's blocks itself, on the clock where time matters).
//! Every process is gone when its test ends.

mod plughost;

use std::process::Command;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use meloet_core::plugins::hosts::{
    DEFAULT_GEOMETRY, HostError, HostNews, HostOptions, Hosts, LoadSpec, Loaded, SlotId, wait,
};
use meloet_engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineParts, InstrumentEvent, MidiIn,
    MixStrip, PluginEffect, PluginInstrument, Recorded, TrackSet, monotonic_us,
};
use meloet_plugin::PluginKey;
use meloet_plugin::ipc::HANG_MS;
use meloet_plugin::wire::{OutputSetting, OutputSettings};

use plughost::{alive, wait_for};

const RATE: u32 = 48_000;
/// A device block long enough that its deadline for plugins (half of it) is far away on a
/// busy PC: for the tests that are not about time.
const LONG: usize = 9600;
const PATIENCE: Duration = Duration::from_secs(20);

struct Rig {
    parts: EngineParts,
    out: Vec<f32>,
}

impl Rig {
    fn new(block: usize) -> Rig {
        let mut parts = Engine::new(EngineConfig {
            sample_rate: RATE,
            max_frames: 256,
            max_tracks: 2,
        });
        parts.engine.prepare(RATE);
        Rig {
            parts,
            out: vec![0.0; block * 2],
        }
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    fn keys(&mut self, track: usize, keys: &[u8]) {
        self.send(EngineCommand::Audition {
            track,
            keys: Chord::new(keys),
            velocity: 100,
        });
    }

    fn instrument(&mut self, track: usize, loaded: Loaded) {
        let instrument = Box::new(PluginInstrument(loaded.slot));
        assert!(self.parts.handle.swap_instrument(track, instrument).is_ok());
    }

    /// One device block, now. The peak of the second half of its left channel (the first
    /// half may hold a fade).
    fn block(&mut self) -> f32 {
        let now = monotonic_us();
        let timing = BlockTiming {
            callback_us: now,
            playback_us: now + 10_000,
        };
        self.parts.engine.process(&mut self.out, timing);
        self.parts.handle.drop_retired();
        let frames = self.out.len() / 2;
        (frames / 2..frames).fold(0.0, |peak, f| self.out[f * 2].abs().max(peak))
    }

    fn block_time(&self) -> Duration {
        Duration::from_secs_f64(self.out.len() as f64 / 2.0 / f64::from(RATE))
    }
}

struct Side {
    hosts: Hosts,
    news: Receiver<(Instant, HostNews)>,
}

fn side() -> Side {
    let (tx, news) = mpsc::channel();
    let options = HostOptions {
        exe: plughost::exe(),
        env: plughost::fake_env(),
        geometry: DEFAULT_GEOMETRY,
    };
    let hosts = Hosts::start(options, move |news| {
        let _ = tx.send((Instant::now(), news));
    });
    Side { hosts, news }
}

fn key(fake: &str) -> PluginKey {
    PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap()
}

/// The fake instrument with four output sets of the plugin host: "a" a stereo pair, "b"
/// mono in the middle, "c" mono on the left, all three on, and "d" mono and off.
const FOUR_OUTPUTS: &str = "outs";

/// Each of `ids` switched off.
fn all_off(ids: &[&str]) -> OutputSettings {
    OutputSettings {
        rule: 1,
        sets: (ids.iter())
            .map(|id| OutputSetting {
                id: (*id).into(),
                gain_db: 0.0,
                pan: None,
                on: Some(false),
            })
            .collect(),
    }
}

impl Side {
    fn load(&self, rig: &Rig, fake: &str) -> Loaded {
        self.load_with(rig, fake, OutputSettings::default())
    }

    fn load_with(&self, rig: &Rig, fake: &str, outputs: OutputSettings) -> Loaded {
        let spec = LoadSpec {
            key: key(fake),
            bundle: "/nowhere/fake.lv2".into(),
            sample_rate: RATE,
            outputs,
        };
        let block = rig.parts.handle.plugin_block().clone();
        wait(|done| self.hosts.load(spec, block, done)).unwrap()
    }

    fn reload(&self, rig: &Rig, id: SlotId) -> Result<Loaded, HostError> {
        let block = rig.parts.handle.plugin_block().clone();
        wait(|done| self.hosts.reload(id, RATE, block, done))
    }

    fn level(&self, id: SlotId) -> f64 {
        wait(|done| self.hosts.param(id, 0, done)).unwrap().0
    }

    fn set_level(&self, id: SlotId, level: f64) {
        wait(|done| self.hosts.set_param(id, 0, level, done)).unwrap();
    }

    fn pids(&self) -> Vec<u32> {
        self.hosts.processes().iter().map(|p| p.pid).collect()
    }

    /// The next `Stopped`, and when it was told.
    fn stopped(&self) -> (Instant, Vec<SlotId>) {
        loop {
            if let (at, HostNews::Stopped { slots, .. }) =
                self.news.recv_timeout(PATIENCE).expect("a plugin stopped")
            {
                return (at, slots);
            }
        }
    }
}

fn kill(pid: u32) {
    let done = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap();
    assert!(done.success());
}

fn gone(pids: &[u32]) -> bool {
    wait_for(Duration::from_secs(5), || {
        pids.iter().all(|&pid| !alive(pid)).then_some(())
    })
    .is_some()
}

fn near(got: f32, want: f32) -> bool {
    (got - want).abs() < 0.02
}

#[test]
fn a_fake_instrument_sounds_and_plugins_of_one_key_share_a_process() {
    let mut rig = Rig::new(LONG);
    let side = side();
    let sine = side.load(&rig, "sine");
    let (first, watch) = (sine.id, sine.watch.clone());
    assert_eq!(sine.plugin.audio_outputs, 2);
    rig.instrument(0, sine);
    assert_eq!(rig.block(), 0.0);
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.5), "the sine at its level");
    assert_eq!(watch.unanswered(), 0);

    // A parameter over the socket, and the plugin's state.
    let params = wait(|done| side.hosts.params(first, done)).unwrap();
    assert_eq!(params.len(), 1);
    side.set_level(first, 0.25);
    rig.block();
    assert!(near(rig.block(), 0.25));
    assert!((side.level(first) - 0.25).abs() < 1e-9);
    let state = wait(|done| side.hosts.state(first, done)).unwrap();
    side.set_level(first, 1.0);
    wait(|done| side.hosts.set_state(first, state, done)).unwrap();
    assert!((side.level(first) - 0.25).abs() < 1e-9);

    // The same key goes into the same process; another key gets its own.
    let second = side.load(&rig, "sine");
    let second_id = second.id;
    rig.instrument(1, second);
    let pass = side.load(&rig, "pass");
    let pass_id = pass.id;
    let effect = Box::new(PluginEffect(pass.slot));
    assert!(
        rig.parts
            .handle
            .set_effect(MixStrip::Track(0), 0, Some(effect))
            .is_ok()
    );
    let processes = side.hosts.processes();
    assert_eq!(processes.len(), 2);
    assert_eq!(processes[0].key, key("sine"));
    assert_eq!(processes[0].slots, [first, second_id]);
    assert_eq!(processes[1].slots, [pass_id]);
    let pids = side.pids();
    assert!(pids.iter().all(|&pid| alive(pid)));
    rig.keys(1, &[60]);
    rig.block();
    // Track 0 at 0.25 through the effect that passes it on, and track 1 at 0.5.
    let both = rig.block();
    assert!(both > 0.4 && both <= 0.77, "{both}");
    assert!(!watch.stopped());

    // Out of the engine first, then out of the process. A process without plugins ends.
    assert!(
        rig.parts
            .handle
            .set_effect(MixStrip::Track(0), 0, None)
            .is_ok()
    );
    rig.send(EngineCommand::ClearTrack { track: 1 });
    rig.block();
    rig.block();
    wait(|done| side.hosts.unload(pass_id, done)).unwrap();
    assert!(gone(&pids[1..]));
    wait(|done| side.hosts.unload(second_id, done)).unwrap();
    assert_eq!(side.pids(), [pids[0]]);
    assert_eq!(
        wait(|done| side.hosts.unload(second_id, done)),
        Err(HostError::UnknownSlot)
    );
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.25), "the first plugin is untouched");
    assert!(side.news.try_recv().is_err(), "nothing stopped");

    // Ending the hosts ends the processes.
    drop(side);
    assert!(gone(&pids));
}

#[derive(Debug, Clone, Copy)]
enum Fault {
    Kill,
    Segv,
    Hang,
}

/// A plugin on track 0 fails while the engine runs on the clock. Returns how many device
/// blocks after the failure the engine gave up on it, and how long after the failure the
/// stop was told.
fn fails(fault: Fault, block: usize, strict: bool) -> (u32, Duration) {
    let mut rig = Rig::new(block);
    let side = side();
    let fake = match fault {
        Fault::Kill => "sine",
        Fault::Segv => "segv",
        Fault::Hang => "hang",
    };
    let loaded = side.load(&rig, fake);
    let (id, watch) = (loaded.id, loaded.watch.clone());
    rig.instrument(0, loaded);
    let pid = side.pids()[0];
    let step = rig.block_time();
    let mut next = Instant::now();
    let mut tick = |rig: &mut Rig| {
        let peak = rig.block();
        next += step;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
        peak
    };
    for _ in 0..30 {
        tick(&mut rig);
    }
    if strict {
        assert!(!watch.stopped(), "stopped before it failed");
    }

    // The two fake faults happen inside the block that brings the plugin its first note.
    match fault {
        Fault::Kill => kill(pid),
        Fault::Segv | Fault::Hang => rig.keys(0, &[60]),
    }
    let failed = Instant::now();
    let mut blocks = 0;
    let mut gave_up = None;
    let told = loop {
        tick(&mut rig);
        blocks += 1;
        if gave_up.is_none() && watch.stopped() {
            gave_up = Some(blocks);
        }
        if let Ok((at, HostNews::Stopped { slots, .. })) = side.news.try_recv() {
            assert_eq!(slots, [id]);
            break at.duration_since(failed);
        }
        assert!(blocks < 2000, "{fault:?}: never told");
    };
    // The process is gone by the time it is told, also the one that hung.
    assert!(!alive(pid), "{fault:?}: the process is still there");
    assert!(side.hosts.processes().is_empty());
    // The engine goes on, with silence on the track. A process that ended may be told
    // before the engine has given up on its slot; it does within the next blocks.
    assert_eq!(tick(&mut rig), 0.0);
    tick(&mut rig);
    assert!(watch.stopped(), "{fault:?}: the engine still waits for it");
    (gave_up.unwrap_or(blocks), told)
}

#[test]
fn a_plugin_that_is_killed_crashes_or_hangs_is_told_and_its_process_is_gone() {
    for fault in [Fault::Kill, Fault::Segv, Fault::Hang] {
        // 1024 frames (21.3 ms): room for a busy PC. The slot is asked in pieces of 256
        // frames, so its two unanswered round trips fit one device block.
        let (blocks, told) = fails(fault, 1024, true);
        eprintln!(
            "{fault:?} at 1024 frames: given up after {blocks} block(s), told after {told:?}"
        );
        if let Fault::Hang = fault {
            // A plugin at work on one block for long (reading a patch) is not a dead one;
            // only after `HANG_MS` it is.
            let hang = Duration::from_millis(u64::from(HANG_MS));
            assert!(
                told >= hang && told < hang + Duration::from_millis(500),
                "{told:?}"
            );
            continue;
        }
        assert!(blocks <= 2, "{fault:?}: {blocks} blocks");
        assert!(told < Duration::from_millis(500), "{fault:?}: {told:?}");
        // What meloet asks for (256 frames, 5.33 ms). Not held to the clock here: on a busy
        // PC this thread, which is not a real-time one, may be late itself.
        let (blocks, told) = fails(fault, 256, false);
        eprintln!("{fault:?} at 256 frames: given up after {blocks} block(s), told after {told:?}");
    }
}

#[test]
fn the_engine_does_not_wait_for_a_process_known_to_have_ended() {
    // No clock here: a block of 200 ms, so a wait up to the deadline would show.
    let mut rig = Rig::new(LONG);
    let side = side();
    let loaded = side.load(&rig, "sine");
    let (id, watch) = (loaded.id, loaded.watch.clone());
    rig.instrument(0, loaded);
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.5));
    let pid = side.pids()[0];
    kill(pid);
    // Told by the pidfd alone: the engine runs no block in between.
    assert_eq!(side.stopped().1, [id]);
    assert!(!watch.stopped(), "the engine has not looked yet");
    let (unanswered, waited) = (watch.unanswered(), watch.waited_ns());

    let began = Instant::now();
    assert_eq!(rig.block(), 0.0);
    let took = began.elapsed();
    assert!(watch.stopped(), "stopped at the first block after the end");
    assert_eq!(watch.unanswered(), unanswered, "no answer was waited for");
    assert_eq!(watch.waited_ns(), waited);
    eprintln!("the first block after the process ended took {took:?}");
}

#[test]
fn the_settings_of_the_output_sets_go_with_the_load_and_after_it() {
    let mut rig = Rig::new(LONG);
    let side = side();
    // A set the plugin does not have is passed over.
    let sine = side.load_with(&rig, "sine", all_off(&["nothing-here"]));
    let id = sine.id;
    rig.instrument(0, sine);
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.5), "the sine at its level");
    let set = wait(|done| (side.hosts).set_outputs(id, all_off(&["nothing-here"]), done));
    assert_eq!(set, Ok(()));
    rig.block();
    assert!(near(rig.block(), 0.5));
    // Only the processes of an export keep the sound of the sets.
    assert!(wait(|done| side.hosts.capture(id, 256, done)).is_err());
}

#[test]
#[ignore = "needs the plugin host's fake instrument with four output sets (the other half of \
            the output sets work); FOUR_OUTPUTS names it"]
fn a_stopped_plugin_comes_back_with_the_settings_of_its_outputs() {
    let mut rig = Rig::new(LONG);
    let side = side();
    let silent = all_off(&["a", "b", "c"]);
    let four = side.load_with(&rig, FOUR_OUTPUTS, silent.clone());
    let id = four.id;
    assert_eq!(four.plugin.outputs.len(), 4, "{:?}", four.plugin.outputs);
    rig.instrument(0, four);
    rig.keys(0, &[69]);
    rig.block();
    assert_eq!(rig.block(), 0.0, "every set is off from the first block");

    wait(|done| side.hosts.set_outputs(id, OutputSettings::default(), done)).unwrap();
    rig.block();
    assert!(rig.block() > 0.01, "the sets the rule hears sound");
    wait(|done| side.hosts.set_outputs(id, silent, done)).unwrap();
    rig.block();
    assert_eq!(rig.block(), 0.0);

    let old = side.pids();
    kill(old[0]);
    side.stopped();
    let again = side.reload(&rig, id).unwrap();
    rig.instrument(0, again);
    rig.send(EngineCommand::AuditionOff);
    rig.block();
    rig.keys(0, &[69]);
    rig.block();
    assert_eq!(rig.block(), 0.0, "the settings came back with it");
}

#[test]
fn a_stopped_plugin_is_loaded_again_with_what_was_remembered() {
    let mut rig = Rig::new(LONG);
    let side = side();
    let one = side.load(&rig, "sine");
    let two = side.load(&rig, "sine");
    let (one_id, two_id) = (one.id, two.id);
    rig.instrument(0, one);
    rig.instrument(1, two);
    assert_eq!(side.reload(&rig, one_id).err(), Some(HostError::NotStopped));

    // One: a state, and a parameter set after it. Two: only a parameter.
    side.set_level(one_id, 0.25);
    wait(|done| side.hosts.state(one_id, done)).unwrap();
    side.set_level(two_id, 0.75);
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.25));

    let old = side.pids();
    assert_eq!(old.len(), 1);
    kill(old[0]);
    let (_, slots) = side.stopped();
    assert_eq!(
        slots,
        [one_id, two_id],
        "everything in the process stops with it"
    );
    assert!(gone(&old));
    rig.block();
    assert_eq!(rig.block(), 0.0, "silence where the plugin was");
    assert_eq!(
        wait(|done| side.hosts.param(one_id, 0, done)),
        Err(HostError::Gone)
    );

    // Both go into one new process.
    let again = side.reload(&rig, one_id).unwrap();
    assert_eq!(again.id, one_id);
    rig.instrument(0, again);
    let again = side.reload(&rig, two_id).unwrap();
    rig.instrument(1, again);
    let new = side.pids();
    assert_eq!(new.len(), 1);
    assert_ne!(new, old);
    assert!((side.level(one_id) - 0.25).abs() < 1e-9, "from the state");
    assert!(
        (side.level(two_id) - 0.75).abs() < 1e-9,
        "from the parameter"
    );
    // The held key was sent to the plugin that is gone: play it again.
    rig.send(EngineCommand::AuditionOff);
    rig.block();
    rig.keys(0, &[69]);
    rig.block();
    assert!(near(rig.block(), 0.25));

    // It can stop and come back again.
    side.set_level(one_id, 0.5);
    kill(new[0]);
    side.stopped();
    let again = side.reload(&rig, one_id).unwrap();
    rig.instrument(0, again);
    assert!((side.level(one_id) - 0.5).abs() < 1e-9);
    // "外す" on the other one: it is let go while stopped.
    wait(|done| side.hosts.unload(two_id, done)).unwrap();
    assert_eq!(
        side.reload(&rig, two_id).err(),
        Some(HostError::UnknownSlot)
    );
    let last = side.pids();
    drop(side);
    assert!(gone(&last));
}

#[test]
fn a_recording_goes_on_when_the_instrument_stops() {
    const BLOCK: usize = 256;
    let mut rig = Rig::new(BLOCK);
    let side = side();
    let sine = side.load(&rig, "sine");
    let watch = sine.watch.clone();
    rig.instrument(0, sine);
    rig.send(EngineCommand::SetPlayTarget { track: 0 });
    rig.send(EngineCommand::Record {
        from: 0,
        count_in_beats: 0,
        muted: TrackSet::default(),
    });
    let pid = side.pids()[0];
    let step = rig.block_time();
    let mut next = Instant::now();
    let mut keys = 0u32;
    let mut killed_at = None;
    let mut recorded = Vec::new();
    // A key down or up every 10 blocks; the process is killed at block 100 of 300.
    for n in 0..300u32 {
        if n % 10 == 5 {
            let event = if keys.is_multiple_of(2) {
                InstrumentEvent::NoteOn {
                    note: 60,
                    velocity: 100,
                }
            } else {
                InstrumentEvent::NoteOff { note: 60 }
            };
            let t_us = monotonic_us();
            assert!(rig.parts.midi_in.push(MidiIn { event, t_us }).is_ok());
            keys += 1;
        }
        if n == 100 {
            kill(pid);
            killed_at = Some((monotonic_us(), Instant::now()));
        }
        rig.block();
        recorded.extend(std::iter::from_fn(|| rig.parts.handle.pop_recorded()));
        next += step;
        std::thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    let (killed_at, killed) = killed_at.unwrap();
    let (told, _) = side.stopped();
    assert!(watch.stopped());
    assert!(!alive(pid));
    assert!(
        recorded
            .iter()
            .all(|r| !matches!(r, Recorded::Stopped { .. })),
        "the recording stopped by itself: {recorded:?}"
    );

    rig.send(EngineCommand::Stop {
        t_us: monotonic_us(),
    });
    rig.block();
    rig.block();
    recorded.extend(std::iter::from_fn(|| rig.parts.handle.pop_recorded()));
    // Only what the recording says.
    recorded.retain(|r| !matches!(r, Recorded::Recent { .. } | Recorded::RecentEnd { .. }));
    assert!(matches!(
        recorded.first(),
        Some(Recorded::Started { from: 0, .. })
    ));
    assert!(matches!(recorded.last(), Some(Recorded::Stopped { .. })));
    let times: Vec<u64> = recorded
        .iter()
        .filter_map(|r| match r {
            Recorded::Event { t_us, .. } => Some(*t_us),
            _ => None,
        })
        .collect();
    let after = times.iter().filter(|&&t| t > killed_at).count();
    assert_eq!(times.len() as u32, keys, "every key played is recorded");
    assert_eq!(after, 20, "the keys played after the plugin stopped too");
    eprintln!(
        "recording: {} keys kept, {after} of them after the kill; the stop was told {:?} after it",
        times.len(),
        told.duration_since(killed),
    );
}
