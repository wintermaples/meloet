//! Child of `pk-ipcbench mem --kind x11`: the empty host plus the X11 window pieces, and nothing else.
//!   pk-ipcx11 child <fd> <len> <index> [--window]
use std::sync::atomic::Ordering::SeqCst;

use pk_ipcbench::*;

fn main() {
    let t_main = now_ns();
    die_with_parent();
    let a: Vec<String> = std::env::args().collect();
    let shm = Shm::attach(a[2].parse().unwrap(), a[3].parse().unwrap());
    let base = a[4].parse::<usize>().unwrap() * MEM_STRIDE;
    shm.u64_at(base + MEM_T_MAIN).store(t_main, SeqCst);
    // Without --window the library is only loaded; the call below keeps it linked.
    let mut win = None;
    let mut note = "{}".to_string();
    if a.iter().any(|x| x == "--window") {
        match pk_core::x11win::HostWindow::open("pk-ipcx11", 200, 100) {
            Ok(w) => win = Some(w),
            Err(e) => note = format!("{{\"windowError\":{e:?}}}"),
        }
    }
    mem_report(&shm, base, 1, note.as_bytes());
    let quit = shm.u32_at(base + MEM_QUIT);
    while quit.load(SeqCst) == 0 {
        if let Some(w) = win.as_mut() { w.pump(); }
        futex_wait(quit, 0, 100_000_000);
    }
    if let Some(w) = win { w.close(); }
}
