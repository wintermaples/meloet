//! Child of `pk-ipcbench mem --kind host`: loads plugins, renders a second of sound with each, and waits.
//!   pk-ipchost child <fd> <len> <index> [--window] [--state-iters N] <spec>...
//!   spec: vst3=<bundle>#<classid> | lv2=<uri> (reads every installed TTL) | lv2b=<uri>@<bundle uri> (reads one bundle,
//!         a new lilv world for each plugin) | lv2s=<uri>@<bundle uri> (one bundle, one world for the whole process)
//! --window opens the editor of the first plugin only. --state-iters saves and restores each plugin's state N times.
use std::path::Path;
use std::sync::atomic::Ordering::SeqCst;

use pk_core::{Kind, MidiEvent, Plugin, UiKind};
use pk_ipcbench::*;
use serde_json::{Value, json};

type Worlds = std::collections::HashMap<String, std::rc::Rc<livi::World>>;

fn load(spec: &str, worlds: &mut Worlds) -> Result<Box<dyn Plugin>, String> {
    let (fmt, id) = spec.split_once('=').ok_or("spec is <format>=<id>")?;
    match fmt {
        "vst3" => {
            let (bundle, cid) = id.split_once('#').ok_or("vst3=<bundle>#<classid>")?;
            Ok(Box::new(pk_vst3::Vst3Plugin::load(Path::new(bundle), cid)?))
        }
        "lv2" => Ok(Box::new(pk_lv2::Lv2Plugin::load(id)?)),
        "lv2b" => {
            let (uri, bundle) = id.split_once('@').ok_or("lv2b=<uri>@<bundle uri>")?;
            Ok(Box::new(pk_lv2::Lv2Plugin::load_bundle(uri, bundle)?))
        }
        "lv2s" => {
            let (uri, bundle) = id.split_once('@').ok_or("lv2s=<uri>@<bundle uri>")?;
            let world = worlds.entry(bundle.to_string()).or_insert_with(|| pk_lv2::bundle_world(bundle)).clone();
            Ok(Box::new(pk_lv2::Lv2Plugin::load_in(world, uri)?))
        }
        _ => Err("format".into()),
    }
}

/// Renders `blocks` blocks and returns each block's time. Instruments hold a chord, effects get a sine.
fn render(p: &mut dyn Plugin, blocks: usize, chord_on: bool, chord_off: bool, phase: &mut f32) -> Vec<u64> {
    let instrument = p.info().kind == Kind::Instrument;
    let mut times = Vec::with_capacity(blocks);
    let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
    let mut input = vec![0f32; BLOCK];
    for b in 0..blocks {
        let mut midi = Vec::new();
        if instrument && b == 0 && chord_on { for k in [60u8, 64, 67] { midi.push(MidiEvent { frame: 0, data: [0x90, k, 100] }); } }
        if instrument && b == blocks - 1 && chord_off { for k in [60u8, 64, 67] { midi.push(MidiEvent { frame: 0, data: [0x80, k, 0] }); } }
        if !instrument {
            for x in input.iter_mut() { *x = 0.3 * phase.sin(); *phase += 220.0 * std::f32::consts::TAU / SR as f32; }
        }
        let t = now_ns();
        p.process(&[&input, &input], &mut [&mut l, &mut r], &midi);
        times.push(now_ns() - t);
    }
    times
}

fn us(v: &mut [u64]) -> Value {
    let s = stats(v);
    json!({ "p50": s.0 as f64 / 1000.0, "p99": s.1 as f64 / 1000.0, "max": s.3 as f64 / 1000.0 })
}

fn main() {
    let t_main = now_ns();
    die_with_parent();
    let a: Vec<String> = std::env::args().collect();
    let shm = Shm::attach(a[2].parse().unwrap(), a[3].parse().unwrap());
    let base = a[4].parse::<usize>().unwrap() * MEM_STRIDE;
    shm.u64_at(base + MEM_T_MAIN).store(t_main, SeqCst);
    let window = a.iter().any(|x| x == "--window");
    let state_iters: usize = a.iter().position(|x| x == "--state-iters").and_then(|i| a.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(0);
    let specs: Vec<&String> = a.iter().skip(5).filter(|s| s.contains('=')).collect();

    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();
    let mut out = Vec::new();
    let mut failed = false;
    let mut worlds = Worlds::new();
    for spec in &specs {
        let t0 = now_ns();
        let mut p = match load(spec, &mut worlds) { Ok(p) => p, Err(e) => { out.push(json!({ "spec": spec, "error": e })); failed = true; continue } };
        let t1 = now_ns();
        if let Err(e) = p.activate(SR, BLOCK) { out.push(json!({ "spec": spec, "error": e })); failed = true; continue }
        let t2 = now_ns();
        let mut phase = 0f32;
        let n = (SR as usize / BLOCK) / 2;
        let first = render(p.as_mut(), 1, true, false, &mut phase)[0];
        let mut held = render(p.as_mut(), n, false, true, &mut phase);
        let _ = render(p.as_mut(), n, false, false, &mut phase);
        let mut r = json!({ "spec": spec, "name": p.info().name, "loadMs": (t1 - t0) as f64 / 1e6, "activateMs": (t2 - t1) as f64 / 1e6,
                            "firstBlockUs": first as f64 / 1000.0, "blockUs": us(&mut held) });
        if state_iters > 0 {
            let (mut save, mut restore, mut after) = (Vec::new(), Vec::new(), Vec::new());
            let (mut bytes, mut errors) = (0, Vec::new());
            let _ = render(p.as_mut(), 1, true, false, &mut phase);
            for _ in 0..state_iters {
                let t = now_ns();
                let st = p.save_state();
                save.push(now_ns() - t);
                match st {
                    Err(e) => { errors.push(e); break }
                    Ok(st) => {
                        bytes = st.len();
                        let t = now_ns();
                        if let Err(e) = p.load_state(&st) { errors.push(e); }
                        restore.push(now_ns() - t);
                        after.push(render(p.as_mut(), 1, false, false, &mut phase)[0]);
                        let _ = render(p.as_mut(), 4, false, false, &mut phase);
                    }
                }
            }
            r["state"] = json!({ "iters": save.len(), "bytes": bytes, "saveUs": us(&mut save), "restoreUs": us(&mut restore),
                                 "firstBlockAfterRestoreUs": us(&mut after), "errors": errors });
        }
        out.push(r);
        plugins.push(p);
    }

    let mut win = None;
    let mut editor = Value::Null;
    if window && !plugins.is_empty() {
        let p = &mut plugins[0];
        if !p.info().uis.contains(&UiKind::X11Embed) {
            editor = json!({ "error": "no X11 editor" });
        } else {
            let t = now_ns();
            match pk_core::x11win::HostWindow::open(&p.info().name.clone(), 400, 300) {
                Err(e) => editor = json!({ "error": e }),
                Ok(mut w) => match p.open_editor(w.id) {
                    Err(e) => { editor = json!({ "error": e }); w.close(); }
                    Ok((cw, ch)) => {
                        if cw > 0 && ch > 0 { w.resize(cw, ch); }
                        let open_ms = (now_ns() - t) as f64 / 1e6;
                        let until = now_ns() + 3_000_000_000;
                        let mut phase = 0f32;
                        while now_ns() < until && w.pump() {
                            p.editor_idle();
                            w.fit_to_child();
                            let _ = render(p.as_mut(), 1, false, false, &mut phase);
                            std::thread::sleep(std::time::Duration::from_millis(16));
                        }
                        editor = json!({ "openMs": open_ms, "size": [cw, ch], "distinctColours": w.distinct_colours().ok() });
                        win = Some(w);
                    }
                },
            }
        }
        failed |= editor.get("error").is_some();
    }

    let report = json!({ "plugins": out, "editor": editor });
    mem_report(&shm, base, if failed { 2 } else { 1 }, report.to_string().as_bytes());
    let quit = shm.u32_at(base + MEM_QUIT);
    while quit.load(SeqCst) == 0 {
        if let Some(w) = win.as_mut() {
            w.pump();
            plugins[0].editor_idle();
            std::thread::sleep(std::time::Duration::from_millis(16));
        } else {
            futex_wait(quit, 0, 200_000_000);
        }
    }
    if let Some(w) = win {
        plugins[0].close_editor();
        w.close();
    }
    drop(plugins);
}
