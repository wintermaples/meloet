//! The export's engine (docs/architecture.md 22-11): a second `meloet_engine::Engine` that
//! no stream runs. The export's thread calls its blocks one after the other, as long as
//! the blocks of the playing engine, and takes what comes out.
//!
//! It is told of the song with the same [`EngineOp`]s the playing engine is
//! ([`super::told`]), and each op becomes the same engine command. The vocal clips go the
//! way they go while playing, through the ring of each track; only that nobody reads
//! ahead on a thread of its own: the rings are filled between two blocks
//! ([`ReadAhead::inline`]), so a block never finds one short and never waits for the disk.

use std::collections::HashSet;
use std::sync::Arc;

use meloet_engine::{
    BlockTiming, Effect, Engine, EngineCommand, EngineConfig, EngineHandle, Instrument, MidiFeed,
    PluginBlock, Sequence, TrackSet, stream_ring,
};
use meloet_model::song::Clip;
use meloet_model::{InstrumentRef, TempoMap, Timeline};

use crate::instruments::{self, Samples};
use crate::made::{MadeKey, MadePlan, seat_audio};
use crate::mixbook;
use crate::pcm::PcmStore;
use crate::play::{EngineOp, MAX_TRACKS, Spot};
use crate::readahead::ReadAhead;
use crate::service::{auto_lanes, sequence_event};

/// How long the engine runs with the song standing still before it plays: the faders and
/// the lanes have come to their values (20 ms), what was put into a slot has come in
/// (10 ms), and the rings of the vocal tracks are full for where the song starts.
const REST_S: f64 = 0.25;

/// The plugins loaded for the export, by the ticket the rules know each under.
#[derive(Default)]
pub struct Plugs {
    pub instruments: Vec<(u64, Box<dyn Instrument>)>,
    pub effects: Vec<(u64, Box<dyn Effect>)>,
}

struct SeatClips {
    seat: usize,
    clips: Vec<Arc<Clip>>,
    plans: Vec<MadePlan>,
    /// The engine has a ring for the seat.
    live: bool,
}

pub struct ExportEngine {
    engine: Engine,
    handle: EngineHandle,
    _midi: MidiFeed,
    readahead: ReadAhead,
    rate: u32,
    block: usize,
    samples: Vec<(InstrumentRef, Samples)>,
    timeline: Option<Arc<Timeline>>,
    clips: Vec<SeatClips>,
    made: HashSet<MadeKey>,
    silent: Vec<usize>,
    out: Vec<f32>,
}

impl ExportEngine {
    /// `block`: the frames of a block, those of the playing engine's stream. `samples`:
    /// the samples of the song's instruments, as far as they are read.
    pub fn new(
        rate: u32,
        block: usize,
        store: Arc<dyn PcmStore>,
        samples: Vec<(InstrumentRef, Samples)>,
    ) -> ExportEngine {
        let mut parts = Engine::new(EngineConfig {
            sample_rate: rate,
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        parts.engine.prepare(rate);
        // The check counts what goes over 0 dB, and a 32 bit float file keeps it; the
        // 16 and 24 bit files cut it where they are written (architecture 23-16).
        parts.engine.keep_overs();
        let block = block.max(1);
        let mut readahead = ReadAhead::inline(store);
        if let Some(wants) = parts.handle.take_stream_wants() {
            readahead.set_wants(wants);
        }
        readahead.pump();
        ExportEngine {
            engine: parts.engine,
            handle: parts.handle,
            _midi: parts.midi_in,
            readahead,
            rate,
            block,
            samples,
            timeline: None,
            clips: Vec::new(),
            made: HashSet::new(),
            silent: Vec::new(),
            out: vec![0.0; block * 2],
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn block(&self) -> usize {
        self.block
    }

    pub fn max_frames(&self) -> usize {
        self.engine.max_frames()
    }

    /// What the plugins of the export read the song's place from.
    pub fn plugin_block(&self) -> Arc<PluginBlock> {
        self.handle.plugin_block().clone()
    }

    /// The map the engine has now.
    pub fn map(&self) -> Arc<TempoMap> {
        self.handle.tempo_map().0.clone()
    }

    /// How many frames the master's inserts delay what comes out.
    pub fn master_latency(&self) -> usize {
        self.engine.master_latency() as usize
    }

    /// Frames of vocal clips that were not in a ring when a block wanted them. Always 0:
    /// the rings are filled between the blocks.
    pub fn missing(&self) -> u64 {
        self.handle.meters().stream_missing()
    }

    /// The audio that is to be made for the vocal clips as the engine was told them.
    pub fn plans(&self) -> Vec<MadePlan> {
        (self.clips.iter())
            .flat_map(|c| c.plans.iter().cloned())
            .collect()
    }

    /// These are in the cache: the clips they were made for play them from now on.
    pub fn set_made(&mut self, made: HashSet<MadeKey>) {
        self.made = made;
        self.resend_clips();
    }

    /// Opens the file of every vocal clip now. Returns how many have none.
    pub fn open_files(&mut self) -> usize {
        self.readahead.open_all()
    }

    fn settle(&mut self) {
        loop {
            self.engine.settle();
            if self.handle.drop_retired() == 0 {
                break;
            }
        }
    }

    /// The engine takes every command: none is lost, or it and the song no longer agree.
    fn push(&mut self, mut command: EngineCommand) {
        loop {
            command = match self.handle.send(command) {
                Ok(()) => break,
                Err(command) => command,
            };
            self.settle();
        }
        self.settle();
    }

    fn sent(&mut self, sent: Result<(), EngineCommand>) {
        match sent {
            Ok(()) => self.settle(),
            Err(command) => self.push(command),
        }
    }

    fn make(&self, instrument: &InstrumentRef) -> Box<dyn Instrument> {
        let samples = (self.samples.iter())
            .find(|(i, _)| i == instrument)
            .map_or(Samples::NotInstalled, |(_, s)| s.clone());
        instruments::make(instrument, samples)
    }

    /// One op of [`super::told::Told`], as the core's audio side does it for the playing
    /// engine (`service.rs`, `Audio::op`). `plugs`: where [`EngineOp::Plug`] finds its
    /// plugin.
    pub fn tell(&mut self, op: EngineOp, plugs: &mut Plugs) {
        match op {
            EngineOp::Instrument { seat, instrument } => {
                let made = self.make(&instrument);
                let sent = self.handle.swap_instrument(seat, made);
                self.sent(sent);
            }
            EngineOp::ClearSeat { seat } => {
                self.clips_gone(seat);
                self.push(EngineCommand::ClearTrack { track: seat });
            }
            EngineOp::PlayTarget { seat: Some(track) } => {
                self.push(EngineCommand::SetPlayTarget { track });
            }
            EngineOp::PlayTarget { seat: None } => self.push(EngineCommand::ClearPlayTarget),
            EngineOp::Insert {
                strip,
                slot,
                insert,
            } => {
                let effect = insert.as_ref().and_then(mixbook::make);
                let sent = (self.handle).set_effect(mixbook::mix_strip(strip), slot, effect);
                self.sent(sent);
            }
            op @ (EngineOp::Strip { .. }
            | EngineOp::Sends { .. }
            | EngineOp::Knobs { .. }
            | EngineOp::Param { .. }
            | EngineOp::Bypass { .. }
            | EngineOp::MoveInsert { .. }) => {
                if let Some(command) = mixbook::command(&op) {
                    self.push(command);
                }
            }
            EngineOp::Plug { ticket, spot } => self.plug(ticket, spot, plugs),
            EngineOp::Stop { t_us } => self.push(EngineCommand::Stop { t_us }),
            EngineOp::Locate { tick } => self.push(EngineCommand::Locate { tick }),
            EngineOp::Timeline { timeline } => {
                // Until the engine has room for it.
                let map = Arc::new(TempoMap::new(&timeline));
                while self.handle.set_tempo_map(map.clone()).is_err() {
                    self.settle();
                }
                self.settle();
                self.timeline = Some(timeline);
                let (map, map_gen) = self.handle.tempo_map();
                self.readahead.set_map(map.clone(), map_gen);
                self.resend_clips();
            }
            EngineOp::Loop { start, end, on } => {
                self.push(EngineCommand::SetLoop { start, end, on });
            }
            EngineOp::Metronome { on } => self.push(EngineCommand::SetMetronome { on }),
            EngineOp::Sequence { track, events } | EngineOp::GrowSequence { track, events } => {
                let sequence = Sequence::new(events.into_iter().map(sequence_event).collect());
                let sent = self.handle.swap_sequence(track, sequence);
                self.sent(sent);
            }
            EngineOp::SilentTracks { tracks } => {
                self.silent = tracks;
                self.send_silent();
            }
            EngineOp::Automation { seat, lanes } => {
                let sent = self.handle.set_automation(seat, auto_lanes(lanes));
                self.sent(sent);
            }
            EngineOp::Clips { seat, clips } => {
                match self.clips.iter_mut().find(|c| c.seat == seat) {
                    Some(entry) => entry.clips = clips,
                    None => self.clips.push(SeatClips {
                        seat,
                        clips,
                        plans: Vec::new(),
                        live: false,
                    }),
                }
                self.send_clips(seat);
            }
            // The files are opened from the place the song's are; it knows its table.
            EngineOp::AudioSources { .. } => {}
            // The export hears no keys and no input, tries nothing out, and starts and
            // stops by itself: opening a song tells none of these. The metronome is never
            // in an export.
            EngineOp::Click { .. }
            | EngineOp::Play { .. }
            | EngineOp::Record { .. }
            | EngineOp::RecordHere { .. }
            | EngineOp::RecordMuted { .. }
            | EngineOp::Audition { .. }
            | EngineOp::AuditionOff
            | EngineOp::TryInstrument { .. }
            | EngineOp::EndTrial { .. }
            | EngineOp::MoveInstrument { .. }
            | EngineOp::PlugSample { .. }
            | EngineOp::PlaySample { .. }
            | EngineOp::PlayPattern { .. }
            | EngineOp::StopSample
            | EngineOp::HoldLane { .. }
            | EngineOp::Voice { .. } => {}
        }
    }

    fn plug(&mut self, ticket: u64, spot: Spot, plugs: &mut Plugs) {
        match spot {
            Spot::Seat(seat) => {
                if let Some(at) = plugs.instruments.iter().position(|(t, _)| *t == ticket) {
                    let (_, plugin) = plugs.instruments.swap_remove(at);
                    let sent = self.handle.swap_instrument(seat, plugin);
                    self.sent(sent);
                }
            }
            Spot::Insert(strip, slot) => {
                if let Some(at) = plugs.effects.iter().position(|(t, _)| *t == ticket) {
                    let (_, plugin) = plugs.effects.swap_remove(at);
                    let strip = mixbook::mix_strip(strip);
                    let sent = self.handle.set_effect(strip, slot, Some(plugin));
                    self.sent(sent);
                }
            }
            Spot::Trial(_) | Spot::Sample => {}
        }
    }

    fn send_silent(&mut self) {
        let tracks = self.silent.iter().copied().collect::<TrackSet>();
        self.push(EngineCommand::SetSilentTracks { tracks });
    }

    /// The engine's table and the rings' clips of a seat, from their start (`Audio::
    /// send_clips` with `anew`).
    fn send_clips(&mut self, seat: usize) {
        let Some(at) = self.clips.iter().position(|c| c.seat == seat) else {
            return;
        };
        let map = self.handle.tempo_map().0.clone();
        let entry = &mut self.clips[at];
        let timeline = self.timeline.as_deref();
        let audio = seat_audio(timeline, &map, self.rate, &entry.clips, &self.made);
        entry.plans = audio.plans;
        if audio.feeds.is_empty() {
            let was_live = std::mem::take(&mut entry.live);
            if entry.clips.is_empty() {
                self.clips.remove(at);
            }
            if was_live {
                self.readahead.set_track(seat, None, Vec::new());
                self.push(EngineCommand::SetClips {
                    track: seat,
                    table: None,
                    refeed: true,
                });
                self.push(EngineCommand::SetStream {
                    track: seat,
                    reader: None,
                });
            }
            return;
        }
        if entry.live {
            self.readahead.set_clips(seat, audio.feeds);
        } else {
            entry.live = true;
            let (writer, reader) = stream_ring(self.rate);
            (self.readahead).set_track(seat, Some((writer, self.rate)), audio.feeds);
            self.push(EngineCommand::SetStream {
                track: seat,
                reader: Some(Box::new(reader)),
            });
        }
        self.push(EngineCommand::SetClips {
            track: seat,
            table: Some(Box::new(audio.table)),
            refeed: true,
        });
    }

    fn resend_clips(&mut self) {
        let seats: Vec<usize> = self.clips.iter().map(|c| c.seat).collect();
        for seat in seats {
            self.send_clips(seat);
        }
    }

    fn clips_gone(&mut self, seat: usize) {
        if let Some(at) = self.clips.iter().position(|c| c.seat == seat)
            && self.clips.remove(at).live
        {
            self.readahead.set_track(seat, None, Vec::new());
        }
    }

    /// Other tracks are silent from now on (the next track of 「トラックごと」).
    pub fn set_silent(&mut self, seats: Vec<usize>) {
        self.silent = seats;
    }

    /// Stops everything that sounds, goes to `tick`, lets the engine rest there, and
    /// plays. `anew`: the engine has played before; it is made ready as for a new stream,
    /// so that nothing of the last time is left in it.
    pub fn play_from(&mut self, tick: u64, anew: bool) {
        self.push(EngineCommand::Stop { t_us: 0 });
        if anew {
            self.settle();
            self.engine.prepare(self.rate);
        }
        // `prepare` forgets which tracks are silent.
        self.send_silent();
        self.push(EngineCommand::Locate { tick });
        let rest = (REST_S * f64::from(self.rate)) as usize;
        for _ in 0..rest.div_ceil(self.block) {
            self.render(self.block);
        }
        self.push(EngineCommand::Play { from: tick });
    }

    /// The song stops where it is; what rings on is still to come out.
    pub fn stop(&mut self) {
        self.push(EngineCommand::Stop { t_us: 0 });
    }

    /// One block of `frames` frames, at most [`ExportEngine::block`]: interleaved stereo.
    pub fn render(&mut self, frames: usize) -> &[f32] {
        let frames = frames.min(self.block);
        self.engine
            .process(&mut self.out[..frames * 2], BlockTiming::default());
        self.handle.drop_retired();
        while self.handle.pop_latency().is_some() {}
        self.readahead.pump();
        &self.out[..frames * 2]
    }
}
