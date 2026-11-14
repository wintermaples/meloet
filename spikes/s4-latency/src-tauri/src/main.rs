mod audio;
mod midi;

use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use midi::MidiMsg;
use serde_json::{Value, json};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn mono_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// What the input meter, mixer meters and transport need each frame. 112 bytes in binary.
#[derive(Clone, Default, serde::Serialize)]
struct Snapshot {
    seq: u64,
    t_ns: u64,
    keys: [u64; 2],
    vel: u8,
    sustain: u8,
    bend: i16,
    modw: u8,
    meters: [f32; 18],
    position: f64,
}

impl Snapshot {
    fn bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(112);
        b.push(2u8);
        b.extend_from_slice(&[self.vel, self.sustain, self.modw]);
        b.extend_from_slice(&self.bend.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&self.seq.to_le_bytes());
        b.extend_from_slice(&self.t_ns.to_le_bytes());
        b.extend_from_slice(&self.keys[0].to_le_bytes());
        b.extend_from_slice(&self.keys[1].to_le_bytes());
        b.extend_from_slice(&self.position.to_le_bytes());
        for m in self.meters { b.extend_from_slice(&m.to_le_bytes()); }
        b
    }
}

fn key_bytes(on: bool, note: u8, vel: u8, t_ns: u64) -> Vec<u8> {
    let mut b = vec![1u8, on as u8, note, vel, 0, 0, 0, 0];
    b.extend_from_slice(&t_ns.to_le_bytes());
    b
}

#[derive(Clone, Copy, PartialEq)]
enum Transport { Event, ChanJson, ChanBin, Pull }

struct App {
    snapshot: Mutex<Snapshot>,
    pull_queue: Mutex<Vec<u8>>,
    transport: Mutex<Option<(Transport, Option<Channel<InvokeResponseBody>>)>>,
    midi_log: Mutex<Vec<Value>>,
    lat_log: Mutex<Vec<audio::LatRec>>,
    audio: Mutex<Option<audio::Running>>,
    audio_info: Value,
    frame_hz: Mutex<f64>,
    out: Option<String>,
}

fn send(app: &tauri::AppHandle, st: &App, json_v: Value, bin: Vec<u8>, is_key: bool) {
    let Some((t, ch)) = st.transport.lock().unwrap().clone() else { return };
    match t {
        Transport::Event => { let _ = app.emit(if is_key { "key" } else { "frame" }, json_v); }
        Transport::ChanJson => { if let Some(c) = ch { let _ = c.send(InvokeResponseBody::Json(json_v.to_string())); } }
        Transport::ChanBin => { if let Some(c) = ch { let _ = c.send(InvokeResponseBody::Raw(bin)); } }
        Transport::Pull => { if is_key { st.pull_queue.lock().unwrap().extend(bin); } }
    }
}

#[tauri::command]
fn clock() -> u64 { mono_ns() }

#[tauri::command]
fn devices(st: tauri::State<'_, Arc<App>>) -> Value {
    json!({ "outputs": audio::list_devices(), "opened": st.audio_info })
}

#[tauri::command]
fn connect(mode: String, hz: f64, channel: Channel<InvokeResponseBody>, st: tauri::State<'_, Arc<App>>) {
    let t = match mode.as_str() { "event" => Transport::Event, "chan-json" => Transport::ChanJson, "chan-bin" => Transport::ChanBin, _ => Transport::Pull };
    *st.transport.lock().unwrap() = Some((t, Some(channel)));
    *st.frame_hz.lock().unwrap() = hz;
}

fn cpu_ticks(pid: u32) -> u64 {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let f: Vec<&str> = s.rsplit_once(')').map(|x| x.1).unwrap_or("").split_whitespace().collect();
    f.get(11).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) + f.get(12).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0)
}

fn descendants(root: u32) -> Vec<(u32, String)> {
    let mut all = Vec::new();
    for e in std::fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = e.file_name().to_string_lossy().parse::<u32>() else { continue };
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        let (Some(a), Some(b)) = (s.find('('), s.rfind(')')) else { continue };
        let ppid: u32 = s[b + 2..].split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0);
        all.push((pid, ppid, s[a + 1..b].to_string()));
    }
    let mut out = vec![];
    let mut frontier = vec![root];
    while let Some(p) = frontier.pop() {
        for (pid, ppid, name) in &all { if *ppid == p { out.push((*pid, name.clone())); frontier.push(*pid); } }
    }
    out
}

#[tauri::command]
fn cpu_sample() -> Value {
    let me = std::process::id();
    let kids: Vec<Value> = descendants(me).into_iter().map(|(p, n)| json!({ "pid": p, "name": n, "ticks": cpu_ticks(p) })).collect();
    json!({ "t_ns": mono_ns(), "self": cpu_ticks(me), "children": kids, "hz": unsafe { libc::sysconf(libc::_SC_CLK_TCK) } })
}

#[tauri::command]
fn finish(result: String, app: tauri::AppHandle, st: tauri::State<'_, Arc<App>>) {
    let mut v: Value = serde_json::from_str(&result).unwrap_or(Value::Null);
    v["rustMidi"] = Value::Array(st.midi_log.lock().unwrap().clone());
    v["audioLatency"] = Value::Array(st.lat_log.lock().unwrap().iter().map(|r| json!({
        "note": r.note, "recvNs": r.recv_ns, "callbackNs": r.callback_ns, "playbackNs": r.playback_ns,
        "recvToCallbackMs": (r.callback_ns as f64 - r.recv_ns as f64) / 1e6,
        "recvToPlaybackMs": (r.playback_ns as f64 - r.recv_ns as f64) / 1e6 })).collect());
    if let Some(a) = st.audio.lock().unwrap().as_ref() {
        let s = &a.shared;
        v["audio"] = json!({ "device": a.device, "host": a.host, "config": a.config,
            "callbacks": s.callbacks.load(Relaxed), "lateCallbacks": s.late_callbacks.load(Relaxed),
            "framesPerCallback": [s.min_frames.load(Relaxed), s.max_frames.load(Relaxed)],
            "maxGapUs": s.max_gap_us.load(Relaxed), "streamErrors": s.stream_errors.load(Relaxed) });
    }
    if let Some(p) = &st.out { std::fs::write(p, serde_json::to_string_pretty(&v).unwrap()).unwrap(); }
    app.exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let query = get("--query").unwrap_or_default();
    let host = get("--host").unwrap_or("PipeWire".into());
    let buffer = get("--buffer").and_then(|b| b.parse().ok());

    let (note_tx, note_rx) = rtrb::RingBuffer::new(256);
    let (lat_tx, mut lat_rx) = rtrb::RingBuffer::new(4096);
    let (ui_tx, ui_rx) = mpsc::channel::<MidiMsg>();
    let (udev_tx, udev_rx) = mpsc::channel();
    midi::spawn(note_tx, ui_tx.clone());
    midi::spawn_udev(udev_tx);
    let running = audio::start(&host, buffer, note_rx, lat_tx);
    let audio_info = match &running {
        Ok(r) => json!({ "ok": true, "device": r.device, "host": r.host, "config": r.config }),
        Err(e) => json!({ "ok": false, "error": e }),
    };
    let state = Arc::new(App {
        snapshot: Mutex::default(), pull_queue: Mutex::default(), transport: Mutex::new(None), midi_log: Mutex::default(),
        lat_log: Mutex::default(), audio: Mutex::new(running.ok()), audio_info, frame_hz: Mutex::new(0.0), out: get("--out"),
    });

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![clock, devices, connect, cpu_sample, finish])
        .register_uri_scheme_protocol("meloet", {
            let st = state.clone();
            move |_ctx, _req| {
                let mut body = st.snapshot.lock().unwrap().bytes();
                body.extend(std::mem::take(&mut *st.pull_queue.lock().unwrap()));
                tauri::http::Response::builder()
                    .header("Content-Type", "application/octet-stream")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(body).unwrap()
            }
        })
        .setup(move |app| {
            app.manage(state.clone());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("s4-latency").inner_size(1280.0, 800.0)
                .initialization_script(format!("window.__S4 = {query:?};"))
                .build()?;
            let h = app.handle().clone();
            let st = state.clone();
            // MIDI -> UI, sent the moment it arrives. Also keeps the snapshot current.
            std::thread::spawn(move || {
                for m in ui_rx {
                    let mut snap = st.snapshot.lock().unwrap();
                    match &m {
                        MidiMsg::Note { on, note, vel, t_ns, .. } => {
                            let (w, b) = ((*note / 64) as usize, *note % 64);
                            if *on { snap.keys[w] |= 1 << b; snap.vel = *vel; } else { snap.keys[w] &= !(1 << b); }
                            let bin = key_bytes(*on, *note, *vel, *t_ns);
                            drop(snap);
                            send(&h, &st, serde_json::to_value(&m).unwrap(), bin, true);
                        }
                        MidiMsg::Ctrl { param: 64, value, .. } => snap.sustain = *value as u8,
                        MidiMsg::Ctrl { param: 1, value, .. } => snap.modw = *value as u8,
                        MidiMsg::Bend { value, .. } => snap.bend = *value as i16,
                        MidiMsg::Device { .. } => { drop(snap); let _ = h.emit("device", serde_json::to_value(&m).unwrap()); }
                        _ => {}
                    }
                    st.midi_log.lock().unwrap().push(serde_json::to_value(&m).unwrap());
                }
            });
            let h = app.handle().clone();
            std::thread::spawn(move || {
                for (action, name, t) in udev_rx { let _ = h.emit("udev", json!({ "action": action, "name": name, "tNs": t })); }
            });
            let h = app.handle().clone();
            let st = state.clone();
            // Periodic frames for the load test, plus draining audio measurements (never from the audio thread).
            std::thread::spawn(move || {
                let mut seq = 0u64;
                loop {
                    let hz = *st.frame_hz.lock().unwrap();
                    while let Ok(r) = lat_rx.pop() { st.lat_log.lock().unwrap().push(r); }
                    if hz <= 0.0 { std::thread::sleep(Duration::from_millis(10)); continue; }
                    std::thread::sleep(Duration::from_secs_f64(1.0 / hz));
                    seq += 1;
                    let (lvl, snap) = {
                        let a = st.audio.lock().unwrap();
                        let lvl = a.as_ref().map_or(0.0, |a| f32::from_bits(a.shared.level_l.load(Relaxed)));
                        let mut s = st.snapshot.lock().unwrap();
                        s.seq = seq;
                        s.t_ns = mono_ns();
                        s.position += 1.0 / hz;
                        for (i, m) in s.meters.iter_mut().enumerate() { *m = (lvl + 0.3 * ((seq as f32 * 0.1) + i as f32).sin().abs()).min(1.0); }
                        (lvl, s.clone())
                    };
                    let _ = lvl;
                    if st.transport.lock().unwrap().as_ref().map(|t| t.0) != Some(Transport::Pull) {
                        send(&h, &st, serde_json::to_value(&snap).unwrap(), snap.bytes(), false);
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run");
}
