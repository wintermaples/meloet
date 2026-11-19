//! Blocks going round with the plugin's end in another process (a forked child that maps the
//! same memory), and what the engine's end sees when that process dies.

use std::sync::Arc;

use meloet_plugin::ipc::futex::monotonic_ns;
use meloet_plugin::ipc::{Bell, EngineEnd, FromPlugin, Geometry, Mapping, PluginEnd, ToPlugin};

const GEOMETRY: Geometry = Geometry {
    max_frames: 256,
    channels: 2,
    slots: 2,
    to_plugin_events: 64,
    from_plugin_events: 64,
};
const SLOT: u32 = 1;

/// Kills the child when the test ends, however it ends.
struct Child(libc::pid_t);

impl Child {
    /// How the child ended: `Ok(exit code)` or `Err(signal)`.
    fn wait(&mut self) -> Result<i32, i32> {
        let mut status = 0;
        // SAFETY: a plain call on this test's own child.
        let got = unsafe { libc::waitpid(self.0, &mut status, 0) };
        assert_eq!(got, self.0);
        self.0 = 0;
        if libc::WIFEXITED(status) {
            Ok(libc::WEXITSTATUS(status))
        } else {
            Err(libc::WTERMSIG(status))
        }
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        if self.0 > 0 {
            // SAFETY: plain calls on this test's own child.
            unsafe {
                libc::kill(self.0, libc::SIGKILL);
                libc::waitpid(self.0, std::ptr::null_mut(), 0);
            }
        }
    }
}

/// The child: answers `blocks` blocks (each output frame is half the input plus the block's
/// frame count; each note comes back as a parameter) and exits with 0. Between the fork and
/// the exit it allocates nothing: the test process has other threads, and one of them may have
/// held the allocator's lock at the fork.
fn serve(map: &Arc<Mapping>, blocks: u32) -> ! {
    let bell = Bell::new(map.clone());
    let Ok(mut end) = PluginEnd::new(map.clone(), SLOT) else {
        // SAFETY: ends the child at once, without running the parent's exit handlers.
        unsafe { libc::_exit(3) }
    };
    let mut buffer = [0.0f32; 256];
    let mut served = 0;
    let give_up = monotonic_ns() + 20_000_000_000;
    while served < blocks && monotonic_ns() < give_up {
        let seen = bell.count();
        let Some(block) = end.take_request() else {
            bell.wait(seen, 100_000_000);
            continue;
        };
        for channel in 0..2 {
            end.read_input(channel, &mut buffer[..block.frames]);
            for x in &mut buffer[..block.frames] {
                *x = *x * 0.5 + block.frames as f32;
            }
            end.write_output(channel, &buffer[..block.frames]);
        }
        while let Some(event) = end.pop_event() {
            if let ToPlugin::Midi { frame, data } = event {
                end.push_event(FromPlugin::Param {
                    id: frame,
                    value: f64::from(data[1]) / 127.0,
                });
            }
        }
        served += 1;
        end.finish();
    }
    // SAFETY: as above.
    unsafe { libc::_exit(if served == blocks { 0 } else { 4 }) }
}

fn fork_child(map: &Arc<Mapping>, blocks: u32) -> Child {
    // SAFETY: the child only runs `serve`, which keeps to system calls and its own stack.
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0);
    if pid == 0 {
        serve(map, blocks);
    }
    Child(pid)
}

fn one_block(engine: &mut EngineEnd, block: u32, frames: usize) {
    assert!(engine.begin(frames), "block {block}");
    let input: Vec<f32> = (0..frames)
        .map(|i| (block as usize * 7 + i) as f32)
        .collect();
    engine.write_input(0, &input);
    engine.write_input(1, &input);
    assert!(engine.push_event(ToPlugin::Midi {
        frame: 0,
        data: [0x90, (block % 128) as u8, 100],
    }));
    engine.submit();
    assert!(engine.wait(monotonic_ns() + 5_000_000_000), "block {block}");
    let mut out = vec![0.0f32; frames];
    for channel in 0..2 {
        engine.read_output(channel, &mut out);
        for (i, x) in out.iter().enumerate() {
            assert_eq!(
                *x,
                input[i] * 0.5 + frames as f32,
                "block {block} frame {i}"
            );
        }
    }
    let note = FromPlugin::Param {
        id: 0,
        value: f64::from(block % 128) / 127.0,
    };
    assert_eq!(engine.pop_event(), Some(note));
    assert_eq!(engine.pop_event(), None);
}

#[test]
fn blocks_go_round_between_two_processes() {
    const BLOCKS: u32 = 2000;
    let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
    let mut engine = EngineEnd::new(map.clone(), SLOT).unwrap();
    let mut child = fork_child(&map, BLOCKS);
    for block in 0..BLOCKS {
        one_block(&mut engine, block, 1 + (block as usize * 53) % 256);
    }
    assert_eq!(child.wait(), Ok(0));
    assert!(!engine.stopped());
}

#[test]
fn a_process_that_dies_stops_its_slot_at_the_second_block() {
    let map = Arc::new(Mapping::create(GEOMETRY).unwrap());
    let mut engine = EngineEnd::new(map.clone(), SLOT).unwrap();
    let mut child = fork_child(&map, 20);
    for block in 0..20 {
        one_block(&mut engine, block, 256);
    }
    assert_eq!(child.wait(), Ok(0));

    // Nobody answers any more. The first block gets no output …
    assert!(engine.begin(256));
    engine.submit();
    let began = monotonic_ns();
    assert!(!engine.wait(began + 4_000_000));
    let waited = monotonic_ns() - began;
    assert!((4_000_000..1_000_000_000).contains(&waited), "{waited}");
    assert!(!engine.stopped());
    // … and the second one waits for it once more, then the slot is stopped for good.
    assert!(!engine.begin(256));
    assert!(engine.overdue());
    let began = monotonic_ns();
    assert!(!engine.await_overdue(began + 4_000_000));
    assert!(engine.stopped());
    assert!(!engine.begin(256));
    assert!(!engine.wait(monotonic_ns() + 1_000_000_000));
    assert!(monotonic_ns() - began < 500_000_000);
}
