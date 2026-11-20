//! The way out: a cpal output stream on the PipeWire host, connected to one named node.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::{BlockTiming, Engine, EngineMeters};

/// A PipeWire sink, by its `node.name` (for example `alsa_output.pci-0000_0e_00.6.analog-stereo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputTarget {
    pub node_name: String,
}

/// Lives inside the stream's data callback. cpal drops the callback on its own thread after the
/// stream has stopped; that drop hands the engine back instead of destroying it.
struct Carrier {
    engine: Option<Engine>,
    back: Producer<Engine>,
}

impl Drop for Carrier {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.take() {
            let _ = self.back.push(engine);
        }
    }
}

/// The engine normally is already in the ring. The wait covers cpal's failure paths, where its
/// thread may still be unwinding when the error reaches us.
fn recover(back: &mut Consumer<Engine>) -> Option<Engine> {
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        if let Ok(engine) = back.pop() {
            return Some(engine);
        }
        if back.is_abandoned() || Instant::now() >= deadline {
            return back.pop().ok();
        }
        std::thread::yield_now();
    }
}

/// Sorts what cpal reports into xruns, the end of the stream, and the rest. On the PipeWire host
/// this runs on the stream's own thread: xruns from inside its process callback, the rest from
/// its main loop (the core's error event when the connection to the daemon breaks, the stream's
/// state going to `Unconnected` or `Error`). A stream that meloet closes also reports
/// `DeviceNotAvailable` on the way out; `closing` keeps that from counting.
fn stream_error_counter(
    meters: Arc<EngineMeters>,
    closing: Arc<AtomicBool>,
) -> impl FnMut(cpal::Error) + Send + 'static {
    let mut lost = false;
    move |error| match error.kind() {
        _ if closing.load(Relaxed) => {}
        cpal::ErrorKind::Xrun => meters.count_xrun(),
        cpal::ErrorKind::DeviceNotAvailable
        | cpal::ErrorKind::StreamInvalidated
        | cpal::ErrorKind::HostUnavailable
        | cpal::ErrorKind::UnsupportedConfig => {
            if !lost {
                lost = true;
                meters.count_stream_lost();
            }
        }
        _ => meters.count_other_error(),
    }
}

pub struct OpenError {
    pub message: String,
    /// The engine that was passed in, so it can be used for another try. `None` only if cpal's
    /// stream thread did not let go of it in time (it timed out talking to PipeWire).
    pub engine: Option<Box<Engine>>,
}

impl OpenError {
    fn new(message: impl fmt::Display, engine: Option<Engine>) -> Self {
        OpenError {
            message: message.to_string(),
            engine: engine.map(Box::new),
        }
    }
}

impl fmt::Debug for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenError")
            .field("message", &self.message)
            .field("engine", &self.engine.is_some())
            .finish()
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OpenError {}

pub struct AudioOutput;

impl AudioOutput {
    /// Opens a stereo f32 stream on `target` and starts it with `engine` inside.
    ///
    /// `request_frames` is only a wish (PipeWire `node.latency`). The size that really runs is
    /// [`EngineMeters::last_frames`]. The engine is prepared here for the device's sample rate.
    pub fn open(
        target: &OutputTarget,
        request_frames: u32,
        engine: Engine,
    ) -> Result<RunningOutput, OpenError> {
        AudioOutput::open_at(target, request_frames, None, engine)
    }

    /// As [`AudioOutput::open`], asking for `request_rate` too. The device's own rate runs
    /// when it does not take the one asked for; what runs is
    /// [`RunningOutput::sample_rate`].
    pub fn open_at(
        target: &OutputTarget,
        request_frames: u32,
        request_rate: Option<u32>,
        mut engine: Engine,
    ) -> Result<RunningOutput, OpenError> {
        // A fresh host each time: cpal's device list (and the object serial it connects to) is a
        // snapshot taken when the host is made.
        let host = match cpal::host_from_id(cpal::HostId::PipeWire) {
            Ok(host) => host,
            Err(e) => return Err(OpenError::new(e, Some(engine))),
        };
        let wanted = cpal::DeviceId::new(cpal::HostId::PipeWire, &target.node_name);
        let device = match host.devices() {
            Ok(mut devices) => {
                devices.find(|d| d.supports_output() && d.id().is_ok_and(|id| id == wanted))
            }
            Err(e) => return Err(OpenError::new(e, Some(engine))),
        };
        let Some(device) = device else {
            let message = format!("no PipeWire output node named {}", target.node_name);
            return Err(OpenError::new(message, Some(engine)));
        };
        let sample_rate = match device.default_output_config() {
            Ok(config) => config.sample_rate(),
            Err(e) => return Err(OpenError::new(e, Some(engine))),
        };
        let takes = |rate: &u32| {
            device.supported_output_configs().is_ok_and(|mut configs| {
                configs.any(|c| (c.min_sample_rate()..=c.max_sample_rate()).contains(rate))
            })
        };
        let sample_rate = request_rate.filter(takes).unwrap_or(sample_rate);

        engine.prepare(sample_rate);
        let meters = engine.meters().clone();
        let (back_tx, mut back) = RingBuffer::new(1);
        let mut carrier = Carrier {
            engine: Some(engine),
            back: back_tx,
        };
        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate,
            buffer_size: cpal::BufferSize::Fixed(request_frames),
        };
        let error_meters = meters.clone();
        let closing = Arc::new(AtomicBool::new(false));
        let built = device.build_output_stream(
            config,
            move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let stamp = info.timestamp();
                let timing = BlockTiming {
                    callback_us: (stamp.callback.as_nanos() / 1_000) as u64,
                    playback_us: (stamp.playback.as_nanos() / 1_000) as u64,
                };
                if let Some(engine) = carrier.engine.as_mut() {
                    engine.process(out, timing);
                }
            },
            stream_error_counter(error_meters, closing.clone()),
            None,
        );
        let stream = match built {
            Ok(stream) => stream,
            Err(e) => return Err(OpenError::new(e, recover(&mut back))),
        };
        if let Err(e) = stream.play() {
            drop(stream);
            return Err(OpenError::new(e, recover(&mut back)));
        }
        Ok(RunningOutput {
            stream,
            closing,
            back,
            meters,
            sample_rate,
        })
    }
}

/// A running stream. Dropping it stops the sound and destroys the engine; use
/// [`RunningOutput::close`] to keep the engine.
pub struct RunningOutput {
    stream: cpal::Stream,
    closing: Arc<AtomicBool>,
    back: Consumer<Engine>,
    meters: Arc<EngineMeters>,
    sample_rate: u32,
}

impl RunningOutput {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn meters(&self) -> &Arc<EngineMeters> {
        &self.meters
    }

    /// Stops the stream and returns the engine with its rings, meters and instruments intact.
    /// Do not call this from inside a stream callback.
    pub fn close(self) -> Option<Engine> {
        let RunningOutput {
            stream,
            closing,
            mut back,
            ..
        } = self;
        closing.store(true, Relaxed);
        drop(stream);
        recover(&mut back)
    }
}
