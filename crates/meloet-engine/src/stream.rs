//! The way from the disk to the audio thread: the ring of a vocal track that the read-ahead
//! thread fills and the engine empties (docs/architecture.md 22-8, 22-17).
//!
//! ```text
//! engine ── StreamWant (generation, anchor, frame) ──▶ read-ahead thread
//! engine ◀── StreamReader ══ ring of 2 s, stereo ══ StreamWriter ◀── the files
//! ```
//!
//! The audio thread never reads a file: every block it takes the frames it needs out of the
//! ring ([`StreamReader::read`]), which takes no memory and waits for nothing. What is not
//! there yet is silence, and is counted.
//!
//! The ring carries the track's audio in the order it sounds, always two channels: the
//! plain audio of the clips, silence between them, the turn of a loop already laid out.
//! Frames are counted from the engine's anchor, as `Transport` counts them. When the place
//! jumps (a locate, play after stop, the loop turned on or off, another map, other clips)
//! the engine makes a new **generation** and says where it is now; the thread starts again
//! from there and what is left of the old generation in the ring is passed over.

use meloet_model::TempoMap;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::clips::song_frame_at;

/// How much audio the ring of a track holds.
pub const STREAM_RING_SECONDS: f64 = 2.0;
/// The read-ahead thread fills a ring again when less than this is left in it.
pub const STREAM_REFILL_SECONDS: f64 = 1.0;
/// The ring is always stereo; a mono file goes to both sides.
pub const STREAM_CHANNELS: usize = 2;

/// What the engine tells the read-ahead thread about one vocal track, on a ring of its own
/// ([`want_ring`]): sent when the generation changes, and while stopped whenever the place
/// does, so that the first two seconds from there are always in the ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamWant {
    /// The seat of the track.
    pub track: u32,
    /// Counts up for the track; chunks of another generation are not played.
    pub generation: u32,
    /// The tempo map the anchor is on (`EngineCommand::SetTempoMap`).
    pub map_gen: u32,
    /// The engine's anchor on the map, in seconds, and `frame` counted from it: the first
    /// frame the engine will ask for. The frames a track with a look-ahead (a plugin's
    /// latency, 20-8) is ahead by are already in it.
    pub anchor_sec: f64,
    pub frame: u64,
    /// The loop the song turns in, in ticks; `None`: no loop. The thread lays the turn out
    /// as the engine makes it (the end at `frame_of(end)`, then the start exactly).
    pub loop_ticks: Option<(u64, u64)>,
}

/// How the frames of a generation lie on the song: the one place where the read-ahead
/// thread works out what the engine will ask for, the turns of the loop already in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLayout {
    /// The song frame of the anchor.
    base: u64,
    turn: Option<Turn>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Turn {
    /// The first frame, from the anchor, at or past the loop's end.
    first_end: u64,
    /// The song frame of the loop's start, and how many frames a lap from there has.
    start: u64,
    lap: u64,
}

impl StreamLayout {
    /// `map`: the one with the want's `map_gen`.
    pub fn new(want: &StreamWant, map: &TempoMap, sample_rate: f64) -> StreamLayout {
        let turn = want.loop_ticks.map(|(start, end)| {
            let start_sec = map.seconds_at(start as f64);
            Turn {
                first_end: map.frame_of(end as f64, want.anchor_sec, sample_rate),
                start: song_frame_at(start_sec, 0, sample_rate),
                lap: map.frame_of(end as f64, start_sec, sample_rate).max(1),
            }
        });
        StreamLayout {
            base: song_frame_at(want.anchor_sec, 0, sample_rate),
            turn,
        }
    }

    /// The song frame that sounds at `frame` of the generation, and how many frames from
    /// there on follow one another in the song (up to where the loop turns).
    pub fn at(&self, frame: u64) -> (u64, u64) {
        match self.turn {
            Some(turn) if frame >= turn.first_end => {
                let into = (frame - turn.first_end) % turn.lap;
                (turn.start + into, turn.lap - into)
            }
            Some(turn) => (self.base + frame, turn.first_end - frame),
            None => (self.base + frame, u64::MAX),
        }
    }
}

/// The read-ahead thread's end of the ring of wants ([`want_ring`],
/// `EngineHandle::take_stream_wants`): `pop()` gives the next one.
pub type StreamWants = Consumer<StreamWant>;

/// A ring for what the engine wants of every track: the engine writes, the read-ahead
/// thread reads.
pub fn want_ring(capacity: usize) -> (Producer<StreamWant>, Consumer<StreamWant>) {
    RingBuffer::new(capacity.max(1))
}

/// A run of frames as the reader finds it in the ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamChunk {
    pub generation: u32,
    /// Which frame the run starts with, counted from the anchor of its generation. A run
    /// that starts later than the one before ended says the thread skipped ahead (it had
    /// fallen behind): the frames between are silence.
    pub first_frame: u64,
    pub frames: u32,
}

/// The read-ahead thread's end.
pub struct StreamWriter {
    chunks: Producer<StreamChunk>,
    samples: Producer<f32>,
    capacity_frames: usize,
}

/// The engine's end.
pub struct StreamReader {
    chunks: Consumer<StreamChunk>,
    samples: Consumer<f32>,
    /// The chunk being read, and how many of its frames are taken.
    open: Option<(StreamChunk, u32)>,
}

/// What one [`StreamReader::read`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct StreamRead {
    /// Frames that came out of the ring.
    pub frames: u32,
    /// Frames that were not there and are silence.
    pub missing: u32,
}

/// The ring of one vocal track at `sample_rate`. Everything it will ever use is taken here.
pub fn stream_ring(sample_rate: u32) -> (StreamWriter, StreamReader) {
    let frames = (f64::from(sample_rate) * STREAM_RING_SECONDS)
        .ceil()
        .max(1.0) as usize;
    let (samples_tx, samples_rx) = RingBuffer::new(frames * STREAM_CHANNELS);
    // Room for a header every 64 frames.
    let (chunks_tx, chunks_rx) = RingBuffer::new(frames / 64 + 64);
    (
        StreamWriter {
            chunks: chunks_tx,
            samples: samples_tx,
            capacity_frames: frames,
        },
        StreamReader {
            chunks: chunks_rx,
            samples: samples_rx,
            open: None,
        },
    )
}

impl StreamWriter {
    /// How many frames are in the ring and not yet taken.
    pub fn waiting_frames(&self) -> usize {
        self.capacity_frames - self.samples.slots() / STREAM_CHANNELS
    }

    /// How many frames one `push` may carry now.
    pub fn room_frames(&self) -> usize {
        match self.chunks.slots() {
            0 => 0,
            _ => self.samples.slots() / STREAM_CHANNELS,
        }
    }

    /// Puts a run in: `interleaved` holds `chunk.frames` frames, left then right. Returns
    /// whether it went in; nothing goes in when there is no room for all of it.
    pub fn push(&mut self, chunk: StreamChunk, interleaved: &[f32]) -> bool {
        debug_assert_eq!(interleaved.len(), chunk.frames as usize * STREAM_CHANNELS);
        if self.chunks.slots() == 0 || self.samples.push_entire_slice(interleaved).is_err() {
            return false;
        }
        // The samples first, the header after: a reader that sees a header finds its samples.
        let _ = self.chunks.push(chunk);
        true
    }

    /// The engine let go of its end: the track is gone, or the engine was made again.
    pub fn is_abandoned(&self) -> bool {
        self.chunks.is_abandoned()
    }
}

impl StreamReader {
    /// Lets `frames` frames of the open chunk go unread.
    fn skip(&mut self, frames: u32) {
        if let Ok(chunk) = self.samples.read_chunk(frames as usize * STREAM_CHANNELS) {
            chunk.commit_all();
        }
    }

    /// Fills `out` (interleaved, left then right) with the frames of `generation` from
    /// `frame` on, `out.len() / 2` of them. Chunks of other generations and frames before
    /// `frame` are passed over; frames that are not in the ring are silence. Never waits and
    /// never takes memory.
    pub fn read(&mut self, generation: u32, frame: u64, out: &mut [f32]) -> StreamRead {
        out.fill(0.0);
        let want = (out.len() / STREAM_CHANNELS) as u32;
        let (mut done, mut got) = (0u32, 0u32);
        while done < want {
            let (chunk, taken) = match self.open {
                Some(open) => open,
                None => match self.chunks.pop() {
                    Ok(chunk) => (chunk, 0),
                    Err(_) => break,
                },
            };
            let left = chunk.frames - taken;
            if chunk.generation != generation || left == 0 {
                self.skip(left);
                self.open = None;
                continue;
            }
            let (at, now) = (
                chunk.first_frame + u64::from(taken),
                frame + u64::from(done),
            );
            if at < now {
                // Frames of a block that is over: the engine went on without them.
                let old = (now - at).min(u64::from(left)) as u32;
                self.skip(old);
                self.open = Some((chunk, taken + old));
                continue;
            }
            if at > now {
                // The thread skipped ahead: silence up to where it went on.
                done += (at - now).min(u64::from(want - done)) as u32;
                self.open = Some((chunk, taken));
                continue;
            }
            let take = left.min(want - done);
            let room = done as usize * STREAM_CHANNELS..(done + take) as usize * STREAM_CHANNELS;
            let filled = self.samples.pop_entire_slice(&mut out[room]).is_ok();
            debug_assert!(filled, "a header comes after its samples");
            done += take;
            got += take;
            self.open = Some((chunk, taken + take));
        }
        StreamRead {
            frames: got,
            missing: want - got,
        }
    }

    /// Lets go of what a `read(generation, frame, …)` would pass over, and takes nothing:
    /// chunks of other generations and frames before `frame`. While the song stands still
    /// the engine does this every block, so that what is left of an old generation does not
    /// keep the ring full. Never waits and never takes memory.
    pub fn settle(&mut self, generation: u32, frame: u64) {
        loop {
            let (chunk, taken) = match self.open {
                Some(open) => open,
                None => match self.chunks.pop() {
                    Ok(chunk) => (chunk, 0),
                    Err(_) => return,
                },
            };
            let left = chunk.frames - taken;
            let at = chunk.first_frame + u64::from(taken);
            if chunk.generation != generation || left == 0 {
                self.skip(left);
                self.open = None;
            } else if at < frame {
                let old = (frame - at).min(u64::from(left)) as u32;
                self.skip(old);
                self.open = Some((chunk, taken + old));
            } else {
                self.open = Some((chunk, taken));
                return;
            }
        }
    }

    /// The read-ahead thread let go of its end.
    pub fn is_abandoned(&self) -> bool {
        self.chunks.is_abandoned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run whose every sample says which frame it is: left `frame`, right `-frame`.
    fn run(generation: u32, first_frame: u64, frames: u32) -> (StreamChunk, Vec<f32>) {
        let samples = (first_frame..first_frame + u64::from(frames))
            .flat_map(|f| [f as f32, -(f as f32)])
            .collect();
        (
            StreamChunk {
                generation,
                first_frame,
                frames,
            },
            samples,
        )
    }

    fn push(writer: &mut StreamWriter, generation: u32, first_frame: u64, frames: u32) -> bool {
        let (chunk, samples) = run(generation, first_frame, frames);
        writer.push(chunk, &samples)
    }

    fn read(
        reader: &mut StreamReader,
        generation: u32,
        frame: u64,
        frames: usize,
    ) -> (Vec<f32>, StreamRead) {
        let mut out = vec![9.0; frames * STREAM_CHANNELS];
        let found = reader.read(generation, frame, &mut out);
        (out.iter().step_by(2).copied().collect(), found)
    }

    #[test]
    fn frames_come_out_in_order_across_chunks_and_blocks() {
        let (mut writer, mut reader) = stream_ring(48_000);
        assert_eq!(writer.room_frames(), 96_000);
        assert!(push(&mut writer, 1, 0, 100));
        assert!(push(&mut writer, 1, 100, 50));
        assert_eq!(writer.waiting_frames(), 150);
        let (left, found) = read(&mut reader, 1, 0, 64);
        assert_eq!(left, (0..64).map(|f| f as f32).collect::<Vec<_>>());
        assert_eq!(
            found,
            StreamRead {
                frames: 64,
                missing: 0
            }
        );
        let (left, found) = read(&mut reader, 1, 64, 64);
        assert_eq!(left, (64..128).map(|f| f as f32).collect::<Vec<_>>());
        assert_eq!(found.missing, 0);
        // The ring runs dry in the middle of a block: the rest is silence, and counted.
        let (left, found) = read(&mut reader, 1, 128, 64);
        assert_eq!(
            left[..22],
            (128..150).map(|f| f as f32).collect::<Vec<_>>()[..]
        );
        assert!(left[22..].iter().all(|&s| s == 0.0));
        assert_eq!(
            found,
            StreamRead {
                frames: 22,
                missing: 42
            }
        );
        assert_eq!(writer.waiting_frames(), 0);
        // Both channels.
        assert!(push(&mut writer, 1, 192, 2));
        let mut out = [9.0; 4];
        reader.read(1, 192, &mut out);
        assert_eq!(out, [192.0, -192.0, 193.0, -193.0]);
    }

    #[test]
    fn a_new_generation_passes_over_what_is_left_of_the_old_one() {
        let (mut writer, mut reader) = stream_ring(48_000);
        push(&mut writer, 1, 0, 1_000);
        push(&mut writer, 1, 1_000, 1_000);
        read(&mut reader, 1, 0, 256);
        // The place jumped: the engine asks for generation 2 before the thread has any.
        let (left, found) = read(&mut reader, 2, 0, 256);
        assert!(left.iter().all(|&s| s == 0.0));
        assert_eq!(
            found,
            StreamRead {
                frames: 0,
                missing: 256
            }
        );
        assert_eq!(writer.waiting_frames(), 0);
        // It comes, from where the engine said it would be by then.
        push(&mut writer, 2, 5_000, 512);
        let (left, found) = read(&mut reader, 2, 5_000, 256);
        assert_eq!(left[0], 5_000.0);
        assert_eq!(found.frames, 256);
    }

    #[test]
    fn frames_of_blocks_that_are_over_are_let_go_and_a_skip_ahead_is_silence() {
        let (mut writer, mut reader) = stream_ring(48_000);
        // The thread was late: its run starts before where the engine is now.
        push(&mut writer, 3, 0, 300);
        let (left, found) = read(&mut reader, 3, 256, 64);
        assert_eq!(left[0], 256.0);
        assert_eq!(
            found,
            StreamRead {
                frames: 44,
                missing: 20
            }
        );
        // It skips ahead to frame 400: silence up to there, then audio in its place.
        push(&mut writer, 3, 400, 100);
        let (left, found) = read(&mut reader, 3, 320, 128);
        assert!(left[..80].iter().all(|&s| s == 0.0));
        assert_eq!(left[80], 400.0);
        assert_eq!(left[127], 447.0);
        assert_eq!(
            found,
            StreamRead {
                frames: 48,
                missing: 80
            }
        );
        // A run wholly in the past is gone in one read.
        push(&mut writer, 3, 500, 10);
        push(&mut writer, 3, 600, 10);
        let (left, _) = read(&mut reader, 3, 600, 10);
        assert_eq!(left[0], 600.0);
    }

    #[test]
    fn settling_lets_go_of_the_old_generation_and_keeps_the_new() {
        let (mut writer, mut reader) = stream_ring(1_000);
        assert!(push(&mut writer, 1, 0, 2_000));
        assert_eq!(writer.room_frames(), 0);
        reader.settle(2, 500);
        assert_eq!(writer.waiting_frames(), 0);
        assert!(push(&mut writer, 2, 400, 1_000));
        reader.settle(2, 500);
        assert_eq!(writer.waiting_frames(), 900);
        reader.settle(2, 500);
        assert_eq!(writer.waiting_frames(), 900);
        let (left, found) = read(&mut reader, 2, 500, 4);
        assert_eq!(left, [500.0, 501.0, 502.0, 503.0]);
        assert_eq!(found.missing, 0);
    }

    #[test]
    fn a_full_ring_takes_nothing_of_a_run() {
        let (mut writer, mut reader) = stream_ring(1_000);
        assert!(push(&mut writer, 1, 0, 1_500));
        assert_eq!(writer.room_frames(), 500);
        assert!(!push(&mut writer, 1, 1_500, 501));
        assert_eq!(writer.waiting_frames(), 1_500);
        assert!(push(&mut writer, 1, 1_500, 500));
        assert_eq!(writer.room_frames(), 0);
        let (left, found) = read(&mut reader, 1, 0, 2_000);
        assert_eq!(
            found,
            StreamRead {
                frames: 2_000,
                missing: 0
            }
        );
        assert_eq!(left[1_999], 1_999.0);
        assert!(!reader.is_abandoned());
        drop(writer);
        assert!(reader.is_abandoned());
    }

    #[test]
    fn a_layout_follows_the_turns_of_the_loop() {
        use meloet_model::{Meter, Tempo, Timeline};
        // 120 a minute at 48 kHz: a quarter note is 24000 frames.
        let tempo = Tempo::from_centi(12_000).unwrap();
        let map = TempoMap::new(&Timeline::new(tempo, Meter::FOUR_FOUR));
        let mut want = StreamWant {
            track: 0,
            generation: 1,
            map_gen: 1,
            anchor_sec: 0.25,
            frame: 0,
            loop_ticks: None,
        };
        let straight = StreamLayout::new(&want, &map, 48_000.0);
        assert_eq!(straight.at(0), (12_000, u64::MAX));
        assert_eq!(straight.at(500_000).0, 512_000);
        want.loop_ticks = Some((1_920, 3_840));
        let turning = StreamLayout::new(&want, &map, 48_000.0);
        assert_eq!(turning.at(0), (12_000, 36_000));
        assert_eq!(turning.at(35_999), (47_999, 1));
        assert_eq!(turning.at(36_000), (24_000, 24_000));
        assert_eq!(turning.at(36_000 + 24_000 + 5), (24_005, 23_995));
        assert_eq!(turning.at(36_000 + 10 * 24_000 - 1), (47_999, 1));
    }

    #[test]
    fn wants_cross_on_a_ring() {
        let (mut tx, mut rx) = want_ring(8);
        let want = StreamWant {
            track: 5,
            generation: 2,
            map_gen: 7,
            anchor_sec: 10.0,
            frame: 4_800,
            loop_ticks: Some((7_680, 15_360)),
        };
        tx.push(want).unwrap();
        assert_eq!(rx.pop(), Ok(want));
    }
}
