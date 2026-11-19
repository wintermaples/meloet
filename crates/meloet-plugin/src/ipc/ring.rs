//! A ring of fixed-size records in the shared memory: one side writes, the other reads.
//!
//! The two positions in the shared memory run freely (they are not wrapped to the capacity);
//! `head - tail` is how many records wait. Each side keeps its own position in its own memory
//! and only publishes it, so nothing the other process writes can move it. The position read
//! from the other side is checked: a distance no ring of this size can have is treated as
//! "full" by the writer and as "drop what is there" by the reader. Records are always reached
//! through `position & (capacity - 1)`, so no position can lead outside the ring.

use std::sync::atomic::{
    AtomicU32,
    Ordering::{Acquire, Release},
};

/// One record. What the fields mean is up to `kind` ([`super::ToPlugin`], [`super::FromPlugin`]).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RawEvent {
    pub frame: u32,
    pub kind: u32,
    pub a: u32,
    pub b: u32,
    pub value: f64,
}

pub const RAW_EVENT_LEN: usize = 24;
const _: () = assert!(std::mem::size_of::<RawEvent>() == RAW_EVENT_LEN);
const _: () = assert!(std::mem::align_of::<RawEvent>() == 8);

/// Where a ring lives. `capacity` is a power of two.
#[derive(Clone, Copy)]
pub(crate) struct Place<'a> {
    pub head: &'a AtomicU32,
    pub tail: &'a AtomicU32,
    /// `capacity` records, 8-aligned.
    pub records: *mut RawEvent,
    pub capacity: u32,
}

/// The writing side's own position.
#[derive(Debug, Default)]
pub(crate) struct Writer {
    head: u32,
}

impl Writer {
    pub fn at(head: u32) -> Writer {
        Writer { head }
    }

    /// False when the ring is full: the record is not written.
    pub fn push(&mut self, place: Place<'_>, event: RawEvent) -> bool {
        let waiting = self.head.wrapping_sub(place.tail.load(Acquire));
        if waiting >= place.capacity {
            return false;
        }
        let at = (self.head & (place.capacity - 1)) as usize;
        // SAFETY: `at < capacity`, and `records` points at `capacity` records inside the
        // mapping. The reader does not look at this record before the store below.
        unsafe { place.records.add(at).write_volatile(event) };
        self.head = self.head.wrapping_add(1);
        place.head.store(self.head, Release);
        true
    }
}

/// The reading side's own position.
#[derive(Debug, Default)]
pub(crate) struct Reader {
    tail: u32,
    dropped: u32,
}

impl Reader {
    pub fn at(tail: u32) -> Reader {
        Reader { tail, dropped: 0 }
    }

    pub fn pop(&mut self, place: Place<'_>) -> Option<RawEvent> {
        let head = place.head.load(Acquire);
        let waiting = head.wrapping_sub(self.tail);
        if waiting == 0 {
            return None;
        }
        if waiting > place.capacity {
            self.tail = head;
            self.dropped = self.dropped.wrapping_add(1);
            place.tail.store(self.tail, Release);
            return None;
        }
        let at = (self.tail & (place.capacity - 1)) as usize;
        // SAFETY: `at < capacity`, inside the mapping. Any bit pattern is a valid `RawEvent`.
        let event = unsafe { place.records.add(at).read_volatile() };
        self.tail = self.tail.wrapping_add(1);
        place.tail.store(self.tail, Release);
        Some(event)
    }

    /// How often the writer's position made no sense and everything waiting was dropped.
    pub fn dropped(&self) -> u32 {
        self.dropped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::Ordering::SeqCst;

    struct Memory {
        head: AtomicU32,
        tail: AtomicU32,
        records: Box<[std::cell::UnsafeCell<RawEvent>]>,
    }
    // SAFETY: the test threads keep to one writer and one reader.
    unsafe impl Sync for Memory {}

    impl Memory {
        fn new(capacity: usize) -> Memory {
            Memory {
                head: AtomicU32::new(0),
                tail: AtomicU32::new(0),
                records: (0..capacity).map(|_| Default::default()).collect(),
            }
        }

        fn place(&self) -> Place<'_> {
            Place {
                head: &self.head,
                tail: &self.tail,
                records: self.records.as_ptr() as *mut RawEvent,
                capacity: self.records.len() as u32,
            }
        }
    }

    fn event(n: u32) -> RawEvent {
        RawEvent {
            frame: n,
            kind: 1,
            a: n.wrapping_mul(3),
            b: !n,
            value: f64::from(n) * 0.5,
        }
    }

    #[test]
    fn records_come_out_in_order_and_a_full_ring_refuses() {
        let memory = Memory::new(8);
        let (mut writer, mut reader) = (Writer::default(), Reader::default());
        assert_eq!(reader.pop(memory.place()), None);
        for n in 0..8 {
            assert!(writer.push(memory.place(), event(n)));
        }
        assert!(!writer.push(memory.place(), event(99)));
        for n in 0..8 {
            assert_eq!(reader.pop(memory.place()), Some(event(n)));
        }
        assert_eq!(reader.pop(memory.place()), None);
        assert!(writer.push(memory.place(), event(8)));
        assert_eq!(reader.pop(memory.place()), Some(event(8)));
    }

    #[test]
    fn positions_wrap_past_u32() {
        let memory = Memory::new(4);
        let start = u32::MAX - 5;
        memory.head.store(start, SeqCst);
        memory.tail.store(start, SeqCst);
        let (mut writer, mut reader) = (Writer::at(start), Reader::at(start));
        for n in 0..40 {
            assert!(writer.push(memory.place(), event(n)));
            assert!(writer.push(memory.place(), event(n + 1000)));
            assert_eq!(reader.pop(memory.place()), Some(event(n)));
            assert_eq!(reader.pop(memory.place()), Some(event(n + 1000)));
        }
        assert_eq!(reader.dropped(), 0);
    }

    #[test]
    fn a_reader_drops_everything_when_the_writer_position_is_nonsense() {
        let memory = Memory::new(8);
        let mut reader = Reader::default();
        memory.head.store(9, SeqCst);
        assert_eq!(reader.pop(memory.place()), None);
        assert_eq!(reader.dropped(), 1);
        assert_eq!(memory.tail.load(SeqCst), 9);
        // In step again from there.
        let mut writer = Writer::at(9);
        assert!(writer.push(memory.place(), event(1)));
        assert_eq!(reader.pop(memory.place()), Some(event(1)));

        memory.head.store(0x8000_0000, SeqCst);
        assert_eq!(reader.pop(memory.place()), None);
        assert_eq!(reader.dropped(), 2);
    }

    #[test]
    fn a_writer_treats_a_nonsense_reader_position_as_full() {
        let memory = Memory::new(8);
        let mut writer = Writer::default();
        memory.tail.store(1, SeqCst);
        assert!(!writer.push(memory.place(), event(1)));
        memory.tail.store(0xffff_0000, SeqCst);
        assert!(!writer.push(memory.place(), event(1)));
        assert_eq!(memory.head.load(SeqCst), 0);
        memory.tail.store(0, SeqCst);
        assert!(writer.push(memory.place(), event(1)));
    }

    #[test]
    fn two_threads_pass_every_record_once() {
        const COUNT: u32 = 200_000;
        let memory = Arc::new(Memory::new(64));
        let theirs = memory.clone();
        let writer = std::thread::spawn(move || {
            let mut writer = Writer::default();
            for n in 0..COUNT {
                while !writer.push(theirs.place(), event(n)) {
                    std::hint::spin_loop();
                }
            }
        });
        let mut reader = Reader::default();
        let mut next = 0;
        while next < COUNT {
            match reader.pop(memory.place()) {
                Some(got) => {
                    assert_eq!(got, event(next));
                    next += 1;
                }
                None => std::hint::spin_loop(),
            }
        }
        writer.join().unwrap();
        assert_eq!(reader.pop(memory.place()), None);
        assert_eq!(reader.dropped(), 0);
    }
}
