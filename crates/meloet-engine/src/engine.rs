//! The mixer the audio thread owns ([`Engine`]) and its control side ([`EngineHandle`]).
//!
//! Between the two there are only `rtrb` rings and atomics.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::instrument::{Instrument, InstrumentEvent};
use crate::rt;

const COMMAND_RING: usize = 256;
const MIDI_RING: usize = 1024;
const LATENCY_RING: usize = 1024;

/// Microseconds on `CLOCK_MONOTONIC`: the clock of [`MidiIn::t_us`] and of [`BlockTiming`].
pub fn monotonic_us() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000 + ts.tv_nsec as u64 / 1_000
}

/// Control side to audio thread.
pub enum EngineCommand {
    /// The instrument must already be prepared for the engine's sample rate and block size;
    /// [`EngineHandle::swap_instrument`] does that. The old one comes back on the retired ring.
    SwapInstrument {
        track: usize,
        instrument: Box<dyn Instrument>,
    },
    /// Linear gain, 1.0 = unity.
    SetTrackGain {
        track: usize,
        gain: f32,
    },
    /// The track that keyboard input plays. The track that was the target gets `AllNotesOff`.
    SetPlayTarget {
        track: usize,
    },
    TestTone,
    AllNotesOff,
}

/// One keyboard event, pushed by the MIDI thread straight into the engine's ring.
#[derive(Clone, Copy, Debug)]
pub struct MidiIn {
    pub event: InstrumentEvent,
    /// When the event was received, [`monotonic_us`].
    pub t_us: u64,
}

/// The MIDI thread's end of the ring into the engine.
pub type MidiFeed = Producer<MidiIn>;

/// When this block was called and when its first frame reaches the DAC, [`monotonic_us`] clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct BlockTiming {
    pub callback_us: u64,
    pub playback_us: u64,
}

/// Written for every NoteOn: `playback_us - key_us` is key-to-sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatencyRecord {
    pub key_us: u64,
    pub callback_us: u64,
    pub playback_us: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub sample_rate: u32,
    /// Largest block handed to an instrument. Bigger device blocks are rendered in pieces.
    pub max_frames: usize,
    pub max_tracks: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            sample_rate: 48_000,
            max_frames: 1024,
            max_tracks: 64,
        }
    }
}

/// Meters shared with the control side. Peaks are "largest since last read": the audio thread
/// only raises them, the reader takes the value and puts 0 back.
pub struct EngineMeters {
    tracks: Box<[[AtomicU32; 2]]>,
    master: [AtomicU32; 2],
    clipped: AtomicBool,
    load_permille: AtomicU32,
    last_frames: AtomicU32,
    callbacks: AtomicU64,
    late_callbacks: AtomicU64,
    xruns: AtomicU64,
    streams_lost: AtomicU64,
    other_errors: AtomicU64,
    sample_rate: AtomicU32,
}

// The bit patterns of non-negative floats order the same way as the floats, so `fetch_max` on
// the bits is `max` on the values.
fn raise(cell: &AtomicU32, peak: f32) {
    if peak > 0.0 {
        cell.fetch_max(peak.to_bits(), Relaxed);
    }
}

fn take(cell: &AtomicU32) -> f32 {
    f32::from_bits(cell.swap(0, Relaxed))
}

impl EngineMeters {
    fn new(config: &EngineConfig) -> Self {
        EngineMeters {
            tracks: (0..config.max_tracks)
                .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
                .collect(),
            master: [AtomicU32::new(0), AtomicU32::new(0)],
            clipped: AtomicBool::new(false),
            load_permille: AtomicU32::new(0),
            last_frames: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            late_callbacks: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            streams_lost: AtomicU64::new(0),
            other_errors: AtomicU64::new(0),
            sample_rate: AtomicU32::new(config.sample_rate),
        }
    }

    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Left and right peak of a track since the last call (after its gain). 1.0 = 0 dBFS.
    pub fn take_track_peak(&self, track: usize) -> [f32; 2] {
        self.tracks
            .get(track)
            .map_or([0.0; 2], |t| [take(&t[0]), take(&t[1])])
    }

    /// Left and right peak of the master since the last call, before the output clamp.
    pub fn take_master_peak(&self) -> [f32; 2] {
        [take(&self.master[0]), take(&self.master[1])]
    }

    /// Whether the master went over 1.0 since the last call.
    pub fn take_clipped(&self) -> bool {
        self.clipped.swap(false, Relaxed)
    }

    /// Time spent in the last block / length of that block, in thousandths.
    pub fn load_permille(&self) -> u32 {
        self.load_permille.load(Relaxed)
    }

    /// Frames in the last device block: the buffer size that is really running.
    pub fn last_frames(&self) -> u32 {
        self.last_frames.load(Relaxed)
    }

    pub fn callbacks(&self) -> u64 {
        self.callbacks.load(Relaxed)
    }

    /// Blocks that started more than 1.5 block lengths after the one before.
    pub fn late_callbacks(&self) -> u64 {
        self.late_callbacks.load(Relaxed)
    }

    /// Underruns and overruns PipeWire reported, over every stream of this engine.
    pub fn xruns(&self) -> u64 {
        self.xruns.load(Relaxed)
    }

    /// Streams that ended on their own: the output went away, the connection to the sound
    /// system broke, or the stream was invalidated. Counted once per stream.
    pub fn streams_lost(&self) -> u64 {
        self.streams_lost.load(Relaxed)
    }

    /// Errors that neither stop the stream nor are an xrun.
    pub fn other_errors(&self) -> u64 {
        self.other_errors.load(Relaxed)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Relaxed)
    }

    pub(crate) fn count_xrun(&self) {
        self.xruns.fetch_add(1, Relaxed);
    }

    pub(crate) fn count_stream_lost(&self) {
        self.streams_lost.fetch_add(1, Relaxed);
    }

    pub(crate) fn count_other_error(&self) {
        self.other_errors.fetch_add(1, Relaxed);
    }
}

struct Track {
    instrument: Option<Box<dyn Instrument>>,
    gain: f32,
    /// Gain at the end of the last block; the next block ramps from here to `gain`.
    gain_now: f32,
}

/// Two soft notes, about -12 dBFS, 0.6 s.
struct TestTone {
    frame: usize,
    total: usize,
    phase: [f32; 2],
}

impl TestTone {
    const NOTES_HZ: [f32; 2] = [783.99, 1046.50];
    const SECOND_NOTE_S: f32 = 0.16;
    const LENGTH_S: f32 = 0.6;
    const LEVEL: f32 = 0.2;

    fn idle() -> Self {
        TestTone {
            frame: 0,
            total: 0,
            phase: [0.0; 2],
        }
    }

    fn start(&mut self, sample_rate: f32) {
        *self = TestTone {
            frame: 0,
            total: (Self::LENGTH_S * sample_rate) as usize,
            phase: [0.0; 2],
        };
    }

    fn add(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: f32) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if self.frame >= self.total {
                return;
            }
            let t = self.frame as f32 / sample_rate;
            let mut s = 0.0;
            for (i, hz) in Self::NOTES_HZ.iter().enumerate() {
                let since = t - Self::SECOND_NOTE_S * i as f32;
                if since < 0.0 {
                    continue;
                }
                let env = (since / 0.005).min(1.0) * (-since / 0.16).exp();
                s += env * (TAU * self.phase[i]).sin();
                self.phase[i] = (self.phase[i] + hz / sample_rate).fract();
            }
            let tail = ((Self::LENGTH_S - t) / 0.04).clamp(0.0, 1.0);
            let s = s * tail * Self::LEVEL;
            *l += s;
            *r += s;
            self.frame += 1;
        }
    }
}

/// Everything [`Engine::new`] makes.
pub struct EngineParts {
    /// Goes to the audio thread (through [`crate::AudioOutput::open`]).
    pub engine: Engine,
    /// Stays with the control side.
    pub handle: EngineHandle,
    /// Goes to the MIDI thread.
    pub midi_in: MidiFeed,
}

/// The part the audio thread owns.
pub struct Engine {
    tracks: Box<[Track]>,
    play_target: usize,
    commands: Consumer<EngineCommand>,
    retired: Producer<Box<dyn Instrument>>,
    midi: Consumer<MidiIn>,
    latency: Producer<LatencyRecord>,
    meters: Arc<EngineMeters>,
    scratch: [Box<[f32]>; 2],
    master: [Box<[f32]>; 2],
    sample_rate: f32,
    max_frames: usize,
    tone: TestTone,
    last_callback: Option<(Instant, f64)>,
}

impl Engine {
    #[allow(clippy::new_ret_no_self)]
    pub fn new(config: EngineConfig) -> EngineParts {
        let config = EngineConfig {
            sample_rate: config.sample_rate.max(1),
            max_frames: config.max_frames.max(1),
            max_tracks: config.max_tracks.max(1),
        };
        let meters = Arc::new(EngineMeters::new(&config));
        Engine::assemble(config, meters)
    }

    fn assemble(config: EngineConfig, meters: Arc<EngineMeters>) -> EngineParts {
        let (command_tx, command_rx) = RingBuffer::new(COMMAND_RING);
        // Every command retires at most one instrument, and commands wait while this is full.
        let (retired_tx, retired_rx) = RingBuffer::new(config.max_tracks.max(16));
        let (midi_tx, midi_rx) = RingBuffer::new(MIDI_RING);
        let (latency_tx, latency_rx) = RingBuffer::new(LATENCY_RING);
        let buffer = || vec![0.0f32; config.max_frames].into_boxed_slice();
        let engine = Engine {
            tracks: (0..config.max_tracks)
                .map(|_| Track {
                    instrument: None,
                    gain: 1.0,
                    gain_now: 1.0,
                })
                .collect(),
            play_target: 0,
            commands: command_rx,
            retired: retired_tx,
            midi: midi_rx,
            latency: latency_tx,
            meters: meters.clone(),
            scratch: [buffer(), buffer()],
            master: [buffer(), buffer()],
            sample_rate: config.sample_rate as f32,
            max_frames: config.max_frames,
            tone: TestTone::idle(),
            last_callback: None,
        };
        let handle = EngineHandle {
            commands: command_tx,
            retired: retired_rx,
            latency: latency_rx,
            meters,
            max_frames: config.max_frames,
        };
        EngineParts {
            engine,
            handle,
            midi_in: midi_tx,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate as u32
    }

    pub fn max_frames(&self) -> usize {
        self.max_frames
    }

    pub fn meters(&self) -> &Arc<EngineMeters> {
        &self.meters
    }

    /// Get ready for a (new) stream. Call outside the audio thread, while no stream is running
    /// this engine. Every instrument is prepared again, so all notes stop.
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1) as f32;
        self.meters.sample_rate.store(sample_rate.max(1), Relaxed);
        for track in self.tracks.iter_mut() {
            if let Some(instrument) = track.instrument.as_mut() {
                instrument.prepare(self.sample_rate, self.max_frames);
            }
        }
        self.tone = TestTone::idle();
        self.last_callback = None;
    }

    /// One device block. `out` is interleaved stereo and is overwritten.
    pub fn process(&mut self, out: &mut [f32], timing: BlockTiming) {
        let _audio_block = rt::enter();
        let began = Instant::now();
        let frames = out.len() / 2;
        let block_s = frames as f64 / f64::from(self.sample_rate);

        if let Some((previous, previous_block_s)) = self.last_callback
            && began.duration_since(previous).as_secs_f64() > previous_block_s * 1.5
        {
            self.meters.late_callbacks.fetch_add(1, Relaxed);
        }
        self.last_callback = Some((began, block_s));
        self.meters.callbacks.fetch_add(1, Relaxed);
        self.meters.last_frames.store(frames as u32, Relaxed);

        self.read_commands();
        self.read_midi(timing);
        for piece in out.chunks_mut(self.max_frames * 2) {
            self.render(piece);
        }

        if block_s > 0.0 {
            let load = began.elapsed().as_secs_f64() / block_s * 1000.0;
            self.meters
                .load_permille
                .store(load.min(f64::from(u16::MAX)) as u32, Relaxed);
        }
    }

    fn read_commands(&mut self) {
        // Stop while the retired ring is full: an instrument must never be dropped here.
        while !self.retired.is_full() {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            match command {
                EngineCommand::SwapInstrument { track, instrument } => {
                    let old = match self.tracks.get_mut(track) {
                        Some(slot) => slot.instrument.replace(instrument),
                        None => Some(instrument),
                    };
                    if let Some(old) = old {
                        // Cannot fail: checked above, and this is the only producer.
                        let _ = self.retired.push(old);
                    }
                }
                EngineCommand::SetTrackGain { track, gain } => {
                    if let Some(slot) = self.tracks.get_mut(track)
                        && gain.is_finite()
                    {
                        slot.gain = gain.max(0.0);
                    }
                }
                EngineCommand::SetPlayTarget { track } => {
                    if track < self.tracks.len() && track != self.play_target {
                        if let Some(instrument) = self.tracks[self.play_target].instrument.as_mut()
                        {
                            instrument.event(InstrumentEvent::AllNotesOff);
                        }
                        self.play_target = track;
                    }
                }
                EngineCommand::TestTone => self.tone.start(self.sample_rate),
                EngineCommand::AllNotesOff => {
                    for instrument in self.tracks.iter_mut().filter_map(|t| t.instrument.as_mut()) {
                        instrument.event(InstrumentEvent::AllNotesOff);
                    }
                }
            }
        }
    }

    fn read_midi(&mut self, timing: BlockTiming) {
        while let Ok(midi) = self.midi.pop() {
            if matches!(midi.event, InstrumentEvent::NoteOn { velocity, .. } if velocity > 0) {
                // Every event starts at frame 0 of this block, so `playback_us` is its first sample.
                let _ = self.latency.push(LatencyRecord {
                    key_us: midi.t_us,
                    callback_us: timing.callback_us,
                    playback_us: timing.playback_us,
                });
            }
            if let Some(instrument) = self.tracks[self.play_target].instrument.as_mut() {
                instrument.event(midi.event);
            }
        }
    }

    /// Renders at most `max_frames` frames.
    fn render(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        let [scratch_l, scratch_r] = &mut self.scratch;
        let [master_l, master_r] = &mut self.master;
        let (scratch_l, scratch_r) = (&mut scratch_l[..frames], &mut scratch_r[..frames]);
        let (master_l, master_r) = (&mut master_l[..frames], &mut master_r[..frames]);
        master_l.fill(0.0);
        master_r.fill(0.0);

        for (track, meter) in self.tracks.iter_mut().zip(self.meters.tracks.iter()) {
            let Some(instrument) = track.instrument.as_mut() else {
                continue;
            };
            instrument.render(scratch_l, scratch_r);
            let step = (track.gain - track.gain_now) / frames as f32;
            let mut gain = track.gain_now;
            let mut peak = [0.0f32; 2];
            for i in 0..frames {
                gain += step;
                let (l, r) = (scratch_l[i] * gain, scratch_r[i] * gain);
                peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
                master_l[i] += l;
                master_r[i] += r;
            }
            track.gain_now = track.gain;
            raise(&meter[0], peak[0]);
            raise(&meter[1], peak[1]);
        }

        self.tone.add(master_l, master_r, self.sample_rate);

        let mut peak = [0.0f32; 2];
        for (frame, (l, r)) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(master_l.iter().zip(master_r.iter()))
        {
            peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
            frame[0] = l.clamp(-1.0, 1.0);
            frame[1] = r.clamp(-1.0, 1.0);
        }
        raise(&self.meters.master[0], peak[0]);
        raise(&self.meters.master[1], peak[1]);
        if peak[0] > 1.0 || peak[1] > 1.0 {
            self.meters.clipped.store(true, Relaxed);
        }
    }
}

/// The part the control side (core) owns.
pub struct EngineHandle {
    commands: Producer<EngineCommand>,
    retired: Consumer<Box<dyn Instrument>>,
    latency: Consumer<LatencyRecord>,
    meters: Arc<EngineMeters>,
    max_frames: usize,
}

impl EngineHandle {
    /// Gives the command back if the ring is full.
    pub fn send(&mut self, command: EngineCommand) -> Result<(), EngineCommand> {
        self.commands
            .push(command)
            .map_err(|rtrb::PushError::Full(command)| command)
    }

    /// Prepares `instrument` here (this may allocate) and sends it to `track`.
    pub fn swap_instrument(
        &mut self,
        track: usize,
        mut instrument: Box<dyn Instrument>,
    ) -> Result<(), EngineCommand> {
        instrument.prepare(self.meters.sample_rate() as f32, self.max_frames);
        self.send(EngineCommand::SwapInstrument { track, instrument })
    }

    /// The next instrument the audio thread is done with. Dropping it is the caller's job.
    pub fn take_retired(&mut self) -> Option<Box<dyn Instrument>> {
        self.retired.pop().ok()
    }

    /// Drops every retired instrument on this thread. Call it now and then (it is what lets
    /// further commands through once the retired ring has filled). Returns how many.
    pub fn drop_retired(&mut self) -> usize {
        let mut count = 0;
        while self.take_retired().is_some() {
            count += 1;
        }
        count
    }

    /// A new, empty engine in place of one that was lost (a stream that never gave it back).
    /// This handle is rewired to it; the meters stay the same `Arc`. Every track starts without
    /// an instrument, at unity gain, with track 0 as the play target. The returned producer
    /// replaces the old one, whose ring nobody reads any more.
    pub fn rebuild(&mut self) -> (Engine, MidiFeed) {
        let config = EngineConfig {
            sample_rate: self.meters.sample_rate(),
            max_frames: self.max_frames,
            max_tracks: self.meters.track_count(),
        };
        let parts = Engine::assemble(config, self.meters.clone());
        *self = parts.handle;
        (parts.engine, parts.midi_in)
    }

    pub fn pop_latency(&mut self) -> Option<LatencyRecord> {
        self.latency.pop().ok()
    }

    pub fn meters(&self) -> &Arc<EngineMeters> {
        &self.meters
    }
}
