//! Where a plugin that runs in another process sits in the engine (docs/architecture.md 20-2).
//!
//! A [`PluginSlot`] is the engine's end of one slot of the shared memory. Wrapped as a
//! [`PluginInstrument`] it goes where any instrument goes (`EngineCommand::SwapInstrument`),
//! wrapped as a [`PluginEffect`] where any effect goes (`EngineHandle::set_effect`), and it
//! comes back on the same retired rings. The engine knows nothing of plugin formats.
//!
//! On the audio thread a slot writes the block into the shared memory, wakes the host process
//! and sleeps on a futex until the answer is there or the block's deadline ([`PluginBlock`])
//! has passed. That sleep is the one wait on the audio thread; nothing here allocates or
//! locks. A block without an answer is silence for an instrument and the untouched sound for
//! an effect. The block after waits for the missing answer once more. If it is still missing
//! then and the host process shows no sign of working towards it (a plugin that hangs, a
//! process that is gone), the slot is stopped with every other slot of that process: they are
//! never waited for again, and [`PluginWatch::stopped`] tells the control side. A process that
//! only has more work than fits the block stays; its slots go without sound until it catches
//! up.
//!
//! The instruments are asked ahead ([`Instrument::ask`]): the engine asks all of them for the
//! next stretch before it goes through its tracks, so the host processes work at the same
//! time and the engine only waits where an answer is not there yet.
//!
//! The control side (meloet-core) makes the slot before it asks the host process to load the
//! plugin, and keeps the [`PluginWatch`].
//!
//! A slot of an export ([`PluginSlot::for_export`], docs/architecture.md 22-11) is not on an
//! audio thread and has no deadline: it sleeps until the answer is there, however late, so
//! no block of an export is ever left without its plugin's sound. It gives up only when the
//! host process cannot answer any more (`meloet_plugin::ipc::HANG_MS`); the slot is stopped
//! then, and the export has failed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering::Relaxed};

use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{EngineEnd, FromPlugin, ToPlugin, Transport, block_flags};

use crate::effect::{Effect, EffectContext};
use crate::instrument::{Instrument, InstrumentEvent};

/// How much of a block may pass, from the start of the engine's callback, before a plugin that
/// has not answered is given up on for that block.
///
/// Nearly the whole block: a plugin that is only slow must not be taken for one that is gone
/// (two blocks in a row without an answer stop a slot for good), and one that is gone costs
/// this wait once. Measured with 16 tracks of Surge XT and a compressor on each
/// (docs/history/measurements/2026-11-19-round5-plughost.md): the engine's block took up to 88 % of
/// its length; at 0.5 slots went unanswered and were stopped, at 0.9 none.
pub const PLUGIN_WAIT_SHARE: f64 = 0.9;

/// What every plugin slot needs to know about the block the engine is in. The engine writes
/// it at the start of each block; the slots read it in that block, on the same thread. One for
/// each engine ([`crate::EngineHandle::plugin_block`]).
#[derive(Default)]
pub struct PluginBlock {
    deadline_ns: AtomicU64,
    flags: AtomicU32,
    /// Numerator in the low half, denominator in the high half.
    meter: AtomicU32,
    tempo_bpm: AtomicU64,
    position_quarters: AtomicU64,
    bar_start_quarters: AtomicU64,
    loop_start_quarters: AtomicU64,
    loop_end_quarters: AtomicU64,
    position_frames: AtomicI64,
    bar: AtomicU64,
}

impl PluginBlock {
    /// Until when this block's plugins are waited for, CLOCK_MONOTONIC in nanoseconds.
    pub fn set_deadline(&self, deadline_ns: u64) {
        self.deadline_ns.store(deadline_ns, Relaxed);
    }

    pub fn deadline_ns(&self) -> u64 {
        self.deadline_ns.load(Relaxed)
    }

    /// Where the song is at the first frame of the stretch about to be rendered.
    pub fn set_transport(&self, transport: &Transport) {
        self.flags.store(transport.flags, Relaxed);
        let meter = u32::from(transport.meter_num) | u32::from(transport.meter_den) << 16;
        self.meter.store(meter, Relaxed);
        self.tempo_bpm.store(transport.tempo_bpm.to_bits(), Relaxed);
        self.position_quarters
            .store(transport.position_quarters.to_bits(), Relaxed);
        self.bar_start_quarters
            .store(transport.bar_start_quarters.to_bits(), Relaxed);
        self.loop_start_quarters
            .store(transport.loop_start_quarters.to_bits(), Relaxed);
        self.loop_end_quarters
            .store(transport.loop_end_quarters.to_bits(), Relaxed);
        self.position_frames
            .store(transport.position_frames, Relaxed);
        self.bar.store(transport.bar, Relaxed);
    }

    pub fn transport(&self) -> Transport {
        let meter = self.meter.load(Relaxed);
        Transport {
            flags: self.flags.load(Relaxed),
            meter_num: meter as u16,
            meter_den: (meter >> 16) as u16,
            tempo_bpm: f64::from_bits(self.tempo_bpm.load(Relaxed)),
            position_quarters: f64::from_bits(self.position_quarters.load(Relaxed)),
            bar_start_quarters: f64::from_bits(self.bar_start_quarters.load(Relaxed)),
            loop_start_quarters: f64::from_bits(self.loop_start_quarters.load(Relaxed)),
            loop_end_quarters: f64::from_bits(self.loop_end_quarters.load(Relaxed)),
            position_frames: self.position_frames.load(Relaxed),
            bar: self.bar.load(Relaxed),
        }
    }
}

/// What the control side reads of a slot while the audio thread runs it.
#[derive(Default)]
pub struct PluginWatch {
    stopped: AtomicBool,
    unanswered: AtomicU64,
    waited_ns: AtomicU64,
    latency: AtomicU32,
}

impl PluginWatch {
    /// Two blocks in a row went without an answer (in an export: one answer could not come
    /// any more). The slot plays silence (an instrument) or lets the sound through (an
    /// effect) from then on, until the control side puts another one in its place.
    pub fn stopped(&self) -> bool {
        self.stopped.load(Relaxed)
    }

    /// Blocks that got no answer in time, since the slot was made.
    pub fn unanswered(&self) -> u64 {
        self.unanswered.load(Relaxed)
    }

    /// How long the audio thread has slept waiting for this plugin's answers in all, in
    /// nanoseconds.
    pub fn waited_ns(&self) -> u64 {
        self.waited_ns.load(Relaxed)
    }

    /// The plugin's delay in frames, as it last said (20-8). The engine adds up a track's by
    /// itself, at every block.
    pub fn latency(&self) -> u32 {
        self.latency.load(Relaxed)
    }
}

/// The engine's end of one plugin in another process.
pub struct PluginSlot {
    end: EngineEnd,
    block: Arc<PluginBlock>,
    watch: Arc<PluginWatch>,
    reset: bool,
    /// The length of the block asked for ahead of its `render` ([`Instrument::ask`]).
    asked: Option<usize>,
    /// The deadline of the engine's block in which an answer was last missed.
    missed_at: u64,
    /// Every answer is waited for without a deadline.
    export: bool,
}

impl PluginSlot {
    /// `end`: made on this slot before the plugin was loaded into it. `block`: the engine's
    /// ([`crate::EngineHandle::plugin_block`]). `latency`: what the host process said when it
    /// loaded the plugin.
    pub fn new(
        end: EngineEnd,
        block: Arc<PluginBlock>,
        latency: u32,
    ) -> (PluginSlot, Arc<PluginWatch>) {
        let watch = Arc::new(PluginWatch::default());
        watch.latency.store(latency, Relaxed);
        let slot = PluginSlot {
            end,
            block,
            watch: watch.clone(),
            reset: false,
            asked: None,
            missed_at: 0,
            export: false,
        };
        (slot, watch)
    }

    /// As [`PluginSlot::new`], for the engine of an export: `block` is that engine's, and
    /// its deadline is not looked at.
    pub fn for_export(
        end: EngineEnd,
        block: Arc<PluginBlock>,
        latency: u32,
    ) -> (PluginSlot, Arc<PluginWatch>) {
        let (mut slot, watch) = PluginSlot::new(end, block, latency);
        slot.export = true;
        (slot, watch)
    }

    fn param_at(&mut self, frame: u32, id: u32, value: f32) {
        if value.is_finite() {
            let value = f64::from(value.clamp(0.0, 1.0));
            self.end.push_event(ToPlugin::Param { frame, id, value });
        }
    }

    fn midi(&mut self, data: [u8; 3]) {
        self.end.push_event(ToPlugin::Midi { frame: 0, data });
    }

    /// Hands the block that `begin` started to the host process.
    fn submit(&mut self) {
        self.end.set_transport(&self.block.transport());
        if std::mem::take(&mut self.reset) {
            self.end.set_flags(block_flags::RESET);
        }
        self.end.submit();
    }

    /// Asks for a block without input now, to be waited for by the next `piece` of the same
    /// length. Does nothing when the slot cannot be asked yet (the answer before is missing);
    /// `piece` deals with that.
    fn ask(&mut self, frames: usize) {
        if self.asked.is_none() && frames <= self.end.max_frames() && self.end.begin(frames) {
            self.submit();
            self.asked = Some(frames);
        }
    }

    /// One round trip for a stretch of at most the slot's block size. True when `left` and
    /// `right` hold the plugin's output; false leaves them as they were.
    fn piece(&mut self, input: bool, left: &mut [f32], right: &mut [f32]) -> bool {
        let deadline = self.block.deadline_ns();
        let ahead = self.asked.take() == Some(left.len()) && !input && self.end.asked();
        if !ahead {
            // The answer to the block before may still be missing: this block waits for it
            // once more. If it comes, its sound is too late to use, and the slot goes on.
            // "Once more" is in the engine's next block, not in a later stretch of the block
            // the answer was missed in: that one has the same deadline, already past.
            // (An export leaves no answer behind: its slot is stopped instead.)
            if !self.end.begin(left.len())
                && !(self.end.overdue()
                    && !self.export
                    && deadline != self.missed_at
                    && self.sleep(|end| end.await_overdue(deadline))
                    && self.end.begin(left.len()))
            {
                if self.end.stopped() {
                    self.watch.stopped.store(true, Relaxed);
                }
                return false;
            }
            if input {
                self.end.write_input(0, left);
                self.end.write_input(1, right);
            }
            self.submit();
        }
        let export = self.export;
        let answered = self.sleep(|end| {
            if export {
                end.wait_through()
            } else {
                end.wait(deadline)
            }
        });
        if export && !answered {
            self.watch.stopped.store(true, Relaxed);
        }
        if answered {
            self.end.read_output(0, left);
            self.end.read_output(1, right);
            // What comes from another process is not trusted to be numbers.
            if left.iter().chain(right.iter()).any(|x| !x.is_finite()) {
                left.fill(0.0);
                right.fill(0.0);
            }
        }
        while let Some(event) = self.end.pop_event() {
            match event {
                FromPlugin::Latency { frames } => self.watch.latency.store(frames, Relaxed),
                // Parameters the plugin moves by itself have nowhere to go yet.
                FromPlugin::Param { .. } => {}
            }
        }
        answered
    }

    /// One of the two waits, with its time and its outcome written down for the control side.
    fn sleep(&mut self, wait: impl FnOnce(&mut EngineEnd) -> bool) -> bool {
        let began = monotonic_ns();
        let answered = wait(&mut self.end);
        self.watch
            .waited_ns
            .fetch_add(monotonic_ns().saturating_sub(began), Relaxed);
        if !answered {
            self.watch.unanswered.fetch_add(1, Relaxed);
            self.missed_at = self.block.deadline_ns();
        }
        answered
    }

    /// Runs the plugin over `left` and `right`, in place. False when any stretch of them got
    /// no answer; those stretches are as they were.
    fn run(&mut self, input: bool, left: &mut [f32], right: &mut [f32]) -> bool {
        let step = self.end.max_frames();
        let mut all = true;
        for (left, right) in left.chunks_mut(step).zip(right.chunks_mut(step)) {
            all &= self.piece(input, left, right);
        }
        all
    }
}

/// A plugin as an instrument.
///
/// `prepare` does nothing: the plugin itself is made ready by the host process, for the sample
/// rate it was loaded with. When the sample rate changes the control side loads it again.
pub struct PluginInstrument(pub PluginSlot);

impl Instrument for PluginInstrument {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}

    fn event(&mut self, ev: InstrumentEvent) {
        let data = match ev {
            InstrumentEvent::NoteOn { note, velocity } => [0x90, note & 0x7f, velocity & 0x7f],
            InstrumentEvent::NoteOff { note } => [0x80, note & 0x7f, 0],
            InstrumentEvent::Control { controller, value } => {
                [0xb0, controller & 0x7f, value & 0x7f]
            }
            InstrumentEvent::PitchBend { value } => {
                let bend = (i32::from(value) + 8192).clamp(0, 16_383);
                [0xe0, (bend & 0x7f) as u8, (bend >> 7) as u8]
            }
            // CC 123, all notes off.
            InstrumentEvent::AllNotesOff => [0xb0, 123, 0],
        };
        self.0.midi(data);
    }

    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        // An instrument overwrites; what is there is not its input.
        left.fill(0.0);
        right.fill(0.0);
        self.0.run(false, left, right);
    }

    fn reset(&mut self) {
        self.0.reset = true;
    }

    fn ask(&mut self, frames: usize) {
        self.0.ask(frames);
    }

    fn latency(&self) -> u32 {
        self.0.watch.latency()
    }

    fn set_param_at(&mut self, frame: u32, id: u32, value: f32) {
        self.0.param_at(frame, id, value);
    }
}

/// A plugin as an effect. A parameter's position is the plugin's parameter id, and its value
/// is 0..=1 whatever the parameter's own range.
///
/// `prepare`: as for [`PluginInstrument`].
pub struct PluginEffect(pub PluginSlot);

impl Effect for PluginEffect {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}

    fn set_param(&mut self, index: u32, value: f32) {
        self.0.param_at(0, index, value);
    }

    fn takes_points(&self) -> bool {
        true
    }

    fn set_param_at(&mut self, frame: u32, index: u32, value: f32) {
        self.0.param_at(frame, index, value);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _ctx: &EffectContext) {
        self.0.run(true, left, right);
    }

    fn reset(&mut self) {
        self.0.reset = true;
    }

    fn latency(&self) -> u32 {
        self.0.watch.latency()
    }
}
