//! The audio thread of the host process: sleeps on the bell of the shared memory and, for
//! every slot meloet asks a block of, runs that slot's plugin.
//!
//! Inside the loop nothing allocates, locks or blocks, apart from the sleep on the bell.
//! Plugins come and go through two `rtrb` rings: the main thread makes a [`SlotRun`] with all
//! its buffers, the audio thread only takes it into its row of slots, and hands it back to be
//! dropped on the main thread.

use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_plugin::ipc::{Bell, FromPlugin, Mapping, PluginEnd, ToPlugin, block_flags};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::formats::{EventSink, ProcessBlock, Processor};

/// How long the thread sleeps when no block is asked for, before it looks at its rings again.
const IDLE_NS: u64 = 100_000_000;
const COMMANDS: usize = 256;
/// Events a processor may send in one block.
const SINK_EVENTS: usize = 64;
/// The priority rtkit would give at most (docs/research/plugin-ipc.md).
const FIFO_PRIORITY: i32 = 20;

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

    /// Ends the audio thread and drops every plugin it held. A thread stuck inside a plugin is
    /// left behind; the process is about to end.
    pub fn stop(mut self) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        if self.send(Command::Quit).is_err() {
            return;
        }
        let until = Instant::now() + ANSWER;
        while !thread.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
        if thread.is_finished() {
            drop(thread.join());
        }
    }
}

/// Asks for a real-time priority for this thread. Without it the thread still runs, late
/// whenever the PC is busy; the log says so.
fn realtime() {
    let param = libc::sched_param {
        sched_priority: FIFO_PRIORITY,
    };
    // SAFETY: a plain call; 0 is the calling thread.
    if unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &param) } != 0 {
        eprintln!(
            "meloet-plughost: no real-time priority for the audio thread: {}",
            std::io::Error::last_os_error()
        );
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
