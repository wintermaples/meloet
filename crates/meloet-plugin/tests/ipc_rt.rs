//! The calls the two audio threads make on a slot never allocate and never free: a counting
//! allocator watches the threads that mark themselves as audio threads. The same kind of watch
//! as in meloet-engine and meloet-sound, which this crate cannot use (they are above it).
//! One test only, so nothing else allocates on a marked thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{
    Bell, EngineEnd, FromPlugin, Geometry, Mapping, PluginEnd, ToPlugin, Transport, block_flags,
};

thread_local! {
    // const-initialised and without a destructor, so touching it never allocates.
    static AUDIO: Cell<bool> = const { Cell::new(false) };
}

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

fn on_audio_thread() -> bool {
    AUDIO.try_with(Cell::get).unwrap_or(false)
}

struct Counting;

// SAFETY: every call is passed on to `System` unchanged.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if on_audio_thread() {
            ALLOCS.fetch_add(1, Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if on_audio_thread() {
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if on_audio_thread() {
            ALLOCS.fetch_add(1, Relaxed);
            FREES.fetch_add(1, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counts() -> (u64, u64) {
    (ALLOCS.load(Relaxed), FREES.load(Relaxed))
}

const GEOMETRY: Geometry = Geometry {
    max_frames: 512,
    channels: 2,
    slots: 4,
    to_plugin_events: 16,
    from_plugin_events: 16,
};

#[test]
fn a_block_allocates_nothing_on_either_audio_thread() {
    // The watch itself works.
    AUDIO.set(true);
    drop(std::hint::black_box(vec![1u8; 64]));
    AUDIO.set(false);
    assert_eq!(counts(), (1, 1));

    let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
    let mut engine = EngineEnd::new(map.clone(), 2).unwrap();
    let quit = Arc::new(AtomicBool::new(false));
    let plugin = {
        let (map, quit) = (map.clone(), quit.clone());
        std::thread::spawn(move || {
            let bell = Bell::new(map.clone());
            let mut end = PluginEnd::new(map, 2).unwrap();
            let mut buffer = vec![0.0f32; 512];
            AUDIO.set(true);
            while !quit.load(Relaxed) {
                let seen = bell.count();
                let Some(block) = end.take_request() else {
                    bell.wait(seen, 20_000_000);
                    continue;
                };
                let _ = (end.transport(), block.flags);
                for channel in 0..2 {
                    end.read_input(channel, &mut buffer[..block.frames]);
                    end.write_output(channel, &buffer[..block.frames]);
                }
                while let Some(event) = end.pop_event() {
                    if let ToPlugin::Param { id, value, .. } = event {
                        // More than the ring holds some blocks: a full ring refuses, it does
                        // not grow.
                        end.push_event(FromPlugin::Param { id, value });
                    }
                }
                end.push_event(FromPlugin::Latency { frames: 64 });
                end.finish();
            }
            AUDIO.set(false);
        })
    };

    let input = vec![0.25f32; 512];
    let mut output = vec![0.0f32; 512];
    let transport = Transport {
        flags: Transport::PLAYING,
        tempo_bpm: 96.0,
        ..Transport::default()
    };
    AUDIO.set(true);
    for block in 0..3000u32 {
        let frames = 1 + (block as usize * 31) % 512;
        assert!(engine.begin(frames));
        engine.write_input(0, &input[..frames]);
        engine.write_input(1, &input[..frames]);
        engine.set_transport(&transport);
        if block % 100 == 0 {
            engine.set_flags(block_flags::RESET);
        }
        // Sometimes more events than the ring holds.
        for n in 0..(block % 24) {
            engine.push_event(ToPlugin::Midi {
                frame: n,
                data: [0x90, 60, 100],
            });
            engine.push_event(ToPlugin::Param {
                frame: n,
                id: n,
                value: 0.5,
            });
        }
        engine.submit();
        assert!(engine.wait(monotonic_ns() + 5_000_000_000));
        engine.read_output(0, &mut output[..frames]);
        engine.read_output(1, &mut output[..frames]);
        while engine.pop_event().is_some() {}
    }
    AUDIO.set(false);
    assert_eq!(output[0], 0.25);

    quit.store(true, Relaxed);
    Bell::new(map).ring();
    plugin.join().unwrap();

    // The path of a slot that stops: a wait that times out, and the block after it.
    AUDIO.set(true);
    assert!(engine.begin(64));
    engine.submit();
    assert!(!engine.wait(monotonic_ns() + 2_000_000));
    assert!(!engine.begin(64));
    assert!(engine.stopped());
    engine.submit();
    assert!(!engine.wait(monotonic_ns() + 2_000_000));
    AUDIO.set(false);

    assert_eq!(counts(), (1, 1));
}
