//! The audio thread of the host process: sleeps on the bell of the shared memory and, for
//! every slot meloet asks a block of, runs that slot's plugin.
//!
//! Inside the loop nothing allocates, locks or blocks, apart from the sleep on the bell.
//! Plugins come and go through two `rtrb` rings: the main thread makes a [`SlotRun`] with all
//! its buffers, the audio thread only takes it into its row of slots, and hands it back to be
//! dropped on the main thread.
//!
//! The thread asks for a real-time priority for itself: with `sched_setscheduler` where the
//! PC's limits allow it (`RLIMIT_RTPRIO`), through rtkit otherwise. Without one it still runs,
//! late whenever the PC is busy, and the log says so. `MELOET_PLUGHOST_RT` chooses for tests
//! and measurements: `off` asks for nothing, `rtkit` asks rtkit even where the limits allow.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{Bell, FromPlugin, Mapping, PluginEnd, ToPlugin, block_flags};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::formats::{EventSink, ProcessBlock, Processor};
use crate::rtcheck;

/// How long the thread sleeps when no block is asked for, before it looks at its rings again.
const IDLE_NS: u64 = 100_000_000;
const COMMANDS: usize = 256;
/// Events a processor may send in one block.
const SINK_EVENTS: usize = 64;
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
            if self.events.len() < self.events.capacity() {
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

enum Command {
    Install(Box<SlotRun>),
    Remove(u32),
    Param { slot: u32, id: u32, value: f64 },
    ForgetParams(u32),
    Quit,
}

enum Returned {
    Installed(u32),
    Removed(u32, Option<Box<SlotRun>>),
}

type Slots = Box<[Option<Box<SlotRun>>]>;

/// The main thread's hold on the audio thread.
pub struct Handle {
    commands: Producer<Command>,
    returned: Consumer<Returned>,
    bell: Bell,
    thread: Option<JoinHandle<Slots>>,
}

/// How long the main thread waits for the audio thread to take a command. The audio thread
/// may be inside a plugin that hangs; then meloet has already seen the slot stop.
const ANSWER: Duration = Duration::from_secs(2);

pub fn start(map: Arc<Mapping>) -> std::io::Result<Handle> {
    let (commands_tx, commands_rx) = RingBuffer::new(COMMANDS);
    let (returned_tx, returned_rx) = RingBuffer::new(COMMANDS);
    let slots: Slots = (0..map.geometry().slots).map(|_| None).collect();
    let bell = Bell::new(map.clone());
    let thread = std::thread::Builder::new()
        .name("plughost-audio".into())
        .spawn(move || run(Bell::new(map), commands_rx, returned_tx, slots))?;
    Ok(Handle {
        commands: commands_tx,
        returned: returned_rx,
        bell,
        thread: Some(thread),
    })
}

impl Handle {
    fn send(&mut self, command: Command) -> Result<(), String> {
        self.commands
            .push(command)
            .map_err(|_| "the audio thread is not taking commands".to_string())?;
        self.bell.ring();
        Ok(())
    }

    fn answer(&mut self) -> Result<Returned, String> {
        let until = Instant::now() + ANSWER;
        loop {
            if let Ok(returned) = self.returned.pop() {
                return Ok(returned);
            }
            if Instant::now() > until {
                return Err("the audio thread did not answer".into());
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    /// Hands the plugin to the audio thread; back once it runs there.
    pub fn install(&mut self, run: SlotRun) -> Result<(), String> {
        let slot = run.slot() as u32;
        self.send(Command::Install(Box::new(run)))?;
        match self.answer()? {
            Returned::Installed(got) if got == slot => Ok(()),
            _ => Err(format!("slot {slot} was not free")),
        }
    }

    /// Takes the plugin back from the audio thread and drops it here.
    pub fn remove(&mut self, slot: u32) -> Result<(), String> {
        self.send(Command::Remove(slot))?;
        match self.answer()? {
            Returned::Removed(got, run) if got == slot => {
                drop(run);
                Ok(())
            }
            _ => Err(format!("slot {slot} did not come back")),
        }
    }

    pub fn set_param(&mut self, slot: u32, id: u32, value: f64) -> Result<(), String> {
        self.send(Command::Param { slot, id, value })
    }

    /// Drops the parameter changes of `set_param` that no block has taken yet. Before a state
    /// is put back: a change made before it must not land after it.
    pub fn forget_params(&mut self, slot: u32) -> Result<(), String> {
        self.send(Command::ForgetParams(slot))
    }

    /// Ends the audio thread and drops every plugin it held; true when it did. A thread stuck
    /// inside a plugin is left behind with its plugins (false): the process is about to end,
    /// and nothing of those plugins may be touched any more.
    pub fn stop(mut self) -> bool {
        let Some(thread) = self.thread.take() else {
            return false;
        };
        if self.send(Command::Quit).is_err() {
            return false;
        }
        let until = Instant::now() + ANSWER;
        while !thread.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
        let ended = thread.is_finished();
        if ended {
            drop(thread.join());
        }
        ended
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
/// the bus). Without one the thread still runs, late whenever the PC is busy.
fn realtime() {
    let choice = std::env::var(RT_ENV).unwrap_or_default();
    let got = match choice.as_str() {
        "off" => Err("switched off".to_string()),
        "rtkit" => realtime_by_rtkit(),
        _ => realtime_by_limit()
            .or_else(|limit| realtime_by_rtkit().map_err(|rtkit| format!("{limit}; {rtkit}"))),
    };
    match got {
        Ok(priority) => eprintln!("meloet-plughost: audio thread at real-time priority {priority}"),
        Err(why) => eprintln!("meloet-plughost: no real-time priority for the audio thread: {why}"),
    }
}

fn run(
    bell: Bell,
    mut commands: Consumer<Command>,
    mut returned: Producer<Returned>,
    mut slots: Slots,
) -> Slots {
    realtime();
    loop {
        let seen = bell.count();
        // A command is taken only while its answer fits.
        while !returned.is_full() {
            let Ok(command) = commands.pop() else {
                break;
            };
            let answer = match command {
                Command::Install(run) => {
                    let slot = run.slot();
                    match slots.get_mut(slot) {
                        Some(place) if place.is_none() => {
                            *place = Some(run);
                            Returned::Installed(slot as u32)
                        }
                        _ => Returned::Removed(u32::MAX, Some(run)),
                    }
                }
                Command::Remove(slot) => {
                    Returned::Removed(slot, slots.get_mut(slot as usize).and_then(Option::take))
                }
                Command::Param { slot, id, value } => {
                    if let Some(Some(run)) = slots.get_mut(slot as usize) {
                        run.queue(id, value);
                    }
                    continue;
                }
                Command::ForgetParams(slot) => {
                    if let Some(Some(run)) = slots.get_mut(slot as usize) {
                        run.events.clear();
                    }
                    continue;
                }
                Command::Quit => return slots,
            };
            // Cannot fail: checked above, and this thread is the only producer.
            let _ = returned.push(answer);
        }
        let mut worked = false;
        for run in slots.iter_mut().flatten() {
            worked |= run.serve();
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
            assert!(engine.begin(1 + (block as usize * 37) % 256));
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
    }
}
