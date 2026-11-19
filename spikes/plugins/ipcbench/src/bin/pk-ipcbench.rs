//! pk-ipcbench: numbers for the out-of-process plugin host, before its design is fixed (round 5).
//!   pk-ipcbench rt [--wake futex|eventfd|spin] [--spin-us N] [--sched other|fifo|mixed] [--prio 20]
//!                  [--children 1] [--mode seq|fanout] [--iters 10000] [--load [N]] [--period-us 5333]
//!       round trips over shared memory between this process ("engine") and child processes ("plugin host")
//!   pk-ipcbench mem --kind idle|x11|host [--procs K] [--repeat N] [--window] [--state-iters N] [specs...]
//!       starts K children of one kind, waits until each says "ready", reads PSS from smaps_rollup
//!       specs (kind host): vst3=<bundle>#<classid> | lv2=<uri> | lv2b|lv2s=<uri>@<bundle uri>, repeat for more instances
//!   (children: rt-child, burn, idle-child)
//! Children never talk over stdout or stderr; everything goes through the shared memory.
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering::SeqCst;

use pk_ipcbench::*;
use serde_json::{Value, json};

const STRIDE: usize = 8192;
const REQ: usize = 0;
const CHILD_SLEEPING: usize = 4;
const DONE: usize = 64;
const PARENT_SLEEPING: usize = 68;
const READY: usize = 128;
const SCHED: usize = 132; // 0 SCHED_OTHER, 1 got SCHED_FIFO, 2 asked and refused
const QUIT: usize = 136;
const T_REQ: usize = 144;
const WAKE_LAT: usize = 152;
const INPUT: usize = 1024;
const OUTPUT: usize = 4096;
const SAMPLES: usize = BLOCK * 2;

fn opt<'a>(a: &'a [String], k: &str) -> Option<&'a str> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).map(String::as_str)
}
fn num<T: std::str::FromStr>(a: &[String], k: &str, d: T) -> T {
    opt(a, k).and_then(|s| s.parse().ok()).unwrap_or(d)
}

fn quiet(c: &mut Command) -> &mut Command {
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

fn eventfd_write(fd: i32) {
    let one = 1u64;
    unsafe { libc::write(fd, &one as *const u64 as *const _, 8) };
}
fn eventfd_read(fd: i32) {
    let mut v = 0u64;
    unsafe { libc::read(fd, &mut v as *mut u64 as *mut _, 8) };
}

fn rt_child(a: &[String]) {
    die_with_parent();
    let p = |i: usize| a[i].parse::<i64>().unwrap();
    let shm = Shm::attach(p(2) as i32, p(3) as usize);
    let base = p(4) as usize * STRIDE;
    let eventfd = a[5] == "eventfd";
    let (prio, efd_req, efd_done) = (p(6) as i32, p(7) as i32, p(8) as i32);
    lock_memory();
    let sched = if prio == 0 { 0 } else if set_fifo(prio).is_ok() { 1 } else { 2 };
    shm.u32_at(base + SCHED).store(sched, SeqCst);
    let (req, done) = (shm.u32_at(base + REQ), shm.u32_at(base + DONE));
    let mut local = [0f32; SAMPLES];
    let mut last = 0u32;
    shm.u32_at(base + READY).store(1, SeqCst);
    loop {
        if eventfd {
            eventfd_read(efd_req);
        } else {
            wait_change(req, last, shm.u32_at(base + CHILD_SLEEPING), now_ns() + 1_000_000_000);
        }
        let r = req.load(SeqCst);
        if shm.u32_at(base + QUIT).load(SeqCst) != 0 { return }
        if r == last { continue }
        shm.u64_at(base + WAKE_LAT).store(now_ns().saturating_sub(shm.u64_at(base + T_REQ).load(SeqCst)), SeqCst);
        unsafe {
            std::ptr::copy_nonoverlapping(shm.bytes_at(base + INPUT, SAMPLES * 4) as *const f32, local.as_mut_ptr(), SAMPLES);
            let mut prev = 0f32;
            for x in local.iter_mut() { let y = 0.5 * *x + 0.25 * prev; prev = *x; *x = y; }
            std::ptr::copy_nonoverlapping(local.as_ptr(), shm.bytes_at(base + OUTPUT, SAMPLES * 4) as *mut f32, SAMPLES);
        }
        last = r;
        if eventfd {
            done.store(r, SeqCst);
            eventfd_write(efd_done);
        } else {
            set_and_wake(done, r, shm.u32_at(base + PARENT_SLEEPING));
        }
    }
}

fn burn() {
    die_with_parent();
    let mut x = 1.0001f64;
    loop {
        for _ in 0..1_000_000 { x = (x * 1.000001).sin().abs() + 1.0; }
        std::hint::black_box(x);
    }
}

struct Killer(Vec<Child>);
impl Drop for Killer {
    fn drop(&mut self) {
        for c in &mut self.0 { let _ = c.kill(); let _ = c.wait(); }
    }
}

fn rt(a: &[String]) -> Value {
    let wake = opt(a, "--wake").unwrap_or("futex").to_string();
    let spin_ns = if wake == "spin" { num(a, "--spin-us", 50u64) * 1000 } else { 0 };
    let sched = opt(a, "--sched").unwrap_or("other").to_string();
    let prio: i32 = num(a, "--prio", 20);
    let children: usize = num(a, "--children", 1);
    let fanout = opt(a, "--mode") == Some("fanout");
    let iters: usize = num(a, "--iters", 10000);
    let period_ns = (num(a, "--period-us", BLOCK as f64 / SR * 1e6) * 1000.0) as u64;
    let load = a.iter().position(|x| x == "--load").map(|i| a.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or_else(|| unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) } as usize));
    let exe = std::env::current_exe().unwrap();
    let eventfd = wake == "eventfd";

    let shm = Shm::create(STRIDE * children);
    let mut burners = Killer(Vec::new());
    for _ in 0..load.unwrap_or(0) { burners.0.push(quiet(Command::new(&exe).arg("burn")).spawn().unwrap()); }
    let mut kids = Killer(Vec::new());
    let mut efds = Vec::new();
    for i in 0..children {
        let (er, ed) = if eventfd { unsafe { (libc::eventfd(0, 0), libc::eventfd(0, 0)) } } else { (-1, -1) };
        efds.push((er, ed));
        let child_prio = if sched == "fifo" { prio } else { 0 };
        kids.0.push(quiet(Command::new(&exe).args(["rt-child", &shm.fd.to_string(), &shm.len.to_string(), &i.to_string(),
            if eventfd { "eventfd" } else { "futex" }, &child_prio.to_string(), &er.to_string(), &ed.to_string()])).spawn().unwrap());
    }
    let t = now_ns();
    while (0..children).any(|i| shm.u32_at(i * STRIDE + READY).load(SeqCst) == 0) {
        if now_ns() - t > 5_000_000_000 { return json!({ "error": "children not ready" }) }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let child_sched: Vec<u32> = (0..children).map(|i| shm.u32_at(i * STRIDE + SCHED).load(SeqCst)).collect();

    let warm = 200usize;
    let mut total = Vec::with_capacity(iters);
    let mut wake_lat = Vec::with_capacity(iters);
    let mut timer_late = Vec::with_capacity(iters);
    let (mut timeouts, mut bad) = (0u64, 0u64);
    let locked = lock_memory();
    let parent_fifo = if sched == "other" { None } else { Some(set_fifo(prio)) };
    if load.is_some() { std::thread::sleep(std::time::Duration::from_secs(1)); }

    let send = |i: usize, seq: u32, t0: u64| {
        let base = i * STRIDE;
        let inp = shm.bytes_at(base + INPUT, SAMPLES * 4) as *mut f32;
        for k in 0..SAMPLES { unsafe { inp.add(k).write(seq as f32 + k as f32 * 1e-3) }; }
        shm.u64_at(base + T_REQ).store(t0, SeqCst);
        if eventfd {
            shm.u32_at(base + REQ).store(seq, SeqCst);
            eventfd_write(efds[i].0);
        } else {
            set_and_wake(shm.u32_at(base + REQ), seq, shm.u32_at(base + CHILD_SLEEPING));
        }
    };
    let wait = |i: usize, seq: u32, deadline: u64| -> bool {
        let done = shm.u32_at(i * STRIDE + DONE);
        if spin_ns > 0 {
            let end = now_ns() + spin_ns;
            loop {
                if done.load(SeqCst) == seq { return true }
                if now_ns() >= end { break }
                std::hint::spin_loop();
            }
        }
        loop {
            let cur = done.load(SeqCst);
            if cur == seq { return true }
            let now = now_ns();
            if now >= deadline { return false }
            if eventfd {
                let mut pfd = libc::pollfd { fd: efds[i].1, events: libc::POLLIN, revents: 0 };
                let ts = libc::timespec { tv_sec: 0, tv_nsec: (deadline - now).min(999_999_999) as _ };
                if unsafe { libc::ppoll(&mut pfd, 1, &ts, std::ptr::null()) } > 0 { eventfd_read(efds[i].1); }
            } else if !wait_change(done, cur, shm.u32_at(i * STRIDE + PARENT_SLEEPING), deadline) {
                return false;
            }
        }
    };

    let mut next = now_ns() + period_ns;
    let started = now_ns();
    for it in 0..iters + warm {
        sleep_until_ns(next);
        let t0 = now_ns();
        let late = t0.saturating_sub(next);
        next += period_ns;
        let seq = it as u32 + 1;
        let deadline = t0 + 100_000_000;
        let mut ok = true;
        if fanout {
            for i in 0..children { send(i, seq, t0); }
            for i in 0..children { ok &= wait(i, seq, deadline); }
        } else {
            for i in 0..children {
                send(i, seq, now_ns());
                ok &= wait(i, seq, deadline);
            }
        }
        let t1 = now_ns();
        if it < warm { continue }
        if !ok { timeouts += 1 }
        let mut worst = 0;
        for i in 0..children {
            let base = i * STRIDE;
            worst = worst.max(shm.u64_at(base + WAKE_LAT).load(SeqCst));
            let out0 = unsafe { (shm.bytes_at(base + OUTPUT, 4) as *const f32).read() };
            if ok && out0 != 0.5 * seq as f32 { bad += 1 }
        }
        total.push(t1 - t0);
        wake_lat.push(worst);
        timer_late.push(late);
    }
    let wall_s = (now_ns() - started) as f64 / 1e9;
    for (i, efd) in efds.iter().enumerate() {
        shm.u32_at(i * STRIDE + QUIT).store(1, SeqCst);
        if eventfd { eventfd_write(efd.0) } else { set_and_wake(shm.u32_at(i * STRIDE + REQ), u32::MAX, shm.u32_at(i * STRIDE + CHILD_SLEEPING)) }
    }
    drop(kids);
    drop(burners);

    let block_us = period_ns as f64 / 1000.0;
    let over_block = total.iter().filter(|&&x| x > period_ns).count();
    let over_1ms = total.iter().filter(|&&x| x > 1_000_000).count();
    let us = |s: (u64, u64, u64, u64)| json!({ "p50": s.0 as f64 / 1000.0, "p99": s.1 as f64 / 1000.0, "p999": s.2 as f64 / 1000.0, "max": s.3 as f64 / 1000.0 });
    let s = stats(&mut total);
    json!({
        "wake": wake, "spinUs": spin_ns / 1000, "sched": sched, "prio": prio, "children": children, "mode": if fanout { "fanout" } else { "seq" },
        "iters": iters, "load": load.unwrap_or(0), "blockUs": block_us, "wallS": wall_s,
        "parentFifo": parent_fifo.map(|r| r.err().unwrap_or_else(|| "ok".into())), "childSched": child_sched, "memLocked": locked,
        "roundTripUs": us(s),
        "pctOfBlock": { "p50": s.0 as f64 / 10.0 / block_us, "p99": s.1 as f64 / 10.0 / block_us, "p999": s.2 as f64 / 10.0 / block_us, "max": s.3 as f64 / 10.0 / block_us },
        "childWakeUs": us(stats(&mut wake_lat)), "timerLateUs": us(stats(&mut timer_late)),
        "over1ms": over_1ms, "overBlock": over_block, "timeouts100ms": timeouts, "badData": bad,
    })
}

fn idle_child(a: &[String]) {
    let t_main = now_ns();
    die_with_parent();
    let shm = Shm::attach(a[2].parse().unwrap(), a[3].parse().unwrap());
    let base = a[4].parse::<usize>().unwrap() * MEM_STRIDE;
    shm.u64_at(base + MEM_T_MAIN).store(t_main, SeqCst);
    mem_report(&shm, base, 1, b"{}");
    let quit = shm.u32_at(base + MEM_QUIT);
    while quit.load(SeqCst) == 0 { futex_wait(quit, 0, 1_000_000_000); }
}

fn smaps(pid: u32) -> Value {
    let s = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap_or_default();
    let kb = |k: &str| s.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let threads = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Threads:")).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()));
    json!({ "pssKb": kb("Pss:"), "pssAnonKb": kb("Pss_Anon:"), "pssFileKb": kb("Pss_File:"), "pssShmemKb": kb("Pss_Shmem:"), "rssKb": kb("Rss:"),
            "privateKb": kb("Private_Clean:") + kb("Private_Dirty:"), "threads": threads })
}

fn mem_once(a: &[String]) -> Value {
    let kind = opt(a, "--kind").unwrap_or("idle");
    let procs: usize = num(a, "--procs", 1);
    let exe = std::env::current_exe().unwrap();
    let sibling = |n: &str| exe.with_file_name(n);
    // Everything that is not one of our own options goes to the child as it is (plugin specs, --window, --state-iters N).
    let mut pass: Vec<&str> = Vec::new();
    let mut i = 2;
    while i < a.len() {
        match a[i].as_str() {
            "--kind" | "--procs" | "--repeat" | "--settle-ms" => i += 1,
            x => pass.push(x),
        }
        i += 1;
    }
    let shm = Shm::create(MEM_STRIDE * procs);
    let mut kids = Killer(Vec::new());
    let mut spawned = Vec::new();
    for i in 0..procs {
        let mut c = match kind {
            "idle" => { let mut c = Command::new(&exe); c.arg("idle-child"); c }
            "x11" => { let mut c = Command::new(sibling("pk-ipcx11")); c.arg("child"); c }
            _ => { let mut c = Command::new(sibling("pk-ipchost")); c.arg("child"); c }
        };
        c.args([shm.fd.to_string(), shm.len.to_string(), i.to_string()]).args(&pass).arg0(format!("pk-ipc-{kind}"));
        spawned.push(now_ns());
        kids.0.push(quiet(&mut c).spawn().unwrap());
    }
    let t = now_ns();
    for (i, kid) in kids.0.iter_mut().enumerate() {
        let phase = shm.u32_at(i * MEM_STRIDE + MEM_PHASE);
        while phase.load(SeqCst) == 0 {
            if now_ns() - t > 300_000_000_000 { return json!({ "error": "timeout waiting for ready" }) }
            if let Ok(Some(st)) = kid.try_wait() { return json!({ "error": format!("child {i} ended early: {st}") }) }
            futex_wait(phase, 0, 100_000_000);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(num(a, "--settle-ms", 500)));
    let mut per = Vec::new();
    let (mut pss, mut rss, mut private) = (0, 0, 0);
    for i in 0..procs {
        let base = i * MEM_STRIDE;
        let mut m = smaps(kids.0[i].id());
        pss += m["pssKb"].as_u64().unwrap_or(0);
        rss += m["rssKb"].as_u64().unwrap_or(0);
        private += m["privateKb"].as_u64().unwrap_or(0);
        let n = shm.u32_at(base + MEM_JSON_LEN).load(SeqCst) as usize;
        let child: Value = serde_json::from_slice(unsafe { std::slice::from_raw_parts(shm.bytes_at(base + MEM_JSON, n), n) }).unwrap_or(Value::Null);
        let ms = |off: usize| (shm.u64_at(base + off).load(SeqCst) as f64 - spawned[i] as f64) / 1e6;
        m["phase"] = json!(shm.u32_at(base + MEM_PHASE).load(SeqCst));
        m["execToMainMs"] = json!(ms(MEM_T_MAIN));
        m["execToReadyMs"] = json!(ms(MEM_T_READY));
        m["child"] = child;
        per.push(m);
    }
    for i in 0..procs {
        let quit = shm.u32_at(i * MEM_STRIDE + MEM_QUIT);
        quit.store(1, SeqCst);
        futex_wake(quit, i32::MAX);
    }
    let t = now_ns();
    let mut clean_exit = true;
    for c in &mut kids.0 {
        while c.try_wait().ok().flatten().is_none() {
            if now_ns() - t > 10_000_000_000 { clean_exit = false; break }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    json!({ "kind": kind, "procs": procs, "args": pass, "sumPssKb": pss, "sumRssKb": rss, "sumPrivateKb": private, "cleanExit": clean_exit, "perProc": per })
}

fn mem(a: &[String]) -> Value {
    let repeat: usize = num(a, "--repeat", 1);
    if repeat <= 1 { return mem_once(a) }
    let (mut to_main, mut to_ready, mut pss) = (Vec::new(), Vec::new(), Vec::new());
    let mut last = Value::Null;
    for _ in 0..repeat {
        last = mem_once(a);
        if last.get("error").is_some() { return last }
        for p in last["perProc"].as_array().unwrap() {
            to_main.push((p["execToMainMs"].as_f64().unwrap() * 1e6) as u64);
            to_ready.push((p["execToReadyMs"].as_f64().unwrap() * 1e6) as u64);
        }
        pss.push(last["sumPssKb"].as_u64().unwrap());
    }
    let ms = |s: (u64, u64, u64, u64)| json!({ "p50": s.0 as f64 / 1e6, "p99": s.1 as f64 / 1e6, "max": s.3 as f64 / 1e6 });
    let p = stats(&mut pss);
    json!({ "repeat": repeat, "execToMainMs": ms(stats(&mut to_main)), "execToReadyMs": ms(stats(&mut to_ready)),
            "sumPssKb": { "p50": p.0, "min": pss[0], "max": p.3 }, "last": last })
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let v = match a.get(1).map(String::as_str) {
        Some("rt") => rt(&a),
        Some("mem") => mem(&a),
        Some("rt-child") => return rt_child(&a),
        Some("burn") => return burn(),
        Some("idle-child") => return idle_child(&a),
        _ => { eprintln!("see the comment at the top of pk-ipcbench.rs"); std::process::exit(1) }
    };
    println!("{v}");
}
