//! The way in: the PC's audio input, for the vocal tracks (docs/architecture.md 22-5 to 22-7,
//! 22-15).
//!
//! The input stream has a thread of its own, apart from the output's; cpal 0.18.2 puts both
//! in one `node.group`, so PipeWire runs them in the same cycle and both stamp their blocks
//! with the same CLOCK_MONOTONIC. The input callback is an audio thread as the output's is:
//! it takes no memory, waits for no lock and touches no file. All it does is put each block
//! into two rings made beforehand and keep the peaks:
//!
//! ```text
//! input callback ─┬─ monitor ring ─▶ the output callback (the voice, through the mixer)
//!                 └─ record ring  ─▶ the disk writer (a thread of ordinary priority)
//! ```
//!
//! A ring carries the blocks as they come: their length differs from call to call and may
//! be 0. When a ring is full the callback does not wait; the block's samples are left out
//! and its header says so, so that the reader can put silence in their place and keep every
//! later frame where it belongs.
//!
//! The output's side of the monitor ring is [`InputFeed`]: the engine takes one device
//! block's worth of frames out of it at every block of its own.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};

/// A PipeWire source, by its `node.name` (for example
/// `alsa_input.usb-Focusrite_Scarlett_2i2-00.analog-stereo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputTarget {
    pub node_name: String,
    /// `node_name` is a sink, and what is opened is its monitor: what the sink plays comes
    /// back as input. For the measurements and tests that need a loop with no cable; the
    /// list a person chooses from has sources only.
    pub sink_monitor: bool,
}

/// Channels the meters have room for. An input with more is opened with its first ones.
pub const MAX_INPUT_CHANNELS: usize = 8;

/// How much the record ring holds: the disk may stall this long before frames are lost.
pub const RECORD_RING_SECONDS: f64 = 4.0;
/// How much the monitor ring holds. The output takes a block out every cycle; more than a
/// few blocks waiting only means the output stopped.
pub const MONITOR_RING_SECONDS: f64 = 0.5;

/// One call of the input callback, as a reader finds it in a ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputBlock {
    /// Which frame of the stream the block starts with, counting every frame the callback
    /// was given since the stream opened (those left out of this ring too).
    pub first_frame: u64,
    /// May be 0, and differs from block to block.
    pub frames: u32,
    /// When the block was captured (`cpal::InputStreamTimestamp::capture`), CLOCK_MONOTONIC
    /// in microseconds: PipeWire's `now` less its `delay`. Whether that is the block's first
    /// frame or its last is measured, not assumed (docs/architecture.md 22-6).
    pub capture_us: u64,
    /// When the callback ran (`cpal::InputStreamTimestamp::callback`).
    pub callback_us: u64,
    /// The ring had no room: the samples of this block are not in it.
    pub dropped: bool,
    /// The input is a sink's monitor ([`InputTarget::sink_monitor`]).
    pub sink_monitor: bool,
}

impl InputBlock {
    /// When the block's first frame was captured, in microseconds on the clock of the
    /// output's `playback` times.
    ///
    /// From a source, `capture_us` is that time: PipeWire's `delay` of a stream from an ALSA
    /// source is the source's latency (a quantum and its headroom), which the frames of the
    /// block wait in the device before the cycle reads them (docs/architecture.md 22-26).
    /// From a sink's monitor it is one block later: a click the engine plays at a frame
    /// comes back stamped exactly one quantum after the `playback` time of that frame, at
    /// every quantum from 64 to 1024 (`tests/input_loop.rs` measures it and keeps watch).
    /// There the block's own length is taken off.
    pub fn first_frame_us(&self, sample_rate: u32) -> f64 {
        let late = if self.sink_monitor { self.frames } else { 0 };
        self.capture_us as f64 - f64::from(late) * 1e6 / f64::from(sample_rate.max(1))
    }
}

/// The writing end of a ring, inside the input callback.
pub struct InputRingWriter {
    blocks: Producer<InputBlock>,
    samples: Producer<f32>,
    channels: u16,
}

/// The reading end of a ring: the engine's for the monitor, the disk writer's for the
/// recording.
pub struct InputRingReader {
    blocks: Consumer<InputBlock>,
    samples: Consumer<f32>,
    channels: u16,
    sample_rate: u32,
}

/// A ring for `seconds` of an input with `channels` channels at `sample_rate`. Everything
/// it will ever use is taken here.
pub fn input_ring(
    channels: u16,
    sample_rate: u32,
    seconds: f64,
) -> (InputRingWriter, InputRingReader) {
    let frames = (f64::from(sample_rate) * seconds).ceil().max(1.0) as usize;
    let (samples_tx, samples_rx) = RingBuffer::new(frames * usize::from(channels.max(1)));
    // Room for a header every 16 frames: PipeWire's smallest quantum is 32.
    let (blocks_tx, blocks_rx) = RingBuffer::new(frames / 16 + 64);
    (
        InputRingWriter {
            blocks: blocks_tx,
            samples: samples_tx,
            channels,
        },
        InputRingReader {
            blocks: blocks_rx,
            samples: samples_rx,
            channels,
            sample_rate,
        },
    )
}

impl InputRingWriter {
    /// Puts a block in: `data` is interleaved, a whole number of frames. Never waits and
    /// never takes memory. Returns whether the samples went in; `false` when the ring was
    /// full (the header then says `dropped`) or had no room even for the header.
    pub fn push(&mut self, mut block: InputBlock, data: &[f32]) -> bool {
        debug_assert_eq!(
            data.len(),
            block.frames as usize * usize::from(self.channels)
        );
        if self.blocks.slots() == 0 {
            return false;
        }
        block.dropped = self.samples.push_entire_slice(data).is_err();
        // The samples first, the header after: a reader that sees a header finds its samples.
        let _ = self.blocks.push(block);
        !block.dropped
    }
}

impl InputRingReader {
    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The next block. Its samples go to the front of `into`, interleaved; `into` must hold
    /// `frames * channels` of the largest block (a block that does not fit is skipped and
    /// comes back as `dropped`). For a `dropped` block `into` is left as it is. `None`:
    /// nothing is waiting. Never waits and never takes memory.
    pub fn pop(&mut self, into: &mut [f32]) -> Option<InputBlock> {
        let mut block = self.blocks.pop().ok()?;
        if block.dropped {
            return Some(block);
        }
        let len = block.frames as usize * usize::from(self.channels);
        match into.get_mut(..len) {
            Some(room) => {
                let filled = self.samples.pop_entire_slice(room).is_ok();
                debug_assert!(filled, "a header comes after its samples");
            }
            None => {
                // Let go of the samples all the same, or every later block would be read
                // from the wrong place.
                if let Ok(chunk) = self.samples.read_chunk(len) {
                    chunk.commit_all();
                }
                block.dropped = true;
            }
        }
        Some(block)
    }

    /// How many blocks wait.
    pub fn waiting(&self) -> usize {
        self.blocks.slots()
    }

    /// The input callback is gone: the stream ended.
    pub fn is_abandoned(&self) -> bool {
        self.blocks.is_abandoned()
    }
}

/// What the input callback counts, read by the other threads.
#[derive(Debug, Default)]
pub struct InputMeters {
    /// The largest sample of each channel since it was last taken, as `f32` bits.
    peaks: [AtomicU32; MAX_INPUT_CHANNELS],
    /// The channel reached 0 dB since it was last taken.
    clipped: [AtomicBool; MAX_INPUT_CHANNELS],
    frames: AtomicU64,
    last_frames: AtomicU32,
    delay_us: AtomicU32,
    record_dropped: AtomicU64,
    monitor_dropped: AtomicU64,
    xruns: AtomicU64,
    stream_lost: AtomicBool,
    sink_monitor: AtomicBool,
    /// The two switches of the tap, set from outside the callback.
    monitoring: AtomicBool,
    recording: AtomicBool,
}

impl InputMeters {
    /// For the input callback: the samples of one block, interleaved.
    pub fn note_block(&self, data: &[f32], channels: u16) {
        let channels = usize::from(channels).max(1);
        let frames = data.len() / channels;
        for ch in 0..channels.min(MAX_INPUT_CHANNELS) {
            let mut peak = 0.0f32;
            for frame in data.chunks_exact(channels) {
                // A NaN from the device does not stay in the peak.
                peak = peak.max(frame[ch].abs());
            }
            if peak >= 1.0 {
                self.clipped[ch].store(true, Relaxed);
            }
            let held = f32::from_bits(self.peaks[ch].load(Relaxed));
            if peak > held {
                self.peaks[ch].store(peak.to_bits(), Relaxed);
            }
        }
        self.frames.fetch_add(frames as u64, Relaxed);
        self.last_frames.store(frames as u32, Relaxed);
    }

    /// Whether the callback feeds the monitor ring: while a vocal track with its ● on has
    /// its monitor on. Off, the ring stays empty and nothing counts as lost.
    pub fn set_monitoring(&self, on: bool) {
        self.monitoring.store(on, Relaxed);
    }

    /// Whether the callback feeds the record ring: from a little before recording starts
    /// (docs/architecture.md 22-6) until it stops. The reader tells the blocks of one
    /// recording from the next by `InputBlock::first_frame`.
    pub fn set_recording(&self, on: bool) {
        self.recording.store(on, Relaxed);
    }

    pub fn monitoring(&self) -> bool {
        self.monitoring.load(Relaxed)
    }

    pub fn recording(&self) -> bool {
        self.recording.load(Relaxed)
    }

    pub fn count_record_dropped(&self, frames: u32) {
        self.record_dropped.fetch_add(u64::from(frames), Relaxed);
    }

    pub fn count_monitor_dropped(&self, frames: u32) {
        self.monitor_dropped.fetch_add(u64::from(frames), Relaxed);
    }

    pub fn count_xrun(&self) {
        self.xruns.fetch_add(1, Relaxed);
    }

    pub fn count_stream_lost(&self) {
        self.stream_lost.store(true, Relaxed);
    }

    /// The largest sample of `channel` since the last call (1.0 is 0 dB), and whether it
    /// reached 0 dB. Both start again from nothing.
    pub fn take_peak(&self, channel: usize) -> (f32, bool) {
        match (self.peaks.get(channel), self.clipped.get(channel)) {
            (Some(peak), Some(clipped)) => (
                f32::from_bits(peak.swap(0f32.to_bits(), Relaxed)),
                clipped.swap(false, Relaxed),
            ),
            _ => (0.0, false),
        }
    }

    /// Every frame the callback was given since the stream opened.
    pub fn frames(&self) -> u64 {
        self.frames.load(Relaxed)
    }

    /// What the PC says the last block took from its capture to the callback, in
    /// microseconds.
    pub fn delay_us(&self) -> u32 {
        self.delay_us.load(Relaxed)
    }

    /// The input is a sink's monitor: see [`InputBlock::first_frame_us`].
    pub fn sink_monitor(&self) -> bool {
        self.sink_monitor.load(Relaxed)
    }

    /// Before the stream starts.
    pub fn set_sink_monitor(&self, on: bool) {
        self.sink_monitor.store(on, Relaxed);
    }

    /// The frames of the last block: what really runs, not what was asked for.
    pub fn last_frames(&self) -> u32 {
        self.last_frames.load(Relaxed)
    }

    /// Frames left out of the record ring because the disk writer fell behind.
    pub fn record_dropped(&self) -> u64 {
        self.record_dropped.load(Relaxed)
    }

    pub fn monitor_dropped(&self) -> u64 {
        self.monitor_dropped.load(Relaxed)
    }

    pub fn xruns(&self) -> u64 {
        self.xruns.load(Relaxed)
    }

    /// The stream ended on its own (the device went away, PipeWire stopped).
    pub fn stream_lost(&self) -> bool {
        self.stream_lost.load(Relaxed)
    }
}

/// What the input callback owns: the two rings and the meters. `block` is the whole of the
/// callback.
pub struct InputTap {
    monitor: InputRingWriter,
    record: InputRingWriter,
    meters: Arc<InputMeters>,
    channels: u16,
    next_frame: u64,
}

impl InputTap {
    /// One call of cpal's data callback: `data` as cpal gives it, the two instants of
    /// `cpal::InputCallbackInfo::timestamp` in microseconds.
    pub fn block(&mut self, data: &[f32], capture_us: u64, callback_us: u64) {
        let _rt = crate::rt::enter();
        let frames = (data.len() / usize::from(self.channels.max(1))) as u32;
        let block = InputBlock {
            first_frame: self.next_frame,
            frames,
            capture_us,
            callback_us,
            dropped: false,
            sink_monitor: self.meters.sink_monitor.load(Relaxed),
        };
        self.next_frame += u64::from(frames);
        let delay = callback_us
            .saturating_sub(capture_us)
            .min(u64::from(u32::MAX));
        self.meters.delay_us.store(delay as u32, Relaxed);
        self.meters.note_block(data, self.channels);
        if self.meters.monitoring() && !self.monitor.push(block, data) {
            self.meters.count_monitor_dropped(frames);
        }
        if self.meters.recording() && !self.record.push(block, data) {
            self.meters.count_record_dropped(frames);
        }
    }
}

/// The ends of an input that is not the callback's.
pub struct InputEnds {
    pub tap: InputTap,
    /// For the engine: the voice of the vocal tracks whose monitor is on.
    pub monitor: InputRingReader,
    /// For the disk writer.
    pub record: InputRingReader,
    pub meters: Arc<InputMeters>,
}

/// The rings and meters of an input with `channels` channels at `sample_rate`, made before
/// the stream is: the tap goes into the callback.
pub fn input_ends(channels: u16, sample_rate: u32) -> InputEnds {
    let (monitor_tx, monitor) = input_ring(channels, sample_rate, MONITOR_RING_SECONDS);
    let (record_tx, record) = input_ring(channels, sample_rate, RECORD_RING_SECONDS);
    let meters = Arc::new(InputMeters::default());
    InputEnds {
        tap: InputTap {
            monitor: monitor_tx,
            record: record_tx,
            meters: meters.clone(),
            channels,
            next_frame: 0,
        },
        monitor,
        record,
        meters,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputError {
    /// No PipeWire source of that name.
    NoSuchInput(String),
    /// What cpal said.
    Other(String),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::NoSuchInput(name) => write!(f, "no PipeWire input node named {name}"),
            InputError::Other(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for InputError {}

fn other(e: impl fmt::Display) -> InputError {
    InputError::Other(e.to_string())
}

/// As the output's: xruns from inside the process callback, the end of the stream from
/// cpal's main loop. A stream that meloet closes says `DeviceNotAvailable` on the way out;
/// `closing` keeps that from counting.
fn stream_error_counter(
    meters: Arc<InputMeters>,
    closing: Arc<AtomicBool>,
) -> impl FnMut(cpal::Error) + Send + 'static {
    move |error| match error.kind() {
        _ if closing.load(Relaxed) => {}
        cpal::ErrorKind::Xrun => meters.count_xrun(),
        cpal::ErrorKind::DeviceNotAvailable
        | cpal::ErrorKind::StreamInvalidated
        | cpal::ErrorKind::HostUnavailable
        | cpal::ErrorKind::UnsupportedConfig => meters.count_stream_lost(),
        _ => {}
    }
}

pub struct AudioInput;

impl AudioInput {
    /// Opens an f32 stream on `target` with all the channels the device has (at most
    /// [`MAX_INPUT_CHANNELS`]) and starts it.
    ///
    /// `sample_rate`: the output's, so that both run at one rate. `request_frames` is only a
    /// wish, as the output's; what runs is [`InputMeters::last_frames`].
    pub fn open(
        target: &InputTarget,
        sample_rate: u32,
        request_frames: u32,
    ) -> Result<RunningInput, InputError> {
        // A fresh host each time, as for the output: cpal's device list is a snapshot.
        let host = cpal::host_from_id(cpal::HostId::PipeWire).map_err(other)?;
        let wanted = cpal::DeviceId::new(cpal::HostId::PipeWire, &target.node_name);
        let device = host.devices().map_err(other)?.find(|d| {
            // cpal lists a sink as a device that takes input too (its monitor).
            let source = d
                .description()
                .is_ok_and(|about| about.direction() == cpal::DeviceDirection::Input);
            d.supports_input()
                && source != target.sink_monitor
                && d.id().is_ok_and(|id| id == wanted)
        });
        let Some(device) = device else {
            return Err(InputError::NoSuchInput(target.node_name.clone()));
        };
        let channels = device
            .default_input_config()
            .map_err(other)?
            .channels()
            .clamp(1, MAX_INPUT_CHANNELS as u16);

        let InputEnds {
            mut tap,
            monitor,
            record,
            meters,
        } = input_ends(channels, sample_rate);
        meters.set_sink_monitor(target.sink_monitor);
        let config = cpal::StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Fixed(request_frames),
        };
        let closing = Arc::new(AtomicBool::new(false));
        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], info: &cpal::InputCallbackInfo| {
                    let stamp = info.timestamp();
                    tap.block(
                        data,
                        (stamp.capture.as_nanos() / 1_000) as u64,
                        (stamp.callback.as_nanos() / 1_000) as u64,
                    );
                },
                stream_error_counter(meters.clone(), closing.clone()),
                None,
            )
            .map_err(other)?;
        stream.play().map_err(other)?;
        Ok(RunningInput {
            stream,
            closing,
            channels,
            sample_rate,
            meters,
            monitor: Some(monitor),
            record: Some(record),
        })
    }
}

/// A running input stream. Dropping it closes the input.
pub struct RunningInput {
    stream: cpal::Stream,
    closing: Arc<AtomicBool>,
    channels: u16,
    sample_rate: u32,
    meters: Arc<InputMeters>,
    monitor: Option<InputRingReader>,
    record: Option<InputRingReader>,
}

impl RunningInput {
    /// How many channels the stream has.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn meters(&self) -> &Arc<InputMeters> {
        &self.meters
    }

    /// The monitor ring's reader, once: it goes to the engine, inside an [`InputFeed`].
    pub fn take_monitor(&mut self) -> Option<InputRingReader> {
        self.monitor.take()
    }

    /// The record ring's reader, once: it goes to the disk writer.
    pub fn take_record(&mut self) -> Option<InputRingReader> {
        self.record.take()
    }

    /// Stops the stream. Do not call this from inside a stream callback.
    pub fn close(self) {
        self.closing.store(true, Relaxed);
        drop(self.stream);
    }
}

/// The largest block of the input the engine takes whole; a longer one counts as lost.
const FEED_BLOCK_FRAMES: usize = 8_192;
/// How many blocks in a row may find a whole block more than they need before the feed
/// lets the oldest go.
const FEED_PATIENCE: u32 = 64;

/// The engine's end of the monitor ring: the input, a block of the output at a time.
///
/// The two callbacks come in no promised order, so a block of the output finds the input's
/// block of the same cycle or the one before: the voice is up to one block late. A block
/// that finds too little plays silence and takes nothing, which is how the feed comes to
/// wait one block behind; one that finds more than a block too much, for long, lets the
/// oldest go, so the voice does not stay late after a hiccup.
pub struct InputFeed {
    reader: InputRingReader,
    meters: Arc<InputMeters>,
    /// One block as it comes out of the ring.
    scratch: Box<[f32]>,
    /// Interleaved frames that wait, as a ring of its own.
    fifo: Box<[f32]>,
    head: usize,
    frames: usize,
    channels: usize,
    /// The device block being rendered has its frames.
    live: bool,
    ahead: u32,
}

impl InputFeed {
    /// Takes all the memory the feed will ever use.
    pub fn new(reader: InputRingReader, meters: Arc<InputMeters>) -> InputFeed {
        let channels = usize::from(reader.channels()).max(1);
        InputFeed {
            reader,
            meters,
            scratch: vec![0.0; FEED_BLOCK_FRAMES * channels].into_boxed_slice(),
            fifo: vec![0.0; 4 * FEED_BLOCK_FRAMES * channels].into_boxed_slice(),
            head: 0,
            frames: 0,
            channels,
            live: false,
            ahead: 0,
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The engine lets go of the feed: the callback stops feeding the ring.
    pub(crate) fn stop(&self) {
        self.meters.set_monitoring(false);
    }

    fn capacity(&self) -> usize {
        self.fifo.len() / self.channels
    }

    fn skip(&mut self, frames: usize) {
        let frames = frames.min(self.frames);
        self.head = (self.head + frames) % self.capacity();
        self.frames -= frames;
    }

    /// Before a device block of `block` frames: `wanted` tells the callback whether to feed
    /// the ring at all. Takes in what waits and says whether the block has its frames.
    pub(crate) fn begin(&mut self, block: usize, wanted: bool) -> bool {
        if self.meters.monitoring() != wanted {
            self.meters.set_monitoring(wanted);
        }
        let capacity = self.capacity();
        while let Some(header) = self.reader.pop(&mut self.scratch) {
            let n = (header.frames as usize).min(capacity);
            if self.frames + n > capacity {
                self.skip(self.frames + n - capacity);
            }
            let at = (self.head + self.frames) % capacity;
            let first = n.min(capacity - at);
            let (ch, lost) = (self.channels, header.dropped);
            for (to, from, len) in [(at, 0, first), (0, first, n - first)] {
                let room = &mut self.fifo[to * ch..(to + len) * ch];
                if lost {
                    room.fill(0.0);
                } else {
                    room.copy_from_slice(&self.scratch[from * ch..(from + len) * ch]);
                }
            }
            self.frames += n;
        }
        if !wanted {
            self.skip(self.frames);
            self.ahead = 0;
        } else if self.frames > 3 * block {
            self.skip(self.frames - block);
            self.ahead = 0;
        } else if self.frames >= 2 * block && block > 0 {
            self.ahead += 1;
            if self.ahead > FEED_PATIENCE {
                self.skip(self.frames - block);
                self.ahead = 0;
            }
        } else {
            self.ahead = 0;
        }
        self.live = wanted && block > 0 && self.frames >= block;
        self.live
    }

    /// The next `frames` frames, a channel to each of `bus`. Only in a block that
    /// [`InputFeed::begin`] said has its frames; in any other the bus is silent.
    pub(crate) fn take(&mut self, frames: usize, bus: &mut [Box<[f32]>]) {
        if !self.live || self.frames < frames {
            for channel in bus.iter_mut() {
                channel[..frames].fill(0.0);
            }
            return;
        }
        let capacity = self.capacity();
        for (ch, channel) in bus.iter_mut().enumerate() {
            if ch >= self.channels {
                channel[..frames].fill(0.0);
                continue;
            }
            for (i, sample) in channel[..frames].iter_mut().enumerate() {
                let value = self.fifo[((self.head + i) % capacity) * self.channels + ch];
                // What is not a number never reaches the mixer.
                *sample = if value.is_finite() { value } else { 0.0 };
            }
        }
        self.skip(frames);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(first_frame: u64, frames: u32) -> InputBlock {
        InputBlock {
            first_frame,
            frames,
            capture_us: 1_000 + first_frame,
            callback_us: 2_000 + first_frame,
            dropped: false,
            sink_monitor: false,
        }
    }

    #[test]
    fn a_source_says_its_first_frame_and_a_monitor_one_block_late() {
        let source = block(0, 480);
        assert_eq!(source.first_frame_us(48_000), source.capture_us as f64);
        let monitor = InputBlock {
            sink_monitor: true,
            ..source
        };
        assert_eq!(
            monitor.first_frame_us(48_000),
            source.capture_us as f64 - 10_000.0
        );
        // The tap says which it is.
        let mut ends = input_ends(1, 48_000);
        ends.meters.set_sink_monitor(true);
        ends.meters.set_recording(true);
        ends.tap.block(&[0.0; 64], 5_000, 6_000);
        let mut into = [0.0; 64];
        assert!(ends.record.pop(&mut into).expect("a block").sink_monitor);
    }

    #[test]
    fn blocks_of_any_length_come_out_as_they_went_in() {
        let (mut tx, mut rx) = input_ring(2, 48_000, 0.1);
        assert_eq!((rx.channels(), rx.sample_rate()), (2, 48_000));
        let mut into = [9.0f32; 16];
        assert_eq!(rx.pop(&mut into), None);

        assert!(tx.push(block(0, 3), &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6]));
        assert!(tx.push(block(3, 0), &[]));
        assert!(tx.push(block(3, 1), &[0.7, 0.8]));
        assert_eq!(rx.waiting(), 3);

        assert_eq!(rx.pop(&mut into), Some(block(0, 3)));
        assert_eq!(into[..7], [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 9.0]);
        assert_eq!(rx.pop(&mut into), Some(block(3, 0)));
        assert_eq!(rx.pop(&mut into), Some(block(3, 1)));
        assert_eq!(into[..3], [0.7, 0.8, 0.3]);
        assert_eq!(rx.pop(&mut into), None);
    }

    #[test]
    fn a_full_ring_leaves_the_samples_out_and_says_so() {
        // Room for 48 frames of one channel.
        let (mut tx, mut rx) = input_ring(1, 48_000, 0.001);
        let data = [0.5f32; 32];
        assert!(tx.push(block(0, 32), &data));
        assert!(!tx.push(block(32, 32), &data), "16 frames of room are left");
        assert!(tx.push(block(64, 16), &data[..16]));

        let mut into = [0.0f32; 32];
        assert_eq!(rx.pop(&mut into), Some(block(0, 32)));
        let lost = rx.pop(&mut into).unwrap();
        assert_eq!(
            (lost.first_frame, lost.frames, lost.dropped),
            (32, 32, true)
        );
        // The block after the gap is whole and says where it belongs.
        into = [0.0; 32];
        assert_eq!(rx.pop(&mut into), Some(block(64, 16)));
        assert_eq!(into[..17], [[0.5f32; 16].as_slice(), &[0.0]].concat()[..]);
    }

    #[test]
    fn a_block_too_long_for_the_reader_is_skipped_whole() {
        let (mut tx, mut rx) = input_ring(1, 48_000, 0.01);
        assert!(tx.push(block(0, 4), &[1.0, 2.0, 3.0, 4.0]));
        assert!(tx.push(block(4, 2), &[5.0, 6.0]));
        let mut small = [0.0f32; 2];
        let skipped = rx.pop(&mut small).unwrap();
        assert!(skipped.dropped);
        assert_eq!(rx.pop(&mut small), Some(block(4, 2)));
        assert_eq!(small, [5.0, 6.0]);
    }

    #[test]
    fn the_tap_counts_frames_keeps_peaks_and_fills_both_rings() {
        let mut ends = input_ends(2, 48_000);
        ends.meters.set_monitoring(true);
        ends.meters.set_recording(true);
        ends.tap.block(&[0.25, -0.5, 0.1, 0.2], 100, 110);
        ends.tap.block(&[], 200, 210);
        ends.tap.block(&[-1.0, 0.0], 300, 310);

        let m = &ends.meters;
        assert_eq!((m.frames(), m.last_frames()), (3, 1));
        assert_eq!(m.take_peak(0), (1.0, true));
        assert_eq!(m.take_peak(1), (0.5, false));
        assert_eq!(m.take_peak(0), (0.0, false), "taken: it starts again");
        assert_eq!(m.take_peak(MAX_INPUT_CHANNELS), (0.0, false));
        assert_eq!((m.record_dropped(), m.monitor_dropped()), (0, 0));

        let mut into = [0.0f32; 8];
        for ring in [&mut ends.monitor, &mut ends.record] {
            let first = ring.pop(&mut into).unwrap();
            assert_eq!(
                (
                    first.first_frame,
                    first.frames,
                    first.capture_us,
                    first.callback_us
                ),
                (0, 2, 100, 110)
            );
            assert_eq!(into[..4], [0.25, -0.5, 0.1, 0.2]);
            assert_eq!(
                ring.pop(&mut into).map(|b| (b.first_frame, b.frames)),
                Some((2, 0))
            );
            assert_eq!(
                ring.pop(&mut into).map(|b| (b.first_frame, b.frames)),
                Some((2, 1))
            );
            assert_eq!(ring.pop(&mut into), None);
        }
    }

    #[test]
    fn a_ring_is_fed_only_while_its_switch_is_on_and_a_full_one_counts_what_it_lost() {
        let InputEnds {
            mut tap,
            mut monitor,
            mut record,
            meters,
        } = input_ends(1, 48_000);
        let tenth = [0.0f32; 4_800];
        for _ in 0..50 {
            tap.block(&tenth, 0, 0);
        }
        assert_eq!(meters.frames(), 240_000);
        assert_eq!((monitor.waiting(), record.waiting()), (0, 0));
        assert_eq!((meters.monitor_dropped(), meters.record_dropped()), (0, 0));

        // The record ring holds four seconds; nobody takes anything out for five.
        meters.set_recording(true);
        for _ in 0..50 {
            tap.block(&tenth, 0, 0);
        }
        assert_eq!(monitor.pop(&mut [0.0; 4_800]), None);
        assert_eq!(meters.record_dropped(), 240_000 - 192_000);
        let mut into = vec![0.0f32; 4_800];
        let mut whole = 0;
        let mut first = None;
        while let Some(b) = record.pop(&mut into) {
            first.get_or_insert(b.first_frame);
            whole += u64::from(!b.dropped) * u64::from(b.frames);
        }
        assert_eq!(whole, 192_000);
        assert_eq!(
            first,
            Some(240_000),
            "frames count on while the ring is not fed"
        );
    }
}
