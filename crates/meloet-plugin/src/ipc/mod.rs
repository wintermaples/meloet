//! The shared memory the sound crosses in (docs/architecture.md 20-2).
//!
//! meloet makes one memfd for each plugin host process and both map it. It holds a header and
//! a row of slots, one for each plugin instance the process runs:
//!
//! ```text
//! header (64 bytes, all u32)
//!    0 magic   4 version   8 max_frames  12 channels  16 slots
//!   20 to_plugin_events   24 from_plugin_events   28 slot_stride
//!   32 bell (futex)       36 plugin_waiting     40 stopped     44 progress
//! slot (slot_stride bytes each, the first at 64)
//!    0 request (futex)    4 done (futex)    8 engine_waiting   12 frames   16 flags
//!   20 to_plugin head    24 to_plugin tail  28 from_plugin head  32 from_plugin tail
//!   36 taken             40 taken_ms
//!   64 Transport (64 bytes)
//!  128 input:  channels × max_frames f32
//!      output: channels × max_frames f32
//!      to_plugin records:   to_plugin_events × 24 bytes
//!      from_plugin records: from_plugin_events × 24 bytes
//! ```
//!
//! One block of one slot, inside one audio block of the engine ([`EngineEnd`], [`PluginEnd`]):
//!
//! 1. The engine writes the input, the events and the transport, stores a new number in
//!    `request`, adds one to `bell` and wakes the host's audio thread if it sleeps.
//! 2. The host's audio thread reads them, runs the plugin, writes the output and its events,
//!    stores the same number in `done` and wakes the engine if it sleeps.
//! 3. The engine, which slept on `done` with a time limit, reads the output.
//!
//! An answer that is not there at the time limit leaves its block without output, and the
//! block after it waits for that answer once more. Whether a slot whose answer is still missing
//! then is stopped depends on what the host process shows of itself. Its audio threads write
//! the request they take into the slot's `taken`, with the time (`taken_ms`, the low bits of
//! CLOCK_MONOTONIC in milliseconds), and count every request taken or answered in the header's
//! `progress`. A request that a thread has held for [`HANG_MS`] is a plugin that hangs;
//! one held for less is a plugin with slow work in its block, and once that has lasted a block
//! and a half the engine no longer sleeps for the answer. A request nobody took
//! while `progress` stood still over two time limits is a process that is gone or frozen. Either stops the slot and raises `stopped` in the header,
//! which stops every other slot of the mapping with it: they are plugins of the same process.
//! A request that waits its turn while the process works on others is only late, however
//! long: a process with more work than fits a block must not be taken for a dead one.
//!
//! The host process may run several audio threads. Each sleeps on `bell`, and
//! `plugin_waiting` counts the sleepers.
//!
//! The engine trusts nothing the host process writes: it keeps the geometry and its own ring
//! positions in its own memory, and every length and position read from the shared memory is
//! brought into range before it is used. The sound and the transport are copied in and out
//! with plain copies (no reference into the shared memory outlives a call), so memory that
//! changes under a process's feet is only ever wrong sound.

mod ends;
pub mod futex;
mod ring;

use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

pub use ends::{Bell, Block, EngineEnd, HANG_MS, PluginEnd};
pub use ring::{RAW_EVENT_LEN, RawEvent};

/// "MLPH", little endian.
pub const MAGIC: u32 = 0x4850_4c4d;
pub const IPC_VERSION: u32 = 1;

const HEADER_LEN: usize = 64;
const SLOT_WORDS_LEN: usize = 64;
const TRANSPORT_LEN: usize = 64;

#[derive(Debug)]
pub enum IpcError {
    Os(std::io::Error),
    /// A number of the geometry is outside what this version allows.
    Geometry(&'static str),
    /// The memory handed over is not a mapping made by [`Mapping::create`].
    Header(&'static str),
    NoSuchSlot(u32),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::Os(e) => write!(f, "shared memory: {e}"),
            IpcError::Geometry(what) => write!(f, "shared memory geometry: {what}"),
            IpcError::Header(what) => write!(f, "shared memory header: {what}"),
            IpcError::NoSuchSlot(slot) => write!(f, "no slot {slot}"),
        }
    }
}

impl std::error::Error for IpcError {}

impl From<std::io::Error> for IpcError {
    fn from(e: std::io::Error) -> IpcError {
        IpcError::Os(e)
    }
}

/// The sizes everything in a mapping is placed by. Chosen by meloet when it makes the mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    /// The largest block, in frames.
    pub max_frames: u32,
    /// Audio channels each way.
    pub channels: u32,
    pub slots: u32,
    /// Records of each slot's ring towards the plugin; a power of two.
    pub to_plugin_events: u32,
    /// Records of each slot's ring from the plugin; a power of two.
    pub from_plugin_events: u32,
}

impl Geometry {
    pub fn validate(&self) -> Result<(), IpcError> {
        let ring = |n: u32| n.is_power_of_two() && (16..=65_536).contains(&n);
        if !(1..=65_536).contains(&self.max_frames) {
            return Err(IpcError::Geometry("max_frames"));
        }
        if !(1..=8).contains(&self.channels) {
            return Err(IpcError::Geometry("channels"));
        }
        if !(1..=1024).contains(&self.slots) {
            return Err(IpcError::Geometry("slots"));
        }
        if !ring(self.to_plugin_events) || !ring(self.from_plugin_events) {
            return Err(IpcError::Geometry("events"));
        }
        Ok(())
    }

    fn audio_len(&self) -> usize {
        self.channels as usize * self.max_frames as usize * 4
    }

    fn input_at(&self) -> usize {
        SLOT_WORDS_LEN + TRANSPORT_LEN
    }

    fn output_at(&self) -> usize {
        self.input_at() + self.audio_len()
    }

    fn ring_to_plugin_at(&self) -> usize {
        // Records hold an f64.
        (self.output_at() + self.audio_len()).next_multiple_of(8)
    }

    fn ring_from_plugin_at(&self) -> usize {
        self.ring_to_plugin_at() + self.to_plugin_events as usize * RAW_EVENT_LEN
    }

    pub fn slot_stride(&self) -> usize {
        (self.ring_from_plugin_at() + self.from_plugin_events as usize * RAW_EVENT_LEN)
            .next_multiple_of(64)
    }

    pub fn total_len(&self) -> usize {
        HEADER_LEN + self.slots as usize * self.slot_stride()
    }
}

#[repr(C)]
struct HeaderWords {
    magic: AtomicU32,
    version: AtomicU32,
    max_frames: AtomicU32,
    channels: AtomicU32,
    slots: AtomicU32,
    to_plugin_events: AtomicU32,
    from_plugin_events: AtomicU32,
    slot_stride: AtomicU32,
    bell: AtomicU32,
    plugin_waiting: AtomicU32,
    stopped: AtomicU32,
    progress: AtomicU32,
    _reserved: [AtomicU32; 4],
}
const _: () = assert!(std::mem::size_of::<HeaderWords>() == HEADER_LEN);

#[repr(C)]
struct SlotWords {
    request: AtomicU32,
    done: AtomicU32,
    engine_waiting: AtomicU32,
    frames: AtomicU32,
    flags: AtomicU32,
    to_plugin_head: AtomicU32,
    to_plugin_tail: AtomicU32,
    from_plugin_head: AtomicU32,
    from_plugin_tail: AtomicU32,
    taken: AtomicU32,
    taken_ms: AtomicU32,
    _reserved: [AtomicU32; 5],
}
const _: () = assert!(std::mem::size_of::<SlotWords>() == SLOT_WORDS_LEN);

/// The memfd, mapped. Shared by the ends of its slots through an `Arc`; unmapped when the last
/// one goes (never on the audio thread: the engine hands retired things back to the control
/// side).
pub struct Mapping {
    ptr: *mut u8,
    len: usize,
    fd: OwnedFd,
    /// This process's own copy. Never read back from the shared memory after `attach`.
    geometry: Geometry,
}

// SAFETY: the mapping is plain shared memory. Everything in it is reached through atomics or
// through copies made in turn (see the module's text); the pointer itself never changes.
unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

fn os_error() -> IpcError {
    IpcError::Os(std::io::Error::last_os_error())
}

impl Mapping {
    /// Makes the memfd, sealed against shrinking and growing, maps it and writes the header.
    /// Every page is touched here, so the audio thread never meets a page for the first time.
    pub fn create(geometry: Geometry) -> Result<Mapping, IpcError> {
        geometry.validate()?;
        let len = geometry.total_len();
        // SAFETY: a valid C string and flags; the result is checked.
        let raw = unsafe {
            libc::memfd_create(
                c"meloet-plughost".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if raw < 0 {
            return Err(os_error());
        }
        // SAFETY: `raw` is a new descriptor nobody else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: plain calls on a descriptor this function owns.
        unsafe {
            if libc::ftruncate(raw, len as libc::off_t) != 0 {
                return Err(os_error());
            }
            let seals = libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_SEAL;
            if libc::fcntl(raw, libc::F_ADD_SEALS, seals) != 0 {
                return Err(os_error());
            }
        }
        let mapping = Mapping::map(fd, len, geometry)?;
        let header = mapping.header();
        header.version.store(IPC_VERSION, Relaxed);
        header.max_frames.store(geometry.max_frames, Relaxed);
        header.channels.store(geometry.channels, Relaxed);
        header.slots.store(geometry.slots, Relaxed);
        header
            .to_plugin_events
            .store(geometry.to_plugin_events, Relaxed);
        header
            .from_plugin_events
            .store(geometry.from_plugin_events, Relaxed);
        header
            .slot_stride
            .store(geometry.slot_stride() as u32, Relaxed);
        header.magic.store(MAGIC, Relaxed);
        Ok(mapping)
    }

    /// Maps a memfd made by [`Mapping::create`] in another process, and reads the geometry
    /// from its header. Refuses anything whose header and size do not agree.
    pub fn attach(fd: OwnedFd) -> Result<Mapping, IpcError> {
        // SAFETY: `stat` is written by fstat before it is read.
        let len = unsafe {
            let mut stat: libc::stat = std::mem::zeroed();
            if libc::fstat(fd.as_raw_fd(), &mut stat) != 0 {
                return Err(os_error());
            }
            stat.st_size as usize
        };
        if len < HEADER_LEN {
            return Err(IpcError::Header("too small"));
        }
        // SAFETY: a plain call on a live descriptor.
        let seals = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 || seals & libc::F_SEAL_SHRINK == 0 {
            // Memory that can shrink under the mapping would end this process with SIGBUS.
            return Err(IpcError::Header("not sealed"));
        }
        let unknown = Geometry {
            max_frames: 0,
            channels: 0,
            slots: 0,
            to_plugin_events: 0,
            from_plugin_events: 0,
        };
        let mut mapping = Mapping::map(fd, len, unknown)?;
        let header = mapping.header();
        if header.magic.load(Relaxed) != MAGIC {
            return Err(IpcError::Header("magic"));
        }
        if header.version.load(Relaxed) != IPC_VERSION {
            return Err(IpcError::Header("version"));
        }
        let geometry = Geometry {
            max_frames: header.max_frames.load(Relaxed),
            channels: header.channels.load(Relaxed),
            slots: header.slots.load(Relaxed),
            to_plugin_events: header.to_plugin_events.load(Relaxed),
            from_plugin_events: header.from_plugin_events.load(Relaxed),
        };
        geometry.validate()?;
        if geometry.total_len() != len {
            return Err(IpcError::Header("size"));
        }
        mapping.geometry = geometry;
        Ok(mapping)
    }

    fn map(fd: OwnedFd, len: usize, geometry: Geometry) -> Result<Mapping, IpcError> {
        // SAFETY: a new shared mapping of `len` bytes of a descriptor this function owns.
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED | libc::MAP_POPULATE,
                fd.as_raw_fd(),
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(os_error());
        }
        // Best effort: without the right to lock memory the pages are only populated.
        // SAFETY: the range is the mapping just made.
        unsafe { libc::mlock(ptr, len) };
        Ok(Mapping {
            ptr: ptr.cast(),
            len,
            fd,
            geometry,
        })
    }

    pub fn geometry(&self) -> Geometry {
        self.geometry
    }

    /// The host process is gone or hangs: every slot of this mapping is stopped from its
    /// next block on, without being waited for. Raised by the engine's end of a slot that
    /// went two blocks without an answer, and by whoever watches the process.
    pub fn mark_stopped(&self) {
        self.header().stopped.store(1, Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.header().stopped.load(Relaxed) != 0
    }

    /// The memfd, to hand to the host process.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn header(&self) -> &HeaderWords {
        // SAFETY: the mapping is at least HEADER_LEN long and page-aligned, and the header is
        // made of atomics only.
        unsafe { &*self.ptr.cast::<HeaderWords>() }
    }

    fn slot_base(&self, slot: u32) -> Result<usize, IpcError> {
        if slot >= self.geometry.slots {
            return Err(IpcError::NoSuchSlot(slot));
        }
        Ok(HEADER_LEN + slot as usize * self.geometry.slot_stride())
    }

    /// `base` comes from [`Mapping::slot_base`].
    fn slot_words(&self, base: usize) -> &SlotWords {
        // SAFETY: `base` is a slot's start: inside the mapping, 64-aligned, atomics only.
        unsafe { &*self.ptr.add(base).cast::<SlotWords>() }
    }

    /// A pointer `offset` bytes into the slot at `base`; `offset` is one of the geometry's.
    fn slot_ptr(&self, base: usize, offset: usize) -> *mut u8 {
        debug_assert!(base + offset <= self.len);
        // SAFETY: inside the mapping (see the geometry's sums).
        unsafe { self.ptr.add(base + offset) }
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: the range was mapped by `map` and nothing refers to it any more.
        unsafe { libc::munmap(self.ptr.cast(), self.len) };
    }
}

/// Where the song is at the first frame of the block, for plugins that follow the tempo.
/// Positions are in quarter notes.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transport {
    /// [`Transport::PLAYING`] and so on.
    pub flags: u32,
    pub meter_num: u16,
    pub meter_den: u16,
    pub tempo_bpm: f64,
    pub position_quarters: f64,
    /// Where the bar the position is in began.
    pub bar_start_quarters: f64,
    pub loop_start_quarters: f64,
    pub loop_end_quarters: f64,
    /// Frames played since the start of the song at the present tempo.
    pub position_frames: i64,
    pub reserved: u64,
}
const _: () = assert!(std::mem::size_of::<Transport>() == TRANSPORT_LEN);

impl Transport {
    pub const PLAYING: u32 = 1;
    pub const RECORDING: u32 = 2;
    pub const LOOPING: u32 = 4;
}

/// Flags of one block, engine to plugin.
pub mod block_flags {
    /// Silence at once and forget what is held (as `Instrument::reset` and `Effect::reset`).
    pub const RESET: u32 = 1;
}

const KIND_MIDI: u32 = 1;
const KIND_PARAM: u32 = 2;
const KIND_LATENCY: u32 = 3;

/// Engine to plugin. `frame` is the frame of the block it applies at.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToPlugin {
    /// A channel message of up to three bytes (note on and off, controller, pitch bend).
    Midi { frame: u32, data: [u8; 3] },
    /// `value` is 0..=1 whatever the parameter's own range.
    Param { frame: u32, id: u32, value: f64 },
}

impl ToPlugin {
    pub fn frame(&self) -> u32 {
        match *self {
            ToPlugin::Midi { frame, .. } | ToPlugin::Param { frame, .. } => frame,
        }
    }

    pub fn encode(self) -> RawEvent {
        match self {
            ToPlugin::Midi { frame, data } => RawEvent {
                frame,
                kind: KIND_MIDI,
                a: u32::from_le_bytes([data[0], data[1], data[2], 0]),
                b: 0,
                value: 0.0,
            },
            ToPlugin::Param { frame, id, value } => RawEvent {
                frame,
                kind: KIND_PARAM,
                a: id,
                b: 0,
                value,
            },
        }
    }

    /// `None` for a record that is not one of these. The frame is brought inside the block.
    pub fn decode(raw: RawEvent, frames: u32) -> Option<ToPlugin> {
        let frame = raw.frame.min(frames.saturating_sub(1));
        match raw.kind {
            KIND_MIDI => {
                let [a, b, c, _] = raw.a.to_le_bytes();
                Some(ToPlugin::Midi {
                    frame,
                    data: [a, b, c],
                })
            }
            KIND_PARAM if raw.value.is_finite() => Some(ToPlugin::Param {
                frame,
                id: raw.a,
                value: raw.value.clamp(0.0, 1.0),
            }),
            _ => None,
        }
    }
}

/// Plugin to engine, from the host's audio thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FromPlugin {
    /// A parameter the plugin moved by itself (an output parameter, a meter).
    Param { id: u32, value: f64 },
    /// The plugin's delay is now this many frames (20-8).
    Latency { frames: u32 },
}

/// A plugin that says it delays by more than this is not believed (about 87 s at 48 kHz).
pub const MAX_LATENCY_FRAMES: u32 = 1 << 22;

impl FromPlugin {
    pub fn encode(self) -> RawEvent {
        match self {
            FromPlugin::Param { id, value } => RawEvent {
                frame: 0,
                kind: KIND_PARAM,
                a: id,
                b: 0,
                value,
            },
            FromPlugin::Latency { frames } => RawEvent {
                frame: 0,
                kind: KIND_LATENCY,
                a: frames,
                b: 0,
                value: 0.0,
            },
        }
    }

    /// `None` for a record that is not one of these; values are brought into range.
    pub fn decode(raw: RawEvent) -> Option<FromPlugin> {
        match raw.kind {
            KIND_PARAM if raw.value.is_finite() => Some(FromPlugin::Param {
                id: raw.a,
                value: raw.value.clamp(0.0, 1.0),
            }),
            KIND_LATENCY => Some(FromPlugin::Latency {
                frames: raw.a.min(MAX_LATENCY_FRAMES),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn geometry() -> Geometry {
        Geometry {
            max_frames: 256,
            channels: 2,
            slots: 3,
            to_plugin_events: 64,
            from_plugin_events: 16,
        }
    }

    #[test]
    fn geometry_places_do_not_overlap_and_are_aligned() {
        let g = geometry();
        assert_eq!(g.input_at(), 128);
        assert_eq!(g.output_at(), 128 + 2 * 256 * 4);
        assert_eq!(g.ring_to_plugin_at() % 8, 0);
        assert!(g.ring_to_plugin_at() >= g.output_at() + 2 * 256 * 4);
        assert_eq!(g.ring_from_plugin_at(), g.ring_to_plugin_at() + 64 * 24);
        assert!(g.slot_stride() >= g.ring_from_plugin_at() + 16 * 24);
        assert_eq!(g.slot_stride() % 64, 0);
        assert_eq!(g.total_len(), 64 + 3 * g.slot_stride());
        // Odd sizes still give aligned records.
        let odd = Geometry {
            max_frames: 3,
            channels: 1,
            ..g
        };
        assert_eq!(odd.ring_to_plugin_at() % 8, 0);
        assert_eq!(odd.slot_stride() % 64, 0);
    }

    #[test]
    fn geometry_out_of_range_is_refused() {
        let g = geometry();
        for bad in [
            Geometry { max_frames: 0, ..g },
            Geometry {
                max_frames: 65_537,
                ..g
            },
            Geometry { channels: 0, ..g },
            Geometry { channels: 9, ..g },
            Geometry { slots: 0, ..g },
            Geometry { slots: 1025, ..g },
            Geometry {
                to_plugin_events: 48,
                ..g
            },
            Geometry {
                to_plugin_events: 8,
                ..g
            },
            Geometry {
                from_plugin_events: 1 << 17,
                ..g
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
            assert!(Mapping::create(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn attach_reads_the_geometry_from_the_header() {
        let made = Mapping::create(geometry()).unwrap();
        assert_eq!(made.len(), geometry().total_len());
        let other = Mapping::attach(made.fd().try_clone_to_owned().unwrap()).unwrap();
        assert_eq!(other.geometry(), geometry());
        assert_eq!(other.len(), made.len());
        // The same memory: a word written through one is seen through the other.
        made.header().bell.store(41, Relaxed);
        assert_eq!(other.header().bell.load(Relaxed), 41);
        assert!(matches!(made.slot_base(3), Err(IpcError::NoSuchSlot(3))));
        assert_eq!(
            made.slot_base(2).unwrap(),
            64 + 2 * geometry().slot_stride()
        );
    }

    #[test]
    fn the_memfd_cannot_be_shrunk_or_grown() {
        let made = Mapping::create(geometry()).unwrap();
        let fd = made.fd().as_raw_fd();
        // SAFETY: plain calls on a live descriptor.
        unsafe {
            assert_ne!(libc::ftruncate(fd, 64), 0);
            assert_ne!(libc::ftruncate(fd, made.len() as libc::off_t * 2), 0);
            assert_ne!(libc::fcntl(fd, libc::F_ADD_SEALS, libc::F_SEAL_WRITE), 0);
        }
    }

    fn memfd(len: usize, sealed: bool) -> OwnedFd {
        // SAFETY: as in `Mapping::create`.
        unsafe {
            let raw = libc::memfd_create(
                c"meloet-test".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            );
            assert!(raw >= 0);
            assert_eq!(libc::ftruncate(raw, len as libc::off_t), 0);
            if sealed {
                assert_eq!(
                    libc::fcntl(
                        raw,
                        libc::F_ADD_SEALS,
                        libc::F_SEAL_SHRINK | libc::F_SEAL_GROW
                    ),
                    0
                );
            }
            OwnedFd::from_raw_fd(raw)
        }
    }

    #[test]
    fn attach_refuses_memory_that_is_not_a_mapping() {
        let refused = |fd: OwnedFd| match Mapping::attach(fd) {
            Err(IpcError::Header(what)) => what,
            Err(other) => panic!("{other}"),
            Ok(_) => panic!("attached"),
        };
        assert_eq!(refused(memfd(16, true)), "too small");
        assert_eq!(refused(memfd(4096, false)), "not sealed");
        assert_eq!(refused(memfd(4096, true)), "magic");

        // A header that names more memory than there is.
        let made = Mapping::create(geometry()).unwrap();
        let short = memfd(made.len() - 64, true);
        // SAFETY: both ranges are valid for `HEADER_LEN` bytes and do not overlap.
        unsafe {
            let to = libc::mmap(
                std::ptr::null_mut(),
                HEADER_LEN,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                short.as_raw_fd(),
                0,
            );
            assert_ne!(to, libc::MAP_FAILED);
            std::ptr::copy_nonoverlapping(made.ptr, to.cast::<u8>(), HEADER_LEN);
            libc::munmap(to, HEADER_LEN);
        }
        assert_eq!(refused(short), "size");

        made.header().version.store(IPC_VERSION + 1, Relaxed);
        assert_eq!(refused(made.fd().try_clone_to_owned().unwrap()), "version");
        made.header().version.store(IPC_VERSION, Relaxed);
        made.header().slots.store(5000, Relaxed);
        assert!(matches!(
            Mapping::attach(made.fd().try_clone_to_owned().unwrap()),
            Err(IpcError::Geometry("slots"))
        ));
    }

    #[test]
    fn events_round_trip() {
        for event in [
            ToPlugin::Midi {
                frame: 5,
                data: [0x90, 60, 100],
            },
            ToPlugin::Param {
                frame: 0,
                id: 2855,
                value: 0.25,
            },
        ] {
            assert_eq!(ToPlugin::decode(event.encode(), 256), Some(event));
        }
        for event in [
            FromPlugin::Param { id: 7, value: 1.0 },
            FromPlugin::Latency { frames: 512 },
        ] {
            assert_eq!(FromPlugin::decode(event.encode()), Some(event));
        }
    }

    #[test]
    fn events_from_the_other_side_are_brought_into_range() {
        let raw = |frame, kind, a, value| RawEvent {
            frame,
            kind,
            a,
            b: 0,
            value,
        };
        assert_eq!(
            ToPlugin::decode(raw(9999, KIND_MIDI, 0x0064_3c90, 0.0), 256),
            Some(ToPlugin::Midi {
                frame: 255,
                data: [0x90, 60, 100]
            })
        );
        assert_eq!(
            ToPlugin::decode(raw(0, KIND_PARAM, 1, 7.0), 256),
            Some(ToPlugin::Param {
                frame: 0,
                id: 1,
                value: 1.0
            })
        );
        assert_eq!(ToPlugin::decode(raw(0, KIND_PARAM, 1, f64::NAN), 256), None);
        assert_eq!(ToPlugin::decode(raw(0, 77, 1, 0.0), 256), None);
        assert_eq!(ToPlugin::decode(raw(0, KIND_LATENCY, 1, 0.0), 256), None);

        assert_eq!(
            FromPlugin::decode(raw(0, KIND_PARAM, 1, -3.0)),
            Some(FromPlugin::Param { id: 1, value: 0.0 })
        );
        assert_eq!(
            FromPlugin::decode(raw(0, KIND_PARAM, 1, f64::INFINITY)),
            None
        );
        assert_eq!(
            FromPlugin::decode(raw(0, KIND_LATENCY, u32::MAX, 0.0)),
            Some(FromPlugin::Latency {
                frames: MAX_LATENCY_FRAMES
            })
        );
        assert_eq!(FromPlugin::decode(raw(0, KIND_MIDI, 1, 0.0)), None);
        assert_eq!(FromPlugin::decode(raw(0, 0, 0, 0.0)), None);
    }
}
