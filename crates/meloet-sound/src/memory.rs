//! Giving the memory of samples that were let go back to the PC (docs/architecture.md 23-12).
//!
//! glibc's malloc keeps freed memory for its next use. Samples that were read on the reading
//! threads mostly lie in those threads' arenas, and memory freed in the middle of an arena
//! stays resident until something trims it: without this, closing a song with 670 MiB of
//! samples kept the process at its size.

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static FREED: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn note_freed(bytes: usize) {
    FREED.fetch_add(bytes, Relaxed);
}

/// Sample bytes freed since the last [`give_back`].
pub fn freed_bytes() -> usize {
    FREED.load(Relaxed)
}

/// Hands malloc's free memory back to the PC (`malloc_trim`, which walks every arena). Takes
/// a few to some tens of ms and holds each arena's lock in turn: on a thread of its own, never
/// on the audio thread.
pub fn give_back() {
    FREED.store(0, Relaxed);
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: malloc_trim takes no pointers and is safe to call from any thread.
    unsafe {
        libc::malloc_trim(0);
    }
}

/// What malloc holds, in bytes (`mallinfo2`): in use, free inside its arenas, and mapped
/// on its own (big blocks, in use).
#[derive(Debug, Clone, Copy)]
pub struct MallocCounts {
    pub in_use: usize,
    pub free: usize,
    pub mapped: usize,
}

/// `None` where malloc is not glibc's. Walks every arena: for measurements only.
pub fn malloc_counts() -> Option<MallocCounts> {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: mallinfo2 takes nothing and returns a plain struct.
        let info = unsafe { libc::mallinfo2() };
        Some(MallocCounts {
            in_use: info.uordblks + info.hblkhd,
            free: info.fordblks,
            mapped: info.hblkhd,
        })
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    None
}
