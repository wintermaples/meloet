//! The two ends of a slot: the engine's (meloet's audio thread) and the plugin's (the host
//! process's audio thread).

use std::sync::Arc;
use std::sync::atomic::Ordering::{Acquire, Relaxed, SeqCst};

use super::ring::{Place, Reader, Writer};
use super::{FromPlugin, Mapping, RawEvent, ToPlugin, Transport, futex};

fn to_plugin_place(map: &Mapping, base: usize) -> Place<'_> {
    let words = map.slot_words(base);
    let geometry = map.geometry();
    Place {
        head: &words.to_plugin_head,
        tail: &words.to_plugin_tail,
        records: map
            .slot_ptr(base, geometry.ring_to_plugin_at())
            .cast::<RawEvent>(),
        capacity: geometry.to_plugin_events,
    }
}

fn from_plugin_place(map: &Mapping, base: usize) -> Place<'_> {
    let words = map.slot_words(base);
    let geometry = map.geometry();
    Place {
        head: &words.from_plugin_head,
        tail: &words.from_plugin_tail,
        records: map
            .slot_ptr(base, geometry.ring_from_plugin_at())
            .cast::<RawEvent>(),
        capacity: geometry.from_plugin_events,
    }
}

/// Copies `src` into a channel of the slot's input or output; what does not fit is left out.
fn write_channel(map: &Mapping, base: usize, area: usize, channel: usize, src: &[f32]) {
    let geometry = map.geometry();
    if channel >= geometry.channels as usize {
        return;
    }
    let frames = src.len().min(geometry.max_frames as usize);
    let at = area + channel * geometry.max_frames as usize * 4;
    // SAFETY: the channel's `max_frames` f32 lie inside the slot (4-aligned: the slot is
    // 64-aligned and `area` is a multiple of 4), `frames` is no more than that, and `src` is
    // this process's own memory.
    unsafe {
        std::ptr::copy_nonoverlapping(src.as_ptr(), map.slot_ptr(base, at).cast::<f32>(), frames);
    }
}

/// Copies a channel of the slot's input or output into `dst`; the rest of `dst` is zeroed.
fn read_channel(map: &Mapping, base: usize, area: usize, channel: usize, dst: &mut [f32]) {
    let geometry = map.geometry();
    if channel >= geometry.channels as usize {
        dst.fill(0.0);
        return;
    }
    let frames = dst.len().min(geometry.max_frames as usize);
    let at = area + channel * geometry.max_frames as usize * 4;
    // SAFETY: as in `write_channel`. Any bit pattern is a valid f32.
    unsafe {
        std::ptr::copy_nonoverlapping(
            map.slot_ptr(base, at).cast::<f32>().cast_const(),
            dst.as_mut_ptr(),
            frames,
        );
    }
    dst[frames..].fill(0.0);
}

/// The engine's end of a slot. Made on the control side before the plugin is loaded into the
/// slot; everything else here is for the audio thread and neither allocates nor locks. The one
/// call that sleeps is [`EngineEnd::wait`], on a futex, up to the deadline it is given.
///
/// One block:
/// `begin` → `write_input` / `push_event` / `set_transport` → `submit` → `wait` →
/// `read_output` / `pop_event`.
///
/// When `wait` times out, the block has no output. If the answer is still missing at the next
/// `begin`, that is two blocks in a row without one: the slot is stopped and is never waited
/// for again ([`EngineEnd::stopped`]).
pub struct EngineEnd {
    map: Arc<Mapping>,
    slot: u32,
    base: usize,
    seq: u32,
    frames: u32,
    flags: u32,
    /// `seq` was asked for and has not been seen in `done`.
    pending: bool,
    stopped: bool,
    to_plugin: Writer,
    from_plugin: Reader,
}

impl EngineEnd {
    /// Takes the slot over and empties it. Nothing of the host process may be using the slot:
    /// call this before the plugin is loaded into it, and let the end go after it is unloaded.
    pub fn new(map: Arc<Mapping>, slot: u32) -> Result<EngineEnd, super::IpcError> {
        let base = map.slot_base(slot)?;
        let words = map.slot_words(base);
        for word in [
            &words.request,
            &words.done,
            &words.engine_waiting,
            &words.frames,
            &words.flags,
            &words.to_plugin_head,
            &words.to_plugin_tail,
            &words.from_plugin_head,
            &words.from_plugin_tail,
        ] {
            word.store(0, SeqCst);
        }
        Ok(EngineEnd {
            map,
            slot,
            base,
            seq: 0,
            frames: 0,
            flags: 0,
            pending: false,
            stopped: false,
            to_plugin: Writer::default(),
            from_plugin: Reader::default(),
        })
    }

    pub fn slot(&self) -> u32 {
        self.slot
    }

    pub fn max_frames(&self) -> usize {
        self.map.geometry().max_frames as usize
    }

    /// Starts a block of `frames` frames (brought into 1..=max_frames). False when the slot is
    /// stopped: do nothing more with it this block.
    pub fn begin(&mut self, frames: usize) -> bool {
        if self.stopped {
            return false;
        }
        if self.pending {
            if self.map.slot_words(self.base).done.load(Acquire) != self.seq {
                self.stopped = true;
                return false;
            }
            // The answer came after its block was over. Its output is not used.
            self.pending = false;
        }
        self.frames = frames.clamp(1, self.max_frames()) as u32;
        true
    }

    /// True once two blocks in a row went without an answer. It stays true.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn write_input(&mut self, channel: usize, samples: &[f32]) {
        let area = self.map.geometry().input_at();
        let n = samples.len().min(self.frames as usize);
        write_channel(&self.map, self.base, area, channel, &samples[..n]);
    }

    pub fn set_transport(&mut self, transport: &Transport) {
        let to = self
            .map
            .slot_ptr(self.base, super::SLOT_WORDS_LEN)
            .cast::<Transport>();
        // SAFETY: the transport's 64 bytes lie inside the slot, 64-aligned. The plugin's end
        // reads them only after `submit`.
        unsafe { to.write_volatile(*transport) };
    }

    /// [`super::block_flags`], for the block being prepared.
    pub fn set_flags(&mut self, flags: u32) {
        self.flags |= flags;
    }

    /// False when the ring is full; the event is lost.
    pub fn push_event(&mut self, event: ToPlugin) -> bool {
        self.to_plugin
            .push(to_plugin_place(&self.map, self.base), event.encode())
    }

    /// Hands the block to the host process and wakes its audio thread if it sleeps.
    pub fn submit(&mut self) {
        if self.stopped || self.pending {
            return;
        }
        let words = self.map.slot_words(self.base);
        // 0 is "nothing asked yet".
        self.seq = self.seq.wrapping_add(1).max(1);
        words.frames.store(self.frames, Relaxed);
        words.flags.store(std::mem::take(&mut self.flags), Relaxed);
        words.request.store(self.seq, SeqCst);
        self.pending = true;
        let header = self.map.header();
        header.bell.fetch_add(1, SeqCst);
        if header.plugin_waiting.load(SeqCst) != 0 {
            futex::wake(&header.bell, 1);
        }
    }

    /// Sleeps until the block is answered or `deadline_ns` ([`futex::monotonic_ns`]). True
    /// when the output is there to read.
    pub fn wait(&mut self, deadline_ns: u64) -> bool {
        if self.stopped || !self.pending {
            return false;
        }
        let words = self.map.slot_words(self.base);
        let seq = self.seq;
        let answered = futex::wait_until(&words.done, &words.engine_waiting, deadline_ns, |done| {
            done == seq
        });
        if answered {
            self.pending = false;
        }
        answered
    }

    /// The answered block's output; frames past the block's length are zeroed.
    pub fn read_output(&self, channel: usize, samples: &mut [f32]) {
        let area = self.map.geometry().output_at();
        let n = samples.len().min(self.frames as usize);
        read_channel(&self.map, self.base, area, channel, &mut samples[..n]);
        samples[n..].fill(0.0);
    }

    /// How often the plugin's ring was at a position no ring can be at and was emptied.
    pub fn garbled(&self) -> u32 {
        self.from_plugin.dropped()
    }

    /// What the plugin said since the last call. Records that are not events are passed over.
    pub fn pop_event(&mut self) -> Option<FromPlugin> {
        let place = from_plugin_place(&self.map, self.base);
        for _ in 0..place.capacity {
            match FromPlugin::decode(self.from_plugin.pop(place)?) {
                Some(event) => return Some(event),
                None => continue,
            }
        }
        None
    }
}

/// One block as the plugin's end sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    /// 1..=max_frames.
    pub frames: usize,
    /// [`super::block_flags`].
    pub flags: u32,
}

/// The plugin's end of a slot, on the host process's audio thread. Made when a plugin is
/// loaded into the slot (after the engine's end exists).
///
/// One block: `take_request` → `read_input` / `pop_event` / `transport` → the plugin runs →
/// `write_output` / `push_event` → `finish`.
pub struct PluginEnd {
    map: Arc<Mapping>,
    slot: u32,
    base: usize,
    /// The request answered last.
    answered: u32,
    current: u32,
    frames: u32,
    to_plugin: Reader,
    from_plugin: Writer,
}

impl PluginEnd {
    pub fn new(map: Arc<Mapping>, slot: u32) -> Result<PluginEnd, super::IpcError> {
        let base = map.slot_base(slot)?;
        let words = map.slot_words(base);
        let answered = words.done.load(SeqCst);
        Ok(PluginEnd {
            to_plugin: Reader::at(words.to_plugin_tail.load(SeqCst)),
            from_plugin: Writer::at(words.from_plugin_head.load(SeqCst)),
            map,
            slot,
            base,
            answered,
            current: answered,
            frames: 0,
        })
    }

    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// The block the engine asked for, if it asked for one since the last `finish`.
    pub fn take_request(&mut self) -> Option<Block> {
        let words = self.map.slot_words(self.base);
        let request = words.request.load(SeqCst);
        if request == self.answered {
            return None;
        }
        self.current = request;
        let max = self.map.geometry().max_frames;
        self.frames = words.frames.load(Relaxed).clamp(1, max);
        Some(Block {
            frames: self.frames as usize,
            flags: words.flags.load(Relaxed),
        })
    }

    /// The block's input; frames past the block's length are zeroed.
    pub fn read_input(&self, channel: usize, samples: &mut [f32]) {
        let area = self.map.geometry().input_at();
        let n = samples.len().min(self.frames as usize);
        read_channel(&self.map, self.base, area, channel, &mut samples[..n]);
        samples[n..].fill(0.0);
    }

    pub fn transport(&self) -> Transport {
        let from = self
            .map
            .slot_ptr(self.base, super::SLOT_WORDS_LEN)
            .cast::<Transport>();
        // SAFETY: inside the slot and aligned; every bit pattern is a valid `Transport`.
        unsafe { from.read_volatile() }
    }

    /// The next event of the block, its frame inside the block.
    pub fn pop_event(&mut self) -> Option<ToPlugin> {
        let place = to_plugin_place(&self.map, self.base);
        for _ in 0..place.capacity {
            match ToPlugin::decode(self.to_plugin.pop(place)?, self.frames) {
                Some(event) => return Some(event),
                None => continue,
            }
        }
        None
    }

    pub fn write_output(&mut self, channel: usize, samples: &[f32]) {
        let area = self.map.geometry().output_at();
        let n = samples.len().min(self.frames as usize);
        write_channel(&self.map, self.base, area, channel, &samples[..n]);
    }

    /// False when the ring is full; the event is lost.
    pub fn push_event(&mut self, event: FromPlugin) -> bool {
        self.from_plugin
            .push(from_plugin_place(&self.map, self.base), event.encode())
    }

    /// Says the block is done and wakes the engine if it sleeps.
    pub fn finish(&mut self) {
        self.answered = self.current;
        let words = self.map.slot_words(self.base);
        futex::store_and_wake(&words.done, self.answered, &words.engine_waiting);
    }
}

/// What the host process's audio thread sleeps on: the engine rings it once for every block
/// it asks of any slot.
pub struct Bell {
    map: Arc<Mapping>,
}

impl Bell {
    pub fn new(map: Arc<Mapping>) -> Bell {
        Bell { map }
    }

    /// Read this before looking at the slots, and hand it to [`Bell::wait`] after.
    pub fn count(&self) -> u32 {
        self.map.header().bell.load(SeqCst)
    }

    /// Sleeps until the bell is rung after `seen` was read, for `timeout_ns` at most.
    pub fn wait(&self, seen: u32, timeout_ns: u64) {
        let header = self.map.header();
        let deadline = futex::monotonic_ns().saturating_add(timeout_ns);
        futex::wait_until(&header.bell, &header.plugin_waiting, deadline, |count| {
            count != seen
        });
    }

    /// Rings it without a block: another thread of the host process has something for the
    /// audio thread (a plugin to put into a slot).
    pub fn ring(&self) {
        let header = self.map.header();
        header.bell.fetch_add(1, SeqCst);
        if header.plugin_waiting.load(SeqCst) != 0 {
            futex::wake(&header.bell, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::geometry;
    use super::super::{MAX_LATENCY_FRAMES, block_flags};
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    const SOON: u64 = 2_000_000_000;

    fn deadline() -> u64 {
        futex::monotonic_ns() + SOON
    }

    /// A plugin's end on another thread: doubles the input, echoes each note as a parameter.
    fn doubler(
        map: Arc<Mapping>,
        slot: u32,
        quit: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<u32> {
        std::thread::spawn(move || {
            let bell = Bell::new(map.clone());
            let mut end = PluginEnd::new(map, slot).unwrap();
            let mut buffer = [0.0f32; 256];
            let mut blocks = 0;
            while !quit.load(SeqCst) {
                let seen = bell.count();
                let Some(block) = end.take_request() else {
                    bell.wait(seen, 50_000_000);
                    continue;
                };
                for channel in 0..2 {
                    end.read_input(channel, &mut buffer[..block.frames]);
                    for x in &mut buffer[..block.frames] {
                        *x *= 2.0;
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
                if block.flags & block_flags::RESET != 0 {
                    end.push_event(FromPlugin::Latency {
                        frames: end.transport().meter_num.into(),
                    });
                }
                blocks += 1;
                end.finish();
            }
            blocks
        })
    }

    #[test]
    fn blocks_go_round_between_two_threads() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let mut engine = EngineEnd::new(map.clone(), 1).unwrap();
        let quit = Arc::new(AtomicBool::new(false));
        let plugin = doubler(map.clone(), 1, quit.clone());

        let mut out = [0.0f32; 256];
        for block in 0..500u32 {
            let frames = 1 + (block as usize * 37) % 256;
            assert!(engine.begin(frames));
            let input: Vec<f32> = (0..frames)
                .map(|i| block as f32 + i as f32 * 0.001)
                .collect();
            engine.write_input(0, &input);
            engine.write_input(1, &input);
            engine.push_event(ToPlugin::Midi {
                frame: 3.min(frames as u32 - 1),
                data: [0x90, 60, 100],
            });
            engine.submit();
            assert!(engine.wait(deadline()), "block {block}");
            for channel in 0..2 {
                out.fill(9.0);
                engine.read_output(channel, &mut out);
                for (i, x) in out.iter().enumerate() {
                    let expected = if i < frames { input[i] * 2.0 } else { 0.0 };
                    assert_eq!(*x, expected, "block {block} frame {i}");
                }
            }
            assert_eq!(
                engine.pop_event(),
                Some(FromPlugin::Param {
                    id: 3.min(frames as u32 - 1),
                    value: 60.0 / 127.0
                })
            );
            assert_eq!(engine.pop_event(), None);
        }

        // The transport and the flags of a block arrive with it.
        assert!(engine.begin(16));
        engine.set_transport(&Transport {
            meter_num: 7,
            ..Transport::default()
        });
        engine.set_flags(block_flags::RESET);
        engine.submit();
        assert!(engine.wait(deadline()));
        assert_eq!(engine.pop_event(), Some(FromPlugin::Latency { frames: 7 }));
        // The flags are for one block only.
        assert!(engine.begin(16));
        engine.submit();
        assert!(engine.wait(deadline()));
        assert_eq!(engine.pop_event(), None);

        quit.store(true, SeqCst);
        Bell::new(map).ring();
        assert_eq!(plugin.join().unwrap(), 502);
        assert!(!engine.stopped());
    }

    #[test]
    fn two_blocks_without_an_answer_stop_the_slot() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let mut engine = EngineEnd::new(map.clone(), 0).unwrap();
        // Nobody is at the plugin's end.
        assert!(engine.begin(64));
        engine.submit();
        let began = futex::monotonic_ns();
        assert!(!engine.wait(began + 5_000_000));
        assert!(futex::monotonic_ns() - began >= 5_000_000);
        assert!(!engine.stopped());
        assert!(!engine.begin(64));
        assert!(engine.stopped());
        // From here on it neither asks nor waits.
        let request = map.slot_words(engine.base).request.load(SeqCst);
        engine.submit();
        assert_eq!(map.slot_words(engine.base).request.load(SeqCst), request);
        let began = futex::monotonic_ns();
        assert!(!engine.wait(began + SOON));
        assert!(futex::monotonic_ns() - began < SOON / 2);
        assert!(!engine.begin(64));
    }

    #[test]
    fn a_late_answer_keeps_the_slot_going() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let mut engine = EngineEnd::new(map.clone(), 0).unwrap();
        let mut plugin = PluginEnd::new(map, 0).unwrap();
        assert!(engine.begin(8));
        engine.write_input(0, &[1.0; 8]);
        engine.submit();
        assert!(!engine.wait(futex::monotonic_ns() + 1_000_000));
        // The answer comes after the block's deadline, before the next block.
        assert_eq!(
            plugin.take_request(),
            Some(Block {
                frames: 8,
                flags: 0
            })
        );
        plugin.write_output(0, &[5.0; 8]);
        plugin.finish();
        assert_eq!(plugin.take_request(), None);
        assert!(engine.begin(8));
        assert!(!engine.stopped());
        engine.submit();
        assert_eq!(
            plugin.take_request(),
            Some(Block {
                frames: 8,
                flags: 0
            })
        );
        plugin.finish();
        assert!(engine.wait(deadline()));
    }

    #[test]
    fn nonsense_from_the_plugin_side_stays_in_range() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let mut engine = EngineEnd::new(map.clone(), 2).unwrap();
        let words = map.slot_words(engine.base);
        assert!(engine.begin(64));
        engine.submit();
        // The other process scribbles over every word it can reach and says it is done.
        words.frames.store(u32::MAX, SeqCst);
        words.from_plugin_head.store(0xdead_beef, SeqCst);
        words.to_plugin_tail.store(0x1234_5678, SeqCst);
        map.header().max_frames.store(u32::MAX, SeqCst);
        map.header().slot_stride.store(u32::MAX, SeqCst);
        words.done.store(engine.seq, SeqCst);
        assert!(engine.wait(deadline()));
        let mut out = [1.0f32; 300];
        engine.read_output(0, &mut out);
        engine.read_output(7, &mut out);
        engine.read_output(99, &mut out);
        assert!(out.iter().all(|x| *x == 0.0));
        assert_eq!(engine.pop_event(), None);
        assert_eq!(engine.max_frames(), 256);
        // A full ring by its own count: events are refused, nothing is written out of place.
        assert!(engine.begin(64));
        assert!(!engine.push_event(ToPlugin::Midi {
            frame: 0,
            data: [0x90, 1, 1]
        }));

        // Records that are not events are passed over; a latency out of range is cut.
        words
            .from_plugin_head
            .store(engine.published_tail(), SeqCst);
        let mut plugin = PluginEnd::new(map.clone(), 2).unwrap();
        let place = from_plugin_place(&map, engine.base);
        plugin.from_plugin.push(
            place,
            RawEvent {
                kind: 99,
                ..RawEvent::default()
            },
        );
        plugin.from_plugin.push(
            place,
            RawEvent {
                kind: super::super::KIND_PARAM,
                value: f64::NAN,
                ..RawEvent::default()
            },
        );
        plugin.push_event(FromPlugin::Latency { frames: u32::MAX });
        assert_eq!(
            engine.pop_event(),
            Some(FromPlugin::Latency {
                frames: MAX_LATENCY_FRAMES
            })
        );
        assert_eq!(engine.pop_event(), None);
    }

    impl EngineEnd {
        fn published_tail(&self) -> u32 {
            self.map.slot_words(self.base).from_plugin_tail.load(SeqCst)
        }
    }

    #[test]
    fn a_new_engine_end_empties_the_slot() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let mut first = EngineEnd::new(map.clone(), 0).unwrap();
        let mut plugin = PluginEnd::new(map.clone(), 0).unwrap();
        for _ in 0..3 {
            assert!(first.begin(4));
            first.push_event(ToPlugin::Param {
                frame: 0,
                id: 1,
                value: 0.5,
            });
            first.submit();
            plugin.take_request().unwrap();
            plugin.finish();
            assert!(first.wait(deadline()));
        }
        drop((first, plugin));
        let mut second = EngineEnd::new(map.clone(), 0).unwrap();
        let mut plugin = PluginEnd::new(map, 0).unwrap();
        assert_eq!(plugin.take_request(), None);
        assert_eq!(plugin.pop_event(), None);
        assert!(second.begin(4));
        second.submit();
        assert_eq!(
            plugin.take_request(),
            Some(Block {
                frames: 4,
                flags: 0
            })
        );
        assert!(matches!(
            EngineEnd::new(Arc::new(Mapping::create(geometry()).unwrap()), 3),
            Err(super::super::IpcError::NoSuchSlot(3))
        ));
    }

    #[test]
    fn the_bell_wakes_a_sleeping_thread_and_times_out_alone() {
        let map = Arc::new(Mapping::create(geometry()).unwrap());
        let bell = Bell::new(map.clone());
        let seen = bell.count();
        let began = futex::monotonic_ns();
        bell.wait(seen, 10_000_000);
        assert!(futex::monotonic_ns() - began >= 10_000_000);

        let theirs = map.clone();
        let ringer = std::thread::spawn(move || {
            while theirs.header().plugin_waiting.load(SeqCst) == 0 {
                std::thread::sleep(Duration::from_millis(1));
            }
            Bell::new(theirs).ring();
        });
        let began = futex::monotonic_ns();
        bell.wait(seen, 10 * SOON);
        assert!(futex::monotonic_ns() - began < SOON);
        assert_ne!(bell.count(), seen);
        ringer.join().unwrap();
    }
}
