//! The song and how it plays: the transport, recording, clips, takes and undo. Pure rules like
//! [`crate::state`]: commands and the engine's reports come in, effects go out.

use std::sync::Arc;

use meloet_model::edit::{self, Edit};
use meloet_model::playback::{PlayEvent, track_events};
use meloet_model::record::{HeldControls, RecEvent, RecEventKind, Recorder};
use meloet_model::{ClipId, Doc, History, Meter, Song, TakeId, Tempo, TickRange, TrackId};
use meloet_protocol::{
    ArrangeView, BarSpan, ClipView, Grid, LiveNote, LiveNotes, LiveNotesSnapshot, MusicalScale,
    PianoRollView, QuantizeAlign, QuantizeSettings, RecordingView, ScaleMode, StatusMessage,
    TakeControl, TakeControlValue, TakeNote, TakeNotes, TickSpan, TrackRef, TransportState,
    TransportView,
};

use crate::state::Effect;

/// Bars the loop gets when it is turned on with no range.
const DEFAULT_LOOP_BARS: u64 = 4;
pub const COUNT_IN_BARS: [u8; 3] = [0, 1, 2];

/// What the listener hears now, read from the engine's clock by the core thread.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heard {
    pub tick: f64,
    pub state: TransportState,
    pub lap: u32,
}

impl Heard {
    fn moving_tick(&self) -> Option<u64> {
        matches!(
            self.state,
            TransportState::Playing | TransportState::Recording
        )
        .then(|| self.tick.max(0.0).floor() as u64)
    }
}

/// What the engine says about a recording, in the order it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordReport {
    Started {
        /// Where the recording starts; while playing, the engine says where the listener was.
        from: u64,
        held: HeldControls,
    },
    Event {
        kind: RecEventKind,
        t_us: u64,
        tick: f64,
        lap: u32,
    },
    LapChanged {
        lap: u32,
    },
    Stopped {
        t_us: u64,
        tick: f64,
        lap: u32,
    },
}

/// Commands to the engine. The core thread turns them into `EngineCommand`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineOp {
    Play {
        from: u64,
    },
    Record {
        from: u64,
        count_in_beats: u32,
        /// Track index that does not play its sequence while recording.
        muted: usize,
    },
    /// While playing: record from where the engine is, without a jump.
    RecordHere {
        muted: usize,
    },
    Stop {
        t_us: u64,
    },
    Locate {
        tick: u64,
    },
    Tempo {
        ticks_per_beat: u32,
        beats_per_bar: u32,
        bpm_centi: u32,
    },
    Loop {
        start: u64,
        end: u64,
        on: bool,
    },
    Metronome {
        on: bool,
    },
    /// The events of the track at this index of the song.
    Sequence {
        track: usize,
        events: Vec<PlayEvent>,
    },
}

/// Why a recording was cut short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    AudioSystem,
    Output,
}

struct Rec {
    id: u32,
    track: TrackId,
    from: u64,
    /// The number the take of the current lap gets: laps with no notes make no take.
    take: u32,
    clip: Option<ClipId>,
    count_in_beats: u32,
    loop_range: Option<TickRange>,
    lap: u32,
    /// Made when the engine says it started.
    recorder: Option<Recorder>,
    /// When stop was asked; keys after it are not recorded.
    stop_us: Option<u64>,
    live: Vec<LiveNote>,
    /// Index into `live` of the note each key holds.
    held: [Option<usize>; 128],
    live_lap: u32,
    next_index: u32,
}

impl Rec {
    fn floor(&self, lap: u32) -> i64 {
        match self.loop_range {
            Some(r) if lap > 0 => r.start() as i64,
            _ => 0,
        }
    }

    fn ceil(&self) -> i64 {
        self.loop_range.map_or(i64::MAX, |r| r.end() as i64)
    }

    /// Cuts the notes still held from earlier laps at the loop end. Returns what changed.
    fn enter_lap(&mut self, lap: u32, changed: &mut Vec<usize>) {
        if lap <= self.live_lap {
            return;
        }
        if self.next_index > 0 {
            self.take += 1;
        }
        self.live_lap = lap;
        self.next_index = 0;
        let ceil = self.ceil();
        for slot in &mut self.held {
            if let Some(i) = slot.take() {
                let n = &mut self.live[i];
                n.length = Some((ceil - n.start as i64).max(1) as u64);
                changed.push(i);
            }
        }
    }

    fn release(&mut self, key: u8, tick: i64, changed: &mut Vec<usize>) {
        if let Some(i) = self.held[usize::from(key)].take() {
            let end = tick.min(self.ceil());
            let n = &mut self.live[i];
            n.length = Some((end - n.start as i64).max(1) as u64);
            changed.push(i);
        }
    }

    fn press(&mut self, key: u8, velocity: u8, tick: i64, lap: u32, changed: &mut Vec<usize>) {
        let start = tick.max(self.floor(lap)).min(self.ceil() - 1).max(0) as u64;
        self.live.push(LiveNote {
            lap,
            index: self.next_index,
            key,
            velocity,
            start,
            length: None,
        });
        self.next_index += 1;
        let i = self.live.len() - 1;
        self.held[usize::from(key)] = Some(i);
        changed.push(i);
    }
}

pub struct Play {
    song: Song,
    history: History,
    state: TransportState,
    /// Where play and record start from; while moving, where it started.
    position: u64,
    metronome: bool,
    count_in_bars: u8,
    selected: Option<ClipId>,
    piano_roll: Option<ClipId>,
    rec: Option<Rec>,
    next_recording: u32,
    live_seq: u64,
    now_us: u64,
    heard: Option<Heard>,
    /// The last heard position while moving, for when the stream goes away.
    last_heard: Option<Heard>,
    /// A clip has been in this song: the empty arrange hint is not shown again.
    had_clip: bool,
    /// The song came from the copy kept before the last save: the file needs writing again
    /// whatever undo does.
    unsaved_anyway: bool,
}

impl Play {
    pub fn new(metronome: bool, count_in_bars: u8) -> Play {
        Play {
            song: Song::new_default(),
            history: History::new(),
            state: TransportState::Stopped,
            position: 0,
            metronome,
            count_in_bars: if COUNT_IN_BARS.contains(&count_in_bars) {
                count_in_bars
            } else {
                1
            },
            selected: None,
            piano_roll: None,
            rec: None,
            next_recording: 1,
            live_seq: 0,
            now_us: 0,
            heard: None,
            last_heard: None,
            had_clip: false,
            unsaved_anyway: false,
        }
    }

    pub fn metronome(&self) -> bool {
        self.metronome
    }

    pub fn count_in_bars(&self) -> u8 {
        self.count_in_bars
    }

    pub fn song(&self) -> &Song {
        &self.song
    }

    pub fn had_clip(&self) -> bool {
        self.had_clip || self.doc().tracks.iter().any(|t| !t.clips.is_empty())
    }

    pub fn is_recording(&self) -> bool {
        self.rec.is_some()
    }

    pub fn is_moving(&self) -> bool {
        self.state != TransportState::Stopped || self.rec.is_some()
    }

    fn doc(&self) -> &Doc {
        &self.song.doc
    }

    fn meter(&self) -> Meter {
        self.doc().meter
    }

    /// Counting in or recording: the song is not to be changed or replaced.
    pub fn busy(&self) -> bool {
        matches!(
            self.state,
            TransportState::CountIn | TransportState::Recording
        ) || self.rec.is_some()
    }

    /// The time and what is heard, before each input.
    pub fn set_now(&mut self, t_us: u64, heard: Option<Heard>) {
        self.now_us = t_us;
        self.heard = heard;
        if let Some(h) = heard
            && self.state != TransportState::Stopped
        {
            self.last_heard = Some(h);
            if self.state == TransportState::CountIn && h.state == TransportState::Recording {
                self.state = TransportState::Recording;
            }
        }
    }

    pub fn armed(&self) -> Option<TrackRef> {
        self.track_ref(self.song.session.armed?)
    }

    fn track_ref(&self, id: TrackId) -> Option<TrackRef> {
        self.doc().track(id).map(|t| TrackRef {
            number: t.id.0,
            name: t.name.clone(),
        })
    }

    fn track_index(&self, id: TrackId) -> Option<usize> {
        self.doc().tracks.iter().position(|t| t.id == id)
    }

    fn bar_head(&self, tick: u64) -> u64 {
        let per_bar = self.meter().ticks_per_bar();
        tick / per_bar * per_bar
    }

    fn bars(&self, range: TickRange) -> BarSpan {
        let per_bar = self.meter().ticks_per_bar();
        BarSpan {
            from: (range.start() / per_bar + 1) as u32,
            to: (range.end().div_ceil(per_bar) + 1) as u32,
        }
    }

    fn loop_on(&self) -> Option<TickRange> {
        self.song
            .session
            .loop_range
            .filter(|_| self.song.session.loop_on)
    }

    // ---- commands ----

    /// Space.
    pub fn play_stop(&mut self, can_play: bool, effects: &mut Vec<Effect>) {
        match self.state {
            TransportState::Stopped if self.rec.is_some() => {}
            TransportState::Stopped => {
                if !can_play {
                    return;
                }
                let from = match self.loop_on() {
                    Some(r) if self.position < r.start() || self.position >= r.end() => r.start(),
                    _ => self.position,
                };
                self.position = from;
                self.state = TransportState::Playing;
                self.last_heard = None;
                effects.push(Effect::Engine(EngineOp::Play { from }));
            }
            _ => self.stop(effects),
        }
    }

    /// R and the record button. Returns the notice to show.
    pub fn record(&mut self, can_play: bool, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        match self.state {
            TransportState::CountIn | TransportState::Recording => {
                self.stop(effects);
                return None;
            }
            _ if self.rec.is_some() || !can_play => return None,
            _ => {}
        }
        let Some(track) = self.song.session.armed else {
            return Some(StatusMessage::NoArmedTrack);
        };
        let index = self.track_index(track)?;
        let playing = self.state == TransportState::Playing;
        let (from, count_in_beats) = if playing {
            let now = self
                .heard
                .and_then(|h| h.moving_tick())
                .unwrap_or(self.position);
            (now, 0)
        } else {
            let from = self.loop_on().map_or(self.position, |r| r.start());
            let beats = u32::from(self.count_in_bars) * u32::from(self.meter().num());
            (from, beats)
        };
        let loop_range = self.loop_on().filter(|r| r.end() > from);
        let clip = self.doc().track(track).and_then(|t| {
            let span = loop_range.unwrap_or_else(|| TickRange::new(from, from + 1).expect("1"));
            t.clips
                .iter()
                .find(|c| c.range().overlaps(span))
                .map(|c| (c.id, c.takes.len() as u32))
        });
        let id = self.next_recording;
        self.next_recording = self.next_recording.wrapping_add(1);
        self.rec = Some(Rec {
            id,
            track,
            from,
            take: clip.map_or(1, |(_, n)| n + 1),
            clip: clip.map(|(c, _)| c),
            count_in_beats,
            loop_range,
            lap: 0,
            recorder: None,
            stop_us: None,
            live: Vec::new(),
            held: [None; 128],
            live_lap: 0,
            next_index: 0,
        });
        self.position = from;
        self.state = if count_in_beats > 0 {
            TransportState::CountIn
        } else {
            TransportState::Recording
        };
        self.last_heard = None;
        effects.push(Effect::Engine(if playing {
            EngineOp::RecordHere { muted: index }
        } else {
            EngineOp::Record {
                from,
                count_in_beats,
                muted: index,
            }
        }));
        None
    }

    fn stop(&mut self, effects: &mut Vec<Effect>) {
        let to = self
            .heard
            .and_then(|h| h.moving_tick())
            .unwrap_or(self.position);
        // Stopped in a count-in, the engine goes back to where the recording was to start.
        if self.state != TransportState::CountIn {
            self.position = to;
        }
        self.state = TransportState::Stopped;
        if let Some(rec) = &mut self.rec {
            rec.stop_us = Some(self.now_us);
        }
        effects.push(Effect::Engine(EngineOp::Stop { t_us: self.now_us }));
    }

    /// Home, and a click on the ruler (moved to the head of that bar).
    pub fn locate(&mut self, tick: u64, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let head = self.bar_head(tick);
        self.position = head;
        effects.push(Effect::Engine(EngineOp::Locate { tick: head }));
    }

    pub fn set_tempo(&mut self, bpm: f64, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Some(tempo) = Tempo::from_bpm(bpm)
            && let Ok(e) = edit::set_tempo(self.doc(), tempo)
        {
            self.apply(e, effects);
        }
    }

    pub fn set_meter(&mut self, num: u8, den: u8, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Some(meter) = Meter::new(num, den)
            && let Ok(e) = edit::set_meter(self.doc(), meter)
        {
            self.apply(e, effects);
        }
    }

    /// Returns whether it changed (the settings are then saved).
    pub fn set_count_in(&mut self, bars: u8) -> bool {
        if !COUNT_IN_BARS.contains(&bars) || bars == self.count_in_bars {
            return false;
        }
        self.count_in_bars = bars;
        true
    }

    pub fn toggle_metronome(&mut self, effects: &mut Vec<Effect>) {
        self.metronome = !self.metronome;
        effects.push(Effect::Engine(EngineOp::Metronome { on: self.metronome }));
    }

    pub fn toggle_loop(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let session = &self.song.session;
        if !session.loop_on && session.loop_range.is_none() {
            let start = self.bar_head(self.position);
            let end = start + DEFAULT_LOOP_BARS * self.meter().ticks_per_bar();
            self.song.session.loop_range = TickRange::new(start, end);
        }
        self.song.session.loop_on = !self.song.session.loop_on;
        self.send_loop(effects);
    }

    pub fn set_loop(&mut self, from_bar: u32, to_bar: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let (a, b) = (from_bar.min(to_bar).max(1), from_bar.max(to_bar).max(1));
        let b = if a == b { a + 1 } else { b };
        let per_bar = self.meter().ticks_per_bar();
        self.song.session.loop_range =
            TickRange::new(u64::from(a - 1) * per_bar, u64::from(b - 1) * per_bar);
        self.song.session.loop_on = true;
        self.send_loop(effects);
    }

    fn send_loop(&self, effects: &mut Vec<Effect>) {
        let s = &self.song.session;
        let (start, end) = s.loop_range.map_or((0, 0), |r| (r.start(), r.end()));
        effects.push(Effect::Engine(EngineOp::Loop {
            start,
            end,
            on: s.loop_on,
        }));
    }

    pub fn toggle_arm(&mut self, track: u32) {
        if self.busy() {
            return;
        }
        let id = TrackId(track);
        if self.doc().track(id).is_none() {
            return;
        }
        let armed = &mut self.song.session.armed;
        *armed = if *armed == Some(id) { None } else { Some(id) };
    }

    pub fn select_clip(&mut self, clip: Option<u32>) {
        match clip.map(ClipId) {
            None => self.selected = None,
            Some(id) if self.doc().clip(id).is_some() => {
                self.selected = Some(id);
                self.piano_roll = Some(id);
            }
            Some(_) => {}
        }
    }

    pub fn delete_selected(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Some(id) = self.selected
            && let Ok(e) = edit::delete_clips(self.doc(), &[id])
        {
            self.apply(e, effects);
            self.selected = None;
        }
    }

    /// Returns the notice to show.
    pub fn step_take(&mut self, delta: i32, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() || delta == 0 {
            return None;
        }
        let (track, clip) = self.doc().clip(self.selected?)?;
        let (track, clip) = (track.clone(), clip.clone());
        let n = clip.takes.len() as i64;
        if n < 2 {
            return None;
        }
        let to = (clip.active as i64 + i64::from(delta)).rem_euclid(n) as usize;
        let e = edit::switch_take(self.doc(), clip.id, to).ok()?;
        self.apply(e, effects);
        Some(StatusMessage::TakeSwitched {
            track: TrackRef {
                number: track.id.0,
                name: track.name.clone(),
            },
            clip: clip.name.clone(),
            take: to as u32 + 1,
            takes: n as u32,
        })
    }

    pub fn undo(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let before = self.doc().clone();
        if self.history.undo(&mut self.song.doc).is_some() {
            self.song.doc.name.clone_from(&before.name);
            self.after_change(&before, effects);
        }
    }

    pub fn redo(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let before = self.doc().clone();
        if self.history.redo(&mut self.song.doc).is_some() {
            self.song.doc.name.clone_from(&before.name);
            self.after_change(&before, effects);
        }
    }

    fn apply(&mut self, e: Edit, effects: &mut Vec<Effect>) {
        let before = self.doc().clone();
        self.history.apply(&mut self.song.doc, e);
        self.after_change(&before, effects);
    }

    /// Sends what the engine must hear again, and lets go of clips that are gone.
    fn after_change(&mut self, before: &Doc, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        for (i, t) in doc.tracks.iter().enumerate() {
            let same = before.track(t.id).is_some_and(|b| Arc::ptr_eq(b, t));
            if !same {
                effects.push(Effect::Engine(EngineOp::Sequence {
                    track: i,
                    events: track_events(doc, t.id),
                }));
            }
        }
        if doc.tempo != before.tempo || doc.meter != before.meter {
            effects.push(Effect::Engine(self.tempo_op()));
        }
        if doc.tracks.iter().any(|t| !t.clips.is_empty()) {
            self.had_clip = true;
        }
        let gone = |id: Option<ClipId>| id.filter(|&c| doc.clip(c).is_some());
        self.selected = gone(self.selected);
        self.piano_roll = gone(self.piano_roll);
    }

    fn tempo_op(&self) -> EngineOp {
        let doc = self.doc();
        EngineOp::Tempo {
            ticks_per_beat: doc.meter.ticks_per_beat() as u32,
            beats_per_bar: u32::from(doc.meter.num()),
            bpm_centi: doc.tempo.centi(),
        }
    }

    /// Everything the engine is told, for a stream that just opened. Whatever it was doing is
    /// stopped there.
    pub fn stream_opened(&mut self, effects: &mut Vec<Effect>) {
        effects.push(Effect::Engine(EngineOp::Stop { t_us: self.now_us }));
        effects.push(Effect::Engine(EngineOp::Locate {
            tick: self.position,
        }));
        effects.push(Effect::Engine(self.tempo_op()));
        self.send_loop(effects);
        effects.push(Effect::Engine(EngineOp::Metronome { on: self.metronome }));
        let doc = &self.song.doc;
        for (i, t) in doc.tracks.iter().enumerate() {
            effects.push(Effect::Engine(EngineOp::Sequence {
                track: i,
                events: track_events(doc, t.id),
            }));
        }
    }

    /// The stream is gone: stops where the listener was. A recording keeps what was played up
    /// to there; returns the number of the take it gave, if any.
    pub fn stream_gone(&mut self, effects: &mut Vec<Effect>) -> Option<u32> {
        if self.state == TransportState::Stopped && self.rec.is_none() {
            return None;
        }
        let last = self.last_heard;
        if self.state != TransportState::CountIn
            && let Some(tick) = last.and_then(|h| h.moving_tick())
        {
            self.position = tick;
        }
        self.state = TransportState::Stopped;
        let rec = self.rec.as_ref()?;
        let (tick, lap) = match last {
            Some(h) if h.state == TransportState::Recording => (h.tick, h.lap),
            _ => (rec.from as f64, 0),
        };
        self.finish(tick, lap, effects)
    }

    /// The keyboard input stopped: keys held in the recording are let go here.
    pub fn keys_lost(&mut self, effects: &mut Vec<Effect>) {
        let Some(h) = self.heard.or(self.last_heard) else {
            return;
        };
        let Some(rec) = &mut self.rec else {
            return;
        };
        let tick = h.tick.round() as i64;
        let keys: Vec<u8> = (0..128u8)
            .filter(|&k| rec.held[usize::from(k)].is_some())
            .collect();
        let Some(recorder) = &mut rec.recorder else {
            return;
        };
        for &key in &keys {
            recorder.push(RecEvent {
                tick,
                lap: h.lap,
                kind: RecEventKind::NoteOff { key },
            });
        }
        let mut changed = Vec::new();
        for key in keys {
            rec.release(key, tick, &mut changed);
        }
        self.send_live(changed, effects);
    }

    // ---- the engine's reports ----

    pub fn report(&mut self, report: RecordReport, effects: &mut Vec<Effect>) {
        match report {
            RecordReport::Started { from, held } => {
                if let Some(rec) = &mut self.rec
                    && rec.recorder.is_none()
                {
                    rec.from = from;
                    rec.loop_range = rec.loop_range.filter(|r| r.end() > from);
                    rec.recorder = Some(Recorder::new(from, rec.loop_range, held));
                    if self.state != TransportState::Stopped {
                        self.position = from;
                    }
                }
            }
            RecordReport::Event {
                kind,
                t_us,
                tick,
                lap,
            } => self.record_event(kind, t_us, tick, lap, effects),
            RecordReport::LapChanged { lap } => {
                let mut changed = Vec::new();
                if let Some(rec) = &mut self.rec
                    && rec.loop_range.is_some()
                {
                    rec.lap = lap;
                    rec.enter_lap(lap, &mut changed);
                }
                self.send_live(changed, effects);
            }
            RecordReport::Stopped { tick, lap, .. } => {
                if self.rec.as_ref().is_some_and(|r| r.recorder.is_some()) {
                    self.finish(tick, lap, effects);
                } else {
                    self.rec = None;
                }
                if self.state != TransportState::Stopped {
                    self.state = TransportState::Stopped;
                }
            }
        }
    }

    fn record_event(
        &mut self,
        kind: RecEventKind,
        t_us: u64,
        tick: f64,
        lap: u32,
        effects: &mut Vec<Effect>,
    ) {
        let Some(rec) = &mut self.rec else {
            return;
        };
        if rec.stop_us.is_some_and(|s| t_us > s) {
            return;
        }
        let Some(recorder) = &mut rec.recorder else {
            return;
        };
        let lap = if rec.loop_range.is_some() { lap } else { 0 };
        let t = tick.round() as i64;
        recorder.push(RecEvent { tick: t, lap, kind });
        let mut changed = Vec::new();
        rec.enter_lap(lap, &mut changed);
        match kind {
            RecEventKind::NoteOn { key, velocity } if key <= 127 && velocity > 0 => {
                rec.release(key, t, &mut changed);
                rec.press(key, velocity.min(127), t, lap, &mut changed);
            }
            RecEventKind::NoteOn { key, .. } | RecEventKind::NoteOff { key } if key <= 127 => {
                rec.release(key, t, &mut changed);
            }
            _ => {}
        }
        self.send_live(changed, effects);
    }

    fn send_live(&mut self, mut changed: Vec<usize>, effects: &mut Vec<Effect>) {
        let Some(rec) = &self.rec else {
            return;
        };
        if changed.is_empty() {
            return;
        }
        changed.sort_unstable();
        changed.dedup();
        self.live_seq += 1;
        effects.push(Effect::LiveNotes(LiveNotes {
            seq: self.live_seq,
            recording: rec.id,
            notes: changed.into_iter().map(|i| rec.live[i]).collect(),
        }));
    }

    /// Puts what was recorded in the song as one step. Returns the number of the shown take.
    fn finish(&mut self, tick: f64, lap: u32, effects: &mut Vec<Effect>) -> Option<u32> {
        let rec = self.rec.take()?;
        let recorded = rec.recorder?.finish(tick.round() as i64, lap)?;
        let (e, added) =
            edit::add_recording(self.doc(), rec.track, recorded.takes, recorded.active).ok()?;
        self.apply(e, effects);
        self.selected = Some(added.clip);
        self.piano_roll = Some(added.clip);
        Some(added.active as u32 + 1)
    }

    // ---- what the screen reads ----

    pub fn live_snapshot(&self) -> LiveNotesSnapshot {
        LiveNotesSnapshot {
            seq: self.live_seq,
            recording: self.rec.as_ref().map(|r| r.id),
            notes: self
                .rec
                .as_ref()
                .map(|r| r.live.clone())
                .unwrap_or_default(),
        }
    }

    /// The take binary of a take in the song as it is now. `None` for a take not there, or
    /// one too long for the binary.
    pub fn take_notes(&self, id: u32) -> Option<Vec<u8>> {
        let take = self
            .doc()
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .flat_map(|c| &c.takes)
            .find(|ct| ct.take.id == TakeId(id))?
            .take
            .clone();
        let u = |v: u64| u32::try_from(v).ok();
        let notes = take
            .notes
            .iter()
            .map(|n| {
                Some(TakeNote {
                    start: u(n.start)?,
                    length: u(n.length)?,
                    key: n.key,
                    velocity: n.velocity,
                    from: None,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let controls = take
            .controls
            .iter()
            .map(|c| {
                Some(TakeControl {
                    tick: u(c.tick)?,
                    value: match c.value {
                        meloet_model::ControlValue::Cc { number, value } => {
                            TakeControlValue::Cc { number, value }
                        }
                        meloet_model::ControlValue::PitchBend(v) => TakeControlValue::PitchBend(v),
                    },
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(
            TakeNotes {
                id,
                length: u(take.length)?,
                origin: None,
                notes,
                controls,
            }
            .encode(),
        )
    }

    pub fn clips_of(&self, track: TrackId) -> Vec<ClipView> {
        self.doc().track(track).map_or_else(Vec::new, |t| {
            t.clips
                .iter()
                .map(|c| {
                    let ct = c.active_take();
                    ClipView {
                        id: c.id.0,
                        name: c.name.clone(),
                        start: c.start,
                        end: c.end,
                        take: c.active as u32 + 1,
                        takes: c.takes.len() as u32,
                        take_id: ct.take.id.0,
                        take_offset: i64::try_from(ct.offset).unwrap_or(i64::MAX),
                        note_count: ct.take.notes.len() as u32,
                    }
                })
                .collect()
        })
    }

    pub fn undo_count(&self) -> u32 {
        self.history.undo_count() as u32
    }

    pub fn redo_count(&self) -> u32 {
        self.history.redo_count() as u32
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.unsaved_anyway || self.history.has_unsaved_changes()
    }

    /// The song as it is now is in its file, under `name` (the first save can add " 2").
    /// The name is not a step: undo and redo keep it.
    pub fn saved_as(&mut self, name: String) {
        self.song.doc.name = name;
        self.history.mark_saved();
        self.unsaved_anyway = false;
    }

    /// Stops a count-in or a recording; what was played comes in with the engine's report.
    pub fn stop_recording(&mut self, effects: &mut Vec<Effect>) {
        if matches!(
            self.state,
            TransportState::CountIn | TransportState::Recording
        ) {
            self.stop(effects);
        }
    }

    /// Puts another song in place of this one, with no steps. `unsaved`: it does not match its
    /// file. Only while not busy.
    pub fn replace_song(&mut self, song: Song, unsaved: bool, effects: &mut Vec<Effect>) {
        let old_tracks = self.song.doc.tracks.len();
        self.song = song;
        self.history = History::new();
        self.unsaved_anyway = unsaved;
        self.state = TransportState::Stopped;
        self.position = 0;
        self.selected = None;
        self.piano_roll = None;
        self.rec = None;
        self.heard = None;
        self.last_heard = None;
        self.had_clip = self.doc().tracks.iter().any(|t| !t.clips.is_empty());
        self.stream_opened(effects);
        for track in self.song.doc.tracks.len()..old_tracks {
            effects.push(Effect::Engine(EngineOp::Sequence {
                track,
                events: Vec::new(),
            }));
        }
    }

    pub fn piano_roll_notes(&self) -> u32 {
        self.piano_roll
            .and_then(|id| self.doc().clip(id))
            .map_or(0, |(_, c)| c.active_take().take.notes.len() as u32)
    }

    pub fn loop_span(&self) -> Option<TickSpan> {
        self.song.session.loop_range.map(|r| TickSpan {
            start: r.start(),
            end: r.end(),
        })
    }

    pub fn transport_view(&self, open: bool) -> TransportView {
        TransportView {
            state: self.state,
            position: self.position,
            metronome: self.metronome,
            count_in_bars: self.count_in_bars,
            count_in_beats: match &self.rec {
                Some(r) => r.count_in_beats,
                None => u32::from(self.count_in_bars) * u32::from(self.meter().num()),
            },
            can_play: open,
            can_record: open && self.song.session.armed.is_some(),
            recording: self.rec.as_ref().map(|r| RecordingView {
                id: r.id,
                track: r.track.0,
                from: r.from,
                take: r.take,
                clip: r.clip.map(|c| c.0),
                lap: r.lap,
            }),
        }
    }

    pub fn piano_roll_view(&self) -> PianoRollView {
        PianoRollView {
            take: self
                .piano_roll
                .and_then(|id| self.doc().clip(id))
                .map(|(_, c)| c.active_take().take.id.0),
            selected_notes: Vec::new(),
            selected_controls: Vec::new(),
            grid: Grid {
                division: 16,
                triplet: false,
            },
            scale: MusicalScale {
                root: 0,
                mode: ScaleMode::Major,
                lock: false,
            },
            quantize: QuantizeSettings {
                division: 16,
                triplet: false,
                strength: 100,
                swing: 0,
                align: QuantizeAlign::Start,
            },
            clipboard_notes: 0,
        }
    }

    pub fn arrange_view(&self) -> ArrangeView {
        ArrangeView {
            selected_clip: self.selected.map(|c| c.0),
            piano_roll_clip: self.piano_roll.map(|c| c.0),
        }
    }

    /// The status line while counting in or recording.
    pub fn moving_status(&self) -> Option<StatusMessage> {
        let rec = self.rec.as_ref()?;
        if self.state == TransportState::CountIn {
            return Some(StatusMessage::CountingIn {
                beats: rec.count_in_beats,
            });
        }
        let track = self.track_ref(rec.track)?;
        Some(match rec.loop_range {
            Some(r) => {
                let bars = self.bars(r);
                StatusMessage::LoopRecording {
                    track,
                    from_bar: bars.from,
                    to_bar: bars.to,
                    lap: rec.lap + 1,
                }
            }
            None => StatusMessage::Recording {
                track,
                take: rec.take,
            },
        })
    }

    /// The left of the status line when stopped and nothing else is to be said.
    pub fn summary(&self) -> Option<StatusMessage> {
        if self.state != TransportState::Stopped || self.rec.is_some() {
            return None;
        }
        let loop_bars = self.loop_on().map(|r| self.bars(r));
        let nothing_selected = self.selected.is_none();
        (loop_bars.is_some() || nothing_selected).then_some(StatusMessage::Summary {
            loop_bars,
            nothing_selected,
            notes: None,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
