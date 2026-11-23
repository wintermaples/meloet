//! The analysis thread (round 12): the master's sound as it is heard, read from the engine's
//! ring ([`AnalysisReader`]), for the telemetry's 24 bands (E7) and loudness (M3). Only for
//! listening: an export has no such thread and its engine writes no ring.
//!
//! The thread is the core's, at the usual priority. It wakes every [`ANALYSIS_TICK`], takes
//! what is in the ring and writes what it finds into [`Readings`], which the telemetry thread
//! reads without waiting. A reader of a new engine ([`Analysis::set_reader`]), a new rate or
//! frames the ring had no room for start the analysis afresh.
//!
//! The bands ([`spectrum`]) are worked only while 「音の動きを見せる」 is on
//! ([`Analysis::set_spectrum`]); off, they read 0.

use std::sync::Arc;
use std::sync::atomic::{AtomicI16, AtomicU8, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use meloet_engine::AnalysisReader;
use meloet_protocol::{SPECTRUM_BANDS, TELEMETRY_NO_VALUE};

use spectrum::Spectrum;

pub mod spectrum;

/// How often the thread takes what is in the ring: a tenth of what the ring holds at 96 kHz.
pub const ANALYSIS_TICK: Duration = Duration::from_millis(10);

/// Wakes in a row with no frames after which the stream is taken as stopped and the bars fall
/// by the time: longer than a block of the largest quantum the PC may force (8192 at 44.1 kHz).
const STALLED_WAKES: u32 = 20;

/// What the analysis found last, in the telemetry's units. Only the analysis thread writes.
pub struct Readings {
    spectrum: [AtomicU8; SPECTRUM_BANDS],
    lufs_short: AtomicI16,
    lufs_integrated: AtomicI16,
    true_peak: AtomicI16,
    /// Frames taken from the ring so far.
    frames: AtomicU64,
}

impl Default for Readings {
    fn default() -> Readings {
        Readings {
            spectrum: std::array::from_fn(|_| AtomicU8::new(0)),
            lufs_short: AtomicI16::new(TELEMETRY_NO_VALUE),
            lufs_integrated: AtomicI16::new(TELEMETRY_NO_VALUE),
            true_peak: AtomicI16::new(TELEMETRY_NO_VALUE),
            frames: AtomicU64::new(0),
        }
    }
}

impl Readings {
    /// The 24 bands, low to high, each −72..0 dB as 0..255 (E7).
    pub fn spectrum(&self) -> [u8; SPECTRUM_BANDS] {
        std::array::from_fn(|band| self.spectrum[band].load(Relaxed))
    }

    /// Short-term loudness, 0.01 LU; `TELEMETRY_NO_VALUE`: none (M3).
    pub fn lufs_short(&self) -> i16 {
        self.lufs_short.load(Relaxed)
    }

    /// Integrated loudness since the play started, 0.01 LU (M3).
    pub fn lufs_integrated(&self) -> i16 {
        self.lufs_integrated.load(Relaxed)
    }

    /// True peak since the play started, 0.01 dBTP (M3).
    pub fn true_peak(&self) -> i16 {
        self.true_peak.load(Relaxed)
    }

    pub fn frames(&self) -> u64 {
        self.frames.load(Relaxed)
    }
}

enum Msg {
    Reader(AnalysisReader),
    Spectrum(bool),
    Quit,
}

/// The thread, from its start to its end (with the owner: dropping this stops it).
pub(crate) struct Analysis {
    tx: Sender<Msg>,
    thread: Option<JoinHandle<()>>,
    readings: Arc<Readings>,
}

impl Analysis {
    pub fn spawn(reader: Option<AnalysisReader>) -> Analysis {
        let (tx, rx) = mpsc::channel();
        let readings = Arc::new(Readings::default());
        let mut side = Side::new(readings.clone());
        if let Some(reader) = reader {
            side.set_reader(reader);
        }
        let thread = std::thread::Builder::new()
            .name("meloet-analysis".into())
            .spawn(move || side.run(&rx))
            .inspect_err(|e| eprintln!("meloet: no thread for the analysis of the sound: {e}"))
            .ok();
        Analysis {
            tx,
            thread,
            readings,
        }
    }

    /// The reader of an engine made anew: the one before is dropped.
    pub fn set_reader(&self, reader: AnalysisReader) {
        let _ = self.tx.send(Msg::Reader(reader));
    }

    /// 「音の動きを見せる」 (E7): off at the start.
    pub fn set_spectrum(&self, on: bool) {
        let _ = self.tx.send(Msg::Spectrum(on));
    }

    pub fn readings(&self) -> &Arc<Readings> {
        &self.readings
    }
}

impl Drop for Analysis {
    fn drop(&mut self) {
        let _ = self.tx.send(Msg::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The work of the thread, apart from its waking.
struct Side {
    reader: Option<AnalysisReader>,
    /// `AnalysisReader::dropped` and the rate as last seen.
    dropped: u64,
    sample_rate: u32,
    readings: Arc<Readings>,
    spectrum: Spectrum,
    spectrum_on: bool,
    /// Wakes in a row that found no frames.
    idle: u32,
}

impl Side {
    fn new(readings: Arc<Readings>) -> Side {
        Side {
            reader: None,
            dropped: 0,
            sample_rate: 0,
            readings,
            spectrum: Spectrum::new(0),
            spectrum_on: false,
            idle: 0,
        }
    }

    fn run(mut self, rx: &Receiver<Msg>) {
        loop {
            match rx.recv_timeout(ANALYSIS_TICK) {
                Ok(Msg::Reader(reader)) => self.set_reader(reader),
                Ok(Msg::Spectrum(on)) => self.set_spectrum(on),
                Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.pump();
        }
    }

    fn set_reader(&mut self, reader: AnalysisReader) {
        self.dropped = reader.dropped();
        self.sample_rate = reader.sample_rate();
        self.reader = Some(reader);
        self.afresh();
    }

    fn set_spectrum(&mut self, on: bool) {
        if on != self.spectrum_on {
            self.spectrum_on = on;
            self.spectrum.restart(self.sample_rate);
            self.write_spectrum();
        }
    }

    /// What was heard before does not join what comes next.
    fn afresh(&mut self) {
        self.spectrum.restart(self.sample_rate);
        self.write_spectrum();
        // round12 stub: M3 (the loudness meters start anew)
    }

    /// Takes what is in the ring. Returns how many frames.
    fn pump(&mut self) -> usize {
        let mut dropped_frames = 0;
        if let Some(reader) = self.reader.as_mut() {
            let (dropped, sample_rate) = (reader.dropped(), reader.sample_rate());
            if (dropped, sample_rate) != (self.dropped, self.sample_rate) {
                // What the ring holds came before the gap (or at the rate before).
                dropped_frames = reader.read(|_| {});
                (self.dropped, self.sample_rate) = (dropped, sample_rate);
                self.afresh();
            }
        }
        let spectrum_on = self.spectrum_on;
        let Side {
            reader, spectrum, ..
        } = self;
        let frames = reader.as_mut().map_or(0, |reader| {
            reader.read(|frames| {
                if spectrum_on {
                    spectrum.push(frames);
                }
                // round12 stub: M3 (ebur128)
            })
        }) + dropped_frames;
        self.readings.frames.fetch_add(frames as u64, Relaxed);
        self.idle = if frames == 0 {
            self.idle.saturating_add(1)
        } else {
            0
        };
        if spectrum_on {
            if self.idle > STALLED_WAKES {
                self.spectrum.fall_for(ANALYSIS_TICK);
            }
            self.write_spectrum();
        }
        frames
    }

    fn write_spectrum(&self) {
        let bars = self.spectrum.bars();
        for (band, bar) in self.readings.spectrum.iter().zip(bars) {
            band.store(bar, Relaxed);
        }
    }
}

#[cfg(test)]
mod tests;
