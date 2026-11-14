//! Placeholder sine synth. The callback never allocates and never locks: notes come in through an
//! SPSC ring, measurements go out through another, meters through atomics.
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::midi::NoteEv;
use crate::mono_ns;

#[derive(Clone, Copy, Debug)]
pub struct LatRec {
    pub note: u8,
    pub recv_ns: u64,
    pub callback_ns: u64,
    pub playback_ns: u64,
}

#[derive(Default)]
pub struct Shared {
    pub level_l: AtomicU32,
    pub level_r: AtomicU32,
    pub callbacks: AtomicU64,
    pub late_callbacks: AtomicU64,
    pub last_frames: AtomicU32,
    pub max_frames: AtomicU32,
    pub min_frames: AtomicU32,
    pub max_gap_us: AtomicU64,
    pub stream_errors: AtomicU64,
}

#[derive(Clone, Copy, Default)]
struct Voice {
    note: u8,
    phase: f32,
    inc: f32,
    amp: f32,
    target: f32,
}

pub struct Running {
    pub stream: cpal::Stream,
    pub shared: Arc<Shared>,
    pub device: String,
    pub host: String,
    pub config: String,
}

pub fn start(host_id: &str, buffer: Option<u32>, mut notes: rtrb::Consumer<NoteEv>, mut lat: rtrb::Producer<LatRec>) -> Result<Running, String> {
    let host = cpal::available_hosts().into_iter().find(|h| format!("{h:?}").eq_ignore_ascii_case(host_id))
        .ok_or_else(|| format!("host {host_id} not available: {:?}", cpal::available_hosts()))
        .and_then(|h| cpal::host_from_id(h).map_err(|e| e.to_string()))?;
    let dev = host.default_output_device().ok_or("no default output device")?;
    let device = dev.description().map(|d| d.name().to_string()).unwrap_or_default();
    let sr = 48000u32;
    let cfg = cpal::StreamConfig {
        channels: 2,
        sample_rate: sr,
        buffer_size: buffer.map_or(cpal::BufferSize::Default, cpal::BufferSize::Fixed),
    };
    let cfg_text = format!("{cfg:?}");
    let shared = Arc::new(Shared::default());
    shared.min_frames.store(u32::MAX, Relaxed);
    let sh = shared.clone();
    let mut voices = [Voice::default(); 16];
    let mut last_cb = 0u64;
    let stream = dev.build_output_stream(cfg, move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
        let ts = info.timestamp();
        let cb_ns = ts.callback.as_nanos() as u64;
        let pb_ns = ts.playback.as_nanos() as u64;
        let frames = (out.len() / 2) as u32;
        // Late = the gap since the previous callback is more than 1.5x this buffer's length.
        let now = mono_ns();
        if last_cb != 0 {
            let gap = now - last_cb;
            let period = frames as u64 * 1_000_000_000 / sr as u64;
            if gap > period * 3 / 2 { sh.late_callbacks.fetch_add(1, Relaxed); }
            sh.max_gap_us.fetch_max(gap / 1000, Relaxed);
        }
        last_cb = now;
        sh.callbacks.fetch_add(1, Relaxed);
        sh.last_frames.store(frames, Relaxed);
        sh.max_frames.fetch_max(frames, Relaxed);
        sh.min_frames.fetch_min(frames, Relaxed);

        while let Ok(n) = notes.pop() {
            if n.on {
                if let Some(v) = voices.iter_mut().find(|v| v.target == 0.0 && v.amp < 1e-4) {
                    let f = 440.0 * 2f32.powf((n.note as f32 - 69.0) / 12.0);
                    *v = Voice { note: n.note, phase: 0.0, inc: f / sr as f32, amp: 0.0, target: 0.15 * n.vel as f32 / 127.0 };
                }
                // New notes start at frame 0 of this buffer, so the first sample is heard at `playback`.
                let _ = lat.push(LatRec { note: n.note, recv_ns: n.t_ns, callback_ns: cb_ns, playback_ns: pb_ns });
            } else {
                for v in voices.iter_mut().filter(|v| v.note == n.note) { v.target = 0.0; }
            }
        }
        let mut peak = 0.0f32;
        for f in out.chunks_exact_mut(2) {
            let mut s = 0.0;
            for v in voices.iter_mut() {
                if v.target == 0.0 && v.amp < 1e-4 { continue; }
                v.amp += (v.target - v.amp) * 0.01;
                s += v.amp * (v.phase * std::f32::consts::TAU).sin();
                v.phase = (v.phase + v.inc).fract();
            }
            f[0] = s;
            f[1] = s;
            peak = peak.max(s.abs());
        }
        sh.level_l.store(peak.to_bits(), Relaxed);
        sh.level_r.store(peak.to_bits(), Relaxed);
    }, {
        let sh = shared.clone();
        move |e| { sh.stream_errors.fetch_add(1, Relaxed); eprintln!("stream error: {e}"); }
    }, None).map_err(|e| format!("build_output_stream: {e}"))?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Running { stream, shared, device, host: format!("{:?}", host.id()), config: cfg_text })
}

/// Every output device each host reports, with the human-facing fields cpal exposes.
pub fn list_devices() -> serde_json::Value {
    let mut out = Vec::new();
    for id in cpal::available_hosts() {
        let Ok(host) = cpal::host_from_id(id) else { continue };
        let default = host.default_output_device().and_then(|d| d.description().ok()).map(|d| d.name().to_string());
        if let Ok(devs) = host.devices() {
            for d in devs {
                let Ok(desc) = d.description() else { continue };
                out.push(serde_json::json!({
                    "host": format!("{id:?}"), "name": desc.name(), "isDefault": default.as_deref() == Some(desc.name()),
                    "deviceType": format!("{:?}", desc.device_type()), "interface": format!("{:?}", desc.interface_type()),
                    "direction": format!("{:?}", desc.direction()), "driver": desc.driver().map(str::to_string),
                    "manufacturer": desc.manufacturer().map(str::to_string),
                    "outputs": d.supports_output(), "inputs": d.supports_input(),
                }));
            }
        }
    }
    serde_json::Value::Array(out)
}
