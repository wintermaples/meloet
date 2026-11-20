//! The audio thread of the host process: sleeps on the bell of the shared memory and, for
//! every slot meloet asks a block of, runs that slot's plugin.
//!
//! Inside the loop nothing allocates, locks or blocks, apart from the sleep on the bell.
//! There may be several such threads (as many as plugins, up to half the CPUs and four): the
//! engine asks all its instruments for a block at once, and the plugins of one process then
//! run side by side. A slot's plugin sits in a [`Seat`] that one thread at a time is at; a
//! thread never waits for a seat. The main thread makes a [`SlotRun`] with all its buffers,
//! puts it into its seat, and takes it out again to drop it.
//!
//! Each thread asks for a real-time priority for itself: with `sched_setscheduler` where the
//! PC's limits allow it (`RLIMIT_RTPRIO`), through rtkit otherwise. Without one it still runs,
//! late whenever the PC is busy, and the log says so. `MELOET_PLUGHOST_RT` chooses for tests
//! and measurements: `off` asks for nothing, `rtkit` asks rtkit even where the limits allow.

use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{Bell, FromPlugin, Mapping, PluginEnd, ToPlugin, block_flags};

use crate::formats::{EventSink, ProcessBlock, Processor};
use crate::rtcheck;

/// How long the thread sleeps when no block is asked for, before it looks at its rings again.
const IDLE_NS: u64 = 100_000_000;
const COMMANDS: usize = 256;
/// Events a processor may send in one block.
const SINK_EVENTS: usize = 64;
/// Parameters the person may hold at once in one plugin's window.
const TOUCHED: usize = 32;
/// The priority asked for: the most rtkit gives (docs/research/plugin-ipc.md). PipeWire gives
/// meloet's own audio thread 88 where the limits allow and the same 20 through rtkit, so this
/// thread never stands above the one it works for.
const PRIORITY: u32 = 20;
const RT_ENV: &str = "MELOET_PLUGHOST_RT";

/// One plugin in one slot, with every buffer its blocks need.
pub struct SlotRun {
    end: PluginEnd,
    processor: Box<dyn Processor>,
    inputs: [Box<[f32]>; 2],
    outputs: [Box<[f32]>; 2],
    events: Vec<ToPlugin>,
    sink: EventSink,
    latency: u32,
    /// The parameters the person has hold of in the plugin's window: meloet's points for
    /// them are passed over (docs/architecture.md 21-10).
    touched: Vec<u32>,
}

impl SlotRun {
    /// `latency`: what meloet was told when the plugin was loaded.
    pub fn new(
        map: &Arc<Mapping>,
        end: PluginEnd,
        processor: Box<dyn Processor>,
        latency: u32,
    ) -> SlotRun {
        let geometry = map.geometry();
        let buffer = || vec![0.0f32; geometry.max_frames as usize].into_boxed_slice();
        SlotRun {
            end,
            processor,
            inputs: [buffer(), buffer()],
            outputs: [buffer(), buffer()],
            // Room for a ring's worth from meloet and as many from the main thread.
            events: Vec::with_capacity(geometry.to_plugin_events as usize + COMMANDS),
            sink: EventSink::with_capacity(SINK_EVENTS),
            latency,
            touched: Vec::with_capacity(TOUCHED),
        }
    }

    fn touch(&mut self, id: u32, on: bool) {
        let at = self.touched.iter().position(|t| *t == id);
        match (at, on) {
            (None, true) if self.touched.len() < self.touched.capacity() => self.touched.push(id),
            (Some(at), false) => {
                self.touched.swap_remove(at);
            }
            _ => {}
        }
    }

    fn slot(&self) -> usize {
        self.end.slot() as usize
    }

    /// A parameter change from the main thread, for the next block.
    fn queue(&mut self, id: u32, value: f64) {
        if self.events.len() < self.events.capacity() {
            self.events.push(ToPlugin::Param {
                frame: 0,
                id,
                value,
            });
        }
    }

    /// Runs the block meloet asked for, if it asked for one.
    fn serve(&mut self) -> bool {
        let Some(block) = self.end.take_request() else {
            return false;
        };
        let _audio = rtcheck::enter();
        let frames = block.frames;
        for (channel, input) in self.inputs.iter_mut().enumerate() {
            self.end.read_input(channel, &mut input[..frames]);
        }
        while let Some(event) = self.end.pop_event() {
            let held = matches!(event, ToPlugin::Param { id, .. } if self.touched.contains(&id));
            if !held && self.events.len() < self.events.capacity() {
                self.events.push(event);
            }
        }
        if block.flags & block_flags::RESET != 0 {
            self.processor.reset();
        }
        let transport = self.end.transport();
        self.sink.clear();
        let [left, right] = &mut self.outputs;
        self.processor.process(&mut ProcessBlock {
            inputs: [&self.inputs[0][..frames], &self.inputs[1][..frames]],
            outputs: [&mut left[..frames], &mut right[..frames]],
            events: &self.events,
            transport: &transport,
            sink: &mut self.sink,
        });
        self.events.clear();
        for (channel, output) in self.outputs.iter().enumerate() {
            self.end.write_output(channel, &output[..frames]);
        }
        for &event in self.sink.events() {
            self.end.push_event(event);
        }
        let latency = self.processor.latency();
        if latency != self.latency && self.end.push_event(FromPlugin::Latency { frames: latency }) {
            self.latency = latency;
        }
        self.end.finish();
        true
    }
}

/// One slot's place in the host process: its plugin, and whether a thread is at it.
struct Seat {
    busy: AtomicBool,
    run: UnsafeCell<Option<Box<SlotRun>>>,
}

// SAFETY: `run` is reached only through `Seat::take`, which lets one thread in at a time.
unsafe impl Sync for Seat {}

impl Seat {
    /// The seat, if no other thread is at it. Never waits.
    fn take(&self) -> Option<Taken<'_>> {
        self.busy
            .compare_exchange(false, true, SeqCst, SeqCst)
            .ok()
            .map(|_| Taken(self))
    }
}

struct Taken<'a>(&'a Seat);

impl Taken<'_> {
    fn run(&mut self) -> &mut Option<Box<SlotRun>> {
        // SAFETY: this thread holds the seat until the guard is dropped.
        unsafe { &mut *self.0.run.get() }
    }
}

impl Drop for Taken<'_> {
    fn drop(&mut self) {
        self.0.busy.store(false, SeqCst);
    }
}

struct Shared {
    map: Arc<Mapping>,
    seats: Box<[Seat]>,
    quit: AtomicBool,
    /// The first audio thread got a real-time priority.
    first_is_realtime: AtomicBool,
}

/// The main thread's hold on the audio threads.
pub struct Handle {
    shared: Arc<Shared>,
    bell: Bell,
    threads: Vec<JoinHandle<()>>,
    loaded: usize,
    most_threads: usize,
}

/// How long the main thread waits for a seat. The audio thread at it may be inside a plugin
/// that hangs; then meloet has already seen the slot stop.
const ANSWER: Duration = Duration::from_secs(2);
/// No more audio threads than this, however many plugins and CPUs there are: the threads of
/// all host processes together must fit what rtkit gives one user (25, docs/research/
/// plugin-ipc.md).
const MOST_THREADS: usize = 4;

pub fn start(map: Arc<Mapping>) -> std::io::Result<Handle> {
    let seats = (0..map.geometry().slots)
        .map(|_| Seat {
            busy: AtomicBool::new(false),
            run: UnsafeCell::new(None),
        })
        .collect();
    let cpus = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut handle = Handle {
        bell: Bell::new(map.clone()),
        shared: Arc::new(Shared {
            map,
            seats,
            quit: AtomicBool::new(false),
            first_is_realtime: AtomicBool::new(false),
        }),
        threads: Vec::new(),
        loaded: 0,
        most_threads: (cpus / 2).clamp(1, MOST_THREADS),
    };
    handle.grow()?;
    Ok(handle)
}

impl Handle {
    /// As many audio threads as there are plugins, up to half the CPUs and [`MOST_THREADS`];
    /// one at least. Threads are never taken away again.
    fn grow(&mut self) -> std::io::Result<()> {
        while self.threads.len() < self.loaded.clamp(1, self.most_threads) {
            let shared = self.shared.clone();
            let first = self.threads.is_empty();
            let thread = std::thread::Builder::new()
                .name(format!("plughost-audio-{}", self.threads.len()))
                .spawn(move || run(shared, first))?;
            self.threads.push(thread);
        }
        Ok(())
    }

    /// The seat of a slot, for the main thread: waits for the audio thread that is at it.
    fn seat(&self, slot: u32) -> Result<Taken<'_>, String> {
        let seat = (self.shared.seats.get(slot as usize)).ok_or(format!("no slot {slot}"))?;
        let until = Instant::now() + ANSWER;
        loop {
            if let Some(taken) = seat.take() {
                return Ok(taken);
            }
            if Instant::now() > until {
                return Err("the audio thread did not answer".into());
            }
            std::thread::sleep(Duration::from_micros(100));
        }
    }

    /// Hands the plugin to the audio threads; they run it from the next block asked for.
    pub fn install(&mut self, run: SlotRun) -> Result<(), String> {
        let slot = run.slot() as u32;
        {
            let mut seat = self.seat(slot)?;
            if seat.run().is_some() {
                return Err(format!("slot {slot} was not free"));
            }
            *seat.run() = Some(Box::new(run));
        }
        self.loaded += 1;
        if let Err(e) = self.grow() {
            // Fewer threads than wanted still run every plugin.
            eprintln!("meloet-plughost: audio thread: {e}");
        }
        // A block asked for while the main thread was at the seat was passed over.
        self.bell.ring();
        Ok(())
    }

    /// Takes the plugin back from the audio threads and drops it here.
    pub fn remove(&mut self, slot: u32) -> Result<(), String> {
        let run = self.seat(slot)?.run().take();
        self.bell.ring();
        if run.is_some() {
            self.loaded = self.loaded.saturating_sub(1);
        }
        drop(run);
        Ok(())
    }

    pub fn set_param(&mut self, slot: u32, id: u32, value: f64) -> Result<(), String> {
        if let Some(run) = self.seat(slot)?.run() {
            run.queue(id, value);
        }
        self.bell.ring();
        Ok(())
    }

    /// The person took hold of a parameter in the window, or let go of it.
    pub fn touch(&mut self, slot: u32, id: u32, on: bool) -> Result<(), String> {
        if let Some(run) = self.seat(slot)?.run() {
            run.touch(id, on);
        }
        Ok(())
    }

    /// Drops the parameter changes of `set_param` that no block has taken yet. Before a state
    /// is put back: a change made before it must not land after it.
    pub fn forget_params(&mut self, slot: u32) -> Result<(), String> {
        if let Some(run) = self.seat(slot)?.run() {
            run.events.clear();
        }
        self.bell.ring();
        Ok(())
    }

    /// Ends the audio threads and drops every plugin they ran; true when it did. A thread
    /// stuck inside a plugin is left behind with all the plugins (false): the process is about
    /// to end, and nothing of those plugins may be touched any more.
    pub fn stop(self) -> bool {
        self.shared.quit.store(true, SeqCst);
        self.bell.ring();
        let until = Instant::now() + ANSWER;
        while self.threads.iter().any(|t| !t.is_finished()) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
            self.bell.ring();
        }
        if self.threads.iter().any(|t| !t.is_finished()) {
            // Never dropped: a thread is still inside one of the plugins.
            std::mem::forget(self.shared);
            return false;
        }
        for thread in self.threads {
            drop(thread.join());
        }
        true
    }
}

/// `sched_setscheduler` for this thread, where the PC's limits allow a real-time priority.
fn realtime_by_limit() -> Result<u32, String> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: plain calls; `limit` is written by the first before it is read, and 0 is the
    // calling thread.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_RTPRIO, &mut limit) != 0 || limit.rlim_cur < 1 {
            return Err("RLIMIT_RTPRIO is 0".into());
        }
        let priority = u64::from(PRIORITY).min(limit.rlim_cur) as u32;
        let param = libc::sched_param {
            sched_priority: priority as i32,
        };
        if libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) != 0 {
            return Err(format!(
                "sched_setscheduler: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(priority)
    }
}

/// Asks rtkit, on the system bus, to make this thread real-time.
fn realtime_by_rtkit() -> Result<u32, String> {
    use dbus::blocking::Connection;
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;

    const RTKIT: &str = "org.freedesktop.RealtimeKit1";
    let error = |e: dbus::Error| format!("rtkit: {e}");
    let bus = Connection::new_system().map_err(error)?;
    let rtkit = bus.with_proxy(
        RTKIT,
        "/org/freedesktop/RealtimeKit1",
        Duration::from_secs(2),
    );
    let most: i32 = rtkit.get(RTKIT, "MaxRealtimePriority").map_err(error)?;
    let run_us: i64 = rtkit.get(RTKIT, "RTTimeUSecMax").map_err(error)?;
    let priority = PRIORITY.min(most.max(1) as u32);
    // rtkit refuses a process that may run without end: it wants a limit on how long a
    // real-time thread runs without sleeping, no longer than its own.
    // SAFETY: plain calls; `limit` is written by the first before it is read.
    unsafe {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::getrlimit(libc::RLIMIT_RTTIME, &mut limit);
        let most = run_us.max(0) as libc::rlim_t;
        limit.rlim_cur = limit.rlim_cur.min(most);
        limit.rlim_max = limit.rlim_max.min(most);
        if libc::setrlimit(libc::RLIMIT_RTTIME, &limit) != 0 {
            return Err(format!(
                "RLIMIT_RTTIME: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    // SAFETY: a plain call.
    let thread = unsafe { libc::gettid() } as u64;
    rtkit
        .method_call::<(), _, _, _>(RTKIT, "MakeThreadRealtime", (thread, priority))
        .map_err(error)?;
    Ok(priority)
}

/// Asks for a real-time priority for this thread, before its loop starts (this may wait for
/// the bus). True when it got one.
fn realtime() -> bool {
    let choice = std::env::var(RT_ENV).unwrap_or_default();
    let got = match choice.as_str() {
        "off" => Err("switched off".to_string()),
        "rtkit" => realtime_by_rtkit(),
        _ => realtime_by_limit()
            .or_else(|limit| realtime_by_rtkit().map_err(|rtkit| format!("{limit}; {rtkit}"))),
    };
    match &got {
        Ok(priority) => eprintln!("meloet-plughost: audio thread at real-time priority {priority}"),
        Err(why) => eprintln!("meloet-plughost: no real-time priority for the audio thread: {why}"),
    }
    got.is_ok()
}

/// One audio thread. All of them do the same: sleep on the bell, and run whatever block is
/// asked for at a seat no other thread is at.
fn run(shared: Arc<Shared>, first: bool) {
    let realtime = realtime();
    if first {
        shared.first_is_realtime.store(realtime, SeqCst);
    } else if !realtime && shared.first_is_realtime.load(SeqCst) {
        // Without a real-time priority (rtkit gives one user only so many) this thread would
        // take blocks and then be late with them, while the real-time threads sleep. Fewer
        // threads serve every seat all the same.
        eprintln!("meloet-plughost: this audio thread ends; the others run the plugins");
        return;
    }
    let bell = Bell::new(shared.map.clone());
    loop {
        let seen = bell.count();
        if shared.quit.load(SeqCst) {
            return;
        }
        let mut worked = false;
        for seat in shared.seats.iter() {
            // A seat another thread is at is passed over: that thread looks at every seat
            // again when it is done, and the main thread rings the bell when it leaves one.
            if let Some(mut taken) = seat.take()
                && let Some(run) = taken.run()
            {
                worked |= run.serve();
            }
        }
        if !worked {
            bell.wait(seen, IDLE_NS);
        }
    }
}

#[cfg(test)]
mod tests {
    //! The audio thread's own code allocates nothing: counted here with the engine's end and
    //! the plugin's end of one slot on the test's thread. What a plugin allocates inside its
    //! own calls is not counted (`crate::rtcheck`).

    use std::path::Path;

    use meloet_plugin::PluginKey;
    use meloet_plugin::ipc::futex::monotonic_ns;
    use meloet_plugin::ipc::{EngineEnd, Geometry};

    use super::*;
    use crate::formats::{Activation, Loaded, fake, lv2, vst3};

    const ACTIVATION: Activation = Activation {
        sample_rate: 48_000.0,
        max_frames: 256,
    };

    /// 300 blocks with keys, parameters from meloet's ring and from the main thread, and a
    /// reset; how often the audio thread's code allocated or freed, and the loudest sample.
    fn drive(mut loaded: Loaded) -> (u64, f32) {
        let map = Arc::new(
            Mapping::create(Geometry {
                max_frames: 256,
                channels: 2,
                slots: 1,
                to_plugin_events: 64,
                from_plugin_events: 64,
            })
            .unwrap(),
        );
        let mut engine = EngineEnd::new(map.clone(), 0).unwrap();
        let end = PluginEnd::new(map.clone(), 0).unwrap();
        let ids: Vec<u32> = (loaded.controller.params().iter())
            .filter(|p| p.automatable)
            .take(8)
            .map(|p| p.id)
            .collect();
        let mut run = SlotRun::new(&map, end, loaded.processor, loaded.plugin.latency);
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        let mut out = [0.0f32; 256];
        let mut said = Vec::new();
        let mut loudest = 0.0f32;
        let before = rtcheck::count();
        for block in 0..300u32 {
            let frames = 1 + (block as usize * 37) % 256;
            assert!(engine.begin(frames));
            engine.write_input(0, &input);
            engine.write_input(1, &input);
            let key = 48 + (block / 20 % 12) as u8;
            match block % 20 {
                0 => engine.push_event(ToPlugin::Midi {
                    frame: 0,
                    data: [0x90, key, 100],
                }),
                15 => engine.push_event(ToPlugin::Midi {
                    frame: 0,
                    data: [0x80, key, 0],
                }),
                7 => engine.push_event(ToPlugin::Midi {
                    frame: 0,
                    data: [0xe0, 0, 80],
                }),
                _ => true,
            };
            if let Some(&id) = ids.get(block as usize % 8) {
                let value = f64::from(block % 10) / 10.0;
                engine.push_event(ToPlugin::Param {
                    frame: 0,
                    id,
                    value,
                });
                // A point inside the block: LV2 runs it in pieces.
                engine.push_event(ToPlugin::Param {
                    frame: frames as u32 / 2,
                    id,
                    value: 1.0 - value,
                });
                run.queue(ids[0], value);
            }
            if block == 200 {
                engine.set_flags(block_flags::RESET);
            }
            engine.submit();
            assert!(run.serve());
            assert!(engine.wait(monotonic_ns()));
            engine.read_output(0, &mut out);
            loudest = out.iter().fold(loudest, |m, x| m.max(x.abs()));
            while engine.pop_event().is_some() {}
            // The main thread's side, between blocks as it would be.
            loaded.controller.poll(&mut said);
        }
        let counted = rtcheck::count() - before;
        drop(run);
        (counted, loudest)
    }

    /// Counts the parameter changes of each block, by id.
    struct Count(Arc<[std::sync::atomic::AtomicU32; 8]>);

    impl Processor for Count {
        fn process(&mut self, block: &mut ProcessBlock<'_>) {
            for cell in self.0.iter() {
                cell.store(0, SeqCst);
            }
            for event in block.events {
                if let ToPlugin::Param { id, .. } = *event {
                    self.0[id as usize % 8].fetch_add(1, SeqCst);
                }
            }
            for output in block.outputs.iter_mut() {
                output.fill(0.0);
            }
        }
        fn reset(&mut self) {}
    }

    /// While the person holds a parameter in the window, meloet's points for it are passed
    /// over; the person's own changes and the other parameters' points are not.
    #[test]
    fn points_of_a_parameter_held_in_the_window_are_passed_over() {
        let map = Arc::new(
            Mapping::create(Geometry {
                max_frames: 64,
                channels: 2,
                slots: 1,
                to_plugin_events: 64,
                from_plugin_events: 64,
            })
            .unwrap(),
        );
        let mut engine = EngineEnd::new(map.clone(), 0).unwrap();
        let end = PluginEnd::new(map.clone(), 0).unwrap();
        let counts: Arc<[std::sync::atomic::AtomicU32; 8]> = Arc::new(Default::default());
        let mut run = SlotRun::new(&map, end, Box::new(Count(counts.clone())), 0);
        let mut block = |run: &mut SlotRun, held: Option<u32>| {
            assert!(engine.begin(64));
            for (frame, id) in [(10, 5), (63, 5), (63, 6)] {
                engine.push_event(ToPlugin::Param {
                    frame,
                    id,
                    value: 0.5,
                });
            }
            if let Some(id) = held {
                run.queue(id, 0.25);
            }
            engine.submit();
            assert!(run.serve());
            assert!(engine.wait(monotonic_ns()));
            [counts[5].load(SeqCst), counts[6].load(SeqCst)]
        };
        assert_eq!(block(&mut run, None), [2, 1]);
        run.touch(5, true);
        assert_eq!(block(&mut run, Some(5)), [1, 1]);
        run.touch(5, false);
        assert_eq!(block(&mut run, None), [2, 1]);
    }

    /// What a plugin makes of a jump of a parameter in the middle of a block: the block of
    /// 512 frames gets the points (255, before) and (256, after) of `name`; the loudness of
    /// its two halves against a block without the jump. Prints what it found.
    fn jump_inside_the_block(key: &str, bundle: &str, name: &str) -> Option<(f32, f32)> {
        if !Path::new(bundle).exists() {
            eprintln!("{bundle} is not installed; nothing tested");
            return None;
        }
        let activation = Activation {
            sample_rate: 48_000.0,
            max_frames: 512,
        };
        let key: PluginKey = key.parse().unwrap();
        let mut loaded = crate::formats::load(&key, Path::new(bundle), activation).unwrap();
        let params = loaded.controller.params();
        let param = (params.iter())
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no {name}"));
        let (id, before) = (param.id, 0.9);
        let map = Arc::new(
            Mapping::create(Geometry {
                max_frames: 512,
                channels: 2,
                slots: 1,
                to_plugin_events: 64,
                from_plugin_events: 64,
            })
            .unwrap(),
        );
        let mut engine = EngineEnd::new(map.clone(), 0).unwrap();
        let end = PluginEnd::new(map.clone(), 0).unwrap();
        let mut run = SlotRun::new(&map, end, loaded.processor, 0);
        let mut out = [0.0f32; 512];
        let mut block = |events: &[ToPlugin]| {
            assert!(engine.begin(512));
            engine.write_input(0, &[0.0; 512]);
            engine.write_input(1, &[0.0; 512]);
            for &event in events {
                engine.push_event(event);
            }
            engine.submit();
            assert!(run.serve());
            assert!(engine.wait(monotonic_ns()));
            engine.read_output(0, &mut out);
            while engine.pop_event().is_some() {}
            out
        };
        let rms = |x: &[f32]| (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt();
        let note = ToPlugin::Midi {
            frame: 0,
            data: [0x90, 57, 100],
        };
        block(&[
            note,
            ToPlugin::Param {
                frame: 0,
                id,
                value: before,
            },
        ]);
        for _ in 0..40 {
            block(&[]);
        }
        let steady = block(&[]);
        let jumped = block(&[
            ToPlugin::Param {
                frame: 255,
                id,
                value: before,
            },
            ToPlugin::Param {
                frame: 256,
                id,
                value: 0.0,
            },
        ]);
        let halves = (
            rms(&jumped[..256]) / rms(&steady[..256]),
            rms(&jumped[256..]) / rms(&steady[256..]),
        );
        eprintln!("{key:?} {name} ({id}) from {before:.3} to 0 at frame 256: halves {halves:?}");
        Some(halves)
    }

    /// `cargo test -p meloet-plughost -- --ignored surge_xt_takes_a_jump --nocapture`
    #[test]
    #[ignore = "needs Surge XT"]
    fn surge_xt_takes_a_jump_inside_the_block() {
        for (key, bundle) in [
            (
                "vst3:ABCDEF019182FAEB566D624153675854",
                "/usr/lib/vst3/Surge XT.vst3",
            ),
            (
                "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
                "/usr/lib/lv2/Surge XT.lv2",
            ),
        ] {
            // JUCE takes the last value of a block for all of it (docs/research/
            // automation-vst3.md): the first half is not as loud as before either.
            if let Some((first, second)) = jump_inside_the_block(key, bundle, "Global Volume") {
                assert!(second < 0.1 && first < 0.9, "{first} {second}");
            }
        }
    }

    #[test]
    fn the_fake_plugins_run_without_allocating() {
        for name in ["sine", "pass"] {
            let (counted, loudest) = drive(fake::make(name, ACTIVATION).unwrap());
            assert_eq!(counted, 0, "{name}");
            assert!(loudest > 0.1, "{name}: {loudest}");
        }
    }

    fn real(key: &str, bundle: &str) {
        if !Path::new(bundle).exists() {
            eprintln!("{bundle} is not installed; nothing tested");
            return;
        }
        let key: PluginKey = key.parse().unwrap();
        let loaded = crate::formats::load(&key, Path::new(bundle), ACTIVATION).unwrap();
        let (counted, loudest) = drive(loaded);
        assert_eq!(counted, 0, "{key:?}");
        assert!(loudest > 0.001, "{key:?}: {loudest}");
    }

    #[test]
    #[ignore = "needs Surge XT, LSP Plugins and Zam plugins"]
    fn real_plugins_run_without_meloet_allocating() {
        // One after another on one thread, as a host process's main thread would load them.
        let _ = (vst3::list, lv2::list);
        real(
            "vst3:ABCDEF019182FAEB566D624153675854",
            "/usr/lib/vst3/Surge XT.vst3",
        );
        real(
            "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
            "/usr/lib/lv2/Surge XT.lv2",
        );
        real(
            "vst3:647370206B31732020202020756E7363",
            "/usr/lib/vst3/lsp-plugins.vst3",
        );
        real(
            "lv2:http://lsp-plug.in/plugins/lv2/compressor_stereo",
            "/usr/lib/lv2/lsp-plugins.lv2",
        );
        real(
            "vst3:2046504473616C63504D435A00000000",
            "/usr/lib/vst3/ZamComp.vst3",
        );
        real("lv2:urn:zamaudio:ZamComp", "/usr/lib/lv2/ZamComp.lv2");
        // What the audio thread does for an LV2 plugin's window.
        for (key, bundle) in [
            (
                "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
                "/usr/lib/lv2/Surge XT.lv2",
            ),
            (
                "lv2:http://lsp-plug.in/plugins/lv2/compressor_stereo",
                "/usr/lib/lv2/lsp-plugins.lv2",
            ),
        ] {
            let key: PluginKey = key.parse().unwrap();
            let loaded = lv2::load_with_window(&key, Path::new(bundle), ACTIVATION).unwrap();
            let (counted, _) = drive(loaded);
            assert_eq!(counted, 0, "{key:?} with a window");
        }
    }
}
