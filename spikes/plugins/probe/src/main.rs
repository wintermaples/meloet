//! pk-probe: runs every bet-2/bet-3 check against one plugin and prints JSON.
//!   pk-probe scan                         scan everything, one child process per item
//!   pk-probe scan-child vst3 <bundle>     (child) print PluginInfo JSON lines
//!   pk-probe scan-child lv2 <uri>
//!   pk-probe test vst3|lv2 <bundle#classid | uri> [--show secs] [--play]
//!   pk-probe crash vst3|lv2 <id> kill|segv|hang
//!   pk-probe worker vst3|lv2 <id> [segv|hang]   (child of `crash`)
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pk_core::{Kind, MidiEvent, Plugin, PluginInfo};
use serde_json::{Value, json};

const SR: f64 = 48000.0;
const BLOCK: usize = 256;

fn load(format: &str, id: &str) -> Result<Box<dyn Plugin>, String> {
    match format {
        "vst3" => {
            let (bundle, cid) = id.split_once('#').ok_or("vst3 id is <bundle>#<classid>")?;
            Ok(Box::new(pk_vst3::Vst3Plugin::load(Path::new(bundle), cid)?))
        }
        "lv2" => Ok(Box::new(pk_lv2::Lv2Plugin::load(id)?)),
        _ => Err("format".into()),
    }
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// Renders `secs` of audio. Instruments get a C major chord for the first half; effects get a 220 Hz sine.
fn render(p: &mut dyn Plugin, secs: f64) -> [Vec<f32>; 2] {
    let total = (SR * secs) as usize;
    let instrument = p.info().kind == Kind::Instrument;
    let mut out = [vec![0.0f32; total], vec![0.0f32; total]];
    let mut pos = 0;
    while pos < total {
        let n = BLOCK.min(total - pos);
        let mut midi = Vec::new();
        if instrument && pos == 0 {
            for k in [60u8, 64, 67] { midi.push(MidiEvent { frame: 0, data: [0x90, k, 100] }); }
        }
        let off = total / 2;
        if instrument && pos <= off && off < pos + n {
            for k in [60u8, 64, 67] { midi.push(MidiEvent { frame: (off - pos) as u32, data: [0x80, k, 0] }); }
        }
        let sine: Vec<f32> = (pos..pos + n).map(|i| (0.3 * (2.0 * std::f64::consts::PI * 220.0 * i as f64 / SR).sin()) as f32).collect();
        let (l, r) = out.split_at_mut(1);
        let mut outs: [&mut [f32]; 2] = [&mut l[0][pos..pos + n], &mut r[0][pos..pos + n]];
        p.process(&[&sine, &sine], &mut outs, &midi);
        p.editor_idle();
        pos += n;
    }
    out
}

/// 10 ms frames of (RMS, zero-crossing rate), normalised: the shape of the sound, not the exact samples.
fn features(x: &[f32]) -> Vec<f64> {
    let mut v = Vec::new();
    for f in x.chunks(480) {
        v.push(rms(f));
        v.push(f.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count() as f64 / f.len() as f64);
    }
    v
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) { sab += (x - ma) * (y - mb); saa += (x - ma).powi(2); sbb += (y - mb).powi(2); }
    sab / (saa.sqrt() * sbb.sqrt()).max(1e-30)
}

fn compare(a: &[Vec<f32>; 2], b: &[Vec<f32>; 2]) -> Value {
    let mut max = 0.0f64;
    for ch in 0..2 { for (x, y) in a[ch].iter().zip(&b[ch]) { max = max.max((*x as f64 - *y as f64).abs()); } }
    let wa: Vec<f64> = a[0].iter().map(|x| *x as f64).collect();
    let wb: Vec<f64> = b[0].iter().map(|x| *x as f64).collect();
    json!({ "maxAbsDiff": max, "waveCorrelation": pearson(&wa, &wb), "shapeCorrelation": pearson(&features(&a[0]), &features(&b[0])),
            "rmsA": rms(&a[0]), "rmsB": rms(&b[0]) })
}

fn test(format: &str, id: &str, show: f64, play: bool) -> Value {
    let mut r = serde_json::Map::new();
    let t = Instant::now();
    let mut p = match load(format, id) { Ok(p) => p, Err(e) => return json!({ "load": { "ok": false, "error": e } }) };
    let info = p.info().clone();
    r.insert("info".into(), serde_json::to_value(&info).unwrap());
    if let Err(e) = p.activate(SR, BLOCK) { r.insert("activate".into(), json!({ "ok": false, "error": e })); return r.into(); }
    r.insert("load".into(), json!({ "ok": true, "ms": t.elapsed().as_secs_f64() * 1000.0 }));

    // Sound
    let a = render(p.as_mut(), 1.0);
    let first_half = rms(&a[0][..a[0].len() / 2]);
    r.insert("sound".into(), json!({ "rmsL": rms(&a[0]), "rmsFirstHalf": first_half, "ok": first_half > 1e-4 }));
    if play { r.insert("play".into(), play_audio(&a)); }

    // Params: read, write, read back
    let params = p.params();
    let picked: Vec<_> = params.iter().filter(|x| x.automatable).take(3).cloned().collect();
    let mut rw = Vec::new();
    for (i, q) in picked.iter().enumerate() {
        let before = p.get_param(q.id);
        let target = [0.25, 0.5, 0.75][i];
        p.set_param(q.id, target);
        let _ = render(p.as_mut(), 0.02);
        rw.push(json!({ "id": q.id, "name": q.name, "before": before, "wrote": target, "readBack": p.get_param(q.id) }));
    }
    r.insert("params".into(), json!({ "count": params.len(), "readWrite": rw }));

    // State. Change up to 40 params so the sound really differs, save, then compare fresh instances only
    // (an instance that has been playing keeps reverb tails and envelope history, so it is not a fair reference):
    //   restoredTwice = two fresh instances given the state  (how repeatable the plugin itself is)
    //   vsDirect      = fresh + state  vs  fresh + the same param values set directly
    //   vsControl     = fresh + state  vs  fresh with nothing done (what "not restored" looks like)
    let mut seed = 7u32;
    let chosen: Vec<(u32, f64)> = params.iter().filter(|x| x.automatable).take(40).map(|q| {
        seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
        (q.id, (seed % 1000) as f64 / 1000.0)
    }).collect();
    for (id, v) in &chosen { p.set_param(*id, *v); }
    let _ = render(p.as_mut(), 0.1);
    let state_res = match p.save_state() {
        Err(e) => json!({ "ok": false, "error": e }),
        Ok(bytes) => {
            // What the controller reports right after our writes is only what we wrote (Surge rounds steps and
            // resets FX params when the FX type changes), so read the original back after applying its own state.
            let _ = p.load_state(&bytes);
            let vals: Vec<f64> = chosen.iter().map(|(id, _)| p.get_param(*id)).collect();
            let fresh = |how: &str| -> Result<([Vec<f32>; 2], Vec<f64>, Option<String>), String> {
                let mut x = load(format, id)?;
                x.activate(SR, BLOCK)?;
                let mut err = None;
                match how {
                    "state" => err = x.load_state(&bytes).err(),
                    "direct" => for (id, v) in &chosen { x.set_param(*id, *v); },
                    _ => {}
                }
                let _ = render(x.as_mut(), 2.0); // patch loads during the first process calls; let it settle
                let pv = chosen.iter().map(|(id, _)| x.get_param(*id)).collect();
                Ok((render(x.as_mut(), 1.0), pv, err))
            };
            match (fresh("state"), fresh("state"), fresh("direct"), fresh("none")) {
                (Ok(a), Ok(b), Ok(c), Ok(d)) => json!({
                    "bytes": bytes.len(), "restore": a.2,
                    "paramsCompared": vals.len(),
                    "paramsEqual": vals.iter().zip(&a.1).all(|(x, y)| (x - y).abs() < 1e-5), // f32 storage
                    "paramsDiffer": chosen.iter().zip(vals.iter().zip(&a.1)).filter(|(_, (x, y))| (*x - *y).abs() >= 1e-5)
                        .map(|((id, _), (x, y))| json!({ "id": id, "saved": x, "restored": y })).collect::<Vec<_>>(),
                    "restoredTwice": compare(&a.0, &b.0), "vsDirect": compare(&a.0, &c.0), "vsControl": compare(&a.0, &d.0) }),
                (a, b, c, d) => json!({ "error": format!("{:?}", [a.err(), b.err(), c.err(), d.err()]) }),
            }
        }
    };
    r.insert("state".into(), state_res);

    // Editor: open and close three times; read pixels back while open.
    let mut eds = Vec::new();
    if info.uis.contains(&pk_core::UiKind::X11Embed) {
        for round in 0..3 {
            let t = Instant::now();
            let mut win = match pk_core::x11win::HostWindow::open(&info.name, 400, 300) { Ok(w) => w, Err(e) => { eds.push(json!({ "error": e })); break } };
            match p.open_editor(win.id) {
                Err(e) => { eds.push(json!({ "round": round, "error": e })); win.close(); break }
                Ok((w, h)) => {
                    if w > 0 && h > 0 { win.resize(w, h); }
                    let opened_ms = t.elapsed().as_secs_f64() * 1000.0;
                    let hold = if round == 0 && show > 0.0 { show } else { 1.5 };
                    let until = Instant::now() + Duration::from_secs_f64(hold);
                    let mut fitted = None;
                    while Instant::now() < until && win.pump() {
                        p.editor_idle();
                        fitted = win.fit_to_child().or(fitted);
                        let _ = render(p.as_mut(), BLOCK as f64 / SR);
                        std::thread::sleep(Duration::from_millis(16));
                    }
                    let colours = win.distinct_colours();
                    let t2 = Instant::now();
                    p.close_editor();
                    win.close();
                    eds.push(json!({ "round": round, "size": [w, h], "childSize": fitted, "openMs": opened_ms, "closeMs": t2.elapsed().as_secs_f64() * 1000.0,
                                     "distinctColours": colours.as_ref().ok(), "readbackError": colours.err() }));
                }
            }
        }
    }
    r.insert("editor".into(), json!(eds));
    drop(p);
    r.insert("unloaded".into(), json!(true));
    r.into()
}

fn play_audio(buf: &[Vec<f32>; 2]) -> Value {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let Some(dev) = host.default_output_device() else { return json!({ "error": "no output device" }) };
    let name = dev.description().map(|d| d.name().to_string()).unwrap_or_default();
    let data = std::sync::Arc::new((buf[0].clone(), buf[1].clone()));
    let pos = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cfg = cpal::StreamConfig { channels: 2, sample_rate: SR as u32, buffer_size: cpal::BufferSize::Default };
    let (d2, p2) = (data.clone(), pos.clone());
    let stream = dev.build_output_stream(cfg, move |out: &mut [f32], _| {
        for f in out.chunks_mut(2) {
            let i = p2.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            f[0] = *d2.0.get(i).unwrap_or(&0.0);
            f[1] = *d2.1.get(i).unwrap_or(&0.0);
        }
    }, |e| eprintln!("stream error: {e}"), None);
    match stream {
        Err(e) => json!({ "device": name, "error": e.to_string() }),
        Ok(s) => { let _ = s.play(); std::thread::sleep(Duration::from_millis(1200)); json!({ "device": name, "host": format!("{:?}", host.id()) }) }
    }
}

fn scan() -> Value {
    let exe = std::env::current_exe().unwrap();
    let mut items: Vec<(String, String)> = Vec::new();
    for dir in pk_vst3::default_paths() {
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().extension().is_some_and(|x| x == "vst3") { items.push(("vst3".into(), e.path().display().to_string())) }
            }
        }
    }
    if let Some(extra) = std::env::var_os("PK_EXTRA_VST3") { items.push(("vst3".into(), extra.to_string_lossy().into_owned())) }
    // LV2 metadata is read in one child (lilv parses third-party TTL files); then each plugin's code runs in its own child.
    let t_meta = Instant::now();
    let out = Command::new(&exe).arg("list-lv2").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let lv2_meta_ms = t_meta.elapsed().as_secs_f64() * 1000.0;
    let mut lv2_info = std::collections::HashMap::new();
    for l in out.lines() {
        if let Some((info, bundle)) = l.split_once('\t').and_then(|(j, b)| serde_json::from_str::<Value>(j).ok().map(|v| (v, b.to_string()))) {
            let uri = info["id"].as_str().unwrap_or("").to_string();
            items.push(("lv2".into(), format!("{uri}\t{bundle}")));
            lv2_info.insert(uri, info);
        }
    }
    let t = Instant::now();
    let mut found = Vec::new();
    let mut failed = Vec::new();
    for (fmt, id) in &items {
        let t1 = Instant::now();
        let mut args = vec!["scan-child", fmt];
        args.extend(id.split('\t'));
        let mut child = Command::new(&exe).args(&args).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() { break Some(s) }
            if Instant::now() > deadline { let _ = child.kill(); let _ = child.wait(); break None }
            std::thread::sleep(Duration::from_millis(5));
        };
        let mut s = String::new();
        child.stdout.take().unwrap().read_to_string(&mut s).ok();
        let ms = t1.elapsed().as_secs_f64() * 1000.0;
        match status {
            Some(st) if st.success() => {
                if fmt == "lv2" {
                    let uri = id.split('\t').next().unwrap_or("");
                    if let Some(v) = lv2_info.get(uri) { found.push(json!({ "ms": ms, "plugin": v })) }
                } else {
                    for l in s.lines() { if let Ok(v) = serde_json::from_str::<Value>(l) { found.push(json!({ "ms": ms, "plugin": v })) } }
                }
            }
            Some(st) => failed.push(json!({ "format": fmt, "id": id, "why": format!("{st}"), "ms": ms })),
            None => failed.push(json!({ "format": fmt, "id": id, "why": "timeout 10 s", "ms": ms })),
        }
    }
    json!({ "items": items.len(), "ok": found.len(), "failed": failed, "totalMs": t.elapsed().as_secs_f64() * 1000.0, "lv2MetadataMs": lv2_meta_ms,
            "hostAlive": true, "plugins": found })
}

fn scan_child(out: &mut std::fs::File, fmt: &str, id: &str, bundle: &str) -> Result<(), String> {
    let infos: Vec<PluginInfo> = match fmt {
        "vst3" => pk_vst3::scan(Path::new(id))?,
        _ => { pk_lv2::probe(id, bundle)?; vec![] }
    };
    for i in infos { let _ = writeln!(out, "{}", serde_json::to_string(&i).unwrap()); }
    Ok(())
}

/// Child side of the crash test: renders blocks forever and reports each one on stdout.
fn worker(mut out: std::fs::File, fmt: &str, id: &str, mode: &str) {
    let mut p = load(fmt, id).expect("load");
    p.activate(SR, BLOCK).expect("activate");
    let mut n = 0u64;
    loop {
        let a = render(p.as_mut(), BLOCK as f64 / SR);
        n += 1;
        if n == 200 && mode == "segv" { unsafe { std::ptr::null_mut::<u32>().write_volatile(1) } }
        if n == 200 && mode == "hang" { loop { std::thread::sleep(Duration::from_secs(1)) } }
        let _ = writeln!(out, "{n} {:.5}", rms(&a[0]));
        let _ = out.flush();
        std::thread::sleep(Duration::from_micros((BLOCK as f64 / SR * 1e6) as u64));
    }
}

/// Parent side: the "engine" keeps a block clock running and treats a silent child as dead after 2 blocks.
fn crash(fmt: &str, id: &str, mode: &str) -> Value {
    let exe = std::env::current_exe().unwrap();
    let wmode = if mode == "kill" { "none" } else { mode };
    let mut child = Command::new(&exe).args(["worker", fmt, id, wmode]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for l in BufReader::new(stdout).lines() { if tx.send(l.ok()).is_err() { break } }
        let _ = tx.send(None);
    });
    let block = Duration::from_secs_f64(BLOCK as f64 / SR);
    let mut last = Instant::now();
    let mut blocks = 0u64;
    let mut injected_at = None;
    let start = Instant::now();
    let mut detected = None;
    let mut engine_blocks_after = 0u64;
    while start.elapsed() < Duration::from_secs(20) {
        match rx.recv_timeout(block) {
            Ok(Some(_)) => { blocks += 1; last = Instant::now(); }
            Ok(None) => { if detected.is_none() { detected = Some(("eof", Instant::now())) } }
            Err(_) => {}
        }
        if mode == "kill" && blocks == 200 && injected_at.is_none() {
            injected_at = Some(Instant::now());
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
        if mode != "kill" && blocks >= 199 && injected_at.is_none() { injected_at = Some(last) }
        if detected.is_none() && blocks > 0 && last.elapsed() > block * 2 { detected = Some(("watchdog", Instant::now())) }
        if detected.is_some() {
            engine_blocks_after += 1; // the engine's own clock keeps going, with this slot bypassed
            if engine_blocks_after > 50 { break }
        }
    }
    let _ = child.kill();
    let st = child.wait().ok().map(|s| format!("{s}"));
    json!({ "mode": mode, "childBlocksBeforeFault": blocks, "childExit": st,
            "detectedBy": detected.map(|d| d.0),
            "detectMsAfterFault": match (injected_at, detected) { (Some(a), Some(d)) => Some(d.1.duration_since(a).as_secs_f64() * 1000.0), _ => None },
            "engineBlocksAfterFault": engine_blocks_after, "parentAlive": true })
}

/// Plugins print to stdout (Dragonfly warns there). Keep a private copy of the real stdout for our
/// JSON and point fd 1 at stderr, so nothing a plugin writes can corrupt the results.
fn private_stdout() -> std::fs::File {
    use std::os::fd::FromRawFd;
    unsafe {
        let fd = libc::dup(1);
        libc::dup2(2, 1);
        std::fs::File::from_raw_fd(fd)
    }
}

fn main() {
    let mut out = private_stdout();
    let a: Vec<String> = std::env::args().collect();
    let arg = |i: usize| a.get(i).map(String::as_str).unwrap_or("");
    let flag = |f: &str| a.iter().position(|x| x == f);
    let v = match arg(1) {
        "scan" => scan(),
        "list-lv2" => { for (p, b) in pk_lv2::scan_all() { let _ = writeln!(out, "{}\t{b}", serde_json::to_string(&p).unwrap()); } return }
        "scan-child" => { if let Err(e) = scan_child(&mut out, arg(2), arg(3), arg(4)) { eprintln!("{e}"); std::process::exit(2) } return }
        "test" => {
            let show = flag("--show").and_then(|i| a.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            test(arg(2), arg(3), show, flag("--play").is_some())
        }
        "worker" => { worker(out, arg(2), arg(3), arg(4)); return }
        "crash" => crash(arg(2), arg(3), arg(4)),
        _ => { eprintln!("see the comment at the top of main.rs"); std::process::exit(1) }
    };
    let _ = writeln!(out, "{}", serde_json::to_string_pretty(&v).unwrap());
}
