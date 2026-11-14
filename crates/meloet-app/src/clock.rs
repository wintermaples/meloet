/// CLOCK_MONOTONIC in nanoseconds: the one clock every time in the protocol uses.
pub fn mono_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid timespec to write into; CLOCK_MONOTONIC always exists on Linux.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_MONOTONIC) failed");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub fn mono_us() -> u64 {
    mono_ns() / 1000
}
