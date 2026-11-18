//! The song and how it plays: the transport, recording, clips, takes and undo. Pure rules like
//! [`crate::state`]: commands and the engine's reports come in, effects go out.

use std::collections::BTreeSet;
use std::sync::Arc;

use meloet_model::controls::{self, ControlKind as ModelControlKind};
use meloet_model::edit::{self, Edit, EditError};
use meloet_model::notes::{self, NoteField as ModelNoteField, NotesEdited, WriteAt};
use meloet_model::playback::{PlayEvent, track_events};
use meloet_model::quantize;
use meloet_model::record::{HeldControls, RecEvent, RecEventKind, Recorder};
use meloet_model::{
    Clip, ClipEdited, ClipId, Doc, History, Meter, Note, OriginIndex, Song, StepKind, TakeId,
    Tempo, TickRange, TrackId, arrange, roll,
};
use meloet_protocol::{
    ArrangeView, BarSpan, ClipEdge, ClipView, ControlKind, Grid, IndexRange, LiveNote, LiveNotes,
    LiveNotesSnapshot, MusicalScale, NOTE_SELECTION_KEYS, NoteField, NoteSelection, PianoRollView,
    QuantizeAlign, QuantizeSettings, RecordingView, ScaleMode, SelectMode, StatusMessage,
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
    /// Sounds a key on the track at this index, and stops the key it sounded before.
    Audition {
        track: usize,
        key: u8,
        velocity: u8,
    },
    AuditionOff,
}

/// Selected notes and control points: indexes, in order, into the notes and the controls of
/// `take`, shown in the piano roll for `clip`. Empty when they are not of the take shown now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NoteSel {
    clip: Option<ClipId>,
    take: Option<TakeId>,
    notes: Vec<u32>,
    controls: Vec<u32>,
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
    note_sel: NoteSel,
    /// One for each step of the history, in the same order: the selected notes before the step
    /// and after it.
    sel_undo: Vec<(NoteSel, NoteSel)>,
    sel_redo: Vec<(NoteSel, NoteSel)>,
    /// Mod C: starts count from the earliest note.
    clipboard: Vec<Note>,
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
    /// Goes up whenever the `Doc` changes or another song takes its place.
    changes: u64,
    /// A second version that matches the file: after "戻さない", the song as saved is both the
    /// version before the recovered step and the one after "戻さない".
    saved_too: Option<u64>,
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
            note_sel: NoteSel::default(),
            sel_undo: Vec::new(),
            sel_redo: Vec::new(),
            clipboard: Vec::new(),
            rec: None,
            next_recording: 1,
            live_seq: 0,
            now_us: 0,
            heard: None,
            last_heard: None,
            had_clip: false,
            unsaved_anyway: false,
            changes: 0,
            saved_too: None,
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
                self.fix_selection();
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

    /// A drag in the arrange view onto the track `track` (`TrackView::number`); `copy`: Alt +
    /// drag. The clip moved, or the copy, is selected and shown in the piano roll.
    pub fn move_clip(
        &mut self,
        clip: u32,
        ticks: i64,
        track: u32,
        copy: bool,
        effects: &mut Vec<Effect>,
    ) {
        if self.busy() {
            return;
        }
        let moved = arrange::move_clip(self.doc(), ClipId(clip), ticks, TrackId(track), copy);
        if let Ok(e) = moved {
            self.apply_clip(e, effects);
        }
    }

    /// The number of the track `rows` rows below (above, when negative) the track of `clip`.
    pub fn track_rows_away(&self, clip: u32, rows: i32) -> Option<u32> {
        let (track, _) = self.doc().clip(ClipId(clip))?;
        let tracks = &self.doc().tracks;
        let at = tracks.iter().position(|t| t.id == track.id)?;
        let to = usize::try_from(at as i64 + i64::from(rows)).ok()?;
        tracks.get(to).map(|t| t.id.0)
    }

    /// Mod D in the arrange view. The copy is selected and shown in the piano roll.
    pub fn duplicate_clip(&mut self, clip: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = arrange::duplicate_clip(self.doc(), ClipId(clip)) {
            self.apply_clip(e, effects);
        }
    }

    pub fn trim_clip(&mut self, clip: u32, edge: ClipEdge, ticks: i64, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let edge = match edge {
            ClipEdge::Start => arrange::ClipEdge::Start,
            ClipEdge::End => arrange::ClipEdge::End,
        };
        if let Ok(e) = arrange::trim_clip(self.doc(), ClipId(clip), edge, ticks) {
            self.apply(e, effects);
        }
    }

    fn apply_clip(&mut self, e: ClipEdited, effects: &mut Vec<Effect>) {
        self.selected = Some(e.clip);
        self.piano_roll = Some(e.clip);
        self.apply(e.edit, effects);
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
            if let Some((sel_before, sel_after)) = self.sel_undo.pop() {
                self.note_sel = sel_before.clone();
                self.sel_redo.push((sel_before, sel_after));
            }
            self.fix_selection();
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
            if let Some((sel_before, sel_after)) = self.sel_redo.pop() {
                self.note_sel = sel_after.clone();
                self.sel_undo.push((sel_before, sel_after));
            }
            self.fix_selection();
        }
    }

    fn apply(&mut self, e: Edit, effects: &mut Vec<Effect>) {
        self.apply_selecting(e, None, effects);
    }

    /// `selected`: the notes of the shown take to select afterwards, or its control points
    /// with `true`. The other kind stays selected: an edit of one leaves the other in order.
    fn apply_selecting(
        &mut self,
        e: Edit,
        selected: Option<(Vec<usize>, bool)>,
        effects: &mut Vec<Effect>,
    ) {
        let sel_before = self.note_sel.clone();
        let before = self.doc().clone();
        self.history.apply(&mut self.song.doc, e);
        self.after_change(&before, effects);
        if let Some((picked, of_controls)) = selected {
            let same_clip = sel_before.clip.is_some() && sel_before.clip == self.piano_roll;
            let kept = |v: &Vec<u32>| if same_clip { v.clone() } else { Vec::new() };
            let picked = picked.into_iter().map(|i| i as u32);
            if of_controls {
                self.set_selection(kept(&sel_before.notes), picked);
            } else {
                self.set_selection(picked, kept(&sel_before.controls));
            }
        }
        self.sel_undo.push((sel_before, self.note_sel.clone()));
        self.sel_redo.clear();
    }

    fn apply_notes(&mut self, edited: Result<NotesEdited, EditError>, effects: &mut Vec<Effect>) {
        if let Ok(n) = edited {
            self.apply_selecting(n.edit, Some((n.selected, n.of_controls)), effects);
        }
    }

    /// Sends what the engine must hear again, and lets go of clips that are gone.
    fn after_change(&mut self, before: &Doc, effects: &mut Vec<Effect>) {
        self.changes += 1;
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
        self.fix_selection();
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
        self.selected = Some(added.clip);
        self.piano_roll = Some(added.clip);
        self.apply(e, effects);
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

    /// The take binary of a take in the song as it is now, in a clip or the origin of one
    /// there. `None` for a take not there, or one too long for the binary.
    pub fn take_notes(&self, id: u32) -> Option<Vec<u8>> {
        let take = self
            .doc()
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .flat_map(|c| &c.takes)
            .flat_map(|ct| {
                std::iter::once(&ct.take).chain(ct.take.origin.as_ref().map(|o| &o.take))
            })
            .find(|t| t.id == TakeId(id))?
            .clone();
        let u = |v: u64| u32::try_from(v).ok();
        let mut notes = Vec::with_capacity(take.notes.len());
        for n in &take.notes {
            notes.push(TakeNote {
                start: u(n.start)?,
                length: u(n.length)?,
                key: n.key,
                velocity: n.velocity,
                from: n.from.map(OriginIndex::get),
            });
        }
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
                origin: take.origin.as_ref().map(|o| o.take.id.0),
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
                        take_offset: ct.offset,
                        note_count: c.note_count() as u32,
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
        self.unsaved_anyway
            || (self.history.has_unsaved_changes()
                && self.saved_too != Some(self.history.version()))
    }

    /// The song as it is now is in its file, under `name` (the first save can add " 2").
    /// The name is not a step: undo and redo keep it.
    pub fn saved_as(&mut self, name: String) {
        self.song.doc.name = name;
        self.history.mark_saved();
        self.unsaved_anyway = false;
        self.saved_too = None;
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
        self.changes += 1;
        self.history = History::new();
        self.unsaved_anyway = unsaved;
        self.saved_too = None;
        self.state = TransportState::Stopped;
        self.position = 0;
        self.selected = None;
        self.piano_roll = None;
        self.note_sel = NoteSel::default();
        self.sel_undo.clear();
        self.sel_redo.clear();
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

    /// The song brought back from the recovery file, as the first step over the song as
    /// saved, which is how the history starts (and what counts as saved). `saved` must have
    /// the recovered song's ids given out already: the screen keeps takes by id.
    pub fn recovered(&mut self, doc: Doc, effects: &mut Vec<Effect>) {
        self.apply(
            Edit {
                doc,
                step: StepKind::Recovered,
            },
            effects,
        );
    }

    /// Puts `saved`, the song as its file has it, in place of a song brought back from the
    /// recovery file: one step, after which the song counts as saved. Undo brings the song
    /// back, unsaved. Only while not busy.
    pub fn discard_recovered(&mut self, mut saved: Doc, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        // Ids given out in the song brought back stay given out: the screen keeps takes by id.
        saved.next_id = saved.next_id.max(self.doc().next_id);
        self.unsaved_anyway = false;
        self.apply(
            Edit {
                doc: saved,
                step: StepKind::DiscardRecovered,
            },
            effects,
        );
        self.saved_too = Some(self.history.version());
        let doc = &self.song.doc;
        let session = &mut self.song.session;
        if doc.track(session.play_target).is_none() {
            session.play_target = doc.tracks[0].id;
        }
        if session.armed.is_some_and(|t| doc.track(t).is_none()) {
            session.armed = None;
        }
    }

    /// Goes up whenever the `Doc` changes or another song takes its place.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    #[cfg(test)]
    pub fn steps(&self) -> Vec<StepKind> {
        self.history.steps().copied().collect()
    }

    pub fn piano_roll_notes(&self) -> u32 {
        self.piano_roll
            .and_then(|id| self.doc().clip(id))
            .map_or(0, |(_, c)| c.note_count() as u32)
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
        let r = &self.song.session.roll;
        let q = r.quantize;
        PianoRollView {
            take: self.shown().map(|c| c.active_take().take.id.0),
            selected_notes: index_ranges(&self.note_sel.notes),
            selected_controls: index_ranges(&self.note_sel.controls),
            grid: Grid {
                division: r.grid.division(),
                triplet: r.grid.triplet,
            },
            scale: MusicalScale {
                root: r.scale.root(),
                mode: scale_mode(r.scale.mode),
                lock: r.scale.lock,
            },
            quantize: QuantizeSettings {
                division: q.grid.division(),
                triplet: q.grid.triplet,
                strength: q.strength(),
                swing: q.swing(),
                align: match q.align {
                    roll::QuantizeAlign::Start => QuantizeAlign::Start,
                    roll::QuantizeAlign::StartAndEnd => QuantizeAlign::StartAndEnd,
                },
            },
            clipboard_notes: self.clipboard.len() as u32,
        }
    }

    pub fn arrange_view(&self) -> ArrangeView {
        ArrangeView {
            selected_clips: self.selected.map(|c| c.0).into_iter().collect(),
            selected_clip: self.selected.map(|c| c.0),
            piano_roll_clip: self.piano_roll.map(|c| c.0),
            clipboard_clips: 0,
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
        let notes = self.note_selection();
        let nothing_selected = self.selected.is_none() && notes.is_none();
        (loop_bars.is_some() || nothing_selected || notes.is_some()).then_some(
            StatusMessage::Summary {
                loop_bars,
                nothing_selected,
                notes,
            },
        )
    }

    fn note_selection(&self) -> Option<NoteSelection> {
        let sel = &self.note_sel.notes;
        if sel.is_empty() {
            return None;
        }
        let take = &self.shown()?.active_take().take;
        Some(NoteSelection {
            count: sel.len() as u32,
            keys: sel
                .iter()
                .take(NOTE_SELECTION_KEYS)
                .filter_map(|&i| take.notes.get(i as usize).map(|n| n.key))
                .collect(),
            quantized: take.origin.is_some(),
        })
    }

    // ---- the piano roll ----

    /// The clip the piano roll shows.
    fn shown(&self) -> Option<&Arc<Clip>> {
        self.doc().clip(self.piano_roll?).map(|(_, c)| c)
    }

    /// The shown clip, when `take` is its shown take.
    fn shown_with(&self, take: u32) -> Option<&Arc<Clip>> {
        self.shown()
            .filter(|c| c.active_take().take.id == TakeId(take))
    }

    /// The shown clip, when `take` is its shown take and the song may change.
    fn aimed(&self, take: u32) -> Option<ClipId> {
        if self.busy() {
            return None;
        }
        self.shown_with(take).map(|c| c.id)
    }

    /// Keeps the selection only while it is of the take shown now.
    fn fix_selection(&mut self) {
        let Some(c) = self.shown() else {
            self.note_sel = NoteSel::default();
            return;
        };
        let take = &c.active_take().take;
        if self.note_sel.clip != Some(c.id) || self.note_sel.take != Some(take.id) {
            self.note_sel = NoteSel::default();
            return;
        }
        let notes: Vec<u32> = self
            .note_sel
            .notes
            .iter()
            .copied()
            .filter(|&i| holds_note(c, i))
            .collect();
        let controls = take.controls.len();
        self.note_sel.notes = notes;
        self.note_sel.controls.retain(|&i| (i as usize) < controls);
    }

    /// Selects these notes and control points of the shown take; indexes past the end, and
    /// notes that start outside the clip, are left out.
    fn set_selection(
        &mut self,
        notes: impl IntoIterator<Item = u32>,
        controls: impl IntoIterator<Item = u32>,
    ) {
        let Some(c) = self.shown() else {
            self.note_sel = NoteSel::default();
            return;
        };
        let take = &c.active_take().take;
        let below = |len: usize, items: &mut dyn Iterator<Item = u32>| -> Vec<u32> {
            let set: BTreeSet<u32> = items.filter(|&i| (i as usize) < len).collect();
            set.into_iter().collect()
        };
        self.note_sel = NoteSel {
            clip: Some(c.id),
            take: Some(take.id),
            notes: below(
                take.notes.len(),
                &mut notes.into_iter().filter(|&i| holds_note(c, i)),
            ),
            controls: below(take.controls.len(), &mut controls.into_iter()),
        };
    }

    pub fn select_notes(&mut self, take: u32, notes: Vec<u32>, mode: SelectMode) {
        if self.shown_with(take).is_none() {
            return;
        }
        let picked = selected_with(&self.note_sel.notes, notes, mode);
        self.set_selection(picked, self.note_sel.controls.clone());
    }

    pub fn select_controls(&mut self, take: u32, points: Vec<u32>, mode: SelectMode) {
        if self.shown_with(take).is_none() {
            return;
        }
        let picked = selected_with(&self.note_sel.controls, points, mode);
        self.set_selection(self.note_sel.notes.clone(), picked);
    }

    pub fn select_all_notes(&mut self) {
        let all: Vec<u32> = self.shown().map_or_else(Vec::new, |c| {
            (0..c.active_take().take.notes.len() as u32)
                .filter(|&i| holds_note(c, i))
                .collect()
        });
        self.set_selection(all, self.note_sel.controls.clone());
    }

    pub fn clear_note_selection(&mut self) {
        self.note_sel.notes.clear();
    }

    /// The write tool. `take: None` only when the piano roll shows no clip: the note goes on the
    /// track the keys play, in the clip there or in a new one. Returns the notice to show.
    pub fn add_note(
        &mut self,
        take: Option<u32>,
        note: Note,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || note.key > 127 {
            return None;
        }
        let at = match take {
            Some(take) => WriteAt::Clip(self.aimed(take)?),
            None if self.shown().is_some() => return None,
            None => WriteAt::Track(self.song.session.play_target),
        };
        let scale = self.song.session.roll.scale;
        if scale.lock && !scale.contains(note.key) {
            return Some(StatusMessage::OutOfScale {
                key: note.key,
                root: scale.root(),
                mode: scale_mode(scale.mode),
            });
        }
        let note = Note {
            length: note.length.max(1),
            velocity: note.velocity.clamp(1, 127),
            ..note
        };
        let written = notes::write_note(self.doc(), at, note).ok()?;
        if take.is_none() {
            self.selected = Some(written.clip);
            self.piano_roll = Some(written.clip);
        }
        self.apply_selecting(written.edit, Some((written.selected, false)), effects);
        None
    }

    pub fn move_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        ticks: i64,
        keys: i32,
        copy: bool,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(clip) = self.aimed(take) {
            let r = notes::move_notes(self.doc(), clip, &indexes(picked), ticks, keys, copy);
            self.apply_notes(r, effects);
        }
    }

    pub fn resize_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        ticks: i64,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(clip) = self.aimed(take) {
            let r = notes::resize_notes(self.doc(), clip, &indexes(picked), ticks);
            self.apply_notes(r, effects);
        }
    }

    pub fn delete_notes(&mut self, take: u32, picked: &[u32], effects: &mut Vec<Effect>) {
        if let Some(clip) = self.aimed(take) {
            let r = notes::delete_notes(self.doc(), clip, &indexes(picked), false);
            self.apply_notes(r, effects);
        }
    }

    pub fn change_velocity(
        &mut self,
        take: u32,
        picked: &[u32],
        delta: i32,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(clip) = self.aimed(take) {
            let r = notes::change_velocity(self.doc(), clip, &indexes(picked), delta);
            self.apply_notes(r, effects);
        }
    }

    pub fn copy_notes(&mut self, take: u32, picked: &[u32]) {
        if let Some(c) = self.shown_with(take)
            && let Ok(notes) = notes::clipboard(c, &indexes(picked))
        {
            self.clipboard = notes;
        }
    }

    /// Copies and deletes, as one step.
    pub fn cut_notes(&mut self, take: u32, picked: &[u32], effects: &mut Vec<Effect>) {
        let Some(clip) = self.aimed(take) else {
            return;
        };
        let picked = indexes(picked);
        let Some(c) = self.shown() else {
            return;
        };
        let (Ok(kept), Ok(edited)) = (
            notes::clipboard(c, &picked),
            notes::delete_notes(self.doc(), clip, &picked, true),
        ) else {
            return;
        };
        self.clipboard = kept;
        self.apply_notes(Ok(edited), effects);
    }

    pub fn paste_notes(&mut self, at: u64, effects: &mut Vec<Effect>) {
        if self.busy() || self.clipboard.is_empty() {
            return;
        }
        if let Some(clip) = self.shown().map(|c| c.id) {
            let r = notes::paste_notes(self.doc(), clip, &self.clipboard, at);
            self.apply_notes(r, effects);
        }
    }

    /// Q and "かけなおす", with the settings of the quantize panel. Returns the notice to show.
    pub fn quantize(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let clip = self.aimed(take)?;
        let settings = self.song.session.roll.quantize;
        let (edited, count) =
            quantize::quantize(self.doc(), clip, &indexes(picked), settings).ok()?;
        self.apply_notes(Ok(edited), effects);
        Some(StatusMessage::Quantized {
            division: settings.grid.division(),
            triplet: settings.grid.triplet,
            strength: settings.strength(),
            count: count as u32,
        })
    }

    pub fn unquantize(&mut self, take: u32, picked: &[u32], effects: &mut Vec<Effect>) {
        if let Some(clip) = self.aimed(take) {
            let r = quantize::unquantize(self.doc(), clip, &indexes(picked));
            self.apply_notes(r, effects);
        }
    }

    pub fn set_notes_field(
        &mut self,
        take: u32,
        picked: &[u32],
        field: NoteField,
        value: i64,
        effects: &mut Vec<Effect>,
    ) {
        let field = match field {
            NoteField::Start => ModelNoteField::Start,
            NoteField::Key => ModelNoteField::Key,
            NoteField::Length => ModelNoteField::Length,
            NoteField::Velocity => ModelNoteField::Velocity,
        };
        if let Some(clip) = self.aimed(take) {
            let r = notes::set_notes_field(self.doc(), clip, &indexes(picked), field, value);
            self.apply_notes(r, effects);
        }
    }

    pub fn add_control(
        &mut self,
        take: u32,
        tick: u64,
        control: ControlKind,
        value: i32,
        effects: &mut Vec<Effect>,
    ) {
        let kind = match control {
            ControlKind::Cc { number } => ModelControlKind::Cc(number),
            ControlKind::PitchBend => ModelControlKind::PitchBend,
        };
        if let Some(clip) = self.aimed(take) {
            let r = controls::add_control(self.doc(), clip, tick, kind, i64::from(value));
            self.apply_notes(r, effects);
        }
    }

    pub fn move_controls(
        &mut self,
        take: u32,
        points: &[u32],
        ticks: i64,
        value: i32,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(clip) = self.aimed(take) {
            let r = controls::move_controls(
                self.doc(),
                clip,
                &indexes(points),
                ticks,
                i64::from(value),
            );
            self.apply_notes(r, effects);
        }
    }

    pub fn delete_controls(&mut self, take: u32, points: &[u32], effects: &mut Vec<Effect>) {
        if let Some(clip) = self.aimed(take) {
            let r = controls::delete_controls(self.doc(), clip, &indexes(points));
            self.apply_notes(r, effects);
        }
    }

    pub fn set_grid(&mut self, division: u16, triplet: bool) {
        if let Some(grid) = roll::Grid::new(division, triplet) {
            self.song.session.roll.grid = grid;
        }
    }

    pub fn set_musical_scale(&mut self, root: u8, mode: ScaleMode, lock: bool) {
        let mode = match mode {
            ScaleMode::Major => roll::ScaleMode::Major,
            ScaleMode::Minor => roll::ScaleMode::Minor,
        };
        if let Some(scale) = roll::MusicalScale::new(root, mode, lock) {
            self.song.session.roll.scale = scale;
        }
    }

    pub fn set_quantize(&mut self, s: QuantizeSettings) {
        let align = match s.align {
            QuantizeAlign::Start => roll::QuantizeAlign::Start,
            QuantizeAlign::StartAndEnd => roll::QuantizeAlign::StartAndEnd,
        };
        if let Some(grid) = roll::Grid::new(s.division, s.triplet)
            && let Some(q) = roll::QuantizeSettings::new(grid, s.strength, s.swing, align)
        {
            self.song.session.roll.quantize = q;
        }
    }

    /// The write tool sounds the key on the track the keys play: not a key it may not write
    /// (outside the locked scale), and nothing without an output.
    pub fn audition(&self, key: u8, velocity: u8, open: bool, effects: &mut Vec<Effect>) {
        let scale = self.song.session.roll.scale;
        if !open || key > 127 || (scale.lock && !scale.contains(key)) {
            return;
        }
        if let Some(track) = self.track_index(self.song.session.play_target) {
            effects.push(Effect::Engine(EngineOp::Audition {
                track,
                key,
                velocity: velocity.clamp(1, 127),
            }));
        }
    }

    pub fn audition_off(&self, open: bool, effects: &mut Vec<Effect>) {
        if open {
            effects.push(Effect::Engine(EngineOp::AuditionOff));
        }
    }
}

/// The selection `now` after `mode` with `picked`.
fn selected_with(now: &[u32], picked: Vec<u32>, mode: SelectMode) -> BTreeSet<u32> {
    let now = now.iter().copied();
    match mode {
        SelectMode::Replace => picked.into_iter().collect(),
        SelectMode::Add => now.chain(picked).collect(),
        SelectMode::Toggle => {
            let mut set: BTreeSet<u32> = now.collect();
            let flips: BTreeSet<u32> = picked.into_iter().collect();
            for i in flips {
                if !set.remove(&i) {
                    set.insert(i);
                }
            }
            set
        }
    }
}

/// Whether note `i` of the clip's shown take starts inside the clip (shown, so selectable).
fn holds_note(c: &Clip, i: u32) -> bool {
    c.active_take()
        .take
        .notes
        .get(i as usize)
        .is_some_and(|n| c.holds(n.start))
}

fn indexes(picked: &[u32]) -> Vec<usize> {
    picked.iter().map(|&i| i as usize).collect()
}

fn scale_mode(mode: roll::ScaleMode) -> ScaleMode {
    match mode {
        roll::ScaleMode::Major => ScaleMode::Major,
        roll::ScaleMode::Minor => ScaleMode::Minor,
    }
}

/// Sorted indexes as ranges, the ones that touch joined.
fn index_ranges(sorted: &[u32]) -> Vec<IndexRange> {
    let mut out: Vec<IndexRange> = Vec::new();
    for &i in sorted {
        match out.last_mut() {
            Some(r) if r.end == i => r.end = i + 1,
            _ => out.push(IndexRange {
                start: i,
                end: i + 1,
            }),
        }
    }
    out
}

#[cfg(test)]
mod arrange_tests;
#[cfg(test)]
mod roll_tests;
#[cfg(test)]
pub(crate) mod tests;
