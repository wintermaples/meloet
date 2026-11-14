//! Marker for "this thread is inside an audio block".
//!
//! With the `rt-check` feature (tests), [`enter`] raises a thread-local flag that a counting
//! global allocator can read through [`in_audio_block`]. Without it, both are no-ops.

#[cfg(feature = "rt-check")]
mod imp {
    use std::cell::Cell;

    thread_local! {
        // const-initialised and without a destructor, so touching it never allocates.
        static DEPTH: Cell<u32> = const { Cell::new(0) };
    }

    pub struct AudioBlock(());

    #[inline]
    pub fn enter() -> AudioBlock {
        DEPTH.with(|d| d.set(d.get() + 1));
        AudioBlock(())
    }

    impl Drop for AudioBlock {
        fn drop(&mut self) {
            DEPTH.with(|d| d.set(d.get() - 1));
        }
    }

    #[inline]
    pub fn in_audio_block() -> bool {
        DEPTH.try_with(|d| d.get() > 0).unwrap_or(false)
    }
}

#[cfg(not(feature = "rt-check"))]
mod imp {
    pub struct AudioBlock(());

    #[inline(always)]
    pub fn enter() -> AudioBlock {
        AudioBlock(())
    }

    #[inline(always)]
    pub fn in_audio_block() -> bool {
        false
    }
}

pub use imp::{AudioBlock, enter, in_audio_block};
