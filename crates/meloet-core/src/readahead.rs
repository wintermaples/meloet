//! Reading the audio of the vocal clips ahead of the song (docs/architecture.md 22-8,
//! 22-18 A1).
//!
//! ```text
//! the files ─ PcmStore ─▶ the read-ahead thread ══ ring of 2 s per track ══▶ engine
//!                               ▲ StreamWant (generation, anchor, frame) ─────┘
//! ```
//!
//! One thread of ordinary priority. For every vocal track it keeps the track's ring full
//! with the audio in the order it sounds: the plain audio of the clips, silence between
//! them, the turns of the loop already laid out. It knows where the engine is only from
//! what the engine says ([`StreamWant`]): a new generation is a new place to start from,
//! and a want of the generation it is feeding says the engine got ahead of it (the disk was
//! slow), so it goes on from there and the place stays right.
//!
//! The fades and the volumes of the clips are not its business; the engine does those.
//!
//! When audio that was made for a clip is ready (22-19 pass 2) the clips come again with other
//! files in the same places ([`ReadAhead::splice_clips`]). The thread then lays the new
//! audio over the old for 10 ms from where it is in the ring: what sounds goes on without
//! a gap, and the engine does nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use meloet_engine::{
    STREAM_CHANNELS, STREAM_REFILL_SECONDS, StreamChunk, StreamLayout, StreamWant, StreamWants,
    StreamWriter,
};
use meloet_model::TempoMap;

use crate::pcm::{FeedClip, PcmFile, PcmStore};

/// Frames read from the disk and put into a ring in one go. Between two of them the thread
/// looks at what the engine said.
const CHUNK: usize = 4_096;
/// How often the thread looks at the rings while nothing is said to it.
const POLL: Duration = Duration::from_millis(5);
/// The same with no vocal track at all.
const IDLE: Duration = Duration::from_millis(200);

/// What the thread counts, for logs and tests.
#[derive(Debug, Default)]
pub struct ReadStats {
    /// Frames put into the rings.
    pub frames: AtomicU64,
    /// Frames passed over because the engine was already past them.
    pub skipped: AtomicU64,
    /// Reads of a file that failed; those frames are silence.
    pub read_errors: AtomicU64,
    /// Generations started.
    pub generations: AtomicU64,
    /// The seats (one bit each, below 64) with a clip whose file the store did not have:
    /// the ones to feed again when the store has more (`AudioStore::generation`).
    pub unopened: AtomicU64,
    /// Audio made for a clip was asked for and was not in the store (somebody emptied the
    /// cache): the core makes it again.
    pub made_lost: AtomicBool,
    /// Times other audio was laid over what the ring held.
    pub splices: AtomicU64,
}

fn seat_bit(seat: u32) -> u64 {
    1u64.checked_shl(seat).unwrap_or(0)
}

enum Msg {
    Wants(StreamWants),
    Map(Arc<TempoMap>, u32),
    Track {
        seat: u32,
        ring: Option<(StreamWriter, u32)>,
        clips: Vec<FeedClip>,
    },
    Clips {
        seat: u32,
        clips: Vec<FeedClip>,
    },
    Splice {
        seat: u32,
        clips: Vec<FeedClip>,
    },
    NoTracks,
    StoreChanged,
    Store(Arc<dyn PcmStore>),
    Quit,
}

/// The core's end of the thread.
pub struct ReadAhead {
    tx: Sender<Msg>,
    thread: Option<JoinHandle<()>>,
    stats: Arc<ReadStats>,
    /// Without a thread ([`ReadAhead::inline`]): the same work, done in [`ReadAhead::pump`].
    side: Option<Side>,
}

impl ReadAhead {
    fn side(store: Arc<dyn PcmStore>) -> (Sender<Msg>, Side) {
        let (tx, rx) = mpsc::channel();
        let side = Side {
            rx,
            store,
            wants: None,
            map: None,
            tracks: Vec::new(),
            bufs: Bufs {
                buf: vec![0.0; CHUNK * STREAM_CHANNELS],
                old: vec![0.0; CHUNK * STREAM_CHANNELS],
                raw: vec![0.0; CHUNK * STREAM_CHANNELS],
            },
            stats: Arc::new(ReadStats::default()),
        };
        (tx, side)
    }

    pub fn spawn(store: Arc<dyn PcmStore>) -> ReadAhead {
        let (tx, side) = ReadAhead::side(store);
        let stats = side.stats.clone();
        let thread = std::thread::Builder::new()
            .name("meloet-readahead".into())
            .spawn(move || side.run())
            .ok();
        if thread.is_none() {
            eprintln!("meloet: the read-ahead thread did not start; vocal clips are silent");
        }
        ReadAhead {
            tx,
            thread,
            stats,
            side: None,
        }
    }

    /// The same reading without a thread, for an engine that is not run by a stream (an
    /// export, docs/architecture.md 22-11): the caller runs the engine's blocks itself and
    /// calls [`ReadAhead::pump`] between them, so the engine never finds a ring short.
    pub fn inline(store: Arc<dyn PcmStore>) -> ReadAhead {
        let (tx, side) = ReadAhead::side(store);
        ReadAhead {
            tx,
            thread: None,
            stats: side.stats.clone(),
            side: Some(side),
        }
    }

    /// For [`ReadAhead::inline`]: takes in what was sent and what the engine said, and
    /// fills every ring that is due. Reads the disk, so not for an audio thread.
    pub fn pump(&mut self) {
        if let Some(side) = &mut self.side {
            while side.pass() == Some(true) {}
        }
    }

    /// For [`ReadAhead::inline`]: opens the file of every clip now, so that what happens to
    /// the files later (the cache is emptied, another song comes) does not reach the
    /// reading. Returns how many clips have no file.
    pub fn open_all(&mut self) -> usize {
        let Some(side) = &mut self.side else {
            return 0;
        };
        side.pass();
        let mut unopened = 0;
        for track in side.tracks.iter_mut() {
            for at in 0..track.clips.len() {
                let key = key_of(&track.clips[at]);
                if track.file(key, &*side.store, &side.stats).is_none() {
                    unopened += 1;
                }
            }
        }
        unopened
    }

    pub fn stats(&self) -> &Arc<ReadStats> {
        &self.stats
    }

    /// What the engine says from now on (`EngineHandle::take_stream_wants`): at the start,
    /// and for an engine made anew.
    pub fn set_wants(&self, wants: StreamWants) {
        let _ = self.tx.send(Msg::Wants(wants));
    }

    /// The map the engine was sent, and its number. Send it when the engine is.
    pub fn set_map(&self, map: Arc<TempoMap>, map_gen: u32) {
        let _ = self.tx.send(Msg::Map(map, map_gen));
    }

    /// The ring of the track on `seat` for an engine at `sample_rate`, and its clips in
    /// the order they sound; `None`: the track has no vocal clips any more. Send it before
    /// the engine gets the other end.
    pub fn set_track(&self, seat: usize, ring: Option<(StreamWriter, u32)>, clips: Vec<FeedClip>) {
        let _ = self.tx.send(Msg::Track {
            seat: seat as u32,
            ring,
            clips,
        });
    }

    /// Other clips for the track on `seat`. Send it before the engine is told to start a
    /// new generation (`EngineCommand::SetClips` with `refeed`).
    pub fn set_clips(&self, seat: usize, clips: Vec<FeedClip>) {
        let _ = self.tx.send(Msg::Clips {
            seat: seat as u32,
            clips,
        });
    }

    /// The clips of the track on `seat` in the same places, with other audio (what was made
    /// for a clip is ready, or is no longer what the clip plays). Send it before
    /// `EngineCommand::SpliceClips`.
    pub fn splice_clips(&self, seat: usize, clips: Vec<FeedClip>) {
        let _ = self.tx.send(Msg::Splice {
            seat: seat as u32,
            clips,
        });
    }

    /// Every ring is let go of (another stream is about to open).
    pub fn clear(&self) {
        let _ = self.tx.send(Msg::NoTracks);
    }

    /// The store may open now what it could not before: the files it did not have are
    /// asked for again. Tell the engine to start the seats of [`ReadStats::unopened`] anew
    /// after this, so that what was fed as silence is read again.
    pub fn store_changed(&self) {
        let _ = self.tx.send(Msg::StoreChanged);
    }

    /// Another store, for a test that brings its own files.
    pub fn set_store(&self, store: Arc<dyn PcmStore>) {
        let _ = self.tx.send(Msg::Store(store));
    }
}

impl Drop for ReadAhead {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The file a clip plays: the audio of a uid, or what was made for the clip.
type FileKey = ([u8; 16], Option<[u8; 32]>);
/// A file as the store gave it.
type Opened = (FileKey, Option<Box<dyn PcmFile>>);

fn key_of(clip: &FeedClip) -> FileKey {
    (clip.uid, clip.made)
}

/// How long other audio for the same clips takes to come in over the old: 10 ms.
fn splice_frames(sample_rate: u32) -> u64 {
    u64::from(sample_rate / 100).max(1)
}

/// The clips as they were, while other audio for them comes in.
struct Splice {
    /// The frame of the generation the new audio starts to come in at.
    from: u64,
    clips: Vec<FeedClip>,
}

struct Fed {
    seat: u32,
    writer: StreamWriter,
    sample_rate: u32,
    /// In the order they sound.
    clips: Vec<FeedClip>,
    /// The files opened for the clips; `None`: the store had none when it was asked.
    files: Vec<Opened>,
    /// The want that made the generation being fed.
    want: Option<StreamWant>,
    /// `None` until the map of the want is here.
    layout: Option<StreamLayout>,
    /// The next frame of the generation to put in.
    next: u64,
    /// The ring is being filled up: from a new generation, or from under a second left.
    filling: bool,
    splice: Option<Splice>,
}

impl Fed {
    fn take(&mut self, want: StreamWant, stats: &ReadStats) {
        match &self.want {
            Some(now) if now.generation == want.generation => {
                if want.frame > self.next {
                    stats.skipped.fetch_add(want.frame - self.next, Relaxed);
                    self.next = want.frame;
                    self.filling = true;
                }
            }
            _ => {
                self.want = Some(want);
                self.splice = None;
                self.layout = None;
                self.next = want.frame;
                // At once, behind what is left of the old generation.
                self.filling = true;
                stats.generations.fetch_add(1, Relaxed);
            }
        }
    }

    fn set_clips(&mut self, clips: Vec<FeedClip>, stats: &ReadStats) {
        self.clips = clips;
        self.clips.sort_by_key(|c| c.start);
        self.splice = None;
        self.ask_again(stats);
    }

    /// Other audio for the same clips: it comes in over the old from the next frame that
    /// goes into the ring.
    fn splice_clips(&mut self, mut clips: Vec<FeedClip>, stats: &ReadStats) {
        clips.sort_by_key(|c| c.start);
        let before = std::mem::replace(&mut self.clips, clips);
        // A splice that has not begun is still from what the ring holds.
        let old = match self.splice.take() {
            Some(splice) if splice.from == self.next => splice.clips,
            _ => before,
        };
        if self.want.is_some() {
            self.splice = Some(Splice {
                from: self.next,
                clips: old,
            });
        }
        self.ask_again(stats);
    }

    /// A file the store did not have is asked for again, and one no clip uses is let go.
    fn ask_again(&mut self, stats: &ReadStats) {
        let clips = &self.clips;
        let old = self.splice.as_ref().map_or(&[][..], |s| &s.clips[..]);
        (self.files).retain(|(key, file)| {
            file.is_some() && clips.iter().chain(old).any(|c| key_of(c) == *key)
        });
        stats.unopened.fetch_and(!seat_bit(self.seat), Relaxed);
    }

    fn file(
        &mut self,
        key: FileKey,
        store: &dyn PcmStore,
        stats: &ReadStats,
    ) -> Option<&mut Box<dyn PcmFile>> {
        let at = match self.files.iter().position(|(k, _)| *k == key) {
            Some(at) => at,
            None => {
                let file = match &key.1 {
                    Some(made) => store.open_made(made, self.sample_rate),
                    None => store.open(&key.0, self.sample_rate),
                };
                if file.is_none() {
                    stats.unopened.fetch_or(seat_bit(self.seat), Relaxed);
                    if key.1.is_some() {
                        stats.made_lost.store(true, Relaxed);
                    }
                }
                self.files.push((key, file));
                self.files.len() - 1
            }
        };
        self.files[at].1.as_mut()
    }

    /// Whether a chunk is to go in now.
    fn due(&mut self) -> bool {
        if self.layout.is_none() || self.writer.is_abandoned() {
            return false;
        }
        let refill = (f64::from(self.sample_rate) * STREAM_REFILL_SECONDS) as usize;
        if self.writer.waiting_frames() < refill {
            self.filling = true;
        }
        if self.filling && self.writer.room_frames() == 0 {
            self.filling = false;
        }
        self.filling
    }

    /// Puts the next frames into the ring, at most [`CHUNK`].
    fn fill(&mut self, store: &dyn PcmStore, bufs: &mut Bufs, stats: &ReadStats) {
        let (Some(layout), Some(want)) = (self.layout, self.want) else {
            return;
        };
        let (song, run) = layout.at(self.next);
        let room = self.writer.room_frames();
        let frames = (CHUNK as u64).min(run).min(room as u64) as usize;
        // The last of this filling: while the song plays there is always a little room
        // again, and the thread must not run after it block by block.
        if room <= CHUNK {
            self.filling = false;
        }
        if frames == 0 {
            return;
        }
        let out = &mut bufs.buf[..frames * STREAM_CHANNELS];
        let clips = std::mem::take(&mut self.clips);
        self.lay(&clips, song, out, &mut bufs.raw, store, stats);
        self.clips = clips;
        let mut spliced_to = None;
        if let Some(splice) = self.splice.take() {
            let end = splice.from + splice_frames(self.sample_rate);
            let mixed = (end.saturating_sub(self.next) as usize).min(frames);
            let old = &mut bufs.old[..mixed * STREAM_CHANNELS];
            self.lay(&splice.clips, song, old, &mut bufs.raw, store, stats);
            let steps = (end - splice.from + 1) as f32;
            for (i, (new, old)) in (out.as_chunks_mut::<2>().0.iter_mut())
                .zip(old.as_chunks::<2>().0)
                .enumerate()
            {
                let weight = (self.next + i as u64 + 1).saturating_sub(splice.from) as f32 / steps;
                for (new, old) in new.iter_mut().zip(old) {
                    *new = old + (*new - old) * weight;
                }
            }
            spliced_to = Some(end);
            self.splice = Some(splice);
        }
        let chunk = StreamChunk {
            generation: want.generation,
            first_frame: self.next,
            frames: frames as u32,
        };
        if self.writer.push(chunk, out) {
            self.next += frames as u64;
            stats.frames.fetch_add(frames as u64, Relaxed);
            if spliced_to.is_some_and(|end| self.next >= end) {
                self.splice = None;
                stats.splices.fetch_add(1, Relaxed);
                let clips = &self.clips;
                (self.files).retain(|(key, _)| clips.iter().any(|c| key_of(c) == *key));
            }
        }
    }

    /// The audio of `clips` for the song frames from `song` on, as many as `out` holds;
    /// silence where no clip is.
    fn lay(
        &mut self,
        clips: &[FeedClip],
        song: u64,
        out: &mut [f32],
        raw: &mut [f32],
        store: &dyn PcmStore,
        stats: &ReadStats,
    ) {
        out.fill(0.0);
        let (from, to) = (song, song + (out.len() / STREAM_CHANNELS) as u64);
        let first = clips.partition_point(|c| c.end <= from);
        for clip in &clips[first..] {
            if clip.start >= to {
                break;
            }
            let (a, b) = (from.max(clip.start), to.min(clip.end));
            let mut file_at = clip.file_frame + (a - clip.start) as i64;
            let mut count = (b - a) as i64;
            let mut out_at = (a - from) as usize;
            if file_at < 0 {
                // The clip's start was pulled out to before the take: silence first.
                let before = (-file_at).min(count);
                file_at += before;
                count -= before;
                out_at += before as usize;
            }
            count = count.min(clip.file_end - file_at);
            if count <= 0 {
                continue;
            }
            let Some(file) = self.file(key_of(clip), store, stats) else {
                continue;
            };
            let channels = usize::from(file.channels().clamp(1, 2));
            let raw = &mut raw[..count as usize * channels];
            match file.read(file_at as u64, raw) {
                Ok(got) => {
                    let into = out[out_at * STREAM_CHANNELS..].as_chunks_mut::<2>().0;
                    for (frame, read) in into.iter_mut().zip(raw.chunks_exact(channels).take(got)) {
                        *frame = [read[0], read[channels - 1]];
                    }
                }
                Err(e) => {
                    if stats.read_errors.fetch_add(1, Relaxed) == 0 {
                        eprintln!("meloet: a vocal clip could not be read: {e}");
                    }
                }
            }
        }
    }
}

/// A chunk as it goes into a ring, the same of the clips as they were before a splice, and
/// a chunk as it comes out of a file.
struct Bufs {
    buf: Vec<f32>,
    old: Vec<f32>,
    raw: Vec<f32>,
}

struct Side {
    rx: Receiver<Msg>,
    store: Arc<dyn PcmStore>,
    wants: Option<StreamWants>,
    map: Option<(Arc<TempoMap>, u32)>,
    tracks: Vec<Fed>,
    bufs: Bufs,
    stats: Arc<ReadStats>,
}

impl Side {
    /// One round: what was said and sent is taken in, and every ring that is due gets a
    /// chunk. Returns whether a ring got one; `None`: the thread is to end.
    fn pass(&mut self) -> Option<bool> {
        // What the engine said first, then what the core sent: the core sends the
        // clips of a new generation before it tells the engine to start one, so they
        // are here by the time its want is.
        let mut said = Vec::new();
        if let Some(wants) = self.wants.as_mut() {
            while let Ok(want) = wants.pop() {
                said.push(want);
            }
        }
        loop {
            match self.rx.try_recv() {
                Ok(Msg::Quit) | Err(mpsc::TryRecvError::Disconnected) => return None,
                Ok(msg) => self.take(msg),
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        for want in said {
            if let Some(track) = self.tracks.iter_mut().find(|t| t.seat == want.track) {
                track.take(want, &self.stats);
            }
        }
        let mut busy = false;
        for track in self.tracks.iter_mut() {
            if track.layout.is_none()
                && let (Some(want), Some((map, map_gen))) = (&track.want, &self.map)
                && want.map_gen == *map_gen
            {
                let rate = f64::from(track.sample_rate);
                track.layout = Some(StreamLayout::new(want, map, rate));
            }
            if track.due() {
                track.fill(&*self.store, &mut self.bufs, &self.stats);
                busy = true;
            }
        }
        Some(busy)
    }

    fn run(mut self) {
        loop {
            match self.pass() {
                None => return,
                Some(true) => continue,
                Some(false) => {}
            }
            let wait = if self.tracks.is_empty() { IDLE } else { POLL };
            match self.rx.recv_timeout(wait) {
                Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(msg) => self.take(msg),
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn take(&mut self, msg: Msg) {
        match msg {
            Msg::Wants(wants) => {
                self.wants = Some(wants);
                // Another engine counts its generations from the start again.
                for track in self.tracks.iter_mut() {
                    track.want = None;
                    track.layout = None;
                    track.filling = false;
                }
            }
            Msg::Map(map, map_gen) => self.map = Some((map, map_gen)),
            Msg::Track { seat, ring, clips } => {
                self.tracks.retain(|t| t.seat != seat);
                self.stats.unopened.fetch_and(!seat_bit(seat), Relaxed);
                if let Some((writer, sample_rate)) = ring {
                    let mut track = Fed {
                        seat,
                        writer,
                        sample_rate,
                        clips: Vec::new(),
                        files: Vec::new(),
                        want: None,
                        layout: None,
                        next: 0,
                        filling: false,
                        splice: None,
                    };
                    track.set_clips(clips, &self.stats);
                    self.tracks.push(track);
                }
            }
            Msg::Clips { seat, clips } => {
                if let Some(track) = self.tracks.iter_mut().find(|t| t.seat == seat) {
                    track.set_clips(clips, &self.stats);
                }
            }
            Msg::Splice { seat, clips } => {
                if let Some(track) = self.tracks.iter_mut().find(|t| t.seat == seat) {
                    track.splice_clips(clips, &self.stats);
                }
            }
            Msg::NoTracks => {
                self.tracks.clear();
                self.stats.unopened.store(0, Relaxed);
            }
            Msg::StoreChanged => {
                for track in self.tracks.iter_mut() {
                    track.ask_again(&self.stats);
                }
            }
            Msg::Store(store) => {
                self.store = store;
                self.stats.unopened.store(0, Relaxed);
                for track in self.tracks.iter_mut() {
                    track.files.clear();
                }
            }
            Msg::Quit => {}
        }
    }
}

#[cfg(test)]
mod tests;
