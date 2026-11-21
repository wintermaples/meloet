//! The file an export writes, whatever its format (docs/architecture.md 22-11, 22-20).
//!
//! The export's thread hands over the finished audio, already at the file's sample rate, as
//! floats; an [`AudioSink`] makes the file's samples of them. Both formats go through the
//! same step from floats to whole numbers (with dither for 16 bits, once), so a wav and a
//! flac file of the same export hold the same samples. A sink writes where it is told to,
//! which is a file of the work place: moving the finished file to where the person wants it
//! is the caller's.

use std::io;
use std::path::Path;

use meloet_protocol::{ExportFormat, RecordFormat};

/// What the file is to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinkSpec {
    pub format: ExportFormat,
    pub sample_rate: u32,
    /// 1 or 2.
    pub channels: u8,
    /// One of `format.bits_choices()`. 32 bits are floats.
    pub bits: RecordFormat,
}

/// A file being written. Not for the audio thread: every call may wait for the disk.
pub trait AudioSink: Send {
    /// Appends frames, interleaved, as many samples as a multiple of the channels.
    fn write(&mut self, interleaved: &[f32]) -> io::Result<()>;

    /// Ends the file so that it reads as a whole one. Returns the frames it holds.
    fn finish(self: Box<Self>) -> io::Result<u64>;

    /// Takes the half-written file away.
    fn abandon(self: Box<Self>);
}

/// Starts the file at `path`, over whatever is there.
///
/// Not written yet: wav is to be written as the recorder writes it (`crate::vocalrec`),
/// flac with `flacenc`.
pub fn create(path: &Path, spec: SinkSpec) -> io::Result<Box<dyn AudioSink>> {
    let _ = (path, spec);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the export's files are not written yet",
    ))
}
