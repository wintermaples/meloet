//! Recording the input to disk (docs/architecture.md 22-6, 22-7).
//!
//! ```text
//! input callback ─ record ring (4 s) ─▶ the disk writer (this thread) ─▶ work/音/<name>.wav
//! ```
//!
//! The disk writer is a thread of ordinary priority. It owns the reading end of the record
//! ring for as long as the input is open, makes the files of a recording before the engine
//! is told to record, and writes what the ring brings as it comes. A block the ring had no
//! room for is written as silence, so every later frame stays where it belongs.
//!
//! A file can be read at any moment: its head goes first, with the lengths that say "to the
//! end of the file", and what was written is on the disk within a second. The lengths are
//! put right when the recording stops, or by [`mend_wav`] at the next start.
//!
//! Where a file's frames belong in the song is counted, not read from the time block by
//! block: the writer is asked which frame of the file was sung at a moment only where the
//! song rolls and where a lap begins ([`DiskWriter::mark`], from the times of the blocks
//! around it), and everything between is frames ([`takes_from`]). So that counting holds,
//! a cycle the input missed is written as silence of its length.
//!
//! The peaks of each file are made as it is written ([`SharedPeaks`], 22-9): the screen
//! draws the wave of a recording from them while it goes on, and they become the file's
//! peaks file when the recording is in the song ([`DiskWriter::write_peaks`]).

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use meloet_engine::{FrameClock, InputBlock, InputMeters, InputRingReader};
use meloet_model::{NewAudioTake, TempoMap};
use meloet_protocol::RecordFormat;
use serde::{Deserialize, Serialize};

use crate::peaks::{PeakBuilder, SharedPeaks};

/// Under the state directory: where recorded audio is written first.
pub const WORK_AUDIO: &str = "work/音";
/// Under the state directory: the note about the recording going on.
pub const REC_NOTE: &str = "work/rec.json";
const REC_NOTE_VERSION: u32 = 1;

const WAV_HEAD: u64 = 44;
/// "The length is not known": a reader takes the data to the end of the file.
const UNKNOWN: u32 = 0xFFFF_FFFF;
/// A wav file's lengths are 32 bits. Recording stops a little before.
const WAV_MAX_DATA: u64 = 0xFFFF_FFFF - WAV_HEAD - 4_096;
/// How many of the newest blocks a moment is placed by.
const MARK_BLOCKS: usize = 32;
/// How long a mark waits for the file's first block.
const MARK_WAIT: Duration = Duration::from_millis(300);
const SYNC_EVERY: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(3);
/// The longest stretch kept in memory for the latency measurement.
const CAPTURE_S: usize = 4;
/// How many late blocks wait for the blocks after them to say whether frames are missing.
const HELD_BLOCKS: usize = 2;

pub fn work_audio_dir(state_dir: &Path) -> PathBuf {
    state_dir.join(WORK_AUDIO)
}

/// `<track name> <audio id>.wav`, with what a file name cannot hold taken out.
pub fn file_name(track_name: &str, id: u32) -> String {
    let clean: String = track_name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '\0') && !c.is_control())
        .collect();
    let clean = clean.trim().trim_matches('.');
    let name = if clean.is_empty() { "歌" } else { clean };
    format!("{name} {id}.wav")
}

// ---- the wav file ----

fn head(sample_rate: u32, channels: u16, format: RecordFormat, data: u32) -> [u8; 44] {
    let bytes = u32::from(format.bits() / 8);
    let block = bytes * u32::from(channels);
    // 1: integer PCM, 3: IEEE float.
    let tag: u16 = if format == RecordFormat::Bits32 { 3 } else { 1 };
    let riff = if data == UNKNOWN {
        UNKNOWN
    } else {
        data + WAV_HEAD as u32 - 8
    };
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&riff.to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&tag.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&sample_rate.to_le_bytes());
    h[28..32].copy_from_slice(&(sample_rate * block).to_le_bytes());
    h[32..34].copy_from_slice(&(block as u16).to_le_bytes());
    h[34..36].copy_from_slice(&u16::from(format.bits()).to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

/// A wav file being written.
pub struct WavFile {
    file: File,
    format: RecordFormat,
    channels: u16,
    sample_rate: u32,
    frames: u64,
    /// For the dither of 16 bits.
    seed: u32,
    bytes: Vec<u8>,
}

impl WavFile {
    /// Makes the file with its head. It must not be there yet.
    pub fn create(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        format: RecordFormat,
    ) -> io::Result<WavFile> {
        let mut file = File::options().write(true).create_new(true).open(path)?;
        file.write_all(&head(sample_rate, channels, format, UNKNOWN))?;
        file.sync_all()?;
        Ok(WavFile {
            file,
            format,
            channels: channels.max(1),
            sample_rate,
            frames: 0,
            seed: 0x9E37_79B9,
            bytes: Vec::new(),
        })
    }

    fn frame_bytes(&self) -> u64 {
        u64::from(self.format.bits() / 8) * u64::from(self.channels)
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// How many more frames the file takes.
    pub fn room(&self) -> u64 {
        (WAV_MAX_DATA / self.frame_bytes()).saturating_sub(self.frames)
    }

    fn noise(&mut self) -> f32 {
        // xorshift32; 24 bits of it make a number in [0, 1).
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// `samples`: interleaved, a whole number of frames of this file's channels.
    pub fn write(&mut self, samples: &[f32]) -> io::Result<()> {
        let mut bytes = std::mem::take(&mut self.bytes);
        bytes.clear();
        for &s in samples {
            let s = if s.is_finite() { s } else { 0.0 };
            match self.format {
                RecordFormat::Bits32 => bytes.extend_from_slice(&s.to_le_bytes()),
                RecordFormat::Bits24 => {
                    let v = (f64::from(s.clamp(-1.0, 1.0)) * 8_388_607.0).round() as i32;
                    bytes.extend_from_slice(&v.to_le_bytes()[..3]);
                }
                RecordFormat::Bits16 => {
                    // Triangular dither of one step each way, before rounding.
                    let dither = self.noise() - self.noise();
                    let v = (s.clamp(-1.0, 1.0) * 32_767.0 + dither).round();
                    let v = v.clamp(-32_768.0, 32_767.0) as i16;
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        let result = self.file.write_all(&bytes);
        self.bytes = bytes;
        result?;
        self.frames += (samples.len() / usize::from(self.channels)) as u64;
        Ok(())
    }

    pub fn write_silence(&mut self, frames: u64) -> io::Result<()> {
        let zeros = [0u8; 4_096];
        let mut left = frames * self.frame_bytes();
        while left > 0 {
            let n = left.min(zeros.len() as u64) as usize;
            self.file.write_all(&zeros[..n])?;
            left -= n as u64;
        }
        self.frames += frames;
        Ok(())
    }

    /// What was written is on the disk when this comes back.
    pub fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }

    /// Puts the true lengths into the head. Returns the frames of the file.
    pub fn finish(mut self) -> io::Result<u64> {
        let data = (self.frames * self.frame_bytes()) as u32;
        self.file.seek(SeekFrom::Start(0))?;
        self.file
            .write_all(&head(self.sample_rate, self.channels, self.format, data))?;
        self.file.sync_all()?;
        Ok(self.frames)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    pub frames: u64,
    pub sample_rate: u32,
    pub channels: u8,
}

/// A file a recording left when meloet ended: cuts a frame that is not whole off its end and
/// puts the true lengths into its head.
pub fn mend_wav(path: &Path) -> io::Result<WavInfo> {
    let mut file = File::options().read(true).write(true).open(path)?;
    let mut h = [0u8; 44];
    file.read_exact(&mut h)?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "not a recording of meloet");
    if &h[0..4] != b"RIFF" || &h[8..16] != b"WAVEfmt " || &h[36..40] != b"data" {
        return Err(bad());
    }
    let channels = u16::from_le_bytes([h[22], h[23]]);
    let sample_rate = u32::from_le_bytes([h[24], h[25], h[26], h[27]]);
    let bits = u16::from_le_bytes([h[34], h[35]]);
    let format = (RecordFormat::ALL.into_iter())
        .find(|f| u16::from(f.bits()) == bits)
        .ok_or_else(bad)?;
    if !(1..=2).contains(&channels) || sample_rate == 0 {
        return Err(bad());
    }
    let frame = u64::from(bits / 8) * u64::from(channels);
    let frames =
        (file.metadata()?.len().saturating_sub(WAV_HEAD) / frame).min(WAV_MAX_DATA / frame);
    file.set_len(WAV_HEAD + frames * frame)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&head(
        sample_rate,
        channels,
        format,
        (frames * frame) as u32,
    ))?;
    file.sync_all()?;
    Ok(WavInfo {
        frames,
        sample_rate,
        channels: channels as u8,
    })
}

// ---- the disk writer ----

/// One file of a recording: these channels of the input, for one vocal track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFile {
    pub path: PathBuf,
    /// One channel of the input (mono) or two (stereo).
    pub channels: Vec<u8>,
}

pub struct RecordJob {
    pub files: Vec<RecordFile>,
    pub format: RecordFormat,
}

/// What a recording wrote.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Written {
    /// Of every file: they are all as long.
    pub frames: u64,
    pub sample_rate: u32,
    /// Frames written as silence because the ring ran full.
    pub dropped: u64,
    /// Frames written as silence because the input missed their cycles (an xrun): the
    /// times of the blocks say so.
    pub gaps: u64,
    /// How many blocks `dropped` is, and how many times the input skipped for `gaps`.
    pub dropped_blocks: u32,
    pub gap_count: u32,
    /// The files reached the size a wav file can have.
    pub full: bool,
    pub error: Option<String>,
}

/// What the latency measurement heard.
#[derive(Debug, Clone, Default)]
pub struct Captured {
    pub blocks: Vec<InputBlock>,
    /// Interleaved, `channels` to a frame.
    pub samples: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Captured {
    pub fn first_frame(&self) -> u64 {
        self.blocks.first().map_or(0, |b| b.first_frame)
    }

    pub fn channel(&self, channel: usize) -> Vec<f32> {
        let step = usize::from(self.channels.max(1));
        self.samples
            .iter()
            .skip(channel)
            .step_by(step)
            .copied()
            .collect()
    }
}

enum Msg {
    Input(Option<(InputRingReader, Arc<InputMeters>)>),
    Start(RecordJob, Sender<io::Result<Vec<SharedPeaks>>>),
    WritePeaks(SharedPeaks, PathBuf, Sender<io::Result<()>>),
    From(f64),
    Stop(Sender<Option<Written>>),
    Mark(f64, Sender<Option<f64>>),
    Capture(bool),
    Captured(Sender<Captured>),
    Quit,
}

pub struct DiskWriter {
    tx: Sender<Msg>,
    status: Arc<Mutex<Written>>,
    thread: Option<JoinHandle<()>>,
}

impl DiskWriter {
    pub fn spawn() -> DiskWriter {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Written::default()));
        let shared = status.clone();
        let thread = std::thread::Builder::new()
            .name("meloet-disk".into())
            .spawn(move || Side::new(shared).run(&rx))
            .ok();
        DiskWriter { tx, status, thread }
    }

    /// The input opened (the reading end of its record ring, and its meters) or closed. A
    /// recording going on ends with the input it was fed by.
    pub fn set_input(&self, input: Option<(InputRingReader, Arc<InputMeters>)>) {
        let _ = self.tx.send(Msg::Input(input));
    }

    /// Makes the files and starts keeping what the input brings. Comes back when the files
    /// are there, with the peaks of each, which grow as it is written.
    pub fn start(&self, job: RecordJob) -> io::Result<Vec<SharedPeaks>> {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::Start(job, reply));
        answer
            .recv()
            .unwrap_or_else(|_| Err(io::Error::other("the disk writer is gone")))
    }

    /// Writes peaks as they are now into a peaks file, on the writer's thread. The answer
    /// comes when the file is there.
    pub fn write_peaks(&self, peaks: SharedPeaks, path: PathBuf) -> Receiver<io::Result<()>> {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::WritePeaks(peaks, path, reply));
        answer
    }

    /// The files begin with the first block captured at or after `t_us`; what came before
    /// is let go of.
    pub fn from(&self, t_us: f64) {
        let _ = self.tx.send(Msg::From(t_us));
    }

    /// Writes what the ring still holds, puts the lengths into the heads, and says what was
    /// written. `None`: nothing was being recorded.
    pub fn stop(&self) -> Option<Written> {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::Stop(reply));
        answer.recv().ok().flatten()
    }

    /// Which frame of the files was captured at `t_us`, with the fraction; before the
    /// files' first frame it is below 0. Waits for the files' first block; `None` when none
    /// comes, and when nothing is recorded.
    pub fn mark(&self, t_us: f64) -> Option<f64> {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::Mark(t_us, reply));
        answer.recv().ok().flatten()
    }

    /// How the recording going on is doing.
    pub fn status(&self) -> Written {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Starts keeping what the input brings in memory (for the latency measurement), from
    /// nothing, or stops.
    pub fn capture(&self, on: bool) {
        let _ = self.tx.send(Msg::Capture(on));
    }

    /// What was kept since `capture(true)`.
    pub fn captured(&self) -> Captured {
        let (reply, answer) = mpsc::channel();
        let _ = self.tx.send(Msg::Captured(reply));
        answer.recv().unwrap_or_default()
    }
}

impl Drop for DiskWriter {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct OpenFile {
    wav: WavFile,
    channels: Vec<u8>,
    scratch: Vec<f32>,
    peaks: SharedPeaks,
}

impl OpenFile {
    /// What was just written, for the wave: `silent` frames of nothing, then `scratch`.
    fn note_peaks(&self, silent: u64, with_scratch: bool) {
        let mut peaks = self.peaks.lock().unwrap_or_else(|e| e.into_inner());
        let zeros = [0.0f32; 512];
        let mut left = silent as usize * usize::from(peaks.channels());
        while left > 0 {
            let n = left.min(zeros.len());
            peaks.push(&zeros[..n]);
            left -= n;
        }
        if with_scratch {
            peaks.push(&self.scratch);
        }
    }
}

struct Job {
    files: Vec<OpenFile>,
    /// `None` until the core says where the files begin: blocks wait in `pending`.
    from_us: Option<f64>,
    pending: Vec<(InputBlock, Vec<f32>)>,
    /// When the files' newest blocks were captured, by the files' frames.
    times: FrameClock,
    gaps: Gaps,
    /// Marks asked for before the files had a block.
    marks: Vec<(f64, Sender<Option<f64>>, Instant)>,
    synced: Instant,
    said: Written,
    stopped: bool,
}

struct Side {
    input: Option<(InputRingReader, Arc<InputMeters>)>,
    into: Vec<f32>,
    job: Option<Job>,
    capture: Option<Captured>,
    status: Arc<Mutex<Written>>,
}

impl Side {
    fn new(status: Arc<Mutex<Written>>) -> Side {
        Side {
            input: None,
            into: Vec::new(),
            job: None,
            capture: None,
            status,
        }
    }

    fn run(mut self, rx: &Receiver<Msg>) {
        loop {
            let busy = self.job.as_ref().is_some_and(|j| !j.stopped) || self.capture.is_some();
            if let Some(job) = &mut self.job {
                job.answer_marks();
            }
            let msg = if busy {
                match rx.recv_timeout(POLL) {
                    Ok(msg) => Some(msg),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => break,
                }
            };
            match msg {
                Some(Msg::Input(input)) => {
                    self.drain();
                    self.end_job();
                    self.switch(false);
                    self.capture = None;
                    self.input = input;
                    let size = self.input.as_ref().map_or(0, |(r, _)| {
                        usize::from(r.channels()) * r.sample_rate() as usize
                    });
                    self.into = vec![0.0; size];
                }
                Some(Msg::Start(job, reply)) => {
                    let _ = reply.send(self.start(job));
                }
                Some(Msg::WritePeaks(peaks, path, reply)) => {
                    // A copy: the screen may read the peaks while the file is written.
                    let copy = peaks.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    let _ = reply.send(copy.write(&path));
                }
                Some(Msg::From(t_us)) => {
                    if let Some(job) = &mut self.job {
                        job.from_us = Some(t_us);
                    }
                    self.drain();
                }
                Some(Msg::Mark(t_us, reply)) => {
                    self.drain();
                    match self.job.as_mut().filter(|j| !j.stopped) {
                        Some(job) => {
                            job.marks.push((t_us, reply, Instant::now()));
                            job.answer_marks();
                        }
                        None => {
                            let _ = reply.send(None);
                        }
                    }
                }
                Some(Msg::Stop(reply)) => {
                    self.drain();
                    self.switch(self.capture.is_some());
                    self.drain();
                    self.end_job();
                    let _ = reply.send(self.job.take().map(|j| j.said));
                    self.say();
                }
                Some(Msg::Capture(on)) => {
                    self.drain();
                    self.capture = match (on, &self.input) {
                        (true, Some((reader, _))) => Some(Captured {
                            channels: reader.channels(),
                            sample_rate: reader.sample_rate(),
                            ..Captured::default()
                        }),
                        _ => None,
                    };
                    self.switch(on || self.job.as_ref().is_some_and(|j| !j.stopped));
                }
                Some(Msg::Captured(reply)) => {
                    self.drain();
                    let _ = reply.send(self.capture.clone().unwrap_or_default());
                }
                Some(Msg::Quit) => break,
                None => self.drain(),
            }
        }
        self.drain();
        self.end_job();
        self.switch(false);
    }

    fn switch(&self, on: bool) {
        if let Some((_, meters)) = &self.input {
            meters.set_recording(on);
        }
    }

    fn say(&self) {
        let said = self
            .job
            .as_ref()
            .map(|j| j.said.clone())
            .unwrap_or_default();
        *self.status.lock().unwrap_or_else(|e| e.into_inner()) = said;
    }

    fn start(&mut self, job: RecordJob) -> io::Result<Vec<SharedPeaks>> {
        self.end_job();
        let Some((reader, _)) = &mut self.input else {
            return Err(io::Error::other("no input is open"));
        };
        // What the ring still holds is from before this recording.
        while reader.pop(&mut self.into).is_some() {}
        let sample_rate = reader.sample_rate();
        let mut files = Vec::new();
        for file in &job.files {
            if let Some(dir) = file.path.parent() {
                fs::create_dir_all(dir)?;
            }
            let channels = file.channels.len().clamp(1, 2) as u16;
            files.push(OpenFile {
                wav: WavFile::create(&file.path, sample_rate, channels, job.format)?,
                channels: file.channels.clone(),
                scratch: Vec::new(),
                peaks: Arc::new(Mutex::new(PeakBuilder::new(channels as u8))),
            });
        }
        let peaks = files.iter().map(|f| f.peaks.clone()).collect();
        if let Some(dir) = job.files.first().and_then(|f| f.path.parent()) {
            File::open(dir)?.sync_all()?;
        }
        self.job = Some(Job {
            files,
            from_us: None,
            pending: Vec::new(),
            times: FrameClock::new(sample_rate, MARK_BLOCKS),
            gaps: Gaps::default(),
            marks: Vec::new(),
            synced: Instant::now(),
            said: Written {
                sample_rate,
                ..Written::default()
            },
            stopped: false,
        });
        self.say();
        self.switch(true);
        Ok(peaks)
    }

    /// Takes everything out of the ring that is there now.
    fn drain(&mut self) {
        let Some((reader, _)) = &mut self.input else {
            return;
        };
        let (channels, rate) = (usize::from(reader.channels()), reader.sample_rate());
        let mut took = false;
        // What waited for the core to say where the files begin goes first.
        if let Some(job) = self
            .job
            .as_mut()
            .filter(|j| !j.stopped && j.from_us.is_some())
        {
            for (block, samples) in std::mem::take(&mut job.pending) {
                job.take(block, &samples, channels, rate);
                took = true;
            }
        }
        while let Some(block) = reader.pop(&mut self.into) {
            took = true;
            let samples = &self.into[..block.frames as usize * channels];
            if let Some(kept) = &mut self.capture
                && kept.samples.len() < CAPTURE_S * rate as usize * channels
            {
                kept.blocks.push(block);
                if block.dropped {
                    kept.samples.resize(kept.samples.len() + samples.len(), 0.0);
                } else {
                    kept.samples.extend_from_slice(samples);
                }
            }
            if let Some(job) = self.job.as_mut().filter(|j| !j.stopped) {
                if job.from_us.is_none() {
                    job.pending.push((block, samples.to_vec()));
                } else {
                    job.take(block, samples, channels, rate);
                }
            }
        }
        let Some(job) = self.job.as_mut().filter(|j| !j.stopped) else {
            return;
        };
        job.answer_marks();
        if job.synced.elapsed() >= SYNC_EVERY {
            job.synced = Instant::now();
            for file in &mut job.files {
                if let Err(e) = file.wav.sync() {
                    job.said.error.get_or_insert(e.to_string());
                }
            }
        }
        if took {
            self.say();
        }
    }

    /// Ends the recording going on: the heads get their lengths. What it wrote stays to be
    /// asked for.
    fn end_job(&mut self) {
        let Some(job) = self.job.as_mut().filter(|j| !j.stopped) else {
            return;
        };
        // Stopped before the core said where the files begin: they begin where the input did.
        if job.from_us.is_none() {
            job.from_us = Some(f64::NEG_INFINITY);
            if let Some((reader, _)) = &self.input {
                let (channels, rate) = (usize::from(reader.channels()), reader.sample_rate());
                for (block, samples) in std::mem::take(&mut job.pending) {
                    job.take(block, &samples, channels, rate);
                }
            }
        }
        if let Some((reader, _)) = &self.input {
            job.settle(usize::from(reader.channels()), reader.sample_rate());
        }
        job.answer_marks();
        for (_, reply, _) in job.marks.drain(..) {
            let _ = reply.send(None);
        }
        job.stopped = true;
        for file in std::mem::take(&mut job.files) {
            if let Err(e) = file.wav.finish() {
                job.said.error.get_or_insert(e.to_string());
            }
        }
        self.say();
    }
}

/// Tells the frames the input never got from a block whose time alone is late.
///
/// A block's time is the time of the cycle it came in. When the input misses cycles, every
/// block after is later than the count of frames says, by the frames that never came. When
/// only a block's time is late (a busy PC, small buffers: measured at 64 frames,
/// docs/measurements/2026-11-21-round7-step3.md 6), the blocks after it are back where the
/// count says. So a late block waits for the next ones: frames are missing before it only
/// if they are late too.
///
/// Both come together as well (measured under a heavier load, three places in 80 s): a
/// cycle is missed, and the next cycle hands over two blocks with one time, the first a
/// cycle late by it. A time is late or right, never early: the hole is as long as the
/// least any of the blocks that wait is late.
#[derive(Default)]
struct Gaps {
    /// When the frame after the last block let through is due.
    due_us: Option<f64>,
    held: Vec<(InputBlock, Vec<f32>)>,
}

impl Gaps {
    /// `write(missed, block, samples)` for every block that is settled now, in the order
    /// they came: `missed` frames never came before it.
    fn take(
        &mut self,
        block: InputBlock,
        samples: &[f32],
        rate: u32,
        write: &mut impl FnMut(u64, InputBlock, &[f32]),
    ) {
        let frame_us = 1e6 / f64::from(rate.max(1));
        let len_us = |b: &InputBlock| f64::from(b.frames) * frame_us;
        let at_us = block.first_frame_us(rate);
        loop {
            // Where the block is due if nothing is missing before those that wait.
            let line_us = (self.due_us)
                .map(|due| due + self.held.iter().map(|(b, _)| len_us(b)).sum::<f64>());
            let late_us = at_us - line_us.unwrap_or(at_us);
            // Half a block of room: the time of a block is the time its cycle woke, which
            // is not exact.
            if late_us <= len_us(&block) / 2.0 {
                for (held, samples) in self.held.drain(..) {
                    write(0, held, &samples);
                }
                self.due_us = Some(at_us + len_us(&block));
                write(0, block, samples);
                return;
            }
            if self.held.len() < HELD_BLOCKS {
                self.held.push((block, samples.to_vec()));
                return;
            }
            self.settle_first(late_us, rate, write);
        }
    }

    /// The first block that waits comes after a hole; the others are looked at again from
    /// where it ends. `next_late_us`: how late the block after those that wait is.
    fn settle_first(
        &mut self,
        next_late_us: f64,
        rate: u32,
        write: &mut impl FnMut(u64, InputBlock, &[f32]),
    ) {
        if self.held.is_empty() {
            return;
        }
        let frame_us = 1e6 / f64::from(rate.max(1));
        let mut rest = std::mem::take(&mut self.held);
        let mut line_us = self
            .due_us
            .unwrap_or_else(|| rest[0].0.first_frame_us(rate));
        let due_us = line_us;
        let mut late_us = next_late_us;
        for (block, _) in &rest {
            late_us = late_us.min(block.first_frame_us(rate) - line_us);
            line_us += f64::from(block.frames) * frame_us;
        }
        let (first, samples) = rest.remove(0);
        self.due_us = Some(due_us + late_us + f64::from(first.frames) * frame_us);
        write(
            (late_us / frame_us).round().max(0.0) as u64,
            first,
            &samples,
        );
        for (block, samples) in rest {
            self.take(block, &samples, rate, write);
        }
    }

    /// No block comes after: what waits is taken as it says.
    fn settle(&mut self, rate: u32, write: &mut impl FnMut(u64, InputBlock, &[f32])) {
        while !self.held.is_empty() {
            self.settle_first(f64::INFINITY, rate, write);
        }
    }
}

impl Job {
    fn take(&mut self, block: InputBlock, samples: &[f32], channels: usize, rate: u32) {
        if block.frames == 0 {
            return;
        }
        let ends_us =
            block.first_frame_us(rate) + f64::from(block.frames) * 1e6 / f64::from(rate.max(1));
        if self.gaps.due_us.is_none() && ends_us <= self.from_us.unwrap_or(f64::NEG_INFINITY) {
            return;
        }
        let mut gaps = std::mem::take(&mut self.gaps);
        gaps.take(block, samples, rate, &mut |missed, block, samples| {
            self.write(missed, block, samples, channels, rate)
        });
        self.gaps = gaps;
    }

    fn settle(&mut self, channels: usize, rate: u32) {
        let mut gaps = std::mem::take(&mut self.gaps);
        gaps.settle(rate, &mut |missed, block, samples| {
            self.write(missed, block, samples, channels, rate)
        });
        self.gaps = gaps;
    }

    /// Writes a block after `missed` frames that never came (the input missed cycles, or
    /// the ring had no room even for a header): those are silence.
    fn write(
        &mut self,
        missed: u64,
        block: InputBlock,
        samples: &[f32],
        channels: usize,
        rate: u32,
    ) {
        let at_us = block.first_frame_us(rate);
        self.said.gaps += missed;
        self.said.gap_count += u32::from(missed > 0);
        let lost = if block.dropped {
            u64::from(block.frames)
        } else {
            0
        };
        self.said.dropped += lost;
        self.said.dropped_blocks += u32::from(block.dropped);
        let frames = u64::from(block.frames);
        for (i, file) in self.files.iter_mut().enumerate() {
            if file.wav.room() < missed + frames {
                self.said.full = true;
                continue;
            }
            let wrote: io::Result<()> = (|| {
                file.wav.write_silence(missed + lost)?;
                if i == 0 && !block.dropped {
                    self.times.note(file.wav.frames(), at_us);
                }
                if block.dropped {
                    file.note_peaks(missed + lost, false);
                    return Ok(());
                }
                file.scratch.clear();
                for frame in samples.chunks_exact(channels.max(1)) {
                    for &ch in file.channels.iter().take(2) {
                        file.scratch
                            .push(frame.get(usize::from(ch)).copied().unwrap_or(0.0));
                    }
                }
                file.wav.write(&file.scratch)?;
                file.note_peaks(missed + lost, true);
                Ok(())
            })();
            if let Err(e) = wrote {
                self.said.error.get_or_insert(e.to_string());
            }
        }
        self.said.frames = self
            .files
            .first()
            .map_or(self.said.frames, |f| f.wav.frames());
    }

    /// Answers the marks that can be answered: once the files have a block, each is where
    /// counting frames from the newest blocks puts its moment.
    fn answer_marks(&mut self) {
        if self.times.is_empty() {
            for (_, reply, _) in self
                .marks
                .extract_if(.., |(_, _, asked)| asked.elapsed() > MARK_WAIT)
            {
                let _ = reply.send(None);
            }
            return;
        }
        for (t_us, reply, _) in self.marks.drain(..) {
            let _ = reply.send(self.times.frame_at(t_us));
        }
    }
}

// ---- where the frames belong in the song ----

/// What a recording knows of where its file belongs: enough to make its takes, at the stop
/// or at the next start.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marks {
    /// Where the recording starts in the song.
    pub from: u64,
    /// The loop it goes round, `[start, end)`.
    pub loop_range: Option<(u64, u64)>,
    /// The song was at tick `.1` when the file's frame `.0` was sung. `None`: not known
    /// (meloet ended before the head was placed); the file's first frame then counts as
    /// sung at `from`.
    pub roll: Option<(f64, f64)>,
    /// The file's frame each further lap begins with, the song at the loop's start.
    pub laps: Vec<f64>,
}

/// The takes of one file, a take for every lap that has frames. `stop`: the tick and the lap
/// the recording was stopped at; `None` when meloet ended while recording, and the file is
/// used to its end.
pub fn takes_from(
    marks: &Marks,
    map: &TempoMap,
    sample_rate: u32,
    file_frames: u64,
    stop: Option<(f64, u32)>,
) -> Vec<NewAudioTake> {
    let rate = f64::from(sample_rate.max(1));
    let (roll_frame, roll_tick) = marks.roll.unwrap_or((0.0, marks.from as f64));
    let last_lap = match (stop, marks.loop_range) {
        (_, None) => 0,
        (Some((_, lap)), Some(_)) => (lap as usize).min(marks.laps.len()),
        (None, Some(_)) => marks.laps.len(),
    };
    let mut takes = Vec::new();
    for lap in 0..=last_lap {
        let (anchor_frame, anchor_tick, mut start) = match (lap, marks.loop_range) {
            (0, _) | (_, None) => (roll_frame, roll_tick, marks.from),
            (n, Some((loop_start, _))) => (marks.laps[n - 1], loop_start as f64, loop_start),
        };
        let anchor_sec = map.seconds_at(anchor_tick);
        let frame_of = |tick: f64| anchor_frame + (map.seconds_at(tick) - anchor_sec) * rate;
        let tick_of = |frame: f64| map.tick_at(anchor_sec + (frame - anchor_frame) / rate);
        // A file that begins after the take's start: the take begins where the file does.
        if frame_of(start as f64) < -0.5 {
            start = tick_of(0.0).ceil().max(0.0) as u64;
        }
        let first_frame = frame_of(start as f64).round().max(0.0) as u64;
        let loop_end = marks.loop_range.map(|(_, end)| end);
        let wanted_end = match (stop, loop_end) {
            (_, Some(end)) if lap < last_lap => end,
            (Some((tick, _)), end) => (tick.ceil().max(0.0) as u64).min(end.unwrap_or(u64::MAX)),
            (None, end) => end.unwrap_or(u64::MAX),
        };
        let file_end = tick_of(file_frames as f64).floor().max(0.0) as u64;
        let end = wanted_end.min(file_end);
        if end <= start || first_frame >= file_frames {
            continue;
        }
        let end_frame = (frame_of(end as f64).round().max(0.0) as u64).min(file_frames);
        if end_frame <= first_frame {
            continue;
        }
        takes.push(NewAudioTake {
            origin: start,
            length: end - start,
            first_frame,
            frames: end_frame - first_frame,
        });
    }
    takes
}

// ---- the note about the recording going on ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteFile {
    /// The file's name in the work folder.
    pub file: String,
    /// The vocal track it was recorded for, and that track's name then.
    pub track: u32,
    pub track_name: String,
    pub channels: u8,
}

/// `work/rec.json`: written when a recording of the input starts, again when more is known
/// of where it belongs, and removed when the recording is in the song.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecNote {
    pub version: u32,
    /// The song's file; `None` for a song never saved.
    pub song: Option<String>,
    pub sample_rate: u32,
    pub bits: u8,
    pub files: Vec<NoteFile>,
    pub marks: Marks,
}

impl RecNote {
    pub fn new(
        song: Option<&Path>,
        sample_rate: u32,
        format: RecordFormat,
        files: Vec<NoteFile>,
        marks: Marks,
    ) -> RecNote {
        RecNote {
            version: REC_NOTE_VERSION,
            song: song.and_then(Path::to_str).map(str::to_owned),
            sample_rate,
            bits: format.bits(),
            files,
            marks,
        }
    }
}

/// Like a song's save: a whole temporary file, synced, renamed into place, the folder synced.
pub fn write_note(state_dir: &Path, note: &RecNote) -> io::Result<()> {
    let path = state_dir.join(REC_NOTE);
    let dir = path.parent().expect("under work/");
    fs::create_dir_all(dir)?;
    let fresh = dir.join(".rec.json.new");
    let result = (|| {
        let mut file = File::create(&fresh)?;
        file.write_all(&serde_json::to_vec(note).map_err(io::Error::other)?)?;
        file.sync_all()?;
        fs::rename(&fresh, &path)?;
        File::open(dir)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&fresh);
    }
    result
}

pub fn remove_note(state_dir: &Path) {
    let path = state_dir.join(REC_NOTE);
    if fs::remove_file(&path).is_ok()
        && let Some(dir) = path.parent()
    {
        let _ = File::open(dir).and_then(|d| d.sync_all());
    }
}

/// The note a recording left, when meloet ended before the recording was in the song. A
/// note that does not read is removed: its files stay in the work folder.
pub fn read_note(state_dir: &Path) -> Option<RecNote> {
    let path = state_dir.join(REC_NOTE);
    let bytes = fs::read(&path).ok()?;
    match serde_json::from_slice::<RecNote>(&bytes) {
        Ok(note) if note.version == REC_NOTE_VERSION => Some(note),
        _ => {
            eprintln!("meloet: {} does not read; it is put away", path.display());
            remove_note(state_dir);
            None
        }
    }
}

#[cfg(test)]
mod tests;
