//! Sleeping on a word of the shared memory until the other process changes it.
//!
//! The futex is shared (no `FUTEX_PRIVATE_FLAG`): the two sides are different processes. Every
//! sleep has a time limit, as a futex does not say that the other side died.

use std::sync::atomic::{AtomicU32, Ordering::SeqCst};

/// CLOCK_MONOTONIC, the clock the futex's time limit runs on.
pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid, writable timespec.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// Woken by [`wake`]. The word may still hold the old value; look again.
    Woken,
    /// The word was not `expected` any more, so there was nothing to sleep for.
    NotEqual,
    TimedOut,
    Interrupted,
}

/// Sleeps while the word holds `expected`, for `timeout_ns` at most. Does not allocate.
pub fn wait(word: &AtomicU32, expected: u32, timeout_ns: u64) -> Wait {
    let ts = libc::timespec {
        tv_sec: (timeout_ns / 1_000_000_000) as libc::time_t,
        tv_nsec: (timeout_ns % 1_000_000_000) as libc::c_long,
    };
    // SAFETY: the word is a live, aligned u32 for the whole call and `ts` is a valid timespec.
    // FUTEX_WAIT reads the word and sleeps; it writes nothing.
    let r = unsafe {
        libc::syscall(
            libc::SYS_futex,
            word.as_ptr(),
            libc::FUTEX_WAIT,
            expected,
            &ts as *const libc::timespec,
            0usize,
            0u32,
        )
    };
    if r == 0 {
        return Wait::Woken;
    }
    // SAFETY: errno of this thread, set by the call above.
    match unsafe { *libc::__errno_location() } {
        libc::EAGAIN => Wait::NotEqual,
        libc::ETIMEDOUT => Wait::TimedOut,
        _ => Wait::Interrupted,
    }
}

/// Wakes up to `count` sleepers on the word; how many were woken.
pub fn wake(word: &AtomicU32, count: u32) -> usize {
    // SAFETY: the word is a live, aligned u32; FUTEX_WAKE only uses its address.
    let r = unsafe {
        libc::syscall(
            libc::SYS_futex,
            word.as_ptr(),
            libc::FUTEX_WAKE,
            count.min(i32::MAX as u32),
            0usize,
            0usize,
            0u32,
        )
    };
    r.max(0) as usize
}

/// Sleeps until `ready(word)` or the deadline ([`monotonic_ns`]); false at the deadline.
///
/// `sleeping` tells the other side that a wake is needed, so it can leave the system call out
/// when nobody sleeps. The order matters on both sides: the sleeper raises `sleeping` and then
/// looks at the word again; the waker ([`store_and_wake`]) stores the word and then looks at
/// `sleeping`. One of the two always sees the other.
pub fn wait_until(
    word: &AtomicU32,
    sleeping: &AtomicU32,
    deadline_ns: u64,
    mut ready: impl FnMut(u32) -> bool,
) -> bool {
    loop {
        let seen = word.load(SeqCst);
        if ready(seen) {
            return true;
        }
        let now = monotonic_ns();
        if now >= deadline_ns {
            return false;
        }
        sleeping.store(1, SeqCst);
        if word.load(SeqCst) == seen {
            wait(word, seen, deadline_ns - now);
        }
        sleeping.store(0, SeqCst);
    }
}

/// The other half of [`wait_until`].
pub fn store_and_wake(word: &AtomicU32, value: u32, sleeping: &AtomicU32) {
    word.store(value, SeqCst);
    if sleeping.load(SeqCst) != 0 {
        wake(word, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn a_wait_ends_at_its_time_limit() {
        let word = AtomicU32::new(7);
        let began = monotonic_ns();
        assert_eq!(wait(&word, 7, 20_000_000), Wait::TimedOut);
        let took = monotonic_ns() - began;
        assert!((20_000_000..2_000_000_000).contains(&took), "{took}");
    }

    #[test]
    fn a_wait_on_a_changed_word_returns_at_once() {
        let word = AtomicU32::new(8);
        assert_eq!(wait(&word, 7, 5_000_000_000), Wait::NotEqual);
    }

    #[test]
    fn a_sleeper_is_woken_by_another_thread() {
        let pair = Arc::new((AtomicU32::new(0), AtomicU32::new(0)));
        let other = pair.clone();
        let waker = std::thread::spawn(move || {
            // Until the sleeper really sleeps, so the wake is the thing that is tested.
            while other.1.load(SeqCst) == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
            std::thread::sleep(Duration::from_millis(20));
            store_and_wake(&other.0, 5, &other.1);
        });
        let began = monotonic_ns();
        assert!(wait_until(&pair.0, &pair.1, began + 10_000_000_000, |v| v == 5));
        assert!(monotonic_ns() - began < 5_000_000_000);
        assert_eq!(pair.1.load(SeqCst), 0);
        waker.join().unwrap();
    }

    #[test]
    fn wait_until_gives_up_at_the_deadline() {
        let (word, sleeping) = (AtomicU32::new(0), AtomicU32::new(0));
        let began = monotonic_ns();
        assert!(!wait_until(&word, &sleeping, began + 10_000_000, |v| v == 1));
        assert!(monotonic_ns() - began >= 10_000_000);
        // A deadline already past: no sleep at all, but a ready word still counts.
        assert!(!wait_until(&word, &sleeping, 0, |v| v == 1));
        assert!(wait_until(&word, &sleeping, 0, |v| v == 0));
    }

    #[test]
    fn no_wake_call_is_needed_when_nobody_sleeps() {
        let (word, sleeping) = (AtomicU32::new(0), AtomicU32::new(0));
        store_and_wake(&word, 3, &sleeping);
        assert_eq!(word.load(SeqCst), 3);
        assert_eq!(wake(&word, 1), 0);
    }
}
