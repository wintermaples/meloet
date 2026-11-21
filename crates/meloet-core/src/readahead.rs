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

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
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
}

impl ReadAhead {
    pub fn spawn(store: Arc<dyn PcmStore>) -> ReadAhead {
        let (tx, rx) = mpsc::channel();
        let stats = Arc::new(ReadStats::default());
        let side = Side {
            rx,
            store,
            wants: None,
            map: None,
            tracks: Vec::new(),
            buf: vec![0.0; CHUNK * STREAM_CHANNELS],
            raw: vec![0.0; CHUNK * STREAM_CHANNELS],
            stats: stats.clone(),
        };
        let thread = std::thread::Builder::new()
            .name("meloet-readahead".into())
            .spawn(move || side.run())
            .ok();
        if thread.is_none() {
            eprintln!("meloet: the read-ahead thread did not start; vocal clips are silent");
        }
        ReadAhead { tx, thread, stats }
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

/// The file of an audio's uid as the store gave it.
type Opened = ([u8; 16], Option<Box<dyn PcmFile>>);

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
        self.ask_again(stats);
    }

    /// A file the store did not have is asked for again, and one no clip uses is let go.
    fn ask_again(&mut self, stats: &ReadStats) {
        let clips = &self.clips;
        (self.files).retain(|(uid, file)| file.is_some() && clips.iter().any(|c| c.uid == *uid));
        stats.unopened.fetch_and(!seat_bit(self.seat), Relaxed);
    }

    fn file(
        &mut self,
        uid: [u8; 16],
        store: &dyn PcmStore,
        stats: &ReadStats,
    ) -> Option<&mut Box<dyn PcmFile>> {
        let at = match self.files.iter().position(|(u, _)| *u == uid) {
            Some(at) => at,
            None => {
                let file = store.open(&uid, self.sample_rate);
                if file.is_none() {
                    stats.unopened.fetch_or(seat_bit(self.seat), Relaxed);
                }
                self.files.push((uid, file));
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
    fn fill(&mut self, store: &dyn PcmStore, buf: &mut [f32], raw: &mut [f32], stats: &ReadStats) {
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
        let out = &mut buf[..frames * STREAM_CHANNELS];
        out.fill(0.0);
        let (from, to) = (song, song + frames as u64);
        let first = self.clips.partition_point(|c| c.end <= from);
        for index in first..self.clips.len() {
            let clip = self.clips[index];
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
            let Some(file) = self.file(clip.uid, store, stats) else {
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
        let chunk = StreamChunk {
            generation: want.generation,
            first_frame: self.next,
            frames: frames as u32,
        };
        if self.writer.push(chunk, out) {
            self.next += frames as u64;
            stats.frames.fetch_add(frames as u64, Relaxed);
        }
    }
}

struct Side {
    rx: Receiver<Msg>,
    store: Arc<dyn PcmStore>,
    wants: Option<StreamWants>,
    map: Option<(Arc<TempoMap>, u32)>,
    tracks: Vec<Fed>,
    /// A chunk as it goes into a ring, and as it comes out of a file.
    buf: Vec<f32>,
    raw: Vec<f32>,
    stats: Arc<ReadStats>,
}

impl Side {
    fn run(mut self) {
        loop {
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
                    Ok(Msg::Quit) | Err(mpsc::TryRecvError::Disconnected) => return,
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
                    track.fill(&*self.store, &mut self.buf, &mut self.raw, &self.stats);
                    busy = true;
                }
            }
            if busy {
                continue;
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
