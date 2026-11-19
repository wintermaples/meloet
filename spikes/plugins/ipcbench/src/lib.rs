//! Shared pieces of the round-5 measurements: a memfd mapping both sides see, futex calls on it,
//! and the small process helpers. Nothing here loads a plugin, so the light binaries stay light.
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::SeqCst};

pub const BLOCK: usize = 256;
pub const SR: f64 = 48000.0;

pub fn now_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub fn sleep_until_ns(t: u64) {
    let ts = libc::timespec { tv_sec: (t / 1_000_000_000) as _, tv_nsec: (t % 1_000_000_000) as _ };
    unsafe { while libc::clock_nanosleep(libc::CLOCK_MONOTONIC, libc::TIMER_ABSTIME, &ts, std::ptr::null_mut()) == libc::EINTR {} }
}

pub struct Shm {
    pub ptr: *mut u8,
    pub len: usize,
    pub fd: i32,
}
unsafe impl Send for Shm {}
unsafe impl Sync for Shm {}

impl Shm {
    /// The fd is left inheritable on purpose: the child gets its number on the command line.
    pub fn create(len: usize) -> Shm {
        unsafe {
            let fd = libc::memfd_create(c"pk-ipc".as_ptr(), libc::MFD_ALLOW_SEALING);
            assert!(fd >= 0, "memfd_create");
            assert_eq!(libc::ftruncate(fd, len as i64), 0);
            libc::fcntl(fd, libc::F_ADD_SEALS, libc::F_SEAL_SHRINK | libc::F_SEAL_GROW);
            Shm::attach(fd, len)
        }
    }
    pub fn attach(fd: i32, len: usize) -> Shm {
        let ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd, 0) };
        assert!(ptr != libc::MAP_FAILED, "mmap");
        Shm { ptr: ptr as *mut u8, len, fd }
    }
    pub fn u32_at(&self, off: usize) -> &AtomicU32 {
        assert!(off + 4 <= self.len && off.is_multiple_of(4));
        unsafe { &*(self.ptr.add(off) as *const AtomicU32) }
    }
    pub fn u64_at(&self, off: usize) -> &AtomicU64 {
        assert!(off + 8 <= self.len && off.is_multiple_of(8));
        unsafe { &*(self.ptr.add(off) as *const AtomicU64) }
    }
    pub fn bytes_at(&self, off: usize, n: usize) -> *mut u8 {
        assert!(off + n <= self.len);
        unsafe { self.ptr.add(off) }
    }
}

/// Shared futex (no FUTEX_PRIVATE_FLAG), relative timeout on CLOCK_MONOTONIC. Returns the errno, 0 on a wake.
pub fn futex_wait(word: &AtomicU32, expect: u32, timeout_ns: u64) -> i32 {
    let ts = libc::timespec { tv_sec: (timeout_ns / 1_000_000_000) as _, tv_nsec: (timeout_ns % 1_000_000_000) as _ };
    let r = unsafe { libc::syscall(libc::SYS_futex, word.as_ptr(), libc::FUTEX_WAIT, expect, &ts as *const libc::timespec, 0usize, 0u32) };
    if r == 0 { 0 } else { std::io::Error::last_os_error().raw_os_error().unwrap_or(-1) }
}

pub fn futex_wake(word: &AtomicU32, n: i32) {
    unsafe { libc::syscall(libc::SYS_futex, word.as_ptr(), libc::FUTEX_WAKE, n, 0usize, 0usize, 0u32) };
}

/// Waits until `word` is no longer `seen`, announcing the sleep in `sleeping` so the other side can skip the
/// wake call when nobody sleeps. False on timeout.
pub fn wait_change(word: &AtomicU32, seen: u32, sleeping: &AtomicU32, deadline_ns: u64) -> bool {
    loop {
        if word.load(SeqCst) != seen { return true }
        let now = now_ns();
        if now >= deadline_ns { return false }
        sleeping.store(1, SeqCst);
        if word.load(SeqCst) == seen { futex_wait(word, seen, deadline_ns - now); }
        sleeping.store(0, SeqCst);
    }
}

pub fn set_and_wake(word: &AtomicU32, value: u32, sleeping: &AtomicU32) {
    word.store(value, SeqCst);
    if sleeping.load(SeqCst) != 0 { futex_wake(word, 1); }
}

pub fn set_fifo(prio: i32) -> Result<(), String> {
    let p = libc::sched_param { sched_priority: prio };
    if unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO, &p) } == 0 { Ok(()) } else { Err(std::io::Error::last_os_error().to_string()) }
}

pub fn lock_memory() -> bool {
    unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) == 0 }
}

/// The measuring children must never outlive the parent. Call first thing in the child.
pub fn die_with_parent() {
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        if libc::getppid() == 1 { libc::_exit(0) }
    }
}

// One child's area in the memory test (`mem`), the same for every kind of child.
pub const MEM_STRIDE: usize = 256 * 1024;
pub const MEM_PHASE: usize = 0; // 0 starting, 1 ready, 2 failed
pub const MEM_QUIT: usize = 4;
pub const MEM_T_MAIN: usize = 8;
pub const MEM_T_READY: usize = 16;
pub const MEM_JSON_LEN: usize = 24;
pub const MEM_JSON: usize = 4096;

pub fn mem_report(shm: &Shm, base: usize, phase: u32, json: &[u8]) {
    let n = json.len().min(MEM_STRIDE - MEM_JSON);
    unsafe { std::ptr::copy_nonoverlapping(json.as_ptr(), shm.bytes_at(base + MEM_JSON, n), n) };
    shm.u32_at(base + MEM_JSON_LEN).store(n as u32, SeqCst);
    shm.u64_at(base + MEM_T_READY).store(now_ns(), SeqCst);
    shm.u32_at(base + MEM_PHASE).store(phase, SeqCst);
    futex_wake(shm.u32_at(base + MEM_PHASE), 1);
}

/// (p50, p99, p99.9, max) of an unsorted list.
pub fn stats(v: &mut [u64]) -> (u64, u64, u64, u64) {
    if v.is_empty() { return (0, 0, 0, 0) }
    v.sort_unstable();
    let at = |p: f64| v[(((v.len() as f64) * p).ceil() as usize).clamp(1, v.len()) - 1];
    (at(0.5), at(0.99), at(0.999), v[v.len() - 1])
}
