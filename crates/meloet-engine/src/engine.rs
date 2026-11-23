//! The mixer the audio thread owns ([`Engine`]) and its control side ([`EngineHandle`]).
//!
//! Between the two there are only `rtrb` rings and atomics.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

use rtrb::{Consumer, Producer, RingBuffer};

use crate::analysis::{AnalysisReader, AnalysisTap, analysis_ring};
use crate::automation::{AutoClock, AutoSet, HOLDS, MIX_STEP};
use crate::clipplay::{ClipPlayer, StreamCtx, StreamNow, StreamPlace};
use crate::clips::ClipTable;
use crate::clock::{HeardPosition, Segment, TransportClock, TransportState, heard};
use crate::effect::{Effect, EffectContext};
use crate::input::{InputFeed, MAX_INPUT_CHANNELS};
use crate::instrument::{Instrument, InstrumentEvent};
use crate::knobs::{KNOB_DEFAULTS, KNOBS, knob_place};
use crate::mixer::{INSERT_SLOTS, Io, MixStrip, RAMP_S, SlotAuto, Strip};
use crate::plugin::{PLUGIN_WAIT_SHARE, PluginBlock};
use crate::probe::probe_wave;
use crate::rt;
use crate::stream::{STREAM_CHANNELS, StreamReader, StreamWant, want_ring};
use crate::transport::{Click, ClickSound, Lane, Sequence, TrackSet};
use meloet_model::{Arp, AutoTarget, Beat, Meter, TICKS_PER_QUARTER, Tempo, TempoMap, Timeline};
use meloet_plugin::ipc::Transport as PluginTransport;

mod arp;
use arp::ArpPlayer;

const COMMAND_RING: usize = 256;
const MIDI_RING: usize = 1024;
const TYPED_RING: usize = 256;
const LATENCY_RING: usize = 1024;
const RECORDED_RING: usize = 4096;
/// Slots of that ring left to what a recording says: the keys played outside one
/// ([`Recorded::Recent`]) stop coming while fewer are free.
const RECORDED_HEADROOM: usize = 256;
const MAP_RING: usize = 16;
const INPUT_RING: usize = 4;
const PROBE_RING: usize = 16;
/// Wants of every track at once, a few times over.
const WANTS_PER_TRACK: usize = 4;
/// Ticks per second for one quarter note per minute.
const TICKS_PER_S_PER_BPM: f64 = TICKS_PER_QUARTER as f64 / 60.0;

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
    /// 「とばす」 and 「もどす」: the sound passes the slot by, or goes through its effect again;
    /// one fades into the other over [`FADE_S`]. The effect keeps running while it is passed
    /// by, so its tail is there when it comes back. The mark belongs to the slot: an effect
    /// put into it later comes in passed by, and `MoveEffect` takes it along.
    SetBypass {
        strip: MixStrip,
        slot: usize,
        bypassed: bool,
    },
    /// The slot `from` goes to `to` and the slots between move by one, with the effects as
    /// they sound. When that changes the order of effects that are heard, the moved one fades
    /// out of the chain over [`FADE_S`] and into it at its new place over another. Until it
    /// has landed, the other commands already number the slots as they will be.
    MoveEffect {
        strip: MixStrip,
        from: usize,
        to: usize,
    },
    /// The knobs of the track's sound as the hand put them ([`Instrument::set_knob`], places
    /// 0..=1 in the order of `crate::knobs`). Kept for the track, whatever instrument comes:
    /// the track's own instrument takes each one that no lane plays now (not the one tried
    /// out in its place); a knob whose lane stops playing, or is held, comes back to it.
    SetKnobs {
        track: usize,
        knobs: [f32; KNOBS],
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
    /// Use [`EngineHandle::grow_sequence`]. As `SwapSequence`, for a sequence that has every
    /// event of the old one and more: what sounds goes on, and nothing the old one sent
    /// already is sent again.
    GrowSequence {
        track: usize,
        sequence: Box<Sequence>,
    },
    /// Use [`EngineHandle::set_tempo_map`]. The song's tempo and meter from now on; the
    /// position stays the tick it is at. The old map comes back on a retired ring. `map_gen`
    /// tells the maps apart and is written into every stretch of the clock.
    SetTempoMap {
        map: Arc<TempoMap>,
        map_gen: u32,
    },
    /// Use [`EngineHandle::set_automation`]. The lanes of a track that play from now on, in
    /// place of the ones before (`None`: no lane plays); the old set comes back on a retired
    /// ring. A fader, a pan or a send goes on from where the set before had got it to.
    SetAutomation {
        track: usize,
        set: Option<Box<AutoSet>>,
    },
    /// A person has hold of what the lane moves (docs/architecture.md 21-9, 21-11): until
    /// `on` is false the lane plays nothing, and the value is the hand's. Kept for the track,
    /// whatever sets come; at most [`HOLDS`] at once.
    HoldLane {
        track: usize,
        target: AutoTarget,
        on: bool,
    },
    /// Use [`EngineHandle::set_input`]. The PC's input, for the voices of the vocal tracks,
    /// in place of the one before (`None`: no input is open); the old feed comes back on a
    /// retired ring.
    SetInput {
        feed: Option<Box<InputFeed>>,
    },
    /// The track sounds the input instead of its instrument (a vocal track), or its
    /// instrument again (`None`). The voice comes and goes over [`RAMP_S`].
    SetVoice {
        track: usize,
        voice: Option<Voice>,
    },
    /// Use [`EngineHandle::set_stream`]. The ring the read-ahead thread fills with the
    /// audio of the track's vocal clips, in place of the one before (`None`: the track has
    /// none); the old reader comes back on a retired ring. The thread is told where to read
    /// from with a [`StreamWant`] of a new generation.
    SetStream {
        track: usize,
        reader: Option<Box<StreamReader>>,
    },
    /// Use [`EngineHandle::set_clips`]. Where the track's vocal clips are, their fades and
    /// their volumes, in place of the table before, which comes back on a retired ring.
    /// `refeed`: the clips are other audio or at other places now, so the thread starts
    /// again (a new generation; the track fades out over [`FADE_S`] and comes back when the
    /// audio does). Without it only volumes and fades changed: what the old table said goes
    /// over to what the new one says in [`RAMP_S`], and the ring stays as it is.
    SetClips {
        track: usize,
        table: Option<Box<ClipTable>>,
        refeed: bool,
    },
    /// The read-ahead thread has other audio for the clips of the track, in the same places
    /// (audio that was made for a clip is ready, 22-19 pass 2), and lays it over what it had, from
    /// where it is in the ring on: the ring stays, and what sounds goes on without a gap.
    /// When nothing of the track sounds (the song stands still, the track is muted, the
    /// place just jumped) a new generation starts instead, so that the seconds that wait
    /// in the ring are the new audio.
    SpliceClips {
        track: usize,
    },
    /// One click of the latency measurement, from the first frame of the next block, after
    /// the master. Which frame that was comes back as a [`ProbeSent`].
    Probe,
    /// `[start, end)` in ticks. A loop with `end <= start` does nothing.
    SetLoop {
        start: u64,
        end: u64,
        on: bool,
    },
    SetMetronome {
        on: bool,
    },
    /// How the metronome sounds from its next click on; a click that sounds already is left
    /// as it is. `gain` is linear (1.0: the click as made; at most 2.0). `recording_only`:
    /// no clicks while playing. The count-in clicks either way.
    SetClick {
        sound: ClickSound,
        gain: f32,
        recording_only: bool,
    },
    Play {
        from: u64,
    },
    /// Count `count_in_beats` clicks of the metronome ([`Meter::ticks_per_click`]), then record
    /// from `from`. The tracks in `muted` do not play
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
    /// While recording: the tracks that do not play their sequence from now on, in place of
    /// the `muted` of `Record` or `RecordHere`. A track that goes quiet lets go of what its
    /// sequence holds; one that comes back picks up at the position.
    SetRecordMuted {
        muted: TrackSet,
    },
    /// `t_us` is when the person asked to stop ([`monotonic_us`]). Keys after it are not recorded.
    Stop {
        t_us: u64,
    },
    Locate {
        tick: u64,
    },
    /// 「アルペジオ」 of a track. While it is on and the track is the play target, the keys
    /// of the keyboards do not sound: they are the chord, whose notes sound one at a time on
    /// the song's grid (while it stands still, from the first key on at the tempo there),
    /// and those notes are what is written down as played. Sequences, auditions and
    /// controllers are left alone. Turning it or its hold off lets go of what it holds.
    SetArp {
        track: usize,
        arp: Arp,
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

/// What a vocal track sounds: channels of the input, while its ● and its monitor are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Voice {
    /// The input's channel that goes left, and the one that goes right (the same one for a
    /// mono input).
    pub left: u8,
    pub right: u8,
    /// Off: silence, with the strip still running.
    pub on: bool,
}

/// A click of the latency measurement left the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeSent {
    /// The click's first frame, counted as [`Recorded::Rolling::frame`] is.
    pub frame: u64,
}

/// What the audio thread hands back while recording, in the order it happened on that thread.
///
/// `frame` counts every frame the engine rendered since it was made (the output's frames,
/// as [`crate::InputBlock::first_frame`] counts the input's): with it a recording of the
/// input is placed by counting frames, not by reading the time again
/// (docs/architecture.md 22-6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Recorded {
    /// Sent when the record command is taken, before anything else of this recording.
    /// `frame`: the first frame of the block that took it.
    Started {
        from: u64,
        count_in_beats: u32,
        frame: u64,
    },
    /// The song is at `tick` at `frame` and is recorded from there: at once, or when the
    /// count-in ends.
    Rolling { frame: u64, tick: f64 },
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
    /// `frame`: the first frame of the new lap, where the song is at the loop's start.
    LapChanged { lap: u32, t_us: u64, frame: u64 },
    /// The recording ended. `tick` and `lap` are the heard position at `t_us`. `frame`: the
    /// first frame of the block that took the stop.
    Stopped {
        t_us: u64,
        tick: f64,
        lap: u32,
        frame: u64,
    },
    /// A keyboard event while the transport is stopped or plays (not counting in, not
    /// recording): what was played can still be kept afterwards. `run` names the stretch of
    /// the transport the key belongs to: a new run begins whenever the play starts, stops or
    /// jumps, a recording starts, or the loop changes while playing. `at`: where the key was
    /// heard in a run that plays; `None` in a run that stands still, where only `t_us` tells
    /// the keys apart.
    Recent {
        event: InstrumentEvent,
        t_us: u64,
        run: u32,
        at: Option<RecentAt>,
    },
    /// Run `run` ended at `t_us`, and the next one began. `at`: where the listener was then,
    /// when the run played and the place is known. Comes before every key of later runs,
    /// but for the keys a stop finds played after its own time: those come first.
    RecentEnd {
        run: u32,
        t_us: u64,
        at: Option<RecentAt>,
    },
}

/// Where in the song the listener was when a key of [`Recorded::Recent`] was played.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecentAt {
    /// Heard position, with the fraction. May be before where the run started.
    pub tick: f64,
    /// How often the loop had gone round in this run.
    pub lap: u32,
}

/// One keyboard event, pushed by the MIDI thread straight into the engine's ring.
#[derive(Clone, Copy, Debug)]
pub struct MidiIn {
    pub event: InstrumentEvent,
    /// When the event was received, [`monotonic_us`].
    pub t_us: u64,
}

/// The end of a ring of keys into the engine: the MIDI thread's, or the PC keyboard's
/// ([`EngineHandle::typed_in`]).
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
    /// dB, as the peaks: the most since the last read.
    master_reduction: AtomicU32,
    master_limited: AtomicBool,
    /// The test tone or the latency click sounded since the last read.
    aside: AtomicBool,
    load_permille: AtomicU32,
    last_frames: AtomicU32,
    callbacks: AtomicU64,
    late_callbacks: AtomicU64,
    xruns: AtomicU64,
    streams_lost: AtomicU64,
    other_errors: AtomicU64,
    record_overflows: AtomicU64,
    stream_missing: AtomicU64,
    sample_rate: AtomicU32,
    out_delay_us: AtomicU32,
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
            master_reduction: AtomicU32::new(0),
            master_limited: AtomicBool::new(false),
            aside: AtomicBool::new(false),
            load_permille: AtomicU32::new(0),
            last_frames: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            late_callbacks: AtomicU64::new(0),
            xruns: AtomicU64::new(0),
            streams_lost: AtomicU64::new(0),
            other_errors: AtomicU64::new(0),
            record_overflows: AtomicU64::new(0),
            stream_missing: AtomicU64::new(0),
            sample_rate: AtomicU32::new(config.sample_rate),
            out_delay_us: AtomicU32::new(0),
        }
    }

    /// What the PC says the last block takes from its callback to the DAC, in microseconds.
    pub fn out_delay_us(&self) -> u32 {
        self.out_delay_us.load(Relaxed)
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

    /// How far the master's limiters that are not passed by held the sound down since the
    /// last call, the most, in dB (0: not at all, or no such limiter;
    /// [`EngineMeters::master_limited`]).
    pub fn take_master_reduction(&self) -> f32 {
        take(&self.master_reduction)
    }

    /// Whether the master had a limiter that is not passed by in the last block.
    pub fn master_limited(&self) -> bool {
        self.master_limited.load(Relaxed)
    }

    /// Whether the test tone or the click of the latency measurement sounded since the last
    /// call: the master's peak then is not the song's alone.
    pub fn take_aside(&self) -> bool {
        self.aside.swap(false, Relaxed)
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

    /// Frames of vocal clips that were not in their ring when the song came to them (the
    /// read-ahead thread was late), over every track. They were silence. The silence a
    /// track starts with after a jump while the song plays is not counted.
    pub fn stream_missing(&self) -> u64 {
        self.stream_missing.load(Relaxed)
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
    /// The sequence is read this many frames ahead of the song, so that a track whose plugins
    /// delay the sound still reaches the master in time with the others (docs/architecture.md
    /// 20-8). The delays of the track's instrument and inserts, added up at every block.
    lookahead: u32,
    /// The lane, reading ahead, has gone round the loop and the song has not yet: it reads
    /// one loop's length behind where its lookahead alone would put it.
    ahead_wrapped: bool,
    /// [`Track::next_due`], kept while what it was worked out from stays the same.
    due: u64,
    due_key: Option<((usize, u32), u64, bool)>,
    auto: Option<Box<AutoSet>>,
    holds: [Option<AutoTarget>; HOLDS],
    /// [`EngineCommand::SetKnobs`].
    knobs: [f32; KNOBS],
    /// Fader and pan, or a send, went where automation took them, not where the hand put
    /// them.
    steered: bool,
    sends_steered: [bool; 2],
    /// The frame, from the anchor, where the mute lane next changes (`u64::MAX`: never).
    mute_due: u64,
    /// A vocal track: the input is heard here, and the instrument is not.
    voice: Option<VoiceState>,
    /// The vocal clips of the track.
    clips: ClipPlayer,
    /// 「アルペジオ」 of the track; it runs while the track is the play target.
    arp: Arp,
}

struct VoiceState {
    voice: Voice,
    /// 0 to 1, on its way to where `voice.on` and the input put it.
    gain: f32,
}

impl VoiceState {
    /// `live`: the input has frames for this block. `bus`: the input's channels over the
    /// stretch.
    fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        bus: &[Box<[f32]>; MAX_INPUT_CHANNELS],
        from: usize,
        live: bool,
        ramp: u32,
    ) {
        let target = if self.voice.on && live { 1.0 } else { 0.0 };
        if self.gain == 0.0 && target == 0.0 {
            left.fill(0.0);
            right.fill(0.0);
            return;
        }
        let step = 1.0 / ramp.max(1) as f32;
        let channel = |ch: u8| &bus[usize::from(ch).min(MAX_INPUT_CHANNELS - 1)][from..];
        let (in_l, in_r) = (channel(self.voice.left), channel(self.voice.right));
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            self.gain = if target > self.gain {
                (self.gain + step).min(1.0)
            } else {
                (self.gain - step).max(target)
            };
            *l = in_l[i] * self.gain;
            *r = in_r[i] * self.gain;
        }
    }
}

/// The song clock on the audio thread (docs/architecture.md 21-3).
///
/// The position is never added up block by block. It is `frames` whole frames after the
/// anchor, and the anchor is a place on the map: the tick is `map.tick_at(anchor_sec + frames /
/// sample rate)`, and the frame of a tick is `map.frame_of(tick, anchor_sec, …)`, both worked
/// out from the anchor in one go, so the answer is the same after ten minutes as after one bar.
/// A new anchor is set where playing starts, where the loop goes round, where the position is
/// moved, where the count-in ends, and when the map or the sample rate changes.
struct Transport {
    state: TransportState,
    map: Arc<TempoMap>,
    map_gen: u32,
    sample_rate: f64,
    /// The position at `frames == 0`, exactly.
    anchor_tick: f64,
    /// `anchor_tick` in the map's seconds.
    anchor_sec: f64,
    /// Frames rendered since the anchor.
    frames: u64,
    /// The latest tick the position is known to have reached: the anchor, or a tempo point or
    /// meter change it went over. What is told of tempo and meter at a frame is never from
    /// before it, even when the last bits of the f64 put the frame a hair before that tick.
    passed_tick: f64,
    /// Goes up whenever the frame of a tick, worked out before, may no longer hold.
    serial: u64,
    count_in: CountIn,
    looping: bool,
    loop_start: u64,
    loop_end: u64,
    /// The position has not passed `loop_end` since it last moved by a jump. A play that starts
    /// after the loop does not go round.
    before_loop_end: bool,
    /// The first frame at or past `loop_end`.
    loop_end_frame: u64,
    loop_start_sec: f64,
    metronome: bool,
    /// The metronome is quiet while playing (`EngineCommand::SetClick`).
    click_recording_only: bool,
    /// Next beat to click, and its frame.
    next_click: Beat,
    click_frame: u64,
    /// The next tempo point or meter change, and its frame (`u64::MAX` when there is none).
    cut_tick: Option<u64>,
    cut_frame: u64,
    record_from: u64,
    /// Ticks per beat of the meter at `record_from`.
    record_beat: u32,
    /// Ticks per click of the metronome there.
    record_click: u32,
    /// Where the current play or recording started; a stop that cannot be placed returns here.
    run_from: u64,
    muted: TrackSet,
    /// Mute and solo: never play their sequence.
    silent: TrackSet,
    /// Muted by their mute lane, where it is read.
    auto_silent: TrackSet,
    session: u32,
    lap: u32,
    /// [`Recorded::Recent`]: the run going on, and how often the loop went round in it.
    run: u32,
    run_lap: u32,
    /// After `RecordHere`: the session whose playing stretches, already sent to the DAC, still
    /// carry keys into this recording.
    punched: Option<u32>,
}

/// The count-in is not on the map: it is a straight line at the tempo and meter of the place
/// the recording starts from, and its clicks are that meter's clicks, carried on backwards.
#[derive(Clone, Copy, Default)]
struct CountIn {
    ticks_per_s: f64,
    /// The first frame at or past `record_from`.
    end_frame: u64,
    /// The head of the bar `record_from` is in.
    bar_tick: i64,
    ticks_per_click: i64,
    clicks_per_bar: i64,
}

/// A position within a millionth of a tick of a whole tick is that tick: the last bits of the
/// f64 must not put a beat or a note that is due right now behind the position.
fn snap(tick: f64) -> f64 {
    let whole = tick.round();
    if (tick - whole).abs() < 1e-6 {
        whole
    } else {
        tick
    }
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
            ahead_wrapped: false,
            due: u64::MAX,
            due_key: None,
            auto: None,
            holds: [None; HOLDS],
            knobs: KNOB_DEFAULTS,
            steered: false,
            sends_steered: [false; 2],
            mute_due: u64::MAX,
            voice: None,
            clips: ClipPlayer::new(),
            arp: Arp::default(),
        }
    }

    /// The track's own instrument (the one set aside while another is tried out) takes the
    /// knobs the hand put, each one that no lane plays now.
    fn apply_knobs(&mut self) {
        let own = match self.aside.as_deref_mut() {
            Some(own) => own,
            None => match self.instrument.as_deref_mut() {
                Some(own) => own,
                None => return,
            },
        };
        for (knob, &place) in self.knobs.iter().enumerate() {
            let knob = knob as u32;
            if !self
                .auto
                .as_deref()
                .is_some_and(|set| set.plays_instrument(knob))
            {
                own.set_knob(knob, place);
            }
        }
    }

    /// Another instrument is on the seat: it takes the hand's knobs, and the lanes of its
    /// parameters are sent again.
    fn instrument_changed(&mut self) {
        self.apply_knobs();
        if let Some(set) = self.auto.as_deref_mut() {
            set.resend_instrument();
        }
    }

    /// The frame, counted from the anchor, where the song comes to the lane's next event
    /// (`u64::MAX` when there is none). A lane that has gone round the loop ahead of the song
    /// counts on from the loop's end, as the song will.
    fn next_due(&mut self, t: &Transport) -> u64 {
        let key = (self.lane.place(), t.serial, self.ahead_wrapped);
        if self.due_key != Some(key) {
            self.due_key = Some(key);
            self.due = match self.lane.next_tick() {
                None => u64::MAX,
                Some(tick) if self.ahead_wrapped => t
                    .loop_end_frame
                    .saturating_add(t.map.frame_of(tick as f64, t.loop_start_sec, t.sample_rate)),
                Some(tick) => t.frame_of(tick as f64),
            };
        }
        self.due
    }

    /// Sends what the sequence has for the next frame.
    ///
    /// A lane that reads ahead comes to the loop's end before the song does. It goes round by
    /// itself there, so that the notes at the loop's start are sent early like any others, and
    /// the song's own jump later leaves it alone.
    fn play_due(&mut self, t: &Transport) {
        let at = t.frames + u64::from(self.lookahead);
        if self.lookahead > 0 && !self.ahead_wrapped && t.loop_active() && at >= t.loop_end_frame {
            self.lane
                .fire_before(t.loop_end, self.instrument.as_deref_mut());
            self.lane.silence_at_loop(self.instrument.as_deref_mut());
            let start = t.loop_start;
            self.lane
                .seek(|tick| tick < start, self.instrument.as_deref_mut());
            self.ahead_wrapped = true;
        }
        while self.next_due(t) <= at {
            self.lane.fire_next(self.instrument.as_deref_mut());
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
        self.instrument.is_none()
            && self.fading.is_none()
            && self.aside.is_none()
            && self.voice.is_none()
            && !self.clips.is_active()
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
        // 120.00 and 4/4 until the song's map arrives.
        let tempo = Tempo::from_centi(12_000).expect("120.00 is a tempo");
        let map = Arc::new(TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR)));
        let mut transport = Transport {
            state: TransportState::Stopped,
            map,
            map_gen: 0,
            sample_rate: f64::from(sample_rate),
            anchor_tick: 0.0,
            anchor_sec: 0.0,
            frames: 0,
            passed_tick: 0.0,
            serial: 0,
            count_in: CountIn::default(),
            looping: false,
            loop_start: 0,
            loop_end: 0,
            before_loop_end: false,
            loop_end_frame: 0,
            loop_start_sec: 0.0,
            metronome: false,
            click_recording_only: false,
            next_click: Beat {
                tick: 0,
                downbeat: true,
            },
            click_frame: 0,
            cut_tick: None,
            cut_frame: u64::MAX,
            record_from: 0,
            record_beat: TICKS_PER_QUARTER as u32,
            record_click: TICKS_PER_QUARTER as u32,
            run_from: 0,
            muted: TrackSet::default(),
            silent: TrackSet::default(),
            auto_silent: TrackSet::default(),
            session: 0,
            lap: 0,
            run: 0,
            run_lap: 0,
            punched: None,
        };
        transport.set_pos(0.0);
        transport
    }

    /// The position of the next frame to render.
    fn tick(&self) -> f64 {
        if self.frames == 0 {
            return self.anchor_tick;
        }
        let since = self.frames as f64 / self.sample_rate;
        if self.state == TransportState::CountIn {
            self.anchor_tick + since * self.count_in.ticks_per_s
        } else {
            self.map.tick_at(self.anchor_sec + since)
        }
    }

    /// [`Transport::tick`] for what is told of the tempo and the meter there.
    fn head_tick(&self) -> f64 {
        self.tick().max(self.passed_tick)
    }

    /// Counting frames from the anchor, the first frame at `tick` or past it.
    fn frame_of(&self, tick: f64) -> u64 {
        if self.state == TransportState::CountIn {
            // The same guard as `TempoMap::frame_of`.
            let frames = (tick - self.anchor_tick) / self.count_in.ticks_per_s * self.sample_rate;
            (frames - 1e-6).ceil().max(0.0) as u64
        } else {
            self.map.frame_of(tick, self.anchor_sec, self.sample_rate)
        }
    }

    /// Ticks per second at `tick`.
    fn speed_at(&self, tick: f64) -> f64 {
        match self.state {
            TransportState::Stopped => 0.0,
            TransportState::CountIn => self.count_in.ticks_per_s,
            _ => self.map.tempo_at(tick) * TICKS_PER_S_PER_BPM,
        }
    }

    /// The map to read `segment` with, if it was rendered on this one.
    fn map_for(&self, segment: &Segment) -> Option<&TempoMap> {
        (segment.map_gen == self.map_gen).then_some(&*self.map)
    }

    /// A new anchor at `tick`. Set the state first: a count-in is laid out from here.
    fn set_pos(&mut self, tick: f64) {
        self.anchor_tick = tick;
        self.anchor_sec = self.map.seconds_at(tick);
        self.frames = 0;
        self.passed_tick = tick;
        self.before_loop_end = tick < self.loop_end as f64;
        self.aim_record_beat();
        if self.state == TransportState::CountIn {
            let at = self.map.meter_at(self.record_from);
            self.count_in = CountIn {
                ticks_per_s: self.map.tempo_at(self.record_from as f64) * TICKS_PER_S_PER_BPM,
                end_frame: 0,
                bar_tick: at.bar_tick as i64,
                ticks_per_click: at.meter.ticks_per_click() as i64,
                clicks_per_bar: i64::from(at.meter.clicks_per_bar()),
            };
            self.count_in.end_frame = self.frame_of(self.record_from as f64);
            let c = &self.count_in;
            let beats = ((tick - c.bar_tick as f64) / c.ticks_per_click as f64).ceil() as i64;
            self.next_click = Beat {
                tick: c.bar_tick + beats * c.ticks_per_click,
                downbeat: beats.rem_euclid(c.clicks_per_bar) == 0,
            };
            self.cut_tick = None;
        } else {
            self.next_click = self.map.next_click(tick);
            self.cut_tick = self.map.next_change(tick);
        }
        self.click_frame = self.frame_of(self.next_click.tick as f64);
        self.cut_frame = self
            .cut_tick
            .map_or(u64::MAX, |cut| self.frame_of(cut as f64));
        self.aim_loop();
    }

    fn aim_record_beat(&mut self) {
        let meter = self.map.meter_at(self.record_from).meter;
        self.record_beat = meter.ticks_per_beat() as u32;
        self.record_click = meter.ticks_per_click() as u32;
    }

    fn aim_loop(&mut self) {
        self.serial += 1;
        self.loop_start_sec = self.map.seconds_at(self.loop_start as f64);
        self.loop_end_frame = self.frame_of(self.loop_end as f64);
    }

    /// After the click of `next_click`: the beat after it.
    fn advance_click(&mut self) {
        if self.state == TransportState::CountIn {
            let c = &self.count_in;
            let tick = self.next_click.tick + c.ticks_per_click;
            let beats = (tick - c.bar_tick).div_euclid(c.ticks_per_click);
            self.next_click = Beat {
                tick,
                downbeat: beats.rem_euclid(c.clicks_per_bar) == 0,
            };
        } else {
            self.next_click = self.map.next_click((self.next_click.tick + 1) as f64);
        }
        self.click_frame = self.frame_of(self.next_click.tick as f64);
    }

    /// After the position went over `cut_tick`: the change after it.
    fn advance_cut(&mut self) {
        if let Some(cut) = self.cut_tick {
            self.passed_tick = self.passed_tick.max(cut as f64);
            self.cut_tick = self.map.next_change(cut as f64);
        }
        self.cut_frame = self
            .cut_tick
            .map_or(u64::MAX, |cut| self.frame_of(cut as f64));
    }

    /// What plugins and effects are told of the song, at the next frame.
    fn for_plugins(&self) -> PluginTransport {
        let mut flags = 0;
        if matches!(
            self.state,
            TransportState::Playing | TransportState::Recording
        ) {
            flags |= PluginTransport::PLAYING;
        }
        if self.state == TransportState::Recording {
            flags |= PluginTransport::RECORDING;
        }
        if self.loop_active() {
            flags |= PluginTransport::LOOPING;
        }
        let quarter = TICKS_PER_QUARTER as f64;
        let tick = self.head_tick();
        let (meter, bar, bar_tick, tempo_bpm, seconds) = if self.state == TransportState::CountIn {
            let c = &self.count_in;
            let at = self.map.meter_at(self.record_from);
            let per_bar = (c.ticks_per_click * c.clicks_per_bar) as f64;
            let bars = ((tick - c.bar_tick as f64) / per_bar).floor();
            let bar_tick = bars * per_bar;
            // Counted back from where the recording starts.
            let seconds = self.map.seconds_at(self.record_from as f64)
                - (self.record_from as f64 - tick) / c.ticks_per_s;
            (
                at.meter,
                (at.bar as f64 + bars).max(0.0) as u64,
                c.bar_tick as f64 + bar_tick,
                c.ticks_per_s / TICKS_PER_S_PER_BPM,
                seconds,
            )
        } else {
            let at = self.map.meter_at(tick.max(0.0) as u64);
            (
                at.meter,
                at.bar,
                at.bar_tick as f64,
                self.map.tempo_at(tick),
                self.anchor_sec + self.frames as f64 / self.sample_rate,
            )
        };
        PluginTransport {
            flags,
            meter_num: u16::from(meter.num()),
            meter_den: u16::from(meter.den()),
            tempo_bpm,
            position_quarters: tick / quarter,
            bar_start_quarters: bar_tick / quarter,
            loop_start_quarters: self.loop_start as f64 / quarter,
            loop_end_quarters: self.loop_end as f64 / quarter,
            position_frames: (seconds * self.sample_rate).round() as i64,
            bar,
        }
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
        if self.silent.contains(track) || self.auto_silent.contains(track) {
            return false;
        }
        match self.state {
            TransportState::Playing => true,
            TransportState::Recording => !self.muted.contains(track),
            _ => false,
        }
    }

    /// The loop as the read-ahead thread is to lay it out for a play from the place of
    /// [`Transport::stream_place`].
    fn stream_loop(&self) -> Option<(u64, u64)> {
        let before_end = match self.state {
            TransportState::CountIn => self.record_from < self.loop_end,
            _ => self.before_loop_end,
        };
        (self.looping && self.loop_end > self.loop_start && before_end)
            .then_some((self.loop_start, self.loop_end))
    }

    /// Where the vocal clips are read from next. While counting in, that is where the
    /// recording will start: the rings are full when the count-in ends.
    fn stream_place(&self) -> StreamPlace {
        let (anchor_sec, frame) = match self.state {
            TransportState::CountIn => (self.map.seconds_at(self.record_from as f64), 0),
            _ => (self.anchor_sec, self.frames),
        };
        StreamPlace {
            map_gen: self.map_gen,
            anchor_sec,
            frame,
            loop_ticks: self.stream_loop(),
        }
    }

    /// The song's place for a track that reads `lookahead` frames ahead; `None` while the
    /// song does not move.
    fn stream_now(&self, lookahead: u32) -> Option<StreamNow> {
        matches!(
            self.state,
            TransportState::Playing | TransportState::Recording
        )
        .then(|| StreamNow {
            at: self.frames + u64::from(lookahead),
            anchor_sec: self.anchor_sec,
            sample_rate: self.sample_rate,
            wrap: (self.loop_active()).then_some((self.loop_end_frame, self.loop_start_sec)),
        })
    }

    fn click_audible(&self) -> bool {
        match self.state {
            TransportState::CountIn => true,
            TransportState::Playing => self.metronome && !self.click_recording_only,
            TransportState::Recording => self.metronome,
            TransportState::Stopped => false,
        }
    }
}

impl AutoClock for Transport {
    fn tick_ahead(&self, ahead: u64) -> f64 {
        let at = self.frames + ahead;
        match self.state {
            TransportState::Stopped => self.tick(),
            TransportState::CountIn => {
                self.anchor_tick + at as f64 / self.sample_rate * self.count_in.ticks_per_s
            }
            _ if self.loop_active() && at >= self.loop_end_frame => {
                let past = (at - self.loop_end_frame) as f64 / self.sample_rate;
                self.map.tick_at(self.loop_start_sec + past)
            }
            _ => self
                .map
                .tick_at(self.anchor_sec + at as f64 / self.sample_rate),
        }
    }

    fn frames_to(&self, tick: f64, ahead: u64) -> Option<u64> {
        let from = self.frames + ahead;
        let looped = self.loop_active();
        if !self.state.is_moving() || looped && from >= self.loop_end_frame {
            return None;
        }
        let frame = self.frame_of(tick);
        if looped && frame >= self.loop_end_frame {
            return None;
        }
        Some(frame.saturating_sub(from))
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

    fn sounding(&self) -> bool {
        self.frame < self.total
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

/// The click of the latency measurement: [`probe_wave`], once, on both sides.
struct Probe {
    wave: Box<[f32]>,
    /// The next sample of the wave; past its end when nothing sounds.
    at: usize,
    asked: bool,
}

impl Probe {
    fn new(sample_rate: u32) -> Probe {
        let wave = probe_wave(sample_rate).into_boxed_slice();
        Probe {
            at: wave.len(),
            wave,
            asked: false,
        }
    }

    fn sounding(&self) -> bool {
        self.at < self.wave.len()
    }

    fn add(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let Some(sample) = self.wave.get(self.at) else {
                return;
            };
            *l += sample;
            *r += sample;
            self.at += 1;
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
    /// The arpeggio of the play target.
    arp: ArpPlayer,
    commands: Consumer<EngineCommand>,
    retired: Producer<Box<dyn Instrument>>,
    retired_effects: Producer<Box<dyn Effect>>,
    retired_sequences: Producer<Box<Sequence>>,
    retired_maps: Producer<Arc<TempoMap>>,
    retired_autos: Producer<Box<AutoSet>>,
    retired_inputs: Producer<Box<InputFeed>>,
    retired_clips: Producer<Box<ClipTable>>,
    retired_streams: Producer<Box<StreamReader>>,
    /// What the read-ahead thread is told.
    wants: Producer<StreamWant>,
    /// The frames of a track's ring over the stretch being rendered, interleaved.
    stream_buf: Box<[f32]>,
    /// The PC's input, while one is open.
    input: Option<Box<InputFeed>>,
    /// The input's channels over the piece being rendered.
    input_bus: [Box<[f32]>; MAX_INPUT_CHANNELS],
    input_live: bool,
    /// Frames rendered since the engine was made, and where the device block began.
    stream_frame: u64,
    block_frame: u64,
    probe: Probe,
    probes: Producer<ProbeSent>,
    analysis: AnalysisTap,
    /// The ticks where the steps of the fader's lanes end, in the stretch being rendered.
    auto_ticks: Box<[f64]>,
    returns: [Strip; 2],
    /// Its fader and pan come before its slots (docs/architecture.md 19-11).
    master_strip: Strip,
    midi: Consumer<MidiIn>,
    /// The keys of the PC keyboard ([`EngineHandle::typed_in`]), read with `midi` in time order.
    typed: Consumer<MidiIn>,
    latency: Producer<LatencyRecord>,
    recorded: Producer<Recorded>,
    clock: Arc<TransportClock>,
    transport: Transport,
    click: Click,
    /// What the next click is made of (`EngineCommand::SetClick`).
    click_sound: ClickSound,
    click_gain: f32,
    /// The stretch being rendered, and the frame of the device block where it began.
    segment: Segment,
    segment_from: usize,
    meters: Arc<EngineMeters>,
    /// Left and right of the track being rendered, then left and right of what fades out on it
    /// (an instrument, or a slot before its effect changed), then left and right of the sound
    /// that goes past a slot.
    scratch: [Box<[f32]>; 6],
    /// Left and right of the master, of return A and of return B, as the strips add up.
    sums: [Box<[f32]>; 6],
    /// The metronome over the block. It joins the output after the master's meter is taken,
    /// so the meters show the song only, and the master's fader and limiter leave it alone.
    click_sum: Box<[f32]>,
    sample_rate: f32,
    max_frames: usize,
    plugin_block: Arc<PluginBlock>,
    tone: TestTone,
    /// The track and keys the last `Audition` sounded, until they are let go.
    audition: Option<(usize, Chord)>,
    last_callback: Option<(Instant, f64)>,
    /// What goes out is cut to ±1.0 (0 dB), to spare ears and speakers.
    clamp_out: bool,
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
        let (maps_tx, maps_rx) = RingBuffer::new(MAP_RING);
        let (autos_tx, autos_rx) = RingBuffer::new(config.max_tracks.max(16));
        let (inputs_tx, inputs_rx) = RingBuffer::new(INPUT_RING);
        // Every command retires at most two tables, and commands wait while fewer fit.
        let (clips_tx, clips_rx) = RingBuffer::new(config.max_tracks.max(16) * 2);
        let (streams_tx, streams_rx) = RingBuffer::new(config.max_tracks.max(16));
        let (wants_tx, wants_rx) = want_ring((config.max_tracks + 1) * WANTS_PER_TRACK);
        let (probes_tx, probes_rx) = RingBuffer::new(PROBE_RING);
        let (recorded_tx, recorded_rx) = RingBuffer::new(RECORDED_RING);
        let clock = Arc::new(TransportClock::default());
        let (midi_tx, midi_rx) = RingBuffer::new(MIDI_RING);
        let (typed_tx, typed_rx) = RingBuffer::new(TYPED_RING);
        let (latency_tx, latency_rx) = RingBuffer::new(LATENCY_RING);
        let (analysis_tap, analysis_reader) = analysis_ring(config.sample_rate);
        let buffer = || vec![0.0f32; config.max_frames].into_boxed_slice();
        let transport = Transport::new(config.sample_rate);
        let map = transport.map.clone();
        let engine = Engine {
            tracks: (0..=config.max_tracks).map(|_| Track::empty()).collect(),
            sample: Track::empty(),
            play_target: Some(0),
            arp: ArpPlayer::new(),
            commands: command_rx,
            retired: retired_tx,
            retired_effects: effects_tx,
            retired_sequences: sequences_tx,
            retired_maps: maps_tx,
            retired_autos: autos_tx,
            retired_inputs: inputs_tx,
            retired_clips: clips_tx,
            retired_streams: streams_tx,
            wants: wants_tx,
            stream_buf: vec![0.0f32; config.max_frames * STREAM_CHANNELS].into_boxed_slice(),
            input: None,
            input_bus: std::array::from_fn(|_| buffer()),
            input_live: false,
            stream_frame: 0,
            block_frame: 0,
            probe: Probe::new(config.sample_rate),
            probes: probes_tx,
            analysis: analysis_tap,
            auto_ticks: vec![0.0; config.max_frames.div_ceil(MIX_STEP) + 1].into_boxed_slice(),
            returns: [Strip::new(), Strip::new()],
            master_strip: Strip::new(),
            midi: midi_rx,
            typed: typed_rx,
            latency: latency_tx,
            recorded: recorded_tx,
            clock: clock.clone(),
            transport,
            click: Click::idle(),
            click_sound: ClickSound::default(),
            click_gain: 1.0,
            segment: Segment::default(),
            segment_from: 0,
            meters: meters.clone(),
            scratch: std::array::from_fn(|_| buffer()),
            sums: std::array::from_fn(|_| buffer()),
            click_sum: buffer(),
            sample_rate: config.sample_rate as f32,
            max_frames: config.max_frames,
            plugin_block: plugin_block.clone(),
            tone: TestTone::idle(),
            audition: None,
            last_callback: None,
            clamp_out: true,
        };
        let handle = EngineHandle {
            commands: command_tx,
            retired: retired_rx,
            retired_effects: effects_rx,
            retired_sequences: sequences_rx,
            retired_maps: maps_rx,
            retired_autos: autos_rx,
            retired_inputs: inputs_rx,
            retired_clips: clips_rx,
            retired_streams: streams_rx,
            wants: Some(wants_rx),
            probes: probes_rx,
            analysis: Some(analysis_reader),
            map,
            map_gen: 0,
            last_map_gen: 0,
            latency: latency_rx,
            recorded: recorded_rx,
            typed: typed_tx,
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

    /// How many frames the master's inserts delay everything by. The tracks are already
    /// lined up with one another (their look-ahead); this is what is left at the head of
    /// what comes out, and what an export cuts off there.
    pub fn master_latency(&self) -> u32 {
        self.master_strip.latency()
    }

    /// What goes out is no longer cut to ±1.0: for an export, which counts the places over
    /// 0 dB and keeps them in a 32 bit float file. Not numbers still come out as silence.
    pub fn keep_overs(&mut self) {
        self.clamp_out = false;
    }

    /// Get ready for a (new) stream. Call outside the audio thread, while no stream is running
    /// this engine. Every instrument and effect is prepared again, so all notes and tails stop,
    /// and the transport stops where it was (without a [`Recorded::Stopped`]: the caller knows the stream ended).
    pub fn prepare(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1) as f32;
        self.meters.sample_rate.store(sample_rate.max(1), Relaxed);
        self.analysis.set_sample_rate(sample_rate.max(1));
        for track in self.tracks.iter_mut() {
            for instrument in track.instrument.iter_mut().chain(track.aside.iter_mut()) {
                instrument.prepare(self.sample_rate, self.max_frames);
            }
            track.aside_left = 0;
            track.fade_left = 0;
            track.lane.forget();
            if let Some(set) = track.auto.as_deref_mut() {
                set.forget();
            }
            track.clips.rest();
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
        let tick = snap(t.tick()).floor().max(0.0);
        t.sample_rate = f64::from(sample_rate.max(1));
        if t.state.is_moving() {
            let run = t.run;
            t.run = run.wrapping_add(1);
            t.run_lap = 0;
            let ended = Recorded::RecentEnd {
                run,
                t_us: monotonic_us(),
                at: None,
            };
            if self.recorded.push(ended).is_err() {
                self.meters.record_overflows.fetch_add(1, Relaxed);
            }
        }
        let t = &mut self.transport;
        t.state = TransportState::Stopped;
        t.set_pos(tick);
        t.silent = TrackSet::default();
        t.auto_silent = TrackSet::default();
        self.tone = TestTone::idle();
        self.probe = Probe::new(sample_rate.max(1));
        self.audition = None;
        self.arp_reset();
        self.click = Click::idle();
        self.last_callback = None;
    }

    /// Takes in the commands that wait, without rendering anything. Call outside the audio
    /// thread, while no stream is running this engine: instruments and the play target then
    /// change without a stream. It stops early while the retired rings are full; empty them
    /// ([`EngineHandle::drop_retired`]) and call again.
    pub fn settle(&mut self) {
        self.arp_reset();
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
        self.aim_streams();
    }

    /// The vocal clips of every track fade out, and the read-ahead thread starts again
    /// from the place the song is at when the next block begins.
    fn streams_jump(&mut self) {
        for track in self.tracks.iter_mut() {
            track.clips.jump(false);
        }
    }

    fn aim_streams(&mut self) {
        let place = self.transport.stream_place();
        for (i, track) in self.tracks.iter_mut().enumerate() {
            track.clips.aim(i, track.lookahead, place, &mut self.wants);
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
        let out_delay = timing.playback_us.saturating_sub(timing.callback_us);
        (self.meters.out_delay_us).store(out_delay.min(u64::from(u32::MAX)) as u32, Relaxed);
        // Plugins in other processes are waited for until this far into the block, all of
        // them together.
        let wait_ns = (block_s * PLUGIN_WAIT_SHARE * 1e9) as u64;
        self.plugin_block
            .set_deadline(meloet_plugin::ipc::futex::monotonic_ns().saturating_add(wait_ns));

        self.block_frame = self.stream_frame;
        self.read_commands(timing);
        if std::mem::take(&mut self.probe.asked) {
            self.probe.at = 0;
            let _ = self.probes.push(ProbeSent {
                frame: self.block_frame,
            });
        }
        let voices = (self.tracks.iter().filter_map(|t| t.voice.as_ref()))
            .any(|v| v.voice.on || v.gain > 0.0);
        self.input_live = match self.input.as_deref_mut() {
            Some(feed) => feed.begin(frames, voices),
            None => false,
        };
        for track in self.tracks.iter_mut() {
            let instrument = track.instrument.as_ref().map_or(0, |i| i.latency());
            track.lookahead = instrument.saturating_add(track.strip.latency());
        }
        self.aim_streams();
        self.read_midi(timing, u64::MAX);
        self.open_segment(0, timing);
        let mut offset = 0;
        for piece in out.chunks_mut(self.max_frames * 2) {
            self.render(piece, offset, timing);
            offset += piece.len() / 2;
        }
        self.close_segment(frames);
        self.stream_frame += frames as u64;

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
            && !self.retired_maps.is_full()
            && !self.retired_autos.is_full()
            && !self.retired_inputs.is_full()
            && self.retired_clips.slots() >= 2
            && !self.retired_streams.is_full()
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
                            slot.instrument_changed();
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
                            slot.lane.replace_sequence(Some(sequence))
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
                EngineCommand::GrowSequence { track, sequence } => {
                    let old = match self.tracks.get_mut(track) {
                        Some(slot) => slot.lane.replace_sequence(Some(sequence)),
                        None => Some(sequence),
                    };
                    if let Some(old) = old {
                        let _ = self.retired_sequences.push(old);
                    }
                    if self.transport.lane_plays(track) {
                        self.seek_lane_past_sent(track);
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
                EngineCommand::SetBypass {
                    strip,
                    slot,
                    bypassed,
                } => {
                    if let Some((strip, smooth)) = self.strip(strip) {
                        strip.set_bypass(slot, bypassed, if smooth { fade } else { 0 });
                    }
                }
                EngineCommand::MoveEffect { strip, from, to } => {
                    if let MixStrip::Track(track) = strip
                        && from < INSERT_SLOTS
                        && to < INSERT_SLOTS
                        && let Some(set) = self.tracks.get_mut(track).and_then(|t| t.auto.as_mut())
                    {
                        set.move_slot(from, to);
                    }
                    if let Some((strip, smooth)) = self.strip(strip) {
                        strip.move_effect(from, to, if smooth { fade } else { 0 });
                    }
                }
                EngineCommand::SetAutomation { track, mut set } => {
                    let old = match self.tracks.get_mut(track) {
                        Some(slot) => {
                            if let Some(new) = set.as_deref_mut() {
                                new.follow(slot.auto.as_deref(), &slot.holds);
                            }
                            let old = std::mem::replace(&mut slot.auto, set);
                            slot.apply_knobs();
                            old
                        }
                        None => set,
                    };
                    if let Some(old) = old {
                        // Cannot fail: checked above, and this is the only producer.
                        let _ = self.retired_autos.push(old);
                    }
                }
                EngineCommand::HoldLane { track, target, on } => {
                    let Some(slot) = self.tracks.get_mut(track) else {
                        continue;
                    };
                    let place = slot.holds.iter().position(|h| *h == Some(target));
                    match (place, on) {
                        (None, true) => {
                            if let Some(free) = slot.holds.iter_mut().find(|h| h.is_none()) {
                                *free = Some(target);
                            }
                        }
                        (Some(at), false) => slot.holds[at] = None,
                        _ => {}
                    }
                    if let Some(set) = slot.auto.as_deref_mut() {
                        set.hold(target, on);
                    }
                    if matches!(target, AutoTarget::Instrument { .. }) {
                        slot.apply_knobs();
                    }
                }
                EngineCommand::SetKnobs { track, knobs } => {
                    let Some(slot) = self.tracks.get_mut(track) else {
                        continue;
                    };
                    for (kept, place) in slot.knobs.iter_mut().zip(knobs) {
                        if let Some(place) = knob_place(place) {
                            *kept = place;
                        }
                    }
                    slot.apply_knobs();
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
                        slot.instrument_changed();
                    } else {
                        slot.lane.forget();
                        if let Some(tried) = slot.instrument.replace(own) {
                            slot.fade_out(tried, fade, &mut self.retired);
                        }
                        slot.instrument_changed();
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
                    slot.instrument_changed();
                    if self.play_target == Some(from) {
                        self.play_target = Some(to);
                        self.arp_forget();
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
                    slot.holds = [None; HOLDS];
                    slot.knobs = KNOB_DEFAULTS;
                    slot.steered = false;
                    slot.sends_steered = [false; 2];
                    slot.mute_due = u64::MAX;
                    slot.voice = None;
                    slot.arp = Arp::default();
                    if let Some(old) = slot.clips.clear(&mut self.retired_clips) {
                        let _ = self.retired_streams.push(old);
                    }
                    self.transport.auto_silent.remove(track);
                    if let Some(old) = slot.auto.take() {
                        let _ = self.retired_autos.push(old);
                    }
                    let all = [
                        slot.instrument.take(),
                        slot.aside.take(),
                        slot.fading.take(),
                    ];
                    for old in all.into_iter().flatten() {
                        let _ = self.retired.push(old);
                    }
                    if let Some(old) = slot.lane.replace_sequence(None) {
                        let _ = self.retired_sequences.push(old);
                    }
                    if self.audition.is_some_and(|(t, _)| t == track) {
                        self.audition = None;
                    }
                    if self.play_target == Some(track) {
                        self.arp_forget();
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
                    self.arp_all_off(timing);
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
                EngineCommand::SetTempoMap { map, map_gen } => {
                    let t = &mut self.transport;
                    // The tick goes on from where it is; only the frames after it change.
                    let tick = snap(t.tick());
                    // A beat due at this very frame may lie a part of a frame behind the
                    // position, where the new map no longer looks for it.
                    let due =
                        (t.state.is_moving() && t.click_frame <= t.frames).then_some(t.next_click);
                    let old = std::mem::replace(&mut t.map, map);
                    t.map_gen = map_gen;
                    t.set_pos(tick);
                    for track in self.tracks.iter_mut() {
                        track.clips.jump(false);
                    }
                    if let Some(beat) = due
                        && t.next_click.tick != beat.tick
                        && t.click_audible()
                    {
                        let (sound, gain) = (self.click_sound, self.click_gain);
                        self.click
                            .start(beat.downbeat, self.sample_rate, sound, gain);
                    }
                    // Cannot fail: checked above, and this is the only producer.
                    let _ = self.retired_maps.push(old);
                }
                EngineCommand::SetInput { feed } => {
                    if let Some(old) = std::mem::replace(&mut self.input, feed) {
                        old.stop();
                        let _ = self.retired_inputs.push(old);
                    }
                }
                EngineCommand::SetVoice { track, voice } => {
                    if let Some(slot) = self.tracks.get_mut(track) {
                        slot.voice = voice.map(|voice| VoiceState {
                            voice,
                            gain: match &slot.voice {
                                Some(before) => before.gain,
                                None => 0.0,
                            },
                        });
                    }
                }
                EngineCommand::SetStream { track, reader } => {
                    let old = match self.tracks.get_mut(track) {
                        Some(slot) => slot.clips.set_reader(reader),
                        None => reader,
                    };
                    if let Some(old) = old {
                        // Cannot fail: checked above, and this is the only producer.
                        let _ = self.retired_streams.push(old);
                    }
                }
                EngineCommand::SetClips {
                    track,
                    table,
                    refeed,
                } => match self.tracks.get_mut(track) {
                    Some(slot) => {
                        (slot.clips).set_table(table, refeed, ramp, &mut self.retired_clips);
                    }
                    None => {
                        if let Some(table) = table {
                            let _ = self.retired_clips.push(table);
                        }
                    }
                },
                EngineCommand::SpliceClips { track } => {
                    if let Some(slot) = self.tracks.get_mut(track) {
                        slot.clips.splice();
                    }
                }
                EngineCommand::Probe => self.probe.asked = true,
                EngineCommand::SetLoop { start, end, on } => {
                    let t = &self.transport;
                    if t.state == TransportState::Playing
                        && (t.looping, t.loop_start, t.loop_end) != (on, start, end)
                    {
                        self.end_run(timing.callback_us, timing);
                    }
                    let t = &mut self.transport;
                    let turned = t.stream_loop();
                    t.looping = on;
                    t.loop_start = start;
                    t.loop_end = end;
                    t.aim_loop();
                    t.before_loop_end = t.loop_end_frame > t.frames
                        || (t.frames == 0 && t.anchor_tick < end as f64);
                    if t.stream_loop() != turned {
                        self.streams_jump();
                    }
                    // A lane that went round the old loop ahead of the song is put back.
                    for track in 0..self.tracks.len() {
                        if self.tracks[track].ahead_wrapped {
                            let slot = &mut self.tracks[track];
                            slot.lane.silence(slot.instrument.as_deref_mut());
                            self.seek_lane(track);
                        }
                    }
                }
                EngineCommand::SetMetronome { on } => self.transport.metronome = on,
                EngineCommand::SetClick {
                    sound,
                    gain,
                    recording_only,
                } => {
                    self.click_sound = sound;
                    self.click_gain = gain;
                    self.transport.click_recording_only = recording_only;
                }
                EngineCommand::Play { from } => {
                    self.end_recording(timing);
                    self.end_run(timing.callback_us, timing);
                    self.silence_lanes();
                    self.transport.state = TransportState::Playing;
                    self.transport.run_from = from;
                    self.transport.set_pos(from as f64);
                    self.streams_jump();
                    self.seek_lanes();
                }
                EngineCommand::Record {
                    from,
                    count_in_beats,
                    muted,
                } => {
                    self.end_recording(timing);
                    self.end_run(timing.callback_us, timing);
                    self.silence_lanes();
                    let t = &mut self.transport;
                    t.session = t.session.wrapping_add(1);
                    t.punched = None;
                    t.lap = 0;
                    t.record_from = from;
                    t.run_from = from;
                    t.muted = muted;
                    let frame = self.block_frame;
                    self.push_recorded(Recorded::Started {
                        from,
                        count_in_beats,
                        frame,
                    });
                    let t = &mut self.transport;
                    if count_in_beats == 0 {
                        t.state = TransportState::Recording;
                        t.set_pos(from as f64);
                        self.seek_lanes();
                        let tick = from as f64;
                        self.push_recorded(Recorded::Rolling { frame, tick });
                    } else {
                        t.state = TransportState::CountIn;
                        let click = t.map.meter_at(from).meter.ticks_per_click();
                        t.set_pos(from as f64 - (u64::from(count_in_beats) * click) as f64);
                    }
                    self.streams_jump();
                }
                EngineCommand::RecordHere { muted } => self.record_here(muted, timing),
                EngineCommand::SetRecordMuted { muted } => self.set_record_muted(muted),
                EngineCommand::Stop { t_us } => self.stop(t_us, timing),
                EngineCommand::Locate { tick } => {
                    self.end_recording(timing);
                    if self.transport.state.is_moving() {
                        self.end_run(timing.callback_us, timing);
                    }
                    let t = &mut self.transport;
                    t.run_from = tick;
                    t.set_pos(tick as f64);
                    let moving = t.state.is_moving();
                    self.streams_jump();
                    if moving {
                        self.silence_lanes();
                        self.seek_lanes();
                    }
                }
                EngineCommand::SetArp { track, arp } => self.set_arp(track, arp, timing),
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
        self.arp_forget();
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
            .map(|s| (s.state, heard(&s, t_us as f64, t.map_for(&s))));
        if matches!(t.state, TransportState::CountIn | TransportState::Recording) {
            let (tick, lap) = match heard_then {
                Some((TransportState::CountIn | TransportState::Recording, h)) => (h.tick, h.lap),
                _ => (t.record_from as f64, 0),
            };
            let frame = self.block_frame;
            self.push_recorded(Recorded::Stopped {
                t_us,
                tick,
                lap,
                frame,
            });
        }
        let to = match heard_then {
            Some((TransportState::Playing | TransportState::Recording, h)) => {
                h.tick.floor().max(0.0)
            }
            _ => self.transport.run_from as f64,
        };
        if self.transport.state.is_moving() {
            self.end_run(t_us, timing);
        }
        self.silence_lanes();
        self.transport.state = TransportState::Stopped;
        self.transport.set_pos(to);
        self.streams_jump();
    }

    fn record_here(&mut self, muted: TrackSet, timing: BlockTiming) {
        let now = timing.callback_us as f64;
        let t = &self.transport;
        if t.state != TransportState::Playing {
            let from = t.tick().max(0.0).floor() as u64;
            let frame = self.block_frame;
            self.push_recorded(Recorded::Started {
                from,
                count_in_beats: 0,
                frame,
            });
            self.push_recorded(Recorded::Stopped {
                t_us: timing.callback_us,
                tick: from as f64,
                lap: 0,
                frame,
            });
            return;
        }
        let heard_tick = self
            .clock
            .segment_at(now)
            .filter(|s| s.session == t.session && s.state == TransportState::Playing)
            .map_or_else(|| t.tick(), |s| heard(&s, now, t.map_for(&s)).tick);
        let from = heard_tick.max(0.0).floor() as u64;
        self.end_run(timing.callback_us, timing);
        let t = &mut self.transport;
        t.punched = Some(t.session);
        t.session = t.session.wrapping_add(1);
        t.lap = 0;
        t.record_from = from;
        t.aim_record_beat();
        t.run_from = from;
        t.muted = muted;
        t.state = TransportState::Recording;
        let (frame, tick) = (self.block_frame, t.tick());
        self.push_recorded(Recorded::Started {
            from,
            count_in_beats: 0,
            frame,
        });
        self.push_recorded(Recorded::Rolling { frame, tick });
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

    /// The run of the transport ends at `t_us` and the next begins
    /// ([`Recorded::RecentEnd`]). Call before the state or the position changes.
    fn end_run(&mut self, t_us: u64, timing: BlockTiming) {
        let run = self.transport.run;
        let at = match self.transport.state {
            TransportState::Stopped => Some(None),
            TransportState::Playing => Some(Some(self.run_place(t_us, timing))),
            // What is played in these is the recording's.
            TransportState::CountIn | TransportState::Recording => None,
        };
        if let Some(at) = at {
            self.push_recorded(Recorded::RecentEnd { run, t_us, at });
        }
        let t = &mut self.transport;
        t.run = run.wrapping_add(1);
        t.run_lap = 0;
    }

    /// Where the listener was at `t_us` in the run going on, which plays. Before the run's
    /// first frame reaches the DAC, the place is counted back from the frames about to be
    /// rendered: a key played in that gap comes just before where the run starts.
    fn run_place(&self, t_us: u64, timing: BlockTiming) -> RecentAt {
        let t = &self.transport;
        let segment = self
            .clock
            .segment_at(t_us as f64)
            .filter(|s| s.run == t.run)
            .unwrap_or_else(|| self.segment_now(0, timing));
        RecentAt {
            tick: heard(&segment, t_us as f64, t.map_for(&segment)).tick,
            lap: segment.run_lap,
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
        let t = &self.transport;
        if let Some(slot) = self.tracks.get_mut(track) {
            // At the song's position, not ahead of it: what lies inside the lookahead is sent
            // at once by the next `play_due` and sounds late, rather than not at all.
            slot.ahead_wrapped = false;
            let instrument = slot.instrument.as_deref_mut();
            if t.frames == 0 {
                let at = t.anchor_tick;
                slot.lane.seek(|tick| (tick as f64) < at, instrument);
            } else {
                // By frames, as `play_due` sends them: an event due at this very frame is
                // still to come, whatever the last bits of the position say.
                slot.lane
                    .seek(|tick| t.frame_of(tick as f64) < t.frames, instrument);
            }
        }
    }

    /// As [`Engine::seek_lane`], but past every event the lane has sent already, those inside
    /// its lookahead and those of the loop's start when it went round ahead: the new sequence
    /// has them too.
    fn seek_lane_past_sent(&mut self, track: usize) {
        let t = &self.transport;
        let Some(slot) = self.tracks.get_mut(track) else {
            return;
        };
        let instrument = slot.instrument.as_deref_mut();
        if t.frames == 0 {
            let at = t.anchor_tick;
            slot.lane.seek(|tick| (tick as f64) < at, instrument);
            return;
        }
        let sent = t.frames + u64::from(slot.lookahead);
        if slot.ahead_wrapped {
            let start = t.loop_start;
            slot.lane.seek(
                |tick| {
                    tick < start
                        || t.loop_end_frame.saturating_add(t.map.frame_of(
                            tick as f64,
                            t.loop_start_sec,
                            t.sample_rate,
                        )) < sent
                },
                instrument,
            );
        } else {
            slot.lane
                .seek(|tick| t.frame_of(tick as f64) < sent, instrument);
        }
    }

    fn set_record_muted(&mut self, muted: TrackSet) {
        let before = self.transport.muted;
        self.transport.muted = muted;
        if self.transport.state != TransportState::Recording {
            return;
        }
        for i in 0..self.tracks.len() {
            match (before.contains(i), muted.contains(i)) {
                (false, true) => {
                    let slot = &mut self.tracks[i];
                    slot.lane.silence(slot.instrument.as_deref_mut());
                }
                (true, false) if self.transport.lane_plays(i) => self.seek_lane(i),
                _ => {}
            }
        }
    }

    /// Keys played after `record_until` sound but are not recorded. The MIDI thread's keys and
    /// the PC keyboard's come in the order they were played; the earlier of the two heads first.
    fn read_midi(&mut self, timing: BlockTiming, record_until: u64) {
        loop {
            let typed_first = match (self.midi.peek(), self.typed.peek()) {
                (Ok(midi), Ok(typed)) => typed.t_us < midi.t_us,
                (Err(_), Ok(_)) => true,
                (_, Err(_)) => false,
            };
            if typed_first {
                let Ok(midi) = self.typed.pop() else { break };
                self.key_in(midi, timing, record_until);
                continue;
            }
            let Ok(midi) = self.midi.pop() else { break };
            // Only the MIDI keyboard's: the PC keyboard's time comes from the screen, and this
            // is the key-to-sound latency the settings show.
            if matches!(midi.event, InstrumentEvent::NoteOn { velocity, .. } if velocity > 0) {
                // Every event starts at frame 0 of this block, so `playback_us` is its first sample.
                let _ = self.latency.push(LatencyRecord {
                    key_us: midi.t_us,
                    callback_us: timing.callback_us,
                    playback_us: timing.playback_us,
                });
            }
            self.key_in(midi, timing, record_until);
        }
    }

    /// One event of a keyboard: it sounds on the play target and is written down as played.
    fn key_in(&mut self, midi: MidiIn, timing: BlockTiming, record_until: u64) {
        if self.arp_key(midi, timing) {
            return;
        }
        if let Some(instrument) = self
            .play_target
            .and_then(|t| self.tracks[t].instrument.as_mut())
        {
            instrument.event(midi.event);
        }
        self.stamp_played(midi, timing, record_until);
    }

    /// Writes down an event that sounded on the play target at `midi.t_us`: for the recording
    /// going on, or else as what can be kept afterwards. Whatever plays notes there by hand
    /// comes through here.
    fn stamp_played(&mut self, midi: MidiIn, timing: BlockTiming, record_until: u64) {
        if midi.t_us <= record_until {
            self.stamp(midi);
        }
        // Played after the stop that is being taken: in the run that stop begins.
        let after_stop = midi.t_us > record_until && self.transport.state.is_moving();
        self.stamp_recent(midi, timing, after_stop);
    }

    /// Sends a keyboard event that no recording takes, with the position the person heard
    /// when they played it while the song plays ([`Recorded::Recent`]).
    fn stamp_recent(&mut self, midi: MidiIn, timing: BlockTiming, after_stop: bool) {
        let t = &self.transport;
        let (run, at) = match t.state {
            _ if after_stop => (t.run.wrapping_add(1), None),
            TransportState::Stopped => (t.run, None),
            TransportState::Playing => (t.run, Some(self.run_place(midi.t_us, timing))),
            TransportState::CountIn | TransportState::Recording => return,
        };
        if self.recorded.slots() <= RECORDED_HEADROOM {
            self.meters.record_overflows.fetch_add(1, Relaxed);
            return;
        }
        self.push_recorded(Recorded::Recent {
            event: midi.event,
            t_us: midi.t_us,
            run,
            at,
        });
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
        let h = heard(&segment, midi.t_us as f64, t.map_for(&segment));
        let near_start = h.tick >= t.record_from as f64 - f64::from(t.record_beat) / 2.0;
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
        self.segment = self.segment_now(frame, timing);
        self.segment_from = frame;
    }

    /// The stretch that starts at `frame` of this block, where the transport is now.
    fn segment_now(&self, frame: usize, timing: BlockTiming) -> Segment {
        let t = &self.transport;
        let tick = t.tick();
        Segment {
            start_us: self.dac_us(frame, timing),
            frame0: self.block_frame + frame as u64,
            frames: 0,
            sample_rate: self.sample_rate as u32,
            tick,
            ticks_per_frame: t.speed_at(tick) / t.sample_rate,
            sec0: t.anchor_sec + t.frames as f64 / t.sample_rate,
            map_gen: t.map_gen,
            state: t.state,
            session: t.session,
            lap: t.lap,
            record_from: t.record_from as i64,
            ticks_per_beat: t.record_click,
            run: t.run,
            run_lap: t.run_lap,
        }
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
        if t.state == TransportState::CountIn && t.frames >= t.count_in.end_frame {
            self.close_segment(frame);
            let t = &mut self.transport;
            t.state = TransportState::Recording;
            t.set_pos(t.record_from as f64);
            let tick = t.record_from as f64;
            self.seek_lanes();
            self.push_recorded(Recorded::Rolling {
                frame: self.block_frame + frame as u64,
                tick,
            });
            self.open_segment(frame, timing);
        }
        let t = &mut self.transport;
        if t.loop_active() && t.frames >= t.loop_end_frame {
            self.close_segment(frame);
            // The lanes that read ahead went round already.
            let went_round: TrackSet = (self.tracks.iter().enumerate())
                .filter(|(_, track)| track.ahead_wrapped)
                .map(|(i, _)| i)
                .collect();
            for track in self.tracks.iter_mut().filter(|t| !t.ahead_wrapped) {
                track.lane.silence_at_loop(track.instrument.as_deref_mut());
            }
            let t = &mut self.transport;
            t.set_pos(t.loop_start as f64);
            if t.state == TransportState::Playing {
                t.run_lap += 1;
            }
            if t.state == TransportState::Recording {
                t.lap += 1;
                let lap = t.lap;
                let t_us = self.dac_us(frame, timing).round() as u64;
                let frame = self.block_frame + frame as u64;
                self.push_recorded(Recorded::LapChanged { lap, t_us, frame });
            }
            for i in 0..self.tracks.len() {
                if went_round.contains(i) {
                    self.tracks[i].ahead_wrapped = false;
                } else if self.transport.lane_plays(i) {
                    self.seek_lane(i);
                }
            }
            self.open_segment(frame, timing);
        }

        self.mute_lanes();
        let t = &self.transport;
        for (i, track) in self.tracks.iter_mut().enumerate() {
            if t.lane_plays(i) {
                track.play_due(t);
            }
        }
        let t = &mut self.transport;
        if t.click_frame <= t.frames {
            if t.click_audible() {
                let (sound, gain) = (self.click_sound, self.click_gain);
                self.click
                    .start(t.next_click.downbeat, self.sample_rate, sound, gain);
            }
            t.advance_click();
        }
        if t.cut_frame <= t.frames {
            t.advance_cut();
        }
    }

    /// The mute lanes, read where the sequences are read: a track whose lane mutes it lets
    /// go of what its sequence holds, one whose lane lets it play picks up at the position.
    fn mute_lanes(&mut self) {
        for i in 0..self.tracks.len() {
            let t = &self.transport;
            let track = &mut self.tracks[i];
            let ahead = u64::from(track.lookahead);
            let (muted, due) = match track.auto.as_deref() {
                Some(set) => {
                    let tick = t.tick_ahead(ahead);
                    let due = (set.next_mute_change(tick))
                        .and_then(|change| t.frames_to(change as f64, ahead))
                        .map_or(u64::MAX, |frames| t.frames + frames);
                    (set.mute_at(tick).unwrap_or(false), due)
                }
                None => (false, u64::MAX),
            };
            track.mute_due = due;
            if muted == t.auto_silent.contains(i) {
                continue;
            }
            if muted {
                self.transport.auto_silent.insert(i);
                track.lane.silence(track.instrument.as_deref_mut());
            } else {
                self.transport.auto_silent.remove(i);
                if self.transport.lane_plays(i) {
                    self.seek_lane(i);
                }
            }
        }
    }

    /// Frames from the next frame to the next one where [`Engine::step_at`] has work, at most
    /// `limit` and at least 1. Call it after `step_at`, which works out what the lanes have
    /// next.
    fn frames_to_next_point(&self, limit: usize) -> usize {
        let t = &self.transport;
        if !t.state.is_moving() {
            return limit;
        }
        let mut n = (limit as u64)
            .min(t.click_frame.saturating_sub(t.frames))
            .min(t.cut_frame.saturating_sub(t.frames));
        if t.state == TransportState::CountIn {
            n = n.min(t.count_in.end_frame.saturating_sub(t.frames));
        }
        let looped = t.loop_active();
        if looped {
            n = n.min(t.loop_end_frame.saturating_sub(t.frames));
        }
        for (i, track) in self.tracks.iter().enumerate() {
            n = n.min(track.mute_due.saturating_sub(t.frames));
            if !t.lane_plays(i) {
                continue;
            }
            let at = t.frames + u64::from(track.lookahead);
            n = n.min(track.due.saturating_sub(at));
            // Where a lane that reads ahead goes round the loop by itself.
            if track.lookahead > 0 && !track.ahead_wrapped && looped {
                n = n.min(t.loop_end_frame.saturating_sub(at));
            }
        }
        n.max(1) as usize
    }

    /// The automation of a stretch of `frames` that comes before the instruments are asked
    /// for it: the points of their parameters, and where the fader's lanes are read.
    fn automate(&mut self, frames: usize) {
        let t = &self.transport;
        let mut mixing = false;
        for track in self.tracks.iter_mut() {
            let Some(set) = track.auto.as_deref_mut() else {
                continue;
            };
            mixing |= set.has_mix();
            // While another instrument is tried out in its place, the lanes of the track's
            // own are not its.
            if set.has_instrument()
                && track.aside.is_none()
                && let Some(instrument) = track.instrument.as_deref_mut()
            {
                set.instrument_points(t, u64::from(track.lookahead), frames, instrument);
            }
        }
        if mixing {
            for (k, tick) in (self.auto_ticks.iter_mut())
                .take(frames.div_ceil(MIX_STEP))
                .enumerate()
            {
                *tick = t.tick_ahead(((k + 1) * MIX_STEP).min(frames) as u64 - 1);
            }
        }
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
        self.click_sum[..frames].fill(0.0);
        if let Some(feed) = self.input.as_deref_mut() {
            feed.take(frames, &mut self.input_bus);
        }
        let fade = fade_frames(self.sample_rate);
        // The returns and the master are not cut into stretches: they hear the tempo at the
        // first frame of the piece.
        let mut piece_ctx = None;

        let mut from = 0;
        while from < frames {
            self.step_at(offset + from, timing);
            self.arp_at(offset + from, timing);
            let arp_left = self.arp_frames_left(offset + from).max(1);
            let next_point = self.frames_to_next_point(frames - from) as u64;
            let to = from + next_point.min(arp_left) as usize;
            let told = self.transport.for_plugins();
            self.plugin_block.set_transport(&told);
            let ctx = EffectContext {
                tempo_bpm: told.tempo_bpm as f32,
            };
            piece_ctx.get_or_insert(ctx);
            self.automate(to - from);
            // Every instrument that renders in another process starts on this stretch now.
            for track in self.tracks.iter_mut().chain([&mut self.sample]) {
                if let Some(instrument) = track.instrument.as_mut() {
                    instrument.ask(to - from);
                }
            }
            let [master_l, master_r, sums @ ..] = &mut self.sums;
            let [a_l, a_r, b_l, b_r] = sums.each_mut().map(|sum| &mut sum[from..to]);
            let mut mix = Mix {
                scratch: &mut self.scratch,
                retired: &mut self.retired,
                retired_effects: &mut self.retired_effects,
                fade_frames: fade,
                ramp_frames: ramp_frames(self.sample_rate),
                clock: &self.transport,
                ticks: &self.auto_ticks,
                input: &self.input_bus,
                input_from: from,
                input_live: self.input_live && self.input.is_some(),
                stream: StreamCtx {
                    buf: &mut self.stream_buf,
                    retired: &mut self.retired_clips,
                    wants: &mut self.wants,
                    now: None,
                    fade,
                    ramp: ramp_frames(self.sample_rate),
                },
                missing: &self.meters.stream_missing,
                ctx,
                master_l: &mut master_l[from..to],
                master_r: &mut master_r[from..to],
                returns: [[a_l, a_r], [b_l, b_r]],
            };
            for (i, (track, meter)) in
                (self.tracks.iter_mut().zip(self.meters.tracks.iter())).enumerate()
            {
                mix.add(track, Some(meter), i);
            }
            mix.add(&mut self.sample, None, usize::MAX);
            self.click.add(&mut self.click_sum[from..to]);
            if self.transport.state.is_moving() {
                self.transport.frames += (to - from) as u64;
            }
            from = to;
        }
        let Some(ctx) = piece_ctx else {
            return;
        };

        let [master_l, master_r, sums @ ..] = &mut self.sums;
        let (master_l, master_r) = (&mut master_l[..frames], &mut master_r[..frames]);
        let [_, _, spare_l, spare_r, dry_l, dry_r] = &mut self.scratch;
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
                dry_l,
                dry_r,
                ctx,
                fade_frames: fade,
            };
            strip.inserts(&mut io, &mut self.retired_effects, None);
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
            dry_l,
            dry_r,
            ctx,
            fade_frames: fade,
        };
        self.master_strip
            .inserts(&mut io, &mut self.retired_effects, None);
        let limited = self.master_strip.limiting_db();
        if let Some(db) = limited {
            raise(&self.meters.master_reduction, db);
        }
        self.meters.master_limited.store(limited.is_some(), Relaxed);
        self.analysis.write(io.left, io.right);
        if self.tone.sounding() || self.probe.sounding() {
            self.meters.aside.store(true, Relaxed);
        }
        // After the master's strip: the test tone tells whether the output sounds, whatever
        // the song's mixer does.
        self.tone.add(io.left, io.right, self.sample_rate);
        self.probe.add(io.left, io.right);

        let mut peak = [0.0f32; 2];
        for ((frame, (l, r)), click) in out
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(io.left.iter().zip(io.right.iter()))
            .zip(&self.click_sum[..frames])
        {
            // What is not a number never reaches the output.
            let (l, r) = if l.is_finite() && r.is_finite() {
                (*l, *r)
            } else {
                (0.0, 0.0)
            };
            peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
            frame[0] = l + click;
            frame[1] = r + click;
            if self.clamp_out {
                frame[0] = frame[0].clamp(-1.0, 1.0);
                frame[1] = frame[1].clamp(-1.0, 1.0);
            }
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
    scratch: &'a mut [Box<[f32]>; 6],
    retired: &'a mut Producer<Box<dyn Instrument>>,
    retired_effects: &'a mut Producer<Box<dyn Effect>>,
    fade_frames: u32,
    /// [`RAMP_S`] in frames: how long a fader's lane takes to go the whole way.
    ramp_frames: u32,
    clock: &'a Transport,
    /// Where each [`MIX_STEP`] of the stretch ends ([`Engine::automate`]).
    ticks: &'a [f64],
    /// The input's channels over the piece; the stretch begins at `input_from`.
    input: &'a [Box<[f32]>; MAX_INPUT_CHANNELS],
    input_from: usize,
    input_live: bool,
    stream: StreamCtx<'a>,
    missing: &'a AtomicU64,
    ctx: EffectContext,
    master_l: &'a mut [f32],
    master_r: &'a mut [f32],
    /// The same stretch of what goes to return A and to return B.
    returns: [[&'a mut [f32]; 2]; 2],
}

impl Mix<'_> {
    /// Renders `track` over the stretch, with what fades out on it, takes it through its
    /// strip, and adds it to the master and to the returns it sends to.
    /// `index`: the track's number, for what its sequence and clips may do now.
    fn add(&mut self, track: &mut Track, meter: Option<&[AtomicU32; 2]>, index: usize) {
        let frames = self.master_l.len();
        let [left, right, old_l, old_r, dry_l, dry_r] = &mut *self.scratch;
        let (left, right) = (&mut left[..frames], &mut right[..frames]);
        let aside_fading = track.aside_left > 0;
        let fading = track.fade_left > 0 || aside_fading;
        match (track.voice.as_mut(), track.instrument.as_mut()) {
            (Some(voice), _) => voice.render(
                left,
                right,
                self.input,
                self.input_from,
                self.input_live,
                self.ramp_frames,
            ),
            (None, Some(instrument)) => instrument.render(left, right),
            (None, None) if fading || track.clips.is_active() => {
                left.fill(0.0);
                right.fill(0.0);
            }
            (None, None) => {
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
        if track.clips.is_active() {
            self.stream.now = self.clock.stream_now(track.lookahead);
            let plays = self.clock.lane_plays(index);
            let missing = track.clips.render(left, right, plays, &mut self.stream);
            if missing > 0 {
                self.missing.fetch_add(u64::from(missing), Relaxed);
            }
        }

        let mut io = Io {
            left,
            right,
            spare_l: old_l,
            spare_r: old_r,
            dry_l,
            dry_r,
            ctx: self.ctx,
            fade_frames: self.fade_frames,
        };
        let auto = (track.auto.as_deref_mut()).map(|set| SlotAuto {
            set,
            clock: self.clock,
        });
        track.strip.inserts(&mut io, self.retired_effects, auto);
        let (peak, finite) = Mix::level(track, io.left, io.right, self.ticks, self.ramp_frames);
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
        for (bus, into) in self.returns.iter_mut().enumerate() {
            Mix::send(
                track,
                bus,
                io.left,
                io.right,
                into,
                self.ticks,
                self.ramp_frames,
            );
        }
    }

    /// Fader and pan, where the hand put them or where the track's lanes take them.
    fn level(
        track: &mut Track,
        left: &mut [f32],
        right: &mut [f32],
        ticks: &[f64],
        ramp: u32,
    ) -> ([f32; 2], bool) {
        let Some(set) = track.auto.as_deref_mut().filter(|set| set.has_level()) else {
            if std::mem::take(&mut track.steered) {
                let (gain, pan) = track.strip.hand();
                track.strip.steer_level(gain, pan, ramp);
            }
            return track.strip.level(left, right);
        };
        track.steered = true;
        let mut peak = [0.0f32; 2];
        let mut finite = true;
        for ((l, r), tick) in (left.chunks_mut(MIX_STEP))
            .zip(right.chunks_mut(MIX_STEP))
            .zip(ticks)
        {
            let (gain, pan) = set.level(*tick, l.len(), ramp as f32, track.strip.hand());
            track.strip.steer_level(gain, pan, l.len() as u32);
            let (p, f) = track.strip.level(l, r);
            peak = [peak[0].max(p[0]), peak[1].max(p[1])];
            finite &= f;
        }
        (peak, finite)
    }

    /// What the track sends to return `bus`, as [`Mix::level`] does it.
    fn send(
        track: &mut Track,
        bus: usize,
        left: &[f32],
        right: &[f32],
        [to_l, to_r]: &mut [&mut [f32]; 2],
        ticks: &[f64],
        ramp: u32,
    ) {
        let Some(set) = track.auto.as_deref_mut().filter(|set| set.has_send(bus)) else {
            if std::mem::take(&mut track.sends_steered[bus]) {
                let hand = track.strip.hand_send(bus);
                track.strip.steer_send(bus, hand, ramp);
            }
            track.strip.send(bus, left, right, [to_l, to_r]);
            return;
        };
        track.sends_steered[bus] = true;
        for (k, tick) in ticks.iter().enumerate() {
            let from = k * MIX_STEP;
            if from >= left.len() {
                break;
            }
            let to = (from + MIX_STEP).min(left.len());
            let hand = track.strip.hand_send(bus);
            let gain = set.send(bus, *tick, to - from, ramp as f32, hand);
            track.strip.steer_send(bus, gain, (to - from) as u32);
            let into = [&mut to_l[from..to], &mut to_r[from..to]];
            track
                .strip
                .send(bus, &left[from..to], &right[from..to], into);
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
    retired_maps: Consumer<Arc<TempoMap>>,
    retired_autos: Consumer<Box<AutoSet>>,
    retired_inputs: Consumer<Box<InputFeed>>,
    retired_clips: Consumer<Box<ClipTable>>,
    retired_streams: Consumer<Box<StreamReader>>,
    wants: Option<Consumer<StreamWant>>,
    probes: Consumer<ProbeSent>,
    analysis: Option<AnalysisReader>,
    /// The map last sent to the engine, and its number.
    map: Arc<TempoMap>,
    map_gen: u32,
    /// The last number given out, also by the handle this one took the place of.
    last_map_gen: u32,
    latency: Consumer<LatencyRecord>,
    recorded: Consumer<Recorded>,
    typed: Producer<MidiIn>,
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

    /// The ring of the PC keyboard (R2): its keys are played as the MIDI thread's are, and
    /// read with them in the order of their `t_us`, which must not go back. Only the thread
    /// that owns this handle pushes into it; [`EngineHandle::rebuild`] makes a new one.
    pub fn typed_in(&mut self) -> &mut MidiFeed {
        &mut self.typed
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

    /// As [`EngineHandle::swap_sequence`], for a `sequence` that has every event of the one
    /// there and more ([`EngineCommand::GrowSequence`]).
    pub fn grow_sequence(&mut self, track: usize, sequence: Sequence) -> Result<(), EngineCommand> {
        self.send(EngineCommand::GrowSequence {
            track,
            sequence: Box::new(sequence),
        })
    }

    pub fn take_retired_sequence(&mut self) -> Option<Box<Sequence>> {
        self.retired_sequences.pop().ok()
    }

    /// Sends the lanes of `track` that play from now on ([`EngineCommand::SetAutomation`]);
    /// none: no lane plays. The set is made here (this allocates).
    pub fn set_automation(
        &mut self,
        track: usize,
        lanes: Vec<crate::AutoLane>,
    ) -> Result<(), EngineCommand> {
        let set = (!lanes.is_empty()).then(|| Box::new(AutoSet::new(lanes)));
        self.send(EngineCommand::SetAutomation { track, set })
    }

    pub fn take_retired_automation(&mut self) -> Option<Box<AutoSet>> {
        self.retired_autos.pop().ok()
    }

    /// Sends the song's tempo map ([`EngineCommand::SetTempoMap`]) and gives it the next
    /// number. A reader of the clock on another thread needs the same map and number
    /// ([`TransportClock::heard_at`]); [`EngineHandle::tempo_map`] has them.
    pub fn set_tempo_map(&mut self, map: Arc<TempoMap>) -> Result<(), EngineCommand> {
        let map_gen = self.last_map_gen.wrapping_add(1).max(1);
        self.send(EngineCommand::SetTempoMap {
            map: map.clone(),
            map_gen,
        })?;
        self.map = map;
        self.map_gen = map_gen;
        self.last_map_gen = map_gen;
        Ok(())
    }

    /// The map last sent, and its number. A new engine starts with 120.00 in 4/4 as number 0.
    pub fn tempo_map(&self) -> (&Arc<TempoMap>, u32) {
        (&self.map, self.map_gen)
    }

    /// [`TransportClock::heard_at`] with the map last sent.
    pub fn heard_at(&self, t_us: u64) -> Option<HeardPosition> {
        self.clock.heard_at(t_us, &self.map, self.map_gen)
    }

    pub fn heard_now(&self) -> Option<HeardPosition> {
        self.heard_at(monotonic_us())
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
        while self.retired_maps.pop().is_ok() {
            count += 1;
        }
        while self.take_retired_automation().is_some() {
            count += 1;
        }
        while self.retired_inputs.pop().is_ok() {
            count += 1;
        }
        while self.retired_clips.pop().is_ok() {
            count += 1;
        }
        while self.retired_streams.pop().is_ok() {
            count += 1;
        }
        count
    }

    /// The ring of a vocal track that the read-ahead thread fills ([`crate::stream_ring`]),
    /// or none ([`EngineCommand::SetStream`]).
    pub fn set_stream(
        &mut self,
        track: usize,
        reader: Option<StreamReader>,
    ) -> Result<(), EngineCommand> {
        self.send(EngineCommand::SetStream {
            track,
            reader: reader.map(Box::new),
        })
    }

    /// The clips of a vocal track ([`EngineCommand::SetClips`]); `None`: it has none.
    pub fn set_clips(
        &mut self,
        track: usize,
        table: Option<ClipTable>,
        refeed: bool,
    ) -> Result<(), EngineCommand> {
        self.send(EngineCommand::SetClips {
            track,
            table: table.map(Box::new),
            refeed,
        })
    }

    /// [`EngineCommand::SpliceClips`].
    pub fn splice_clips(&mut self, track: usize) -> Result<(), EngineCommand> {
        self.send(EngineCommand::SpliceClips { track })
    }

    /// What the engine tells the read-ahead thread: there is one ring for the life of the
    /// engine, so this gives it once, and once more after [`EngineHandle::rebuild`].
    pub fn take_stream_wants(&mut self) -> Option<Consumer<StreamWant>> {
        self.wants.take()
    }

    /// The master's sound for an analysis thread ([`crate::AnalysisReader`]): once for the
    /// life of the engine, and once more after [`EngineHandle::rebuild`]. The engine writes
    /// into the ring only from now on, while the reader is not dropped.
    pub fn take_analysis(&mut self) -> Option<AnalysisReader> {
        self.analysis.take().map(AnalysisReader::start)
    }

    /// The PC's input for the voices of the vocal tracks, or none. The feed before comes
    /// back on a retired ring ([`EngineHandle::drop_retired`]). An engine made anew
    /// ([`EngineHandle::rebuild`]) has none.
    pub fn set_input(&mut self, feed: Option<InputFeed>) -> Result<(), EngineCommand> {
        self.send(EngineCommand::SetInput {
            feed: feed.map(Box::new),
        })
    }

    /// The clicks of the latency measurement that left the engine.
    pub fn pop_probe(&mut self) -> Option<ProbeSent> {
        self.probes.pop().ok()
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
        let mut parts = Engine::assemble(config, self.meters.clone(), self.plugin_block.clone());
        // The new engine has map 0 again, but the numbers given out go on: a reader that
        // still holds a map from before must not take it for the new engine's.
        parts.handle.last_map_gen = self.last_map_gen;
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
