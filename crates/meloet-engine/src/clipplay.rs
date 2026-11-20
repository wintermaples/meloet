//! The vocal clips of a track on the audio thread (docs/architecture.md 22-8, 22-18 A1):
//! every stretch takes its frames out of the track's ring and multiplies them by what the
//! clip table says, and tells the read-ahead thread when the place jumps.

use rtrb::Producer;

use crate::clips::{ClipTable, song_frame_at};
use crate::stream::{STREAM_CHANNELS, StreamReader, StreamWant};

/// Where the song is for a track, at the first frame of a stretch.
#[derive(Clone, Copy)]
pub(crate) struct StreamNow {
    /// Frames from the anchor, the track's look-ahead already in it.
    pub at: u64,
    pub anchor_sec: f64,
    pub sample_rate: f64,
    /// While the loop turns: the first frame at or past its end, and its start in seconds.
    pub wrap: Option<(u64, f64)>,
}

impl StreamNow {
    fn song_frame(&self, at: u64) -> u64 {
        match self.wrap {
            Some((end, start_sec)) if at >= end => {
                song_frame_at(start_sec, at - end, self.sample_rate)
            }
            _ => song_frame_at(self.anchor_sec, at, self.sample_rate),
        }
    }
}

/// Where the read-ahead thread is to start from, the same for every track but for its
/// look-ahead.
#[derive(Clone, Copy)]
pub(crate) struct StreamPlace {
    pub map_gen: u32,
    pub anchor_sec: f64,
    pub frame: u64,
    pub loop_ticks: Option<(u64, u64)>,
}

/// What a stretch needs besides the track.
pub(crate) struct StreamCtx<'a> {
    /// Interleaved, two channels, as long as the longest stretch.
    pub buf: &'a mut [f32],
    pub retired: &'a mut Producer<Box<ClipTable>>,
    pub wants: &'a mut Producer<StreamWant>,
    /// `None`: the song stands still (stopped, or counting in).
    pub now: Option<StreamNow>,
    /// 10 ms and 20 ms in frames.
    pub fade: u32,
    pub ramp: u32,
}

pub(crate) struct ClipPlayer {
    reader: Option<Box<StreamReader>>,
    table: Option<Box<ClipTable>>,
    /// The table before, while the new one comes in over it.
    before: Option<Box<ClipTable>>,
    blend_left: u32,
    generation: u32,
    /// The frame of `generation` the next stretch asks the ring for.
    next: u64,
    /// The want that made `generation`.
    said: Option<StreamWant>,
    /// The place jumped and the thread has not been told.
    stale: bool,
    /// The look-ahead the last want counted in.
    ahead: u32,
    /// What sounded before a jump, fading out: its generation and next frame.
    tail: Option<(u32, u64)>,
    /// 0 to 1: the track plays its clips (mute, solo, a jump).
    gate: f32,
    /// 0 to 1: audio that came late into the ring comes in smoothly.
    rise: f32,
    /// What the table said for the last frame that sounded.
    held: f32,
    /// The last stretch read nothing: the next one starts at once, without a ramp.
    idle: bool,
    /// A whole stretch of this generation came out of the ring already: frames that are
    /// not there from now on are the thread being late, not the thread starting.
    fed: bool,
    told_missing: bool,
}

impl ClipPlayer {
    pub fn new() -> ClipPlayer {
        ClipPlayer {
            reader: None,
            table: None,
            before: None,
            blend_left: 0,
            generation: 0,
            next: 0,
            said: None,
            stale: false,
            ahead: 0,
            tail: None,
            gate: 0.0,
            rise: 1.0,
            held: 0.0,
            idle: true,
            fed: false,
            told_missing: false,
        }
    }

    /// Something may sound here.
    pub fn is_active(&self) -> bool {
        self.reader.is_some() && (self.table.is_some() || self.tail.is_some())
    }

    /// The place jumped: what sounds fades out, and the thread is told at the next block.
    /// `anew`: also when the place turns out to be the one the ring was filled for.
    pub fn jump(&mut self, anew: bool) {
        if !self.stale && self.gate > 0.0 && self.tail.is_none() {
            self.tail = Some((self.generation, self.next));
        }
        self.stale = true;
        if anew {
            self.said = None;
        }
    }

    /// Another ring. The one before comes back.
    pub fn set_reader(&mut self, reader: Option<Box<StreamReader>>) -> Option<Box<StreamReader>> {
        self.tail = None;
        self.gate = 0.0;
        self.idle = true;
        self.stale = true;
        self.said = None;
        std::mem::replace(&mut self.reader, reader)
    }

    /// Another table. `refeed`: the clips are other audio now, not only other volumes and
    /// fades. Needs room for two in `retired`.
    pub fn set_table(
        &mut self,
        table: Option<Box<ClipTable>>,
        refeed: bool,
        ramp: u32,
        retired: &mut Producer<Box<ClipTable>>,
    ) {
        let old = std::mem::replace(&mut self.table, table);
        if let Some(before) = self.before.take() {
            let _ = retired.push(before);
        }
        self.blend_left = 0;
        let sounding = self.gate > 0.0 && self.tail.is_none() && !self.stale;
        if !refeed && sounding && self.table.is_some() && old.is_some() {
            self.before = old;
            self.blend_left = ramp;
        } else if let Some(old) = old {
            let _ = retired.push(old);
        }
        if refeed {
            self.jump(true);
        }
    }

    /// Everything that has to be freed comes out. Needs room for two tables.
    pub fn clear(&mut self, retired: &mut Producer<Box<ClipTable>>) -> Option<Box<StreamReader>> {
        for table in [self.table.take(), self.before.take()]
            .into_iter()
            .flatten()
        {
            let _ = retired.push(table);
        }
        let reader = self.reader.take();
        *self = ClipPlayer::new();
        reader
    }

    /// A new stream is about to run, or none does: nothing fades.
    pub fn rest(&mut self) {
        self.tail = None;
        self.gate = 0.0;
        self.rise = 1.0;
        self.idle = true;
        self.blend_left = 0;
        self.stale = true;
        self.said = None;
    }

    /// Before a block: tells the thread where to read from, when the place jumped or the
    /// track's look-ahead changed. A place the ring is filled for already stays.
    pub fn aim(
        &mut self,
        track: usize,
        lookahead: u32,
        place: StreamPlace,
        wants: &mut Producer<StreamWant>,
    ) {
        self.told_missing = false;
        if self.reader.is_none() {
            return;
        }
        if lookahead != self.ahead {
            self.ahead = lookahead;
            self.jump(false);
        }
        if !self.stale {
            return;
        }
        let want = StreamWant {
            track: track as u32,
            generation: self.generation,
            map_gen: place.map_gen,
            anchor_sec: place.anchor_sec,
            frame: place.frame + u64::from(lookahead),
            loop_ticks: place.loop_ticks,
        };
        if self.said == Some(want) && self.next == want.frame {
            self.stale = false;
            return;
        }
        let want = StreamWant {
            generation: self.generation.wrapping_add(1),
            ..want
        };
        // A full ring: the track is silent until the want goes out with a later block.
        if wants.push(want).is_ok() {
            self.generation = want.generation;
            self.fed = false;
            self.next = want.frame;
            self.said = Some(want);
            self.stale = false;
        }
    }

    fn end_blend(&mut self, retired: &mut Producer<Box<ClipTable>>) {
        self.blend_left = 0;
        if !retired.is_full()
            && let Some(before) = self.before.take()
        {
            let _ = retired.push(before);
        }
    }

    /// Adds the clips over a stretch to `left` and `right`. `plays`: the track is not
    /// muted. Returns how many frames the thread was late with (not the ones a new
    /// generation starts with after a jump while the song plays).
    pub fn render(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        plays: bool,
        ctx: &mut StreamCtx,
    ) -> u32 {
        let frames = left.len();
        let moving = ctx.now.is_some();
        let Some(reader) = self.reader.as_deref_mut() else {
            return 0;
        };
        let buf = &mut ctx.buf[..frames * STREAM_CHANNELS];
        if let Some((generation, from)) = self.tail {
            reader.read(generation, from, buf);
            let step = 1.0 / ctx.fade.max(1) as f32;
            for (i, frame) in buf.as_chunks::<2>().0.iter().enumerate() {
                self.gate = (self.gate - step).max(0.0);
                let gain = self.gate * self.held * self.rise;
                left[i] += frame[0] * gain;
                right[i] += frame[1] * gain;
            }
            self.tail = (self.gate > 0.0).then_some((generation, from + frames as u64));
            if moving {
                self.next += frames as u64;
            }
            self.end_blend(ctx.retired);
            return 0;
        }
        let (Some(now), Some(table), false) = (ctx.now, self.table.as_deref(), self.stale) else {
            if !self.stale {
                if moving {
                    self.next += frames as u64;
                }
                reader.settle(self.generation, self.next);
            }
            self.gate = 0.0;
            self.rise = 1.0;
            self.idle = true;
            self.end_blend(ctx.retired);
            return 0;
        };

        let found = reader.read(self.generation, self.next, buf);
        self.next += frames as u64;
        let target = if plays { 1.0 } else { 0.0 };
        if std::mem::take(&mut self.idle) {
            self.gate = target;
        }
        if found.missing > 0 {
            // What did come of this stretch is left out: it comes in from silence once the
            // ring has all of a stretch again.
            self.rise = 0.0;
            self.gate = target;
            self.end_blend(ctx.retired);
            if !std::mem::replace(&mut self.told_missing, true)
                && let Some(said) = self.said
            {
                // Where the engine is now: the thread goes on from here, not from where it
                // fell behind.
                let _ = ctx.wants.push(StreamWant {
                    frame: self.next,
                    ..said
                });
            }
            return if self.fed { found.missing } else { 0 };
        }
        self.fed = true;

        let step = 1.0 / ctx.fade.max(1) as f32;
        let ramp = ctx.ramp.max(1) as f32;
        let regions = table.regions();
        // The stretch may go over the loop's end for a track that reads ahead.
        let first = match now.wrap {
            Some((end, _)) if now.at < end => frames.min((end - now.at) as usize),
            _ => frames,
        };
        for (from, to) in [(0, first), (first, frames)] {
            if from == to {
                continue;
            }
            let song = now.song_frame(now.at + from as u64);
            let mut index = table.from(song);
            for i in from..to {
                let at = song + (i - from) as u64;
                while regions.get(index).is_some_and(|r| r.end <= at) {
                    index += 1;
                }
                let mut gain = regions.get(index).map_or(0.0, |r| r.gain_at(at));
                if self.blend_left > 0 {
                    if let Some(before) = self.before.as_deref() {
                        let old = self.blend_left as f32 / ramp;
                        gain += (before.gain_at(at) - gain) * old.min(1.0);
                    }
                    self.blend_left -= 1;
                }
                self.held = gain;
                self.gate = if target > self.gate {
                    (self.gate + step).min(1.0)
                } else {
                    (self.gate - step).max(target)
                };
                self.rise = (self.rise + step).min(1.0);
                let gain = gain * self.gate * self.rise;
                left[i] += buf[2 * i] * gain;
                right[i] += buf[2 * i + 1] * gain;
            }
        }
        if self.blend_left == 0 && self.before.is_some() {
            self.end_blend(ctx.retired);
        }
        0
    }
}
