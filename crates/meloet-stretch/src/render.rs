//! Makes the sound of one whole clip: reads the file where [`Follower`] says, writes the clip.

use crate::{Error, Follower, Stretcher};
use std::sync::atomic::{AtomicBool, Ordering};

/// Output frames per `process` call. Fixed, so the result never depends on how the caller's
/// `write` cuts its blocks. Short enough that a bent `p` is followed closely (5 ms at 48 kHz)
/// and that giving up is quick.
pub const RENDER_STEP: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum RenderError<E> {
    /// `cancel` was set. What was written so far is not a whole clip.
    Cancelled,
    Stretch(Error),
    /// `read` or `write` failed.
    Io(E),
}

impl<E: std::fmt::Display> std::fmt::Display for RenderError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Cancelled => f.write_str("stretch: cancelled"),
            RenderError::Stretch(e) => e.fmt(f),
            RenderError::Io(e) => e.fmt(f),
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for RenderError<E> {}

/// Renders `out_frames` frames so that file frame `p(j)` sounds at output frame `j`.
///
/// - `stretcher`: configured, with transpose and formant already set. It is reset first.
/// - `p`: see [`Follower`]. Must not go backwards; called only with `0..=out_frames`.
/// - `read(start, channels)`: fill every channel with file frames from `start` on. `start` may be
///   negative and the range may pass the end of the file: those frames must be filled with zeros.
///   The reads go forward without gaps (the first one starts about `block + interval` frames
///   before the first frame a block uses).
/// - `write(channels)`: the next frames of the clip, in order; at most [`RENDER_STEP`] at a time.
/// - `progress(percent)`: called when the whole percent changes, ending with 100.
/// - `cancel`: looked at before every step.
pub fn render<P, R, W, E>(
    stretcher: &mut Stretcher,
    out_frames: u64,
    p: P,
    mut read: R,
    mut write: W,
    mut progress: impl FnMut(u8),
    cancel: &AtomicBool,
) -> Result<(), RenderError<E>>
where
    P: Fn(u64) -> f64,
    R: FnMut(i64, &mut [&mut [f32]]) -> Result<(), E>,
    W: FnMut(&[&[f32]]) -> Result<(), E>,
{
    let channels = stretcher.channels();
    let mut follower = Follower::new(p, out_frames, stretcher.latency(), RENDER_STEP);
    let mut input: Vec<Vec<f32>> = vec![Vec::new(); channels];
    let mut output: Vec<Vec<f32>> = vec![vec![0.0; RENDER_STEP]; channels];
    let mut fill = |input: &mut Vec<Vec<f32>>, start: i64, frames: usize| {
        for c in input.iter_mut() {
            c.resize(frames, 0.0);
        }
        let mut refs: Vec<&mut [f32]> = input.iter_mut().map(|c| c.as_mut_slice()).collect();
        read(start, &mut refs).map_err(RenderError::Io)
    };

    let mut shown = 0u8;
    progress(0);
    stretcher.reset();
    let seek = follower.seek();
    fill(&mut input, seek.src_start, seek.src_frames)?;
    stretcher
        .seek(&as_refs(&input), seek.rate)
        .map_err(RenderError::Stretch)?;

    while let Some(step) = follower.next_step() {
        if cancel.load(Ordering::Relaxed) {
            return Err(RenderError::Cancelled);
        }
        fill(&mut input, step.src_start, step.src_frames)?;
        {
            let mut outs: Vec<&mut [f32]> = output
                .iter_mut()
                .map(|c| &mut c[..step.out_frames])
                .collect();
            stretcher
                .process(&as_refs(&input), &mut outs)
                .map_err(RenderError::Stretch)?;
        }
        if step.skip < step.out_frames {
            let kept: Vec<&[f32]> = output
                .iter()
                .map(|c| &c[step.skip..step.out_frames])
                .collect();
            write(&kept).map_err(RenderError::Io)?;
        }
        let percent = ((follower.done() * 100.0) as u8).min(99);
        if percent != shown {
            shown = percent;
            progress(percent);
        }
    }
    progress(100);
    Ok(())
}

fn as_refs(channels: &[Vec<f32>]) -> Vec<&[f32]> {
    channels.iter().map(|c| c.as_slice()).collect()
}
