//! The mixer the audio thread owns ([`Engine`]) and its control side ([`EngineHandle`]).
//!
//! Between the two there are only `rtrb` rings and atomics.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::clock::{Segment, TransportClock, TransportState, heard};
use crate::effect::{Effect, EffectContext};
use crate::instrument::{Instrument, InstrumentEvent};
use crate::mixer::{INSERT_SLOTS, Io, MixStrip, RAMP_S, Strip};
use crate::plugin::{PLUGIN_WAIT_SHARE, PluginBlock};
use crate::rt;
use crate::transport::{Click, Lane, Sequence, TrackSet};

const COMMAND_RING: usize = 256;
const MIDI_RING: usize = 1024;
const LATENCY_RING: usize = 1024;
const RECORDED_RING: usize = 4096;

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
    /// [`EngineHandle::swap_instrument`] does that. The old one fades out under the new one for
    /// [`FADE_S`] and then comes back on the retired ring.
    SwapInstrument {
        track: usize,
        instrument: Box<dyn Instrument>,
    },
    /// Fader and pan of a strip. `gain` is linear (1.0 = unity; a muted return gets 0), `pan`
    /// is −1.0 (all left) to 1.0 (all right), see [`crate::pan_gains`]. The strip gets there in
    /// a straight line over [`RAMP_S`]. A track with no instrument takes it at once, as every
    /// strip does while no stream runs ([`Engine::settle`]).
    SetStrip {
        strip: MixStrip,
        gain: f32,
        pan: f32,
    },
    /// How much of a track goes to return A and to return B, linear, taken after its fader
    /// and pan. Gets there as `SetStrip` does.
    SetSends {
        track: usize,
        sends: [f32; 2],
    },
    /// Use [`EngineHandle::set_effect`]. Puts the effect into an insert slot, or empties the
    /// slot. What the slot sounded like before fades into what it sounds like now over
    /// [`FADE_S`], and the effect that was there comes back on the retired ring.
    SetEffect {
        strip: MixStrip,
        slot: usize,
        effect: Option<Box<dyn Effect>>,
    },
    /// [`Effect::set_param`] of the effect in the slot. Does nothing on an empty slot.
    SetEffectParam {
        strip: MixStrip,
        slot: usize,
        param: u32,
        value: f32,
    },
    /// The track that keyboard input plays. The track that was the target gets `AllNotesOff`.
    SetPlayTarget {
        track: usize,
    },
    /// Keyboard input plays no track (a song with no tracks). The track that was the target
    /// gets `AllNotesOff`.
    ClearPlayTarget,
    /// Use [`EngineHandle::try_instrument`]. Puts the instrument on the track for a trial: the
    /// track's own instrument fades out and is set aside, silent, and `EndTrial` brings it back.
    /// A trial instrument that is there already fades out and comes back on the retired ring.
    TryInstrument {
        track: usize,
        instrument: Box<dyn Instrument>,
    },
    /// Ends the trial on a track. `keep`: the trial instrument is now the track's own, and the
    /// one set aside comes back on the retired ring. Otherwise the one set aside plays again
    /// and the trial instrument fades out and comes back. Does nothing on a track with no trial.
    EndTrial {
        track: usize,
        keep: bool,
    },
    /// Moves the instrument of `from` to `to` as it sounds, held keys and all; `from` is left
    /// empty, and what `to` had comes back on the retired ring. If `from` was the play target,
    /// `to` is now. Sequences, strips and an instrument set aside stay where they are.
    MoveInstrument {
        from: usize,
        to: usize,
    },
    /// Empties a track: its instrument, an instrument set aside, its effects and its sequence
    /// come back on the retired rings, and its strip is a new one again (unity, the middle,
    /// no sends).
    ClearTrack {
        track: usize,
    },
    /// Use [`EngineHandle::play_sample`]. Puts the instrument on the sample seat: a place of its
    /// own beside the tracks, which no key, sequence, mute or solo reaches and no meter of a
    /// track shows. It goes to the master, with no sends. What it plays is the instrument's own business (a
    /// phrase it sounds by itself). One that is there already fades out and comes back on the
    /// retired ring. [`Engine::prepare`] empties the seat.
    PlaySample {
        instrument: Box<dyn Instrument>,
    },
    /// Empties the sample seat: its instrument fades out and comes back on the retired ring.
    StopSample,
    TestTone,
    AllNotesOff,
    /// Sounds keys on a track, as the keyboard would, and lets go of the keys the last
    /// `Audition` sounded. Not recorded.
    Audition {
        track: usize,
        keys: Chord,
        velocity: u8,
    },
    /// Lets go of the keys the last `Audition` sounded.
    AuditionOff,
    /// The tracks whose sequence does not play (mute and solo). A track that goes silent lets
    /// go of what its sequence holds at once; one that comes back picks up at the position.
    /// Keys played and auditioned still sound on them. [`Engine::prepare`] empties the set.
    SetSilentTracks {
        tracks: TrackSet,
    },
    /// Use [`EngineHandle::swap_sequence`]. The old sequence comes back on the retired ring.
    /// Notes the old one left sounding are released, and the pedal, wheel and bend it moved go
    /// back to 0.
    SwapSequence {
        track: usize,
        sequence: Box<Sequence>,
    },
    /// One beat is `ticks_per_beat` ticks (480 for a quarter note), `bpm_centi` is beats per
    /// minute × 100. Ignored if any of them is 0.
    SetTempo {
        ticks_per_beat: u32,
        beats_per_bar: u32,
        bpm_centi: u32,
    },
    /// `[start, end)` in ticks. A loop with `end <= start` does nothing.
    SetLoop {
        start: u64,
        end: u64,
        on: bool,
    },
    SetMetronome {
        on: bool,
    },
    Play {
        from: u64,
    },
    /// Count `count_in_beats` beats, then record from `from`. The tracks in `muted` do not play
    /// their sequence while recording.
    Record {
        from: u64,
        count_in_beats: u32,
        muted: TrackSet,
    },
    /// While playing: record from the next frame on, without moving and without a count-in, so
    /// nothing heard jumps. The recording starts where the listener is when the command is
    /// taken (`Started::from`); keys heard before it are kept within half a beat, as at the end
    /// of a count-in. When not playing, a `Started` and a `Stopped` come back at once.
    RecordHere {
        muted: TrackSet,
    },
    /// `t_us` is when the person asked to stop ([`monotonic_us`]). Keys after it are not recorded.
    Stop {
        t_us: u64,
    },
    Locate {
        tick: u64,
    },
    /// How many frames early a track's sequence is read and sent to its instrument, so that a
    /// track whose plugins delay the sound still reaches the master in time with the others
    /// (docs/architecture.md 20-8). The sum of the delays of the track's instrument and
    /// inserts ([`crate::PluginWatch::latency`]); 0 for a track without plugins.
    SetLookahead {
        track: usize,
        frames: u32,
    },
}

/// Up to [`Chord::CAPACITY`] keys, kept without allocating.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chord {
    keys: [u8; Chord::CAPACITY],
    len: u8,
}

impl Chord {
    pub const CAPACITY: usize = 16;

    /// Keys past the capacity, over 127, or already in are left out.
    pub fn new(keys: &[u8]) -> Chord {
        let mut chord = Chord::default();
        for &key in keys {
            if usize::from(chord.len) == Chord::CAPACITY {
                break;
            }
            if key <= 127 && !chord.keys().contains(&key) {
                chord.keys[usize::from(chord.len)] = key;
                chord.len += 1;
            }
        }
        chord
    }

    pub fn keys(&self) -> &[u8] {
        &self.keys[..usize::from(self.len)]
    }
}

/// What the audio thread hands back while recording, in the order it happened on that thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Recorded {
    /// Sent when the record command is taken, before anything else of this recording.
    Started { from: u64, count_in_beats: u32 },
    /// A keyboard event and where the person was in the song when they played it.
    Event {
        event: InstrumentEvent,
        t_us: u64,
        /// Heard position, with the fraction. May be up to half a beat before `from`.
        tick: f64,
        lap: u32,
    },
    /// The loop went round while recording. `t_us` is when the first frame of the new lap
    /// reaches the DAC.
    LapChanged { lap: u32, t_us: u64 },
    /// The recording ended. `tick` and `lap` are the heard position at `t_us`.
    Stopped { t_us: u64, tick: f64, lap: u32 },
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
    /// Tracks 0 to `max_tracks - 1`. One more comes after them, the audition track (number
    /// `max_tracks`): a place for an instrument that is tried out before it is on a track of
    /// the song. It takes every command a track takes and has a strip like the others.
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
    returns: [[AtomicU32; 2]; 2],
    master: [AtomicU32; 2],
    clipped: AtomicBool,
    load_permille: AtomicU32,
    last_frames: AtomicU32,
    callbacks: AtomicU64,
    late_callbacks: AtomicU64,
    xruns: AtomicU64,
    streams_lost: AtomicU64,
    other_errors: AtomicU64,
    record_overflows: AtomicU64,
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
            tracks: (0..=config.max_tracks)
                .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
                .collect(),
            returns: Default::default(),
            master: [AtomicU32::new(0), AtomicU32::new(0)],
            clipped: AtomicBool::new(false),
            load_permille: AtomicU32::new(0),
            last_frames: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            late_callbacks: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            streams_lost: AtomicU64::new(0),
            other_errors: AtomicU64::new(0),
            record_overflows: AtomicU64::new(0),
            sample_rate: AtomicU32::new(config.sample_rate),
        }
    }

    /// Without the audition track, whose number this is.
    pub fn track_count(&self) -> usize {
        self.tracks.len() - 1
    }

    /// Left and right peak of a track since the last call (after its fader and pan).
    /// 1.0 = 0 dBFS.
    pub fn take_track_peak(&self, track: usize) -> [f32; 2] {
        self.tracks
            .get(track)
            .map_or([0.0; 2], |t| [take(&t[0]), take(&t[1])])
    }

    /// Left and right peak of a return (0 is A, 1 is B) since the last call, after its fader
    /// and pan.
    pub fn take_return_peak(&self, bus: usize) -> [f32; 2] {
        self.returns
            .get(bus)
            .map_or([0.0; 2], |r| [take(&r[0]), take(&r[1])])
    }

    /// Left and right peak of what goes out since the last call, before the output clamp.
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

    /// [`Recorded`] items lost because nobody emptied the ring.
    pub fn record_overflows(&self) -> u64 {
        self.record_overflows.load(Relaxed)
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

/// How long an instrument that was replaced goes on sounding, fading out under the new one, so
/// that what it was sounding does not end in a click.
pub const FADE_S: f32 = 0.010;

struct Track {
    instrument: Option<Box<dyn Instrument>>,
    /// The track's own instrument while another is tried out in its place.
    aside: Option<Box<dyn Instrument>>,
    /// Frames the one set aside still fades out for; at 0 it is reset and silent.
    aside_left: u32,
    /// The instrument that was here before, until it has faded out and the retired ring takes it.
    fading: Option<Box<dyn Instrument>>,
    fade_left: u32,
    strip: Strip,
    lane: Lane,
    /// `EngineCommand::SetLookahead`. Kept here for the reading of the sequence, which does not
    /// use it yet.
    #[allow(dead_code)]
    lookahead: u32,
}

/// The song clock on the audio thread.
///
/// Positions are kept as ticks × `scale`, where `scale` = 6000 × sample rate. One frame then
/// moves the position by exactly `bpm_centi × ticks_per_beat`, so a position in frames never
/// drifts, at any tempo.
struct Transport {
    state: TransportState,
    /// Position of the next frame to render.
    pos: i128,
    scale: i128,
    ticks_per_beat: u32,
    beats_per_bar: u32,
    bpm_centi: u32,
    looping: bool,
    loop_start: u64,
    loop_end: u64,
    /// The position has not passed `loop_end` since it last moved by a jump. A play that starts
    /// after the loop does not go round.
    before_loop_end: bool,
    metronome: bool,
    /// Next beat to click, in ticks.
    next_click: i128,
    record_from: u64,
    /// Where the current play or recording started; a stop that cannot be placed returns here.
    run_from: u64,
    muted: TrackSet,
    /// Mute and solo: never play their sequence.
    silent: TrackSet,
    session: u32,
    lap: u32,
    /// After `RecordHere`: the session whose playing stretches, already sent to the DAC, still
    /// carry keys into this recording.
    punched: Option<u32>,
}

impl Track {
    fn empty() -> Track {
        Track {
            instrument: None,
            aside: None,
            aside_left: 0,
            fading: None,
            fade_left: 0,
            strip: Strip::new(),
            lane: Lane::default(),
            lookahead: 0,
        }
    }

    /// `old` was replaced: it fades out from here. One that was still fading goes back at once.
    fn fade_out(
        &mut self,
        old: Box<dyn Instrument>,
        frames: u32,
        retired: &mut Producer<Box<dyn Instrument>>,
    ) {
        if let Some(before) = self.fading.replace(old) {
            // Cannot fail: every command is taken only while three fit, and this is the only
            // producer.
            let _ = retired.push(before);
        }
        self.fade_left = frames;
    }

    /// Nothing sounds here and nothing is about to: a change to the strip need not be smooth.
    fn is_empty(&self) -> bool {
        self.instrument.is_none() && self.fading.is_none() && self.aside.is_none()
    }

    /// Ends the fades without rendering them. Not on the audio thread while a stream runs.
    fn end_fades(&mut self, retired: &mut Producer<Box<dyn Instrument>>) {
        if self.aside_left > 0 {
            self.aside_left = 0;
            if let Some(own) = self.aside.as_mut() {
                own.reset();
            }
        }
        self.fade_left = 0;
        if !retired.is_full()
            && let Some(old) = self.fading.take()
        {
            let _ = retired.push(old);
        }
    }
}

fn fade_frames(sample_rate: f32) -> u32 {
    ((FADE_S * sample_rate) as u32).max(1)
}

fn ramp_frames(sample_rate: f32) -> u32 {
    ((RAMP_S * sample_rate) as u32).max(1)
}

impl Transport {
    fn new(sample_rate: u32) -> Self {
        Transport {
            state: TransportState::Stopped,
            pos: 0,
            scale: 6000 * i128::from(sample_rate),
            ticks_per_beat: 480,
            beats_per_bar: 4,
            bpm_centi: 12_000,
            looping: false,
            loop_start: 0,
            loop_end: 0,
            before_loop_end: false,
            metronome: false,
            next_click: 0,
            record_from: 0,
            run_from: 0,
            muted: TrackSet::default(),
            silent: TrackSet::default(),
            session: 0,
            lap: 0,
            punched: None,
        }
    }

    fn step(&self) -> i128 {
        i128::from(self.bpm_centi) * i128::from(self.ticks_per_beat)
    }

    /// Frames from the next frame until the first one at or past `tick`.
    fn frames_until(&self, tick: i128) -> i128 {
        let gap = tick * self.scale - self.pos;
        if gap <= 0 {
            0
        } else {
            (gap + self.step() - 1) / self.step()
        }
    }

    fn set_pos(&mut self, tick: i128) {
        self.pos = tick * self.scale;
        self.before_loop_end = self.pos < i128::from(self.loop_end) * self.scale;
        self.aim_click();
    }

    fn aim_click(&mut self) {
        let beat = i128::from(self.ticks_per_beat) * self.scale;
        self.next_click = -(-self.pos).div_euclid(beat) * i128::from(self.ticks_per_beat);
    }

    fn loop_active(&self) -> bool {
        self.looping
            && self.loop_end > self.loop_start
            && self.before_loop_end
            && matches!(
                self.state,
                TransportState::Playing | TransportState::Recording
            )
    }

    fn lane_plays(&self, track: usize) -> bool {
        if self.silent.contains(track) {
            return false;
        }
        match self.state {
            TransportState::Playing => true,
            TransportState::Recording => !self.muted.contains(track),
            _ => false,
        }
    }

    fn click_audible(&self) -> bool {
        match self.state {
            TransportState::CountIn => true,
            TransportState::Playing | TransportState::Recording => self.metronome,
            TransportState::Stopped => false,
        }
    }

    fn tick(&self) -> f64 {
        self.pos as f64 / self.scale as f64
    }

    fn ticks_per_frame(&self) -> f64 {
        if self.state.is_moving() {
            self.step() as f64 / self.scale as f64
        } else {
            0.0
        }
    }
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
    /// The sample seat ([`EngineCommand::PlaySample`]); only its instrument and its fade are used.
    sample: Track,
    play_target: Option<usize>,
    commands: Consumer<EngineCommand>,
    retired: Producer<Box<dyn Instrument>>,
    retired_effects: Producer<Box<dyn Effect>>,
    retired_sequences: Producer<Box<Sequence>>,
    returns: [Strip; 2],
    /// Its fader and pan come before its slots (docs/architecture.md 19-11).
    master_strip: Strip,
    midi: Consumer<MidiIn>,
    latency: Producer<LatencyRecord>,
    recorded: Producer<Recorded>,
    clock: Arc<TransportClock>,
    transport: Transport,
    click: Click,
    /// The stretch being rendered, and the frame of the device block where it began.
    segment: Segment,
    segment_from: usize,
    meters: Arc<EngineMeters>,
    /// Left and right of the track being rendered, then left and right of what fades out on it
    /// (an instrument, or a slot before its effect changed).
    scratch: [Box<[f32]>; 4],
    /// Left and right of the master, of return A and of return B, as the strips add up.
    sums: [Box<[f32]>; 6],
    sample_rate: f32,
    max_frames: usize,
    plugin_block: Arc<PluginBlock>,
    tone: TestTone,
    /// The track and keys the last `Audition` sounded, until they are let go.
    audition: Option<(usize, Chord)>,
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
        Engine::assemble(config, meters, Arc::new(PluginBlock::default()))
    }

    fn assemble(
        config: EngineConfig,
        meters: Arc<EngineMeters>,
        plugin_block: Arc<PluginBlock>,
    ) -> EngineParts {
        let (command_tx, command_rx) = RingBuffer::new(COMMAND_RING);
        // Every command retires at most three instruments, and commands wait while fewer than
        // three fit.
        let (retired_tx, retired_rx) = RingBuffer::new(config.max_tracks.max(16));
        // Room for every effect of every strip at once; commands wait while those of one
        // strip do not fit.
        let (effects_tx, effects_rx) = RingBuffer::new((config.max_tracks + 4) * 2 * INSERT_SLOTS);
        let (sequences_tx, sequences_rx) = RingBuffer::new(config.max_tracks.max(16));
        let (recorded_tx, recorded_rx) = RingBuffer::new(RECORDED_RING);
        let clock = Arc::new(TransportClock::default());
        let (midi_tx, midi_rx) = RingBuffer::new(MIDI_RING);
        let (latency_tx, latency_rx) = RingBuffer::new(LATENCY_RING);
        let buffer = || vec![0.0f32; config.max_frames].into_boxed_slice();
        let engine = Engine {
            tracks: (0..=config.max_tracks).map(|_| Track::empty()).collect(),
            sample: Track::empty(),
            play_target: Some(0),
            commands: command_rx,
            retired: retired_tx,
            retired_effects: effects_tx,
            retired_sequences: sequences_tx,
            returns: [Strip::new(), Strip::new()],
            master_strip: Strip::new(),
            midi: midi_rx,
            latency: latency_tx,
            recorded: recorded_tx,
            clock: clock.clone(),
            transport: Transport::new(config.sample_rate),
            click: Click::idle(),
            segment: Segment::default(),
            segment_from: 0,
            meters: meters.clone(),
            scratch: [buffer(), buffer(), buffer(), buffer()],
            sums: std::array::from_fn(|_| buffer()),
            sample_rate: config.sample_rate as f32,
            max_frames: config.max_frames,
            plugin_block: plugin_block.clone(),
            tone: TestTone::idle(),
            audition: None,
            last_callback: None,
        };
        let handle = EngineHandle {
            commands: command_tx,
            retired: retired_rx,
            retired_effects: effects_rx,
            retired_sequences: sequences_rx,
            latency: latency_rx,
            recorded: recorded_rx,
            clock,
            meters,
            max_frames: config.max_frames,
            plugin_block,
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
    /// this engine. Every instrument and effect is prepared again, so all notes and tails stop,
    /// and the transport stops where it was (without a [`Recorded::Stopped`]: the caller knows the stream ended).
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1) as f32;
        self.meters.sample_rate.store(sample_rate.max(1), Relaxed);
        for track in self.tracks.iter_mut() {
            for instrument in track.instrument.iter_mut().chain(track.aside.iter_mut()) {
                instrument.prepare(self.sample_rate, self.max_frames);
            }
            track.aside_left = 0;
            track.fade_left = 0;
            track.lane.forget();
        }
        self.sample.fade_left = 0;
        // What is still here when the ring is full is dropped, which is allowed on this thread.
        for track in self.tracks.iter_mut().chain([&mut self.sample]) {
            if let Some(old) = track.fading.take() {
                let _ = self.retired.push(old);
            }
        }
        if let Some(old) = self.sample.instrument.take() {
            let _ = self.retired.push(old);
        }
        let (rate, max_frames) = (self.sample_rate, self.max_frames);
        for strip in (self.tracks.iter_mut().map(|t| &mut t.strip))
            .chain(&mut self.returns)
            .chain([&mut self.master_strip])
        {
            strip.prepare(rate, max_frames, &mut self.retired_effects);
        }
        let t = &mut self.transport;
        let scale = 6000 * i128::from(sample_rate.max(1));
        t.pos = t.pos.div_euclid(t.scale) * scale;
        t.scale = scale;
        t.state = TransportState::Stopped;
        t.silent = TrackSet::default();
        self.tone = TestTone::idle();
        self.audition = None;
        self.click = Click::idle();
        self.last_callback = None;
    }

    /// Takes in the commands that wait, without rendering anything. Call outside the audio
    /// thread, while no stream is running this engine: instruments and the play target then
    /// change without a stream. It stops early while the retired rings are full; empty them
    /// ([`EngineHandle::drop_retired`]) and call again.
    pub fn settle(&mut self) {
        self.read_commands(BlockTiming::default());
        for track in self.tracks.iter_mut().chain([&mut self.sample]) {
            track.end_fades(&mut self.retired);
        }
        for strip in (self.tracks.iter_mut().map(|t| &mut t.strip))
            .chain(&mut self.returns)
            .chain([&mut self.master_strip])
        {
            strip.end_fades(&mut self.retired_effects);
        }
    }

    /// The strip, and whether a change to it is to be smooth.
    fn strip(&mut self, strip: MixStrip) -> Option<(&mut Strip, bool)> {
        match strip {
            MixStrip::Track(track) => {
                let track = self.tracks.get_mut(track)?;
                let smooth = !track.is_empty();
                Some((&mut track.strip, smooth))
            }
            MixStrip::Return(bus) => Some((self.returns.get_mut(bus)?, true)),
            MixStrip::Master => Some((&mut self.master_strip, true)),
        }
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
        // Plugins in other processes are waited for until this far into the block, all of
        // them together. The transport they are told is filled in with the plugin formats.
        let wait_ns = (block_s * PLUGIN_WAIT_SHARE * 1e9) as u64;
        self.plugin_block
            .set_deadline(meloet_plugin::ipc::futex::monotonic_ns().saturating_add(wait_ns));

        self.read_commands(timing);
        self.read_midi(timing, u64::MAX);
        self.open_segment(0, timing);
        let mut offset = 0;
        for piece in out.chunks_mut(self.max_frames * 2) {
            self.render(piece, offset, timing);
            offset += piece.len() / 2;
        }
        self.close_segment(frames);

        if block_s > 0.0 {
            let load = began.elapsed().as_secs_f64() / block_s * 1000.0;
            self.meters
                .load_permille
                .store(load.min(f64::from(u16::MAX)) as u32, Relaxed);
        }
    }

    fn read_commands(&mut self, timing: BlockTiming) {
        // Stop while a retired ring is full: an instrument, an effect or a sequence must never
        // be dropped here.
        let fade = fade_frames(self.sample_rate);
        let ramp = ramp_frames(self.sample_rate);
        while self.retired.slots() >= 3
            && self.retired_effects.slots() >= 2 * INSERT_SLOTS
            && !self.retired_sequences.is_full()
        {
            let Ok(command) = self.commands.pop() else {
                break;
            };
            match command {
                EngineCommand::SwapInstrument { track, instrument } => {
                    match self.tracks.get_mut(track) {
                        Some(slot) => {
                            slot.lane.forget();
                            if let Some(old) = slot.instrument.replace(instrument) {
                                slot.fade_out(old, fade, &mut self.retired);
                            }
                        }
                        None => {
                            // Cannot fail: checked above, and this is the only producer.
                            let _ = self.retired.push(instrument);
                        }
                    }
                    if self.transport.lane_plays(track) {
                        self.seek_lane(track);
                    }
                }
                EngineCommand::SwapSequence { track, sequence } => {
                    let old = match self.tracks.get_mut(track) {
                        Some(slot) => {
                            slot.lane.silence(slot.instrument.as_deref_mut());
                            slot.lane.sequence.replace(sequence)
                        }
                        None => Some(sequence),
                    };
                    if let Some(old) = old {
                        let _ = self.retired_sequences.push(old);
                    }
                    if self.transport.lane_plays(track) {
                        self.seek_lane(track);
                    }
                }
                EngineCommand::SetStrip { strip, gain, pan } => {
                    if let Some((strip, smooth)) = self.strip(strip) {
                        strip.set_level(gain, pan, if smooth { ramp } else { 0 });
                    }
                }
                EngineCommand::SetSends { track, sends } => {
                    if let Some((strip, smooth)) = self.strip(MixStrip::Track(track)) {
                        strip.set_sends(sends, if smooth { ramp } else { 0 });
                    }
                }
                EngineCommand::SetEffect {
                    strip,
                    slot,
                    effect,
                } => {
                    let retired = &mut self.retired_effects;
                    let place = match strip {
                        MixStrip::Track(track) => self.tracks.get_mut(track).map(|t| {
                            let smooth = !t.is_empty();
                            (&mut t.strip, smooth)
                        }),
                        MixStrip::Return(bus) => self.returns.get_mut(bus).map(|s| (s, true)),
                        MixStrip::Master => Some((&mut self.master_strip, true)),
                    };
                    match place {
                        Some((strip, smooth)) => {
                            strip.set_effect(slot, effect, if smooth { fade } else { 0 }, retired);
                        }
                        None => {
                            if let Some(effect) = effect {
                                // Cannot fail: checked above, and this is the only producer.
                                let _ = retired.push(effect);
                            }
                        }
                    }
                }
                EngineCommand::SetEffectParam {
                    strip,
                    slot,
                    param,
                    value,
                } => {
                    if let Some((strip, _)) = self.strip(strip) {
                        strip.set_param(slot, param, value);
                    }
                }
                EngineCommand::SetPlayTarget { track } => {
                    if track < self.tracks.len() {
                        self.set_play_target(Some(track));
                    }
                }
                EngineCommand::ClearPlayTarget => self.set_play_target(None),
                EngineCommand::TryInstrument { track, instrument } => {
                    match self.tracks.get_mut(track) {
                        Some(slot) => {
                            slot.lane.forget();
                            let old = slot.instrument.replace(instrument);
                            if slot.aside.is_none() {
                                slot.aside_left = if old.is_some() { fade } else { 0 };
                                slot.aside = old;
                            } else if let Some(old) = old {
                                slot.fade_out(old, fade, &mut self.retired);
                            }
                        }
                        None => {
                            let _ = self.retired.push(instrument);
                        }
                    }
                    self.replay_lane(track);
                }
                EngineCommand::EndTrial { track, keep } => {
                    let Some(slot) = self.tracks.get_mut(track) else {
                        continue;
                    };
                    let Some(mut own) = slot.aside.take() else {
                        continue;
                    };
                    if slot.aside_left > 0 {
                        slot.aside_left = 0;
                        own.reset();
                    }
                    if keep {
                        let _ = self.retired.push(own);
                    } else {
                        slot.lane.forget();
                        if let Some(tried) = slot.instrument.replace(own) {
                            slot.fade_out(tried, fade, &mut self.retired);
                        }
                        self.replay_lane(track);
                    }
                }
                EngineCommand::MoveInstrument { from, to } => {
                    if from == to || from >= self.tracks.len() || to >= self.tracks.len() {
                        continue;
                    }
                    self.audition_off();
                    let moved = self.tracks[from].instrument.take();
                    self.tracks[from].lane.forget();
                    let slot = &mut self.tracks[to];
                    slot.lane.forget();
                    if let Some(old) = std::mem::replace(&mut slot.instrument, moved) {
                        let _ = self.retired.push(old);
                    }
                    if self.play_target == Some(from) {
                        self.play_target = Some(to);
                    }
                    self.replay_lane(to);
                }
                EngineCommand::ClearTrack { track } => {
                    let Some(slot) = self.tracks.get_mut(track) else {
                        continue;
                    };
                    slot.lane.forget();
                    slot.strip.clear(&mut self.retired_effects);
                    slot.aside_left = 0;
                    slot.fade_left = 0;
                    let all = [
                        slot.instrument.take(),
                        slot.aside.take(),
                        slot.fading.take(),
                    ];
                    for old in all.into_iter().flatten() {
                        let _ = self.retired.push(old);
                    }
                    if let Some(old) = slot.lane.sequence.take() {
                        let _ = self.retired_sequences.push(old);
                    }
                    if self.audition.is_some_and(|(t, _)| t == track) {
                        self.audition = None;
                    }
                }
                EngineCommand::PlaySample { instrument } => {
                    if let Some(old) = self.sample.instrument.replace(instrument) {
                        self.sample.fade_out(old, fade, &mut self.retired);
                    }
                }
                EngineCommand::StopSample => {
                    if let Some(old) = self.sample.instrument.take() {
                        self.sample.fade_out(old, fade, &mut self.retired);
                    }
                }
                EngineCommand::TestTone => self.tone.start(self.sample_rate),
                EngineCommand::AllNotesOff => {
                    self.audition = None;
                    for instrument in self.tracks.iter_mut().filter_map(|t| t.instrument.as_mut()) {
                        instrument.event(InstrumentEvent::AllNotesOff);
                    }
                }
                EngineCommand::Audition {
                    track,
                    keys,
                    velocity,
                } => {
                    self.audition_off();
                    if let Some(instrument) = self
                        .tracks
                        .get_mut(track)
                        .and_then(|t| t.instrument.as_mut())
                    {
                        for &note in keys.keys() {
                            instrument.event(InstrumentEvent::NoteOn { note, velocity });
                        }
                        self.audition = Some((track, keys));
                    }
                }
                EngineCommand::AuditionOff => self.audition_off(),
                EngineCommand::SetSilentTracks { tracks } => self.set_silent(tracks),
                EngineCommand::SetTempo {
                    ticks_per_beat,
                    beats_per_bar,
                    bpm_centi,
                } => {
                    if ticks_per_beat > 0 && beats_per_bar > 0 && bpm_centi > 0 {
                        let t = &mut self.transport;
                        t.ticks_per_beat = ticks_per_beat;
                        t.beats_per_bar = beats_per_bar;
                        t.bpm_centi = bpm_centi;
                        t.aim_click();
                    }
                }
                EngineCommand::SetLoop { start, end, on } => {
                    let t = &mut self.transport;
                    t.looping = on;
                    t.loop_start = start;
                    t.loop_end = end;
                    t.before_loop_end = t.pos < i128::from(end) * t.scale;
                }
                EngineCommand::SetMetronome { on } => self.transport.metronome = on,
                EngineCommand::SetLookahead { track, frames } => {
                    if let Some(slot) = self.tracks.get_mut(track) {
                        slot.lookahead = frames;
                    }
                }
                EngineCommand::Play { from } => {
                    self.end_recording(timing);
                    self.silence_lanes();
                    self.transport.state = TransportState::Playing;
                    self.transport.run_from = from;
                    self.transport.set_pos(i128::from(from));
                    self.seek_lanes();
                }
                EngineCommand::Record {
                    from,
                    count_in_beats,
                    muted,
                } => {
                    self.end_recording(timing);
                    self.silence_lanes();
                    let t = &mut self.transport;
                    t.session = t.session.wrapping_add(1);
                    t.punched = None;
                    t.lap = 0;
                    t.record_from = from;
                    t.run_from = from;
                    t.muted = muted;
                    self.push_recorded(Recorded::Started {
                        from,
                        count_in_beats,
                    });
                    let t = &mut self.transport;
                    if count_in_beats == 0 {
                        t.state = TransportState::Recording;
                        t.set_pos(i128::from(from));
                        self.seek_lanes();
                    } else {
                        t.state = TransportState::CountIn;
                        t.set_pos(
                            i128::from(from)
                                - i128::from(count_in_beats) * i128::from(t.ticks_per_beat),
                        );
                    }
                }
                EngineCommand::RecordHere { muted } => self.record_here(muted, timing),
                EngineCommand::Stop { t_us } => self.stop(t_us, timing),
                EngineCommand::Locate { tick } => {
                    self.end_recording(timing);
                    let t = &mut self.transport;
                    t.run_from = tick;
                    t.set_pos(i128::from(tick));
                    if t.state.is_moving() {
                        self.silence_lanes();
                        self.seek_lanes();
                    }
                }
            }
        }
    }

    fn set_play_target(&mut self, target: Option<usize>) {
        if target == self.play_target {
            return;
        }
        if let Some(instrument) = self
            .play_target
            .and_then(|t| self.tracks[t].instrument.as_mut())
        {
            instrument.event(InstrumentEvent::AllNotesOff);
        }
        self.play_target = target;
    }

    /// After a track got another instrument: its sequence picks up at the position.
    fn replay_lane(&mut self, track: usize) {
        if self.transport.lane_plays(track) {
            self.seek_lane(track);
        }
    }

    fn audition_off(&mut self) {
        if let Some((track, keys)) = self.audition.take()
            && let Some(instrument) = self.tracks[track].instrument.as_mut()
        {
            for &note in keys.keys() {
                instrument.event(InstrumentEvent::NoteOff { note });
            }
        }
    }

    fn set_silent(&mut self, tracks: TrackSet) {
        let before = self.transport.silent;
        self.transport.silent = tracks;
        for i in 0..self.tracks.len() {
            match (before.contains(i), tracks.contains(i)) {
                (false, true) => {
                    let slot = &mut self.tracks[i];
                    slot.lane.silence(slot.instrument.as_deref_mut());
                }
                (true, false) if self.transport.lane_plays(i) => self.seek_lane(i),
                _ => {}
            }
        }
    }

    /// Takes in the keyboard first: keys played before `t_us` still belong to the recording.
    fn stop(&mut self, t_us: u64, timing: BlockTiming) {
        self.read_midi(timing, t_us);
        let t = &self.transport;
        let heard_then = self
            .clock
            .segment_at(t_us as f64)
            .filter(|s| s.session == t.session)
            .map(|s| (s.state, heard(&s, t_us as f64)));
        if matches!(t.state, TransportState::CountIn | TransportState::Recording) {
            let (tick, lap) = match heard_then {
                Some((TransportState::CountIn | TransportState::Recording, h)) => (h.tick, h.lap),
                _ => (t.record_from as f64, 0),
            };
            self.push_recorded(Recorded::Stopped { t_us, tick, lap });
        }
        let to = match heard_then {
            Some((TransportState::Playing | TransportState::Recording, h)) => {
                (h.tick.floor() as i128).max(0)
            }
            _ => i128::from(self.transport.run_from),
        };
        self.silence_lanes();
        self.transport.state = TransportState::Stopped;
        self.transport.set_pos(to);
    }

    fn record_here(&mut self, muted: TrackSet, timing: BlockTiming) {
        let now = timing.callback_us as f64;
        let t = &self.transport;
        if t.state != TransportState::Playing {
            let from = t.tick().max(0.0).floor() as u64;
            self.push_recorded(Recorded::Started {
                from,
                count_in_beats: 0,
            });
            self.push_recorded(Recorded::Stopped {
                t_us: timing.callback_us,
                tick: from as f64,
                lap: 0,
            });
            return;
        }
        let heard_tick = self
            .clock
            .segment_at(now)
            .filter(|s| s.session == t.session && s.state == TransportState::Playing)
            .map_or_else(|| t.tick(), |s| heard(&s, now).tick);
        let from = heard_tick.max(0.0).floor() as u64;
        let t = &mut self.transport;
        t.punched = Some(t.session);
        t.session = t.session.wrapping_add(1);
        t.lap = 0;
        t.record_from = from;
        t.run_from = from;
        t.muted = muted;
        t.state = TransportState::Recording;
        self.push_recorded(Recorded::Started {
            from,
            count_in_beats: 0,
        });
        for (i, slot) in self.tracks.iter_mut().enumerate() {
            if muted.contains(i) {
                slot.lane.silence(slot.instrument.as_deref_mut());
            }
        }
    }

    /// A command that starts something else while recording first ends the recording, as if
    /// stopped at the start of this block.
    fn end_recording(&mut self, timing: BlockTiming) {
        if matches!(
            self.transport.state,
            TransportState::CountIn | TransportState::Recording
        ) {
            self.stop(timing.callback_us, timing);
        }
    }

    fn push_recorded(&mut self, item: Recorded) {
        if self.recorded.push(item).is_err() {
            self.meters.record_overflows.fetch_add(1, Relaxed);
        }
    }

    fn silence_lanes(&mut self) {
        for track in self.tracks.iter_mut() {
            track.lane.silence(track.instrument.as_deref_mut());
        }
    }

    /// Puts each playing lane at the transport position and chases its controls.
    fn seek_lanes(&mut self) {
        for i in 0..self.tracks.len() {
            if self.transport.lane_plays(i) {
                self.seek_lane(i);
            }
        }
    }

    fn seek_lane(&mut self, track: usize) {
        let (pos, scale) = (self.transport.pos, self.transport.scale);
        if let Some(slot) = self.tracks.get_mut(track) {
            slot.lane.seek(pos, scale, slot.instrument.as_deref_mut());
        }
    }

    /// Keys played after `record_until` sound but are not recorded.
    fn read_midi(&mut self, timing: BlockTiming, record_until: u64) {
        while let Ok(midi) = self.midi.pop() {
            if matches!(midi.event, InstrumentEvent::NoteOn { velocity, .. } if velocity > 0) {
                // Every event starts at frame 0 of this block, so `playback_us` is its first sample.
                let _ = self.latency.push(LatencyRecord {
                    key_us: midi.t_us,
                    callback_us: timing.callback_us,
                    playback_us: timing.playback_us,
                });
            }
            if let Some(instrument) = self
                .play_target
                .and_then(|t| self.tracks[t].instrument.as_mut())
            {
                instrument.event(midi.event);
            }
            if midi.t_us <= record_until {
                self.stamp(midi);
            }
        }
    }

    /// Sends a keyboard event to the recording with the position the person heard when they
    /// played it. Keys from the count-in are kept only within half a beat of the start.
    fn stamp(&mut self, midi: MidiIn) {
        let t = &self.transport;
        if !matches!(t.state, TransportState::CountIn | TransportState::Recording) {
            return;
        }
        let Some(segment) = self.clock.segment_at(midi.t_us as f64) else {
            return;
        };
        let punched =
            t.punched == Some(segment.session) && segment.state == TransportState::Playing;
        if segment.session != t.session && !punched {
            return;
        }
        let h = heard(&segment, midi.t_us as f64);
        let near_start = h.tick >= t.record_from as f64 - f64::from(t.ticks_per_beat) / 2.0;
        let wanted = match h.state {
            TransportState::Recording => true,
            TransportState::CountIn => near_start,
            TransportState::Playing => punched && near_start,
            TransportState::Stopped => false,
        };
        if wanted {
            self.push_recorded(Recorded::Event {
                event: midi.event,
                t_us: midi.t_us,
                tick: h.tick,
                lap: if punched { 0 } else { h.lap },
            });
        }
    }

    fn dac_us(&self, frame: usize, timing: BlockTiming) -> f64 {
        timing.playback_us as f64 + frame as f64 * 1e6 / f64::from(self.sample_rate)
    }

    fn open_segment(&mut self, frame: usize, timing: BlockTiming) {
        let t = &self.transport;
        self.segment = Segment {
            start_us: self.dac_us(frame, timing),
            frames: 0,
            sample_rate: self.sample_rate as u32,
            tick: t.tick(),
            ticks_per_frame: t.ticks_per_frame(),
            state: t.state,
            session: t.session,
            lap: t.lap,
            record_from: t.record_from as i64,
            ticks_per_beat: t.ticks_per_beat,
        };
        self.segment_from = frame;
    }

    fn close_segment(&mut self, frame: usize) {
        if frame > self.segment_from {
            self.segment.frames = (frame - self.segment_from) as u32;
            self.clock.push(&self.segment);
        }
    }

    /// Jumps (end of count-in, loop end) and everything due at the next frame. `frame` counts
    /// from the start of the device block.
    fn step_at(&mut self, frame: usize, timing: BlockTiming) {
        if !self.transport.state.is_moving() {
            return;
        }
        let t = &mut self.transport;
        if t.state == TransportState::CountIn && t.pos >= i128::from(t.record_from) * t.scale {
            self.close_segment(frame);
            let t = &mut self.transport;
            t.state = TransportState::Recording;
            t.set_pos(i128::from(t.record_from));
            self.seek_lanes();
            self.open_segment(frame, timing);
        }
        let t = &mut self.transport;
        if t.loop_active() && t.pos >= i128::from(t.loop_end) * t.scale {
            self.close_segment(frame);
            self.silence_lanes();
            let t = &mut self.transport;
            t.set_pos(i128::from(t.loop_start));
            if t.state == TransportState::Recording {
                t.lap += 1;
                let lap = t.lap;
                let t_us = self.dac_us(frame, timing).round() as u64;
                self.push_recorded(Recorded::LapChanged { lap, t_us });
            }
            self.seek_lanes();
            self.open_segment(frame, timing);
        }

        let (pos, scale) = (self.transport.pos, self.transport.scale);
        for (i, track) in self.tracks.iter_mut().enumerate() {
            if self.transport.lane_plays(i) {
                track
                    .lane
                    .fire_due(pos, scale, track.instrument.as_deref_mut());
            }
        }
        let t = &mut self.transport;
        if t.next_click * t.scale <= t.pos {
            if t.click_audible() {
                let bar = i128::from(t.ticks_per_beat) * i128::from(t.beats_per_bar);
                self.click
                    .start(t.next_click.rem_euclid(bar) == 0, self.sample_rate);
            }
            t.next_click += i128::from(t.ticks_per_beat);
        }
    }

    /// Frames from the next frame to the next one where [`Engine::step_at`] has work, at most
    /// `limit` and at least 1.
    fn frames_to_next_point(&self, limit: usize) -> usize {
        let t = &self.transport;
        if !t.state.is_moving() {
            return limit;
        }
        let mut n = (limit as i128).min(t.frames_until(t.next_click));
        if t.state == TransportState::CountIn {
            n = n.min(t.frames_until(i128::from(t.record_from)));
        }
        if t.loop_active() {
            n = n.min(t.frames_until(i128::from(t.loop_end)));
        }
        for (i, track) in self.tracks.iter().enumerate() {
            if t.lane_plays(i)
                && let Some(tick) = track.lane.next_tick()
            {
                n = n.min(t.frames_until(i128::from(tick)));
            }
        }
        n.max(1) as usize
    }

    /// Renders at most `max_frames` frames. `offset` is the frame of the device block where
    /// this piece begins.
    fn render(&mut self, out: &mut [f32], offset: usize, timing: BlockTiming) {
        let frames = out.len() / 2;
        if frames == 0 {
            return;
        }
        for sum in &mut self.sums {
            sum[..frames].fill(0.0);
        }
        let fade = fade_frames(self.sample_rate);
        let t = &self.transport;
        let ctx = EffectContext {
            tempo_bpm: t.bpm_centi as f32 / 100.0 * t.ticks_per_beat as f32 / 480.0,
        };

        let mut from = 0;
        while from < frames {
            self.step_at(offset + from, timing);
            let to = from + self.frames_to_next_point(frames - from);
            let [master_l, master_r, sums @ ..] = &mut self.sums;
            let [a_l, a_r, b_l, b_r] = sums.each_mut().map(|sum| &mut sum[from..to]);
            let mut mix = Mix {
                scratch: &mut self.scratch,
                retired: &mut self.retired,
                retired_effects: &mut self.retired_effects,
                fade_frames: fade,
                ctx,
                master_l: &mut master_l[from..to],
                master_r: &mut master_r[from..to],
                returns: [[a_l, a_r], [b_l, b_r]],
            };
            for (track, meter) in self.tracks.iter_mut().zip(self.meters.tracks.iter()) {
                mix.add(track, Some(meter));
            }
            mix.add(&mut self.sample, None);
            self.click.add(mix.master_l, mix.master_r);
            if self.transport.state.is_moving() {
                self.transport.pos += self.transport.step() * (to - from) as i128;
            }
            from = to;
        }

        let [master_l, master_r, sums @ ..] = &mut self.sums;
        let (master_l, master_r) = (&mut master_l[..frames], &mut master_r[..frames]);
        let [_, _, spare_l, spare_r] = &mut self.scratch;
        let sums = sums.as_chunks_mut::<2>().0;
        for ((strip, [left, right]), meter) in (self.returns.iter_mut())
            .zip(sums)
            .zip(&self.meters.returns)
        {
            let mut io = Io {
                left: &mut left[..frames],
                right: &mut right[..frames],
                spare_l,
                spare_r,
                ctx,
                fade_frames: fade,
            };
            strip.inserts(&mut io, &mut self.retired_effects);
            let (peak, finite) = strip.level(io.left, io.right);
            if !finite {
                continue;
            }
            raise(&meter[0], peak[0]);
            raise(&meter[1], peak[1]);
            for i in 0..frames {
                master_l[i] += io.left[i];
                master_r[i] += io.right[i];
            }
        }

        self.master_strip.level(master_l, master_r);
        let mut io = Io {
            left: master_l,
            right: master_r,
            spare_l,
            spare_r,
            ctx,
            fade_frames: fade,
        };
        self.master_strip
            .inserts(&mut io, &mut self.retired_effects);
        // After the master's strip: the test tone tells whether the output sounds, whatever
        // the song's mixer does.
        self.tone.add(io.left, io.right, self.sample_rate);

        let mut peak = [0.0f32; 2];
        for (frame, (l, r)) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(io.left.iter().zip(io.right.iter()))
        {
            // What is not a number never reaches the output.
            let (l, r) = if l.is_finite() && r.is_finite() {
                (*l, *r)
            } else {
                (0.0, 0.0)
            };
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

/// One stretch of the master, and what it takes to render a track into it.
struct Mix<'a> {
    scratch: &'a mut [Box<[f32]>; 4],
    retired: &'a mut Producer<Box<dyn Instrument>>,
    retired_effects: &'a mut Producer<Box<dyn Effect>>,
    fade_frames: u32,
    ctx: EffectContext,
    master_l: &'a mut [f32],
    master_r: &'a mut [f32],
    /// The same stretch of what goes to return A and to return B.
    returns: [[&'a mut [f32]; 2]; 2],
}

impl Mix<'_> {
    /// Renders `track` over the stretch, with what fades out on it, takes it through its
    /// strip, and adds it to the master and to the returns it sends to.
    fn add(&mut self, track: &mut Track, meter: Option<&[AtomicU32; 2]>) {
        let frames = self.master_l.len();
        let [left, right, old_l, old_r] = &mut *self.scratch;
        let (left, right) = (&mut left[..frames], &mut right[..frames]);
        let aside_fading = track.aside_left > 0;
        let fading = track.fade_left > 0 || aside_fading;
        match track.instrument.as_mut() {
            Some(instrument) => instrument.render(left, right),
            None if fading => {
                left.fill(0.0);
                right.fill(0.0);
            }
            None => {
                retire_faded(track, self.retired);
                return;
            }
        }
        for (old, frames_left) in [
            (&mut track.fading, &mut track.fade_left),
            (&mut track.aside, &mut track.aside_left),
        ] {
            let (Some(old), true) = (old.as_mut(), *frames_left > 0) else {
                *frames_left = 0;
                continue;
            };
            let n = frames.min(*frames_left as usize);
            let (old_l, old_r) = (&mut old_l[..n], &mut old_r[..n]);
            old.render(old_l, old_r);
            for i in 0..n {
                let level = (*frames_left - i as u32) as f32 / self.fade_frames as f32;
                left[i] += old_l[i] * level;
                right[i] += old_r[i] * level;
            }
            *frames_left -= n as u32;
        }
        if aside_fading && track.aside_left == 0 {
            // Silent from here on, and it comes back with nothing held.
            if let Some(own) = track.aside.as_mut() {
                own.reset();
            }
        }
        retire_faded(track, self.retired);

        let mut io = Io {
            left,
            right,
            spare_l: old_l,
            spare_r: old_r,
            ctx: self.ctx,
            fade_frames: self.fade_frames,
        };
        track.strip.inserts(&mut io, self.retired_effects);
        let (peak, finite) = track.strip.level(io.left, io.right);
        // An instrument that gives something that is not a number is left out of the sums:
        // one track must not take the master and the returns with it.
        if !finite {
            return;
        }
        if let Some(meter) = meter {
            raise(&meter[0], peak[0]);
            raise(&meter[1], peak[1]);
        }
        for i in 0..frames {
            self.master_l[i] += io.left[i];
            self.master_r[i] += io.right[i];
        }
        for (bus, [to_l, to_r]) in self.returns.iter_mut().enumerate() {
            track.strip.send(bus, io.left, io.right, [to_l, to_r]);
        }
    }
}

/// An instrument that has faded out waits on its track for room in the ring.
fn retire_faded(track: &mut Track, retired: &mut Producer<Box<dyn Instrument>>) {
    if track.fade_left == 0
        && !retired.is_full()
        && let Some(old) = track.fading.take()
    {
        let _ = retired.push(old);
    }
}

/// The part the control side (core) owns.
pub struct EngineHandle {
    commands: Producer<EngineCommand>,
    retired: Consumer<Box<dyn Instrument>>,
    retired_effects: Consumer<Box<dyn Effect>>,
    retired_sequences: Consumer<Box<Sequence>>,
    latency: Consumer<LatencyRecord>,
    recorded: Consumer<Recorded>,
    clock: Arc<TransportClock>,
    meters: Arc<EngineMeters>,
    max_frames: usize,
    plugin_block: Arc<PluginBlock>,
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

    /// Prepares `instrument` here (this may allocate) and sends it to `track` for a trial
    /// ([`EngineCommand::TryInstrument`]).
    pub fn try_instrument(
        &mut self,
        track: usize,
        mut instrument: Box<dyn Instrument>,
    ) -> Result<(), EngineCommand> {
        instrument.prepare(self.meters.sample_rate() as f32, self.max_frames);
        self.send(EngineCommand::TryInstrument { track, instrument })
    }

    /// Prepares `instrument` here (this may allocate) and puts it on the sample seat
    /// ([`EngineCommand::PlaySample`]).
    pub fn play_sample(
        &mut self,
        mut instrument: Box<dyn Instrument>,
    ) -> Result<(), EngineCommand> {
        instrument.prepare(self.meters.sample_rate() as f32, self.max_frames);
        self.send(EngineCommand::PlaySample { instrument })
    }

    /// Prepares `effect` here (this may allocate) and puts it into an insert slot
    /// ([`EngineCommand::SetEffect`]); `None` empties the slot. Give the effect its parameter
    /// values first: they then apply from its first frame.
    pub fn set_effect(
        &mut self,
        strip: MixStrip,
        slot: usize,
        mut effect: Option<Box<dyn Effect>>,
    ) -> Result<(), EngineCommand> {
        if let Some(effect) = effect.as_mut() {
            effect.prepare(self.meters.sample_rate() as f32, self.max_frames);
        }
        self.send(EngineCommand::SetEffect {
            strip,
            slot,
            effect,
        })
    }

    /// The number of the audition track, the one after the tracks.
    pub fn audition_track(&self) -> usize {
        self.meters.track_count()
    }

    /// Sends `sequence` to `track`. The old one comes back through [`EngineHandle::drop_retired`]
    /// or [`EngineHandle::take_retired_sequence`].
    pub fn swap_sequence(&mut self, track: usize, sequence: Sequence) -> Result<(), EngineCommand> {
        self.send(EngineCommand::SwapSequence {
            track,
            sequence: Box::new(sequence),
        })
    }

    pub fn take_retired_sequence(&mut self) -> Option<Box<Sequence>> {
        self.retired_sequences.pop().ok()
    }

    /// The next instrument the audio thread is done with. Dropping it is the caller's job.
    pub fn take_retired(&mut self) -> Option<Box<dyn Instrument>> {
        self.retired.pop().ok()
    }

    /// The next effect the audio thread is done with. Dropping it is the caller's job.
    pub fn take_retired_effect(&mut self) -> Option<Box<dyn Effect>> {
        self.retired_effects.pop().ok()
    }

    /// Drops every retired instrument, effect and sequence on this thread. Call it now and then (it is
    /// what lets further commands through once a retired ring has filled). Returns how many.
    pub fn drop_retired(&mut self) -> usize {
        let mut count = 0;
        while self.take_retired().is_some() {
            count += 1;
        }
        while self.take_retired_effect().is_some() {
            count += 1;
        }
        while self.take_retired_sequence().is_some() {
            count += 1;
        }
        count
    }

    pub fn pop_recorded(&mut self) -> Option<Recorded> {
        self.recorded.pop().ok()
    }

    /// The heard song position, readable from any thread without waiting. After
    /// [`EngineHandle::rebuild`] this is a new clock.
    pub fn clock(&self) -> &Arc<TransportClock> {
        &self.clock
    }

    /// A new, empty engine in place of one that was lost (a stream that never gave it back).
    /// This handle is rewired to it; the meters stay the same `Arc`. Every track starts without
    /// an instrument, every strip is a new one, and track 0 is the play target. The returned producer
    /// replaces the old one, whose ring nobody reads any more.
    pub fn rebuild(&mut self) -> (Engine, MidiFeed) {
        let config = EngineConfig {
            sample_rate: self.meters.sample_rate(),
            max_frames: self.max_frames,
            max_tracks: self.meters.track_count(),
        };
        let parts = Engine::assemble(config, self.meters.clone(), self.plugin_block.clone());
        *self = parts.handle;
        (parts.engine, parts.midi_in)
    }

    /// What every [`crate::PluginSlot`] of this engine is made with. The same `Arc` after
    /// [`EngineHandle::rebuild`], so the slots made before it still read the right block.
    pub fn plugin_block(&self) -> &Arc<PluginBlock> {
        &self.plugin_block
    }

    pub fn pop_latency(&mut self) -> Option<LatencyRecord> {
        self.latency.pop().ok()
    }

    pub fn meters(&self) -> &Arc<EngineMeters> {
        &self.meters
    }
}
