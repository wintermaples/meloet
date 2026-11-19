//! Marks the stretches of the audio thread that run meloet's own code, so that the tests of
//! this crate can count what is allocated and freed there (what a plugin allocates inside its
//! own calls is its business). Outside tests both marks are nothing.

#[cfg(test)]
mod imp {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    thread_local! {
        // const-initialised and without a destructor, so touching them never allocates.
        static OURS: Cell<u32> = const { Cell::new(0) };
        static PLUGIN: Cell<u32> = const { Cell::new(0) };
    }
    static COUNT: AtomicU64 = AtomicU64::new(0);

    pub struct Audio(());

    pub fn enter() -> Audio {
        OURS.with(|d| d.set(d.get() + 1));
        Audio(())
    }

    impl Drop for Audio {
        fn drop(&mut self) {
            OURS.with(|d| d.set(d.get() - 1));
        }
    }

    pub fn plugin<R>(call: impl FnOnce() -> R) -> R {
        PLUGIN.with(|d| d.set(d.get() + 1));
        let result = call();
        PLUGIN.with(|d| d.set(d.get() - 1));
        result
    }

    fn counted() -> bool {
        OURS.try_with(|d| d.get() > 0).unwrap_or(false)
            && PLUGIN.try_with(|d| d.get() == 0).unwrap_or(false)
    }

    struct Counting;

    // SAFETY: every call is passed on to `System` unchanged.
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if counted() {
                COUNT.fetch_add(1, Relaxed);
            }
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            if counted() {
                COUNT.fetch_add(1, Relaxed);
            }
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            if counted() {
                COUNT.fetch_add(1, Relaxed);
            }
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static GLOBAL: Counting = Counting;

    /// Allocations, frees and reallocations made in meloet's own code inside [`enter`], on
    /// this thread or any other, since the process started.
    pub fn count() -> u64 {
        COUNT.load(Relaxed)
    }
}

#[cfg(not(test))]
mod imp {
    pub struct Audio(());

    #[inline(always)]
    pub fn enter() -> Audio {
        Audio(())
    }

    #[inline(always)]
    pub fn plugin<R>(call: impl FnOnce() -> R) -> R {
        call()
    }
}

#[cfg(test)]
pub use imp::count;
pub use imp::{enter, plugin};

#[cfg(test)]
mod tests {
    #[test]
    fn only_meloets_own_code_inside_an_audio_block_is_counted() {
        // Other tests of this binary count too, and all of them expect nothing: so this one
        // looks only at what its own marks add.
        let outside = std::hint::black_box(vec![1u8; 64]);
        drop(outside);
        let audio = super::enter();
        let before = super::count();
        super::plugin(|| drop(std::hint::black_box(vec![1u8; 64])));
        assert_eq!(super::count(), before);
        drop(audio);
    }
}
