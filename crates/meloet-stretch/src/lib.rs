//! Time-stretch and pitch-shift for audio clips: a thin, safe wrapper over Signalsmith Stretch
//! (MIT, vendored unchanged in `vendor/`), and the helper that makes one clip follow a tempo map.
//!
//! Not for the audio thread. The maker thread of the core renders a whole clip with [`render`]
//! and keeps the result; the engine plays the kept file.
//!
//! All `unsafe` and all C++ of meloet's stretching live in this crate.

mod ffi;
mod follow;
mod render;

pub use follow::{Follower, Latency, Seek, Step};
pub use render::{RENDER_STEP, RenderError, render};

use std::ptr::NonNull;

/// The vendored version of Signalsmith Stretch (the git tag). Part of the key of made sounds:
/// another version may give other samples.
pub const STRETCH_VERSION: &str = "1.4.0";

const SEED: std::ffi::c_long = 0x6d65_6c6f;

/// Block and interval of the spectral frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preset {
    /// `presetDefault`: block 0.12 s, interval 0.03 s. Where we start for voice: the README of
    /// 1.4.0 names no preset for voice, and this is the one with the denser frames.
    Default,
    /// `presetCheaper`: block 0.1 s, interval 0.04 s. Fewer frames, less work.
    Cheaper,
    /// `configure` with these sizes, in frames.
    Blocks { block: usize, interval: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Channels, sample rate or block sizes outside what we pass on to the library.
    BadConfig,
    /// The library threw (in practice: out of memory).
    Library,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::BadConfig => f.write_str("stretch: bad configuration"),
            Error::Library => f.write_str("stretch: the library failed"),
        }
    }
}

impl std::error::Error for Error {}

/// One Signalsmith Stretch instance, 32-bit float, split computation off (that mode only spreads
/// the work for audio threads and costs one interval of latency).
///
/// Memory, as read in `signalsmith-stretch.h` and `stft.h` of the vendored version:
/// - [`Stretcher::new`] and [`Stretcher::configure`] allocate (every buffer is sized there).
/// - [`Stretcher::process`], [`Stretcher::seek`], [`Stretcher::output_seek`], [`Stretcher::flush`]
///   and [`Stretcher::reset`] only `resize` or `assign` vectors within the capacity `configure`
///   gave them, so they do not allocate. The author does not promise this, and nothing here
///   checks it by machine. On the Rust side these calls do not allocate either.
/// - The setters store numbers.
pub struct Stretcher {
    raw: NonNull<ffi::MeloetStretch>,
    channels: usize,
    // Channel pointers for the call in progress; stale between calls and never read then.
    in_ptrs: Vec<*const f32>,
    out_ptrs: Vec<*mut f32>,
}

// SAFETY: the C++ object owns all of its memory and has no thread affinity (no thread-locals, no
// globals). The pointer vectors are only read by the library during the call that filled them.
unsafe impl Send for Stretcher {}

impl Stretcher {
    pub fn new(channels: usize, sample_rate: f32, preset: Preset) -> Result<Self, Error> {
        // SAFETY: no preconditions; returns null on failure.
        let raw = NonNull::new(unsafe { ffi::meloet_stretch_new(SEED) }).ok_or(Error::Library)?;
        let mut this = Stretcher {
            raw,
            channels: 0,
            in_ptrs: Vec::new(),
            out_ptrs: Vec::new(),
        };
        this.configure(channels, sample_rate, preset)?;
        Ok(this)
    }

    /// Sets the sizes again and clears all state. Allocates.
    pub fn configure(
        &mut self,
        channels: usize,
        sample_rate: f32,
        preset: Preset,
    ) -> Result<(), Error> {
        if !(1..=64).contains(&channels) || !(8_000.0..=768_000.0).contains(&sample_rate) {
            return Err(Error::BadConfig);
        }
        if let Preset::Blocks { block, interval } = preset {
            // The windows need an overlap of two or more; the upper bound keeps every size inside `int`.
            if interval < 16 || block < interval * 2 || block > 1 << 20 {
                return Err(Error::BadConfig);
            }
        }
        let ch = channels as i32;
        // SAFETY: `raw` is a live object; the arguments were checked above.
        let rc = unsafe {
            match preset {
                Preset::Default => {
                    ffi::meloet_stretch_preset_default(self.raw.as_ptr(), ch, sample_rate)
                }
                Preset::Cheaper => {
                    ffi::meloet_stretch_preset_cheaper(self.raw.as_ptr(), ch, sample_rate)
                }
                Preset::Blocks { block, interval } => ffi::meloet_stretch_configure(
                    self.raw.as_ptr(),
                    ch,
                    block as i32,
                    interval as i32,
                ),
            }
        };
        if rc != 0 {
            self.channels = 0;
            return Err(Error::Library);
        }
        self.channels = channels;
        self.in_ptrs = vec![std::ptr::null(); channels];
        self.out_ptrs = vec![std::ptr::null_mut(); channels];
        Ok(())
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Frames in one spectral block.
    pub fn block_samples(&self) -> usize {
        // SAFETY: `raw` is a live object; the call only reads it.
        unsafe { ffi::meloet_stretch_block_samples(self.raw.as_ptr()) as usize }
    }

    /// Output frames between two spectral blocks.
    pub fn interval_samples(&self) -> usize {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_interval_samples(self.raw.as_ptr()) as usize }
    }

    /// How far the input you have given must be ahead of the centre of the block being worked on.
    pub fn input_latency(&self) -> usize {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_input_latency(self.raw.as_ptr()) as usize }
    }

    /// How far the output you read is behind the centre of the block being worked on.
    pub fn output_latency(&self) -> usize {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_output_latency(self.raw.as_ptr()) as usize }
    }

    /// Both halves, for [`Follower`].
    pub fn latency(&self) -> Latency {
        Latency {
            input: self.input_latency(),
            output: self.output_latency(),
            block: self.block_samples(),
            seek: self.seek_length(),
        }
    }

    /// The most input frames [`Stretcher::seek`] keeps (block + interval).
    pub fn seek_length(&self) -> usize {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_seek_length(self.raw.as_ptr()) as usize }
    }

    /// Pitch in semitones (fractions are cents). `tonality_limit` is a frequency divided by the
    /// sample rate above which the map bends back towards 1:1; `None` shifts everything.
    pub fn set_transpose_semitones(&mut self, semitones: f32, tonality_limit: Option<f32>) {
        // SAFETY: `raw` is a live object; the call stores numbers.
        unsafe {
            ffi::meloet_stretch_set_transpose_semitones(
                self.raw.as_ptr(),
                semitones,
                tonality_limit.unwrap_or(0.0),
            )
        };
    }

    /// Moves the formants by `factor`. With `compensate_pitch` the formants are first put back to
    /// where they were before the transpose, so `(1.0, true)` keeps the character of a voice and
    /// `(1.0, false)` lets it move with the pitch (the formant step is skipped entirely).
    pub fn set_formant_factor(&mut self, factor: f32, compensate_pitch: bool) {
        // SAFETY: as above.
        unsafe {
            ffi::meloet_stretch_set_formant_factor(
                self.raw.as_ptr(),
                factor,
                compensate_pitch as i32,
            )
        };
    }

    /// A rough fundamental of the sound, as frequency divided by the sample rate, for the formant
    /// step. `0.0`: the library estimates it for every block.
    pub fn set_formant_base(&mut self, base: f32) {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_set_formant_base(self.raw.as_ptr(), base) };
    }

    /// Clears the buffers and the phase state. Keeps the sizes, the transpose and the formant settings.
    pub fn reset(&mut self) {
        // SAFETY: as above.
        unsafe { ffi::meloet_stretch_reset(self.raw.as_ptr()) };
    }

    /// Gives past input without making output: after this, the input given so far ends at the end
    /// of `input`. Only the last [`Stretcher::seek_length`] frames are kept; shorter input is
    /// padded with silence in front. `playback_rate` (input frames per output frame) is a hint
    /// for the first block.
    pub fn seek(&mut self, input: &[&[f32]], playback_rate: f64) -> Result<(), Error> {
        let frames = self.fill_in(input);
        // SAFETY: `in_ptrs` holds `channels` pointers, each valid for `frames` reads while `input` is borrowed.
        done(unsafe {
            ffi::meloet_stretch_seek(
                self.raw.as_ptr(),
                self.in_ptrs.as_ptr(),
                frames,
                playback_rate,
            )
        })
    }

    /// The library's own way to start a sound from silence: resets, takes `input` (the first
    /// `input_latency() + rate * output_latency()` frames of the sound) and prepares so that the
    /// next output is the start of the sound. [`render`] does not use it (see `Follower`).
    pub fn output_seek(&mut self, input: &[&[f32]]) -> Result<(), Error> {
        let frames = self.fill_in(input);
        // SAFETY: as in `seek`.
        done(unsafe {
            ffi::meloet_stretch_output_seek(self.raw.as_ptr(), self.in_ptrs.as_ptr(), frames)
        })
    }

    /// Takes `input` and makes `output`; the two lengths set the stretch of this call. Inside one
    /// call the input position moves in a straight line. Either length may be zero.
    pub fn process(&mut self, input: &[&[f32]], output: &mut [&mut [f32]]) -> Result<(), Error> {
        let in_frames = self.fill_in(input);
        let out_frames = self.fill_out(output);
        // SAFETY: `in_ptrs` and `out_ptrs` hold `channels` pointers each, valid for `in_frames`
        // reads and `out_frames` writes while the slices are borrowed. Input and output cannot
        // overlap: one is borrowed shared, the other unique.
        done(unsafe {
            ffi::meloet_stretch_process(
                self.raw.as_ptr(),
                self.in_ptrs.as_ptr(),
                in_frames,
                self.out_ptrs.as_ptr(),
                out_frames,
            )
        })
    }

    /// Reads what is left without more input, fading out, then clears the state.
    pub fn flush(&mut self, output: &mut [&mut [f32]], playback_rate: f32) -> Result<(), Error> {
        let frames = self.fill_out(output);
        // SAFETY: `out_ptrs` holds `channels` pointers, each valid for `frames` writes.
        done(unsafe {
            ffi::meloet_stretch_flush(
                self.raw.as_ptr(),
                self.out_ptrs.as_ptr(),
                frames,
                playback_rate,
            )
        })
    }

    fn fill_in(&mut self, input: &[&[f32]]) -> i32 {
        assert_eq!(input.len(), self.channels, "stretch: channel count");
        let frames = input.first().map_or(0, |c| c.len());
        assert!(
            input.iter().all(|c| c.len() == frames),
            "stretch: channels of different lengths"
        );
        for (p, c) in self.in_ptrs.iter_mut().zip(input) {
            *p = c.as_ptr();
        }
        i32::try_from(frames).expect("stretch: too many frames in one call")
    }

    fn fill_out(&mut self, output: &mut [&mut [f32]]) -> i32 {
        assert_eq!(output.len(), self.channels, "stretch: channel count");
        let frames = output.first().map_or(0, |c| c.len());
        assert!(
            output.iter().all(|c| c.len() == frames),
            "stretch: channels of different lengths"
        );
        for (p, c) in self.out_ptrs.iter_mut().zip(output.iter_mut()) {
            *p = c.as_mut_ptr();
        }
        i32::try_from(frames).expect("stretch: too many frames in one call")
    }
}

fn done(rc: std::ffi::c_int) -> Result<(), Error> {
    if rc == 0 { Ok(()) } else { Err(Error::Library) }
}

impl Drop for Stretcher {
    fn drop(&mut self) {
        // SAFETY: `raw` came from `meloet_stretch_new` and is freed once, here.
        unsafe { ffi::meloet_stretch_free(self.raw.as_ptr()) };
    }
}
