//! The read-ahead thread against rings the test reads itself, with files that are numbers:
//! every sample says which frame of which file it is.

use std::io;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use meloet_engine::{StreamReader, song_frame, stream_ring};
use meloet_model::{Meter, Tempo, Timeline};

use super::*;

const RATE: u32 = 48_000;

fn uid(n: u8) -> [u8; 16] {
    [n; 16]
}

/// What file `n` holds at `frame`, on the left; a stereo file has its negative on the right.
fn sample(n: u8, frame: u64) -> f32 {
    f32::from(n) * 100_000.0 + (frame % 100_000) as f32
}

struct Numbers {
    n: u8,
    frames: u64,
    stereo: bool,
    slow: Arc<Mutex<Duration>>,
}

impl PcmFile for Numbers {
    fn channels(&self) -> u8 {
        if self.stereo { 2 } else { 1 }
    }
    fn frames(&self) -> u64 {
        self.frames
    }
    fn read(&mut self, frame: u64, out: &mut [f32]) -> io::Result<usize> {
        let slow = *self.slow.lock().unwrap();
        if !slow.is_zero() {
            std::thread::sleep(slow);
        }
        let channels = usize::from(self.channels());
        let count = (self.frames.saturating_sub(frame) as usize).min(out.len() / channels);
        for (i, at) in out.chunks_exact_mut(channels).take(count).enumerate() {
            let value = sample(self.n, frame + i as u64);
            at[0] = value;
            at[channels - 1] = if self.stereo { -value } else { value };
        }
        Ok(count)
    }
}

#[derive(Default)]
struct Store {
    /// How long every read takes.
    slow: Arc<Mutex<Duration>>,
    /// Files with this number and above are not there.
    gone_from: Mutex<u8>,
    opened: AtomicU64,
    stereo: AtomicBool,
}

impl Store {
    fn new() -> Arc<Store> {
        let store = Store::default();
        *store.gone_from.lock().unwrap() = u8::MAX;
        Arc::new(store)
    }
}

impl PcmStore for Store {
    fn open(&self, uid: &[u8; 16], sample_rate: u32) -> Option<Box<dyn PcmFile>> {
        assert_eq!(sample_rate, RATE);
        self.opened.fetch_add(1, Relaxed);
        (uid[0] < *self.gone_from.lock().unwrap()).then(|| {
            Box::new(Numbers {
                n: uid[0],
                frames: 1_000_000,
                stereo: self.stereo.load(Relaxed),
                slow: self.slow.clone(),
            }) as Box<dyn PcmFile>
        })
    }
}

fn map() -> Arc<TempoMap> {
    // 120 a minute: a quarter note is 24000 frames.
    let tempo = Tempo::from_centi(12_000).unwrap();
    Arc::new(TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR)))
}

fn clip(start: u64, end: u64, n: u8, file_frame: i64) -> FeedClip {
    FeedClip {
        start,
        end,
        uid: uid(n),
        file_frame,
        file_end: 1_000_000,
    }
}

struct Rig {
    ahead: ReadAhead,
    wants: rtrb_free::Wants,
    reader: StreamReader,
    generation: u32,
}

/// The writing end of the ring of wants, without naming the ring's crate here.
mod rtrb_free {
    use meloet_engine::{StreamWant, StreamWants, want_ring};

    pub struct Wants(Box<dyn FnMut(StreamWant) + Send>);

    impl Wants {
        pub fn new() -> (Wants, StreamWants) {
            let (mut tx, rx) = want_ring(64);
            (
                // A full ring lets a want go, as the engine does.
                Wants(Box::new(move |want| {
                    let _ = tx.push(want);
                })),
                rx,
            )
        }

        pub fn say(&mut self, want: StreamWant) {
            (self.0)(want);
        }
    }
}

const SEAT: usize = 5;

impl Rig {
    fn new(store: Arc<Store>, clips: Vec<FeedClip>) -> Rig {
        let ahead = ReadAhead::spawn(store);
        let (wants, rx) = rtrb_free::Wants::new();
        ahead.set_wants(rx);
        ahead.set_map(map(), 1);
        let (writer, reader) = stream_ring(RATE);
        ahead.set_track(SEAT, Some((writer, RATE)), clips);
        Rig {
            ahead,
            wants,
            reader,
            generation: 0,
        }
    }

    /// A new generation from `frame` after an anchor at `anchor_sec`.
    fn jump(&mut self, anchor_sec: f64, frame: u64, loop_ticks: Option<(u64, u64)>) {
        self.generation += 1;
        self.wants.say(StreamWant {
            track: SEAT as u32,
            generation: self.generation,
            map_gen: 1,
            anchor_sec,
            frame,
            loop_ticks,
        });
        // As the engine does at its next block: what is left of the old generation goes.
        self.reader.settle(self.generation, frame);
    }

    fn fed(&self) -> u64 {
        self.ahead.stats().frames.load(Relaxed)
    }

    fn wait(&self, what: &str, done: impl Fn(&Rig) -> bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while !done(self) {
            assert!(Instant::now() < until, "{what}");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The ring is full and the thread rests.
    fn wait_full(&self, frames: u64) {
        self.wait("the ring fills", |rig| rig.fed() >= frames);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(self.fed(), frames);
    }

    /// Left and right of `frames` frames from `frame` on, and how many were not there.
    fn read(&mut self, frame: u64, frames: usize) -> (Vec<[f32; 2]>, u32) {
        let mut out = vec![9.0f32; frames * 2];
        let found = self.reader.read(self.generation, frame, &mut out);
        (out.as_chunks::<2>().0.to_vec(), found.missing)
    }
}

#[test]
fn the_ring_holds_the_clips_in_the_order_they_sound_with_silence_between() {
    let clips = vec![
        clip(1_000, 3_000, 1, 500),
        // The start pulled out to 200 frames before the take, and a file that ends early.
        FeedClip {
            file_end: 1_500,
            ..clip(3_000, 5_000, 2, -200)
        },
        clip(90_000, 100_000, 3, 0),
    ];
    let mut rig = Rig::new(Store::new(), clips);
    rig.jump(0.0, 0, None);
    // Two seconds, while nothing is taken: where a stopped song is, a play starts at once.
    rig.wait_full(96_000);
    let (out, missing) = rig.read(0, 96_000);
    assert_eq!(missing, 0);
    for (f, frame) in out.iter().enumerate() {
        let f = f as u64;
        let want = match f {
            1_000..3_000 => sample(1, 500 + f - 1_000),
            3_200..4_700 => sample(2, f - 3_200),
            90_000..96_000 => sample(3, f - 90_000),
            _ => 0.0,
        };
        // A mono file is on both sides.
        assert_eq!(*frame, [want, want], "frame {f}");
    }
    // It goes on by itself as the ring empties.
    rig.wait("the ring fills again", |rig| rig.fed() == 192_000);
    let (out, missing) = rig.read(96_000, 6_000);
    assert_eq!(missing, 0);
    assert_eq!(out[0][0], sample(3, 6_000));
    assert_eq!(out[3_999][0], sample(3, 9_999));
    assert_eq!(out[4_000][0], 0.0);
}

#[test]
fn it_fills_two_seconds_and_again_when_under_one_is_left() {
    let mut rig = Rig::new(Store::new(), vec![clip(0, 900_000, 1, 0)]);
    rig.jump(0.0, 0, None);
    rig.wait_full(96_000);
    // Half a second taken: a second and a half is left, and it rests.
    assert_eq!(rig.read(0, 24_000).1, 0);
    rig.wait_full(96_000);
    // Under a second left: it fills up.
    assert_eq!(rig.read(24_000, 30_000).1, 0);
    rig.wait_full(96_000 + 54_000);
    let (out, missing) = rig.read(54_000, 96_000);
    assert_eq!(missing, 0);
    assert_eq!(out[0], [sample(1, 54_000); 2]);
    assert_eq!(out[95_999], [sample(1, 149_999); 2]);
}

#[test]
fn a_new_generation_starts_from_the_new_place_and_a_stereo_file_keeps_its_sides() {
    let store = Store::new();
    store.stereo.store(true, Relaxed);
    let mut rig = Rig::new(store, vec![clip(0, 900_000, 1, 0)]);
    rig.jump(0.0, 0, None);
    rig.wait_full(96_000);
    // Four seconds on, the engine already 300 frames past its anchor (a track that reads
    // ahead by its plugins' delay).
    rig.jump(4.0, 300, None);
    rig.wait("the new generation is fed", |rig| rig.fed() > 96_000);
    // What is left of the old generation is passed over.
    let (out, missing) = rig.read(300, 4_096);
    assert_eq!(missing, 0);
    let first = sample(1, 192_300);
    assert_eq!(out[0], [first, -first]);
    assert_eq!(out[4_095][0], sample(1, 192_300 + 4_095));
    assert_eq!(rig.ahead.stats().generations.load(Relaxed), 2);
}

#[test]
fn the_turns_of_the_loop_are_laid_out_ahead() {
    // The loop is the second quarter note: song frames 24000 to 48000.
    let clips = vec![clip(0, 30_000, 1, 0), clip(30_000, 900_000, 2, 0)];
    let mut rig = Rig::new(Store::new(), clips);
    let m = map();
    assert_eq!(song_frame(&m, 1_920.0, f64::from(RATE)), 24_000);
    // From a quarter of a second before the loop.
    rig.jump(0.25, 0, Some((1_920, 3_840)));
    rig.wait_full(96_000);
    let (out, missing) = rig.read(0, 96_000);
    assert_eq!(missing, 0);
    for (f, frame) in out.iter().enumerate() {
        let f = f as u64;
        let song = if f < 36_000 {
            12_000 + f
        } else {
            24_000 + (f - 36_000) % 24_000
        };
        let want = if song < 30_000 {
            sample(1, song)
        } else {
            sample(2, song - 30_000)
        };
        assert_eq!(frame[0], want, "frame {f}");
    }
}

#[test]
fn a_slow_disk_is_silence_and_the_place_stays_right() {
    let store = Store::new();
    let mut rig = Rig::new(store.clone(), vec![clip(0, 900_000, 1, 0)]);
    rig.jump(0.0, 0, None);
    rig.wait_full(96_000);
    // The disk nearly stands still while the song plays: 256 frames every 5 ms.
    *store.slow.lock().unwrap() = Duration::from_millis(400);
    let (mut frame, mut lost) = (0u64, 0u64);
    let began = Instant::now();
    while lost < 20_000 {
        assert!(began.elapsed() < Duration::from_secs(20));
        let (out, missing) = rig.read(frame, 256);
        for (i, got) in out.iter().enumerate() {
            let want = sample(1, frame + i as u64);
            assert!(got[0] == want || got[0] == 0.0, "{frame} + {i}: {got:?}");
        }
        frame += 256;
        lost += u64::from(missing);
        if missing > 0 {
            // As the engine does: where it is now.
            rig.wants.say(StreamWant {
                track: SEAT as u32,
                generation: rig.generation,
                map_gen: 1,
                anchor_sec: 0.0,
                frame,
                loop_ticks: None,
            });
        }
        std::thread::sleep(Duration::from_micros(5_333));
    }
    // The disk is back: the thread goes on from where the engine is, not from where it
    // fell behind, and every frame is the one that belongs there.
    *store.slow.lock().unwrap() = Duration::ZERO;
    let began = Instant::now();
    let mut whole = 0;
    while whole < 100 {
        assert!(began.elapsed() < Duration::from_secs(20));
        let (out, missing) = rig.read(frame, 256);
        for (i, got) in out.iter().enumerate() {
            let want = sample(1, frame + i as u64);
            assert!(got[0] == want || got[0] == 0.0, "{frame} + {i}: {got:?}");
        }
        if missing == 0 {
            assert_eq!(out[0][0], sample(1, frame));
            whole += 1;
        } else {
            whole = 0;
            rig.wants.say(StreamWant {
                track: SEAT as u32,
                generation: rig.generation,
                map_gen: 1,
                anchor_sec: 0.0,
                frame: frame + 256,
                loop_ticks: None,
            });
        }
        frame += 256;
        std::thread::sleep(Duration::from_micros(5_333));
    }
    assert!(rig.ahead.stats().skipped.load(Relaxed) > 0);
}

#[test]
fn a_file_that_is_not_there_is_silence_until_the_store_has_it() {
    let store = Store::new();
    *store.gone_from.lock().unwrap() = 2;
    let clips = vec![clip(0, 1_000, 1, 0), clip(1_000, 900_000, 2, 0)];
    let mut rig = Rig::new(store.clone(), clips.clone());
    rig.jump(0.0, 0, None);
    rig.wait_full(96_000);
    let (out, missing) = rig.read(0, 2_000);
    assert_eq!(missing, 0);
    assert_eq!(out[999][0], sample(1, 999));
    assert!(out[1_000..].iter().all(|f| *f == [0.0; 2]));
    // Asked once for each file, not at every chunk.
    assert_eq!(store.opened.load(Relaxed), 2);

    *store.gone_from.lock().unwrap() = u8::MAX;
    rig.ahead.store_changed();
    rig.jump(0.0, 0, None);
    rig.wait("the new generation is fed", |rig| rig.fed() > 96_000);
    let (out, _) = rig.read(0, 2_000);
    assert_eq!(out[1_000][0], sample(2, 0));
    assert_eq!(out[1_999][0], sample(2, 999));
}

#[test]
fn other_clips_come_with_the_next_generation_and_a_track_can_go() {
    let mut rig = Rig::new(Store::new(), vec![clip(0, 900_000, 1, 0)]);
    rig.jump(0.0, 0, None);
    rig.wait_full(96_000);
    // The clip moved a second to the right; the core says so, then the engine jumps.
    rig.ahead.set_clips(SEAT, vec![clip(48_000, 900_000, 1, 0)]);
    rig.jump(0.0, 0, None);
    rig.wait("the new generation is fed", |rig| rig.fed() > 96_000);
    let (out, missing) = rig.read(0, 50_000);
    assert_eq!(missing, 0);
    assert!(out[..48_000].iter().all(|f| *f == [0.0; 2]));
    assert_eq!(out[48_000][0], sample(1, 0));

    rig.ahead.set_track(SEAT, None, Vec::new());
    rig.wait("the thread lets go of the ring", |rig| {
        rig.reader.is_abandoned()
    });
}
