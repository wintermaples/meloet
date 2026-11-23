//! The song and how it plays: the transport, recording, clips, takes and undo. Pure rules like
//! [`crate::state`]: commands and the engine's reports come in, effects go out.

use std::collections::BTreeSet;
use std::sync::Arc;

use meloet_engine::TrackSet;
use meloet_model::arrange::KeptClip;
use meloet_model::controls::{self, ControlKind as ModelControlKind};
use meloet_model::edit::{self, Edit, EditError, RecordingAdded};
use meloet_model::notes::{self, NoteField as ModelNoteField, NotesEdited, WriteAt};
use meloet_model::playback::{PlayEvent, track_events};
use meloet_model::quantize;
use meloet_model::record::{HeldControls, RecEvent, RecEventKind, Recorder, overdub_recording};
use meloet_model::{
    AutoTarget, Bus, Clip, ClipEdited, ClipId, Doc, Gain, History, Insert, InstrumentGroup,
    InstrumentRef, Note, OriginIndex, Pan, Sends, Song, StepKind, TakeId, TempoMap, TickRange,
    Timeline, TrackId, arrange, roll, timeline as timeline_edit,
};
use meloet_protocol::{
    ArrangeView, BarSpan, ClipEdge, ClipSpot, ClipView, ControlKind, Grid, IndexRange, KeyCount,
    LiveNote, LiveNotes, LiveNotesSnapshot, MusicalScale, NOTE_SELECTION_KEYS, NoteField,
    NoteSelection, PianoRollView, QuantizeAlign, QuantizeSettings, RecordFormat, RecordingView,
    ScaleMode, SelectMode, StatusMessage, TakeControl, TakeControlValue, TakeNote, TakeNotes,
    TickSpan, TimelineRef, TrackRef, TransportState, TransportView,
};

use crate::instruments::Sounds;
use crate::state::Effect;

/// Bars the loop gets when it is turned on with no range.
const DEFAULT_LOOP_BARS: u64 = 4;
/// How long before a pass of a loop recording ends the notes of 「重ねる」 are sent to the
/// engine: more than the engine plays ahead of what is heard, with the time between two
/// ticks of the core thread.
const SEND_PASS_AHEAD_S: f64 = 0.15;
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
    /// The song is at `tick` at the engine's frame `frame`, and is recorded from there.
    /// `file_frame`: the frame of the files of the input that was sung when it was heard
    /// (`DiskWriter::mark`); `None` when no files are written.
    Rolling {
        frame: u64,
        tick: f64,
        file_frame: Option<f64>,
    },
    /// `frame`: the engine's frame the lap begins with; `file_frame` as for `Rolling`.
    LapChanged {
        lap: u32,
        frame: u64,
        file_frame: Option<f64>,
    },
    Stopped {
        t_us: u64,
        tick: f64,
        lap: u32,
    },
}

/// Commands to the engine. The core thread turns them into `EngineCommand`s. A track is named
/// by its seat ([`Play::track_seats`]), not by its place in the song.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineOp {
    Play {
        from: u64,
    },
    Record {
        from: u64,
        count_in_beats: u32,
        /// The seats that are recorded to: they do not play their sequence and their
        /// clips while recording.
        muted: TrackSet,
    },
    /// While playing: record from where the engine is, without a jump.
    RecordHere {
        muted: TrackSet,
    },
    Stop {
        t_us: u64,
    },
    Locate {
        tick: u64,
    },
    /// The song's tempo and meter, for the engine's tempo map.
    Timeline {
        timeline: Arc<Timeline>,
    },
    Loop {
        start: u64,
        end: u64,
        on: bool,
    },
    Metronome {
        on: bool,
    },
    /// The events of the track on this seat.
    Sequence {
        track: usize,
        events: Vec<PlayEvent>,
    },
    /// As `Sequence`, for events that have every event the seat has and more (a pass of
    /// 「重ねる」): what sounds goes on.
    GrowSequence {
        track: usize,
        events: Vec<PlayEvent>,
    },
    /// While recording: the seats that do not play their sequence from now on, in place of
    /// the `muted` of `Record` or `RecordHere`.
    RecordMuted {
        muted: TrackSet,
    },
    /// Sounds keys on this seat, and stops the keys it sounded before.
    Audition {
        track: usize,
        keys: Vec<u8>,
        velocity: u8,
    },
    AuditionOff,
    /// The seats of the tracks whose clips do not play (mute and solo).
    SilentTracks {
        tracks: Vec<usize>,
    },
    // The ones below reach the engine without a stream too: it keeps its instruments and its
    // play target from one stream to the next.
    /// A new instrument of this kind on the seat, in place of what is there.
    Instrument {
        seat: usize,
        instrument: InstrumentRef,
    },
    /// A new instrument on the seat for a trial; the seat's own is set aside, or the one
    /// tried before is put away.
    TryInstrument {
        seat: usize,
        instrument: InstrumentRef,
    },
    /// `keep`: the instrument tried is the seat's own now. Else the one set aside is back.
    EndTrial {
        seat: usize,
        keep: bool,
    },
    /// The instrument of `from` goes to the empty seat `to` as it sounds.
    MoveInstrument {
        from: usize,
        to: usize,
    },
    /// The seat's track is gone: no instrument and no events are left on it.
    ClearSeat {
        seat: usize,
    },
    /// The seat the keys play; `None` in a song with no tracks.
    PlayTarget {
        seat: Option<usize>,
    },
    /// The fader and the pan of a strip. A muted return has the gain −∞.
    Strip {
        strip: StripSeat,
        gain: Gain,
        pan: Pan,
    },
    /// What the track on the seat sends to return A and to return B.
    Sends {
        seat: usize,
        sends: Sends,
    },
    /// A new effect with these values in the insert slot, in place of what is there; `None`
    /// empties the slot.
    Insert {
        strip: StripSeat,
        slot: usize,
        insert: Option<Insert>,
    },
    /// A parameter of the effect in the slot.
    Param {
        strip: StripSeat,
        slot: usize,
        param: u32,
        value: f32,
    },
    /// 「とばす」: the sound passes the slot by, or goes through it again. The mark is the
    /// slot's; what is put into the slot later comes in with it.
    Bypass {
        strip: StripSeat,
        slot: usize,
        bypassed: bool,
    },
    /// The slot `from` goes to `to` with what it holds, as it sounds, and the slots between
    /// move by one.
    MoveInsert {
        strip: StripSeat,
        from: usize,
        to: usize,
    },
    /// The plugin loaded under `ticket` goes into the engine at `spot`, in place of what is
    /// there (the silent instrument, the empty slot, or the plugin as it was before it was
    /// loaded again).
    Plug {
        ticket: u64,
        spot: Spot,
    },
    /// The plugin loaded under `ticket` plays the sample phrase of `group` on the engine's
    /// sample seat. For an open stream only.
    PlugSample {
        ticket: u64,
        group: InstrumentGroup,
    },
    // The two below are for an open stream only, as the ones at the top.
    /// The sample phrase of this instrument's group, played with it on the engine's sample
    /// seat from its start. A sample that is playing fades out under it.
    PlaySample {
        instrument: InstrumentRef,
    },
    StopSample,
    /// For an open stream only, as `Sequence`. The lanes of the track on the seat that play
    /// from now on, in place of the ones before (`play/autosync.rs`).
    Automation {
        seat: usize,
        lanes: Vec<SeatLane>,
    },
    /// A person has hold of what a lane of the track on the seat moves, or let go of it.
    HoldLane {
        seat: usize,
        target: AutoTarget,
        on: bool,
    },
    /// For the engine at any time, as `Instrument`: the track on the seat is a vocal track
    /// and sounds the input (`None`: it is not one any more).
    Voice {
        seat: usize,
        voice: Option<VoiceSpec>,
    },
    /// The vocal clips of the track on the seat, as the song has them (none: it has none).
    /// Kept by the core's audio side, which makes the engine's table and the read-ahead
    /// thread's clips of them for the stream that runs (`play/audiosync.rs`).
    Clips {
        seat: usize,
        clips: Vec<Arc<meloet_model::song::Clip>>,
    },
    /// The song's audio table, for the place that opens the files
    /// (`audiofiles::AudioStore::set_sources`). Comes before the `Clips` that use it.
    AudioSources {
        sources: Arc<Vec<Arc<meloet_model::AudioSource>>>,
    },
}

/// A strip of the engine's mixer. A track is named by its seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StripSeat {
    Track(usize),
    Return(Bus),
    Master,
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

/// What is selected, as kept for undo: the notes and points, and the clips.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Sel {
    notes: NoteSel,
    clips: Vec<ClipId>,
    clicked: Option<ClipId>,
    /// The selected tempo point, meter change or marker.
    timeline: Option<TimelineRef>,
    /// The selected automation points.
    points: Option<automation::PointSel>,
}

/// The clips an edit leaves selected, the one of them clicked (`ArrangeView::selected_clip`),
/// and the one the piano roll shows afterwards (`None`: as it was).
struct ClipPick {
    clips: Vec<ClipId>,
    clicked: Option<ClipId>,
    shown: Option<ClipId>,
}

impl ClipPick {
    /// One clip, clicked and shown.
    fn one(clip: ClipId) -> ClipPick {
        ClipPick {
            clips: vec![clip],
            clicked: Some(clip),
            shown: Some(clip),
        }
    }

    fn none() -> ClipPick {
        ClipPick {
            clips: Vec::new(),
            clicked: None,
            shown: None,
        }
    }
}

/// Why a recording was cut short.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    AudioSystem,
    Output,
}

struct Rec {
    id: u32,
    /// The keys track; `None` when only vocal tracks are recorded.
    track: Option<TrackId>,
    vocal: vocal::VocalPart,
    from: u64,
    /// The number the take of the current lap gets: laps with no notes make no take.
    take: u32,
    clip: Option<ClipId>,
    count_in_beats: u32,
    loop_range: Option<TickRange>,
    lap: u32,
    /// Made when the engine says it started.
    recorder: Option<Recorder>,
    /// The recording ended at this tick and lap, and waits for the disk writer to say what
    /// it wrote.
    stopped: Option<(f64, u32)>,
    /// When stop was asked; keys after it are not recorded.
    stop_us: Option<u64>,
    live: Vec<LiveNote>,
    /// Index into `live` of the note each key holds.
    held: [Option<usize>; 128],
    live_lap: u32,
    next_index: u32,
    /// 「重ねる」 for the pass going on: its notes go into the shown take of `clip` (`take` is
    /// that take's number), as one step when the pass is over.
    overdub: bool,
    /// The passes before this one are over.
    closed: u32,
    /// The pass whose notes so far were sent to the engine before its end, and the events
    /// sent (`None`: nothing was).
    ahead: Option<(u32, Option<Vec<PlayEvent>>)>,
    keys_seat: Option<usize>,
    /// The seats of the vocal tracks recorded.
    vocal_seats: TrackSet,
}

impl Rec {
    /// The seats that do not play their sequence: what a track recorded before is not heard
    /// while it records again, unless the new notes go on top of it.
    fn muted(&self) -> TrackSet {
        let mut muted = self.vocal_seats;
        if !self.overdub
            && let Some(seat) = self.keys_seat
        {
            muted.insert(seat);
        }
        muted
    }

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
        if self.next_index > 0 && !self.overdub {
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
    /// While playing: the last move of the play line was Home to the loop's head.
    homed: bool,
    metronome: bool,
    /// 「重ねる」: a recording on a keys track adds to the clip there. Read when a recording
    /// starts and at each pass of a loop.
    overdub: bool,
    count_in_bars: u8,
    /// Every selected clip, in order of id.
    clips_sel: Vec<ClipId>,
    /// The clip clicked last, while it is selected (`ArrangeView::selected_clip`).
    selected: Option<ClipId>,
    piano_roll: Option<ClipId>,
    note_sel: NoteSel,
    /// One for each step of the history, in the same order: what was selected before the step
    /// and after it.
    sel_undo: Vec<(Sel, Sel)>,
    sel_redo: Vec<(Sel, Sel)>,
    /// What the history band says of steps that the `Doc`s around them do not tell.
    step_notes: history::StepNotes,
    /// Mod C: starts count from the earliest note.
    clipboard: Vec<Note>,
    /// Mod C in the arrange view.
    clip_board: Vec<KeptClip>,
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
    /// Goes up whenever another `Doc` is put in from outside the history (another song, or
    /// "戻さない"): take ids start again per song, so the screen lets go of the takes it kept.
    serial: u32,
    /// A second version that matches the file: after "戻さない", the song as saved is both the
    /// version before the recovered step and the one after "戻さない".
    saved_too: Option<u64>,
    /// What the engine was last told by `EngineOp::SilentTracks`.
    silent_sent: Vec<usize>,
    /// The version of the `Doc` a save that is under way writes.
    saving: Option<u64>,
    seats: Seats,
    /// What the engine was last told by `EngineOp::PlayTarget`.
    target_sent: Option<usize>,
    shelf: Shelf,
    sounds: Sounds,
    mixer: Mixer,
    /// What the engine was last told about the mixer.
    mix_told: MixTold,
    plugs: Plugs,
    wins: Windows,
    panel: Panel,
    tl: timeline::Tl,
    auto: automation::Auto,
    autosync: autosync::AutoSync,
    /// What the engine was last told by `EngineOp::Voice`, in the order of the seats.
    voices_sent: Vec<(usize, VoiceSpec)>,
    uid_seed: u64,
    /// Blocks of the vocal recording going on, or of the last one, that are not what came
    /// in.
    faults: Option<meloet_protocol::RecordingFaults>,
    /// What is known of the song's audio files (found, being read, missing).
    audio: audio::AudioBook,
}

impl Play {
    pub fn new(metronome: bool, count_in_bars: u8) -> Play {
        let song = Song::new_default();
        let seats = Seats::for_song(&song.doc);
        let mut play = Play {
            target_sent: song.session.play_target.and_then(|t| seats.of(t)),
            seats,
            shelf: Shelf::default(),
            sounds: Sounds::default(),
            mixer: Mixer::default(),
            mix_told: MixTold::new(),
            plugs: Plugs::default(),
            wins: Windows::default(),
            autosync: autosync::AutoSync::default(),
            panel: Panel::default(),
            tl: timeline::Tl::default(),
            auto: automation::Auto::default(),
            song,
            history: History::new(),
            state: TransportState::Stopped,
            position: 0,
            homed: false,
            metronome,
            overdub: false,
            count_in_bars: if COUNT_IN_BARS.contains(&count_in_bars) {
                count_in_bars
            } else {
                1
            },
            clips_sel: Vec::new(),
            selected: None,
            piano_roll: None,
            note_sel: NoteSel::default(),
            sel_undo: Vec::new(),
            sel_redo: Vec::new(),
            step_notes: history::StepNotes::default(),
            clipboard: Vec::new(),
            clip_board: Vec::new(),
            rec: None,
            next_recording: 1,
            live_seq: 0,
            now_us: 0,
            heard: None,
            last_heard: None,
            had_clip: false,
            unsaved_anyway: false,
            changes: 0,
            serial: 0,
            saved_too: None,
            silent_sent: Vec::new(),
            saving: None,
            voices_sent: Vec::new(),
            uid_seed: 0,
            faults: None,
            audio: audio::AudioBook::default(),
        };
        // As with the play target: the engine gets the mixer of the first song with
        // `engine_seats`, so it counts as told.
        play.sync_mix(&mut Vec::new());
        play
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

    /// The tempo map of the song as it is now.
    fn map(&self) -> TempoMap {
        TempoMap::new(&self.doc().timeline)
    }

    /// The clicks of the metronome in a bar where a recording from `tick` counts in.
    fn clicks_per_bar_at(&self, tick: u64) -> u32 {
        self.map().meter_at(tick).meter.clicks_per_bar()
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

    /// The track the keys of a recording go to: the selected one, or with a vocal track
    /// selected the keys track selected before it.
    pub fn record_target(&self) -> Option<TrackRef> {
        self.track_ref(self.song.keys_target()?)
    }

    /// The track as the screen names it now: its number is its place, from 1.
    fn track_ref(&self, id: TrackId) -> Option<TrackRef> {
        let index = self.track_index(id)?;
        Some(TrackRef {
            number: index as u32 + 1,
            name: self.doc().tracks[index].name.clone(),
        })
    }

    /// How many automation lanes the song has: what a step took away with an effect or an
    /// instrument is the difference (spec 6 「帯もいっしょに消えた」).
    fn lane_count(&self) -> u32 {
        (self.doc().tracks.iter())
            .map(|t| t.automation.len() as u32)
            .sum()
    }

    fn track_index(&self, id: TrackId) -> Option<usize> {
        self.doc().tracks.iter().position(|t| t.id == id)
    }

    fn bar_head(&self, tick: u64) -> u64 {
        self.map().meter_at(tick).bar_tick
    }

    /// The bars the range touches, as shown (from 1): its first bar, and the bar line at its
    /// end or after it.
    fn bars(&self, range: TickRange) -> BarSpan {
        let map = self.map();
        let shown = |bar: u64| u32::try_from(bar + 1).unwrap_or(u32::MAX);
        BarSpan {
            from: shown(map.meter_at(range.start()).bar),
            to: shown(map.meter_at(map.bar_at_or_after(range.end())).bar),
        }
    }

    fn loop_on(&self) -> Option<TickRange> {
        self.doc().loop_range.filter(|_| self.song.session.loop_on)
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
                self.homed = false;
                self.state = TransportState::Playing;
                self.last_heard = None;
                effects.push(Effect::Engine(EngineOp::Play { from }));
            }
            _ => self.stop(effects),
        }
    }

    /// R and the record button: to the keys track and to every vocal track with its ● on,
    /// with one count-in (spec 3, 24-C). `input`: the PC's input, when it is open; `format`
    /// and `x_us`: how the files are written and placed. Returns the notice to show.
    pub fn record(
        &mut self,
        can_play: bool,
        input: Option<(OpenInput, RecordFormat, i64)>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        match self.state {
            TransportState::CountIn | TransportState::Recording => {
                self.stop(effects);
                return None;
            }
            _ if self.rec.is_some() || !can_play || self.auditioning() => return None,
            _ => {}
        }
        let track = self.song.keys_target();
        let any_armed = !self.song.session.armed_vocals.is_empty();
        if track.is_none() && !any_armed {
            return Some(StatusMessage::NoArmedTrack);
        }
        let keys_seat = match track {
            Some(track) => Some(self.seats.of(track)?),
            None => None,
        };
        let playing = self.state == TransportState::Playing;
        let (from, count_in_beats) = if playing {
            let now = self
                .heard
                .and_then(|h| h.moving_tick())
                .unwrap_or(self.position);
            (now, 0)
        } else {
            let from = self.loop_on().map_or(self.position, |r| r.start());
            let beats = u32::from(self.count_in_bars) * self.clicks_per_bar_at(from);
            (from, beats)
        };
        let loop_range = self.loop_on().filter(|r| r.end() > from);
        let (vocals, no_input) =
            self.vocals_to_record(input.map(|(open, ..)| open), from, loop_range);
        let notice = no_input.then_some(StatusMessage::NoInput);
        if track.is_none() && vocals.is_empty() {
            return notice;
        }
        let overdub = self.overdub && track.is_some();
        let clip = track.and_then(|t| self.doc().track(t)).and_then(|t| {
            let span = loop_range.unwrap_or_else(|| TickRange::new(from, from + 1).expect("1"));
            t.clips.iter().find(|c| c.range().overlaps(span)).map(|c| {
                let take = if overdub { c.active } else { c.takes.len() };
                (c.id, take as u32)
            })
        });
        let vocal_seats: TrackSet =
            (vocals.iter().filter_map(|v| self.seats.of(v.track))).collect();
        let id = self.next_recording;
        self.next_recording = self.next_recording.wrapping_add(1);
        let setup = input.filter(|_| !vocals.is_empty());
        if let Some((_, format, x_us)) = setup {
            // Before the engine is told: the files are there when the first block comes.
            effects.push(Effect::StartVocalRecording {
                files: Play::vocal_file_asks(&vocals),
                format,
                x_us,
            });
        }
        self.rec = Some(Rec {
            id,
            track,
            vocal: vocal::VocalPart {
                tracks: vocals,
                sample_rate: setup.map_or(0, |(open, ..)| open.sample_rate),
                format: setup.map_or(RecordFormat::default(), |(_, format, _)| format),
                ..vocal::VocalPart::default()
            },
            from,
            take: clip.map_or(1, |(_, n)| n + 1),
            clip: clip.map(|(c, _)| c),
            count_in_beats,
            loop_range,
            lap: 0,
            recorder: None,
            stopped: None,
            stop_us: None,
            live: Vec::new(),
            held: [None; 128],
            live_lap: 0,
            next_index: 0,
            overdub,
            closed: 0,
            ahead: None,
            keys_seat,
            vocal_seats,
        });
        let muted = self.rec.as_ref().expect("just made").muted();
        self.position = from;
        self.state = if count_in_beats > 0 {
            TransportState::CountIn
        } else {
            TransportState::Recording
        };
        self.last_heard = None;
        effects.push(Effect::Engine(if playing {
            EngineOp::RecordHere { muted }
        } else {
            EngineOp::Record {
                from,
                count_in_beats,
                muted,
            }
        }));
        notice
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

    /// Home, a click on the ruler and a position typed: to `tick` as it is (the screen snaps).
    pub fn locate(&mut self, tick: u64, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        self.position = tick;
        self.homed = false;
        effects.push(Effect::Engine(EngineOp::Locate { tick }));
    }

    /// Home: with the loop on, to the loop's head; from there, to the song's start. While
    /// playing the line does not stay at the head, so a Home just before counts as being there.
    pub fn go_to_start(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let head = self.loop_on().map(|r| r.start()).filter(|&head| {
            let at_head = if self.state == TransportState::Playing {
                self.homed && self.position == head
            } else {
                self.play_line() == head
            };
            !at_head
        });
        self.locate(head.unwrap_or(0), effects);
        self.homed = head.is_some();
    }

    /// Where the play line is: while moving, what the listener hears.
    fn play_line(&self) -> u64 {
        self.heard
            .and_then(|h| h.moving_tick())
            .unwrap_or(self.position)
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
        self.song.session.loop_on = !self.song.session.loop_on;
        let range = match self.doc().loop_range {
            None if self.song.session.loop_on => {
                let map = self.map();
                let head = map.meter_at(self.position);
                TickRange::new(head.bar_tick, map.tick_of_bar(head.bar + DEFAULT_LOOP_BARS))
            }
            _ => None,
        };
        self.put_loop(range, effects);
    }

    /// Makes `range` the loop's range, which is one step when it is another range than
    /// before, and tells the engine the loop as it is now.
    fn put_loop(&mut self, range: Option<TickRange>, effects: &mut Vec<Effect>) {
        match range.map(|r| timeline_edit::set_loop(self.doc(), r)) {
            // The engine hears of the new range with the step.
            Some(Ok(e)) => self.apply(e, effects),
            _ => self.send_loop(effects),
        }
    }

    pub fn set_loop(&mut self, from_bar: u32, to_bar: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let (a, b) = (from_bar.min(to_bar).max(1), from_bar.max(to_bar).max(1));
        let b = if a == b { a + 1 } else { b };
        let map = self.map();
        let range = TickRange::new(
            map.tick_of_bar(u64::from(a - 1)),
            map.tick_of_bar(u64::from(b - 1)),
        );
        if range.is_none() {
            return;
        }
        self.song.session.loop_on = true;
        self.put_loop(range, effects);
    }

    fn send_loop(&self, effects: &mut Vec<Effect>) {
        let range = self.doc().loop_range;
        let (start, end) = range.map_or((0, 0), |r| (r.start(), r.end()));
        effects.push(Effect::Engine(EngineOp::Loop {
            start,
            end,
            on: self.song.session.loop_on,
        }));
    }

    /// A click on a track's header, or (`from_arm`) on its ●: the keys play that track and a
    /// recording goes to it. Not while counting in or recording: the recording stays on its
    /// track, and the keys with it. Returns the notice to show.
    pub fn select_track(&mut self, track: u32, from_arm: bool) -> Option<StatusMessage> {
        let id = TrackId(track);
        if self.busy() || self.song.session.play_target == Some(id) {
            return None;
        }
        let said = self.track_ref(id)?;
        let before = self.song.session.play_target.replace(id);
        self.selected_track_changed(before);
        from_arm.then_some(StatusMessage::ArmMoved { track: said })
    }

    /// A click on a clip, or (`None`) where there is no clip.
    pub fn select_clip(&mut self, clip: Option<u32>) {
        match clip.map(ClipId) {
            None => {
                self.clips_sel.clear();
                self.selected = None;
            }
            Some(id) if self.doc().clip(id).is_some() => {
                self.clips_sel = vec![id];
                self.click(id);
            }
            Some(_) => {}
        }
    }

    /// The clip becomes the one clicked: shown in the piano roll, its track selected.
    fn click(&mut self, id: ClipId) {
        if let Some((track, _)) = self.doc().clip(id) {
            if !self.busy() {
                self.song.session.play_target = Some(track.id);
            }
            self.selected = Some(id);
            self.piano_roll = Some(id);
            self.fix_selection();
        }
    }

    /// Shift + click (`Add` or `Toggle` with one clip, which is then clicked) and boxes.
    pub fn select_clips(&mut self, clips: Vec<u32>, mode: SelectMode) {
        let there: Vec<u32> = clips
            .into_iter()
            .filter(|&c| self.doc().clip(ClipId(c)).is_some())
            .collect();
        let now: Vec<u32> = self.clips_sel.iter().map(|c| c.0).collect();
        let one = match (mode, there.as_slice()) {
            (SelectMode::Add | SelectMode::Toggle, &[c]) => Some(ClipId(c)),
            _ => None,
        };
        self.clips_sel = selected_with(&now, there, mode)
            .into_iter()
            .map(ClipId)
            .collect();
        match one.filter(|c| self.clips_sel.contains(c)) {
            Some(id) => self.click(id),
            None => {
                let kept = self
                    .selected
                    .filter(|c| mode == SelectMode::Toggle && self.clips_sel.contains(c));
                self.selected = kept.or_else(|| self.only_clip());
            }
        }
    }

    /// Mod A in the arrange view.
    pub fn select_all_clips(&mut self) {
        self.clips_sel = self
            .doc()
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| c.id)
            .collect();
        self.clips_sel.sort_unstable();
        self.selected = self.only_clip();
    }

    fn only_clip(&self) -> Option<ClipId> {
        match self.clips_sel[..] {
            [one] => Some(one),
            _ => None,
        }
    }

    fn clip_ids(clips: &[u32]) -> Vec<ClipId> {
        clips.iter().map(|&c| ClipId(c)).collect()
    }

    /// Delete in the arrange view: every selected clip, as one step.
    pub fn delete_selected(&mut self, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::delete_clips(self.doc(), &self.clips_sel, false) {
            self.apply_selecting(e, None, Some(ClipPick::none()), effects);
        }
    }

    /// A drag of clips together in the arrange view, `rows` rows of the tracks down (up when
    /// negative); `copy`: Alt + drag. Returns the notice to show.
    pub fn move_clips(
        &mut self,
        clips: &[u32],
        ticks: i64,
        rows: i32,
        copy: bool,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let moved = arrange::move_clips(self.doc(), &Self::clip_ids(clips), ticks, rows, copy);
        let notice = if copy {
            StatusMessage::ClipsNotDuplicated
        } else {
            StatusMessage::ClipsNotMoved
        };
        self.apply_placed(moved, notice, effects)
    }

    /// Mod D in the arrange view. Returns the notice to show.
    pub fn duplicate_clips(
        &mut self,
        clips: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let copied = arrange::duplicate_clips(self.doc(), &Self::clip_ids(clips));
        self.apply_placed(copied, StatusMessage::ClipsNotDuplicated, effects)
    }

    /// Mod C in the arrange view.
    pub fn copy_clips(&mut self, clips: &[u32]) {
        if let Ok(kept) = arrange::copy_clips(self.doc(), &Self::clip_ids(clips)) {
            self.clip_board = kept;
        }
    }

    /// Mod X in the arrange view: a copy, and the clips deleted as one step.
    pub fn cut_clips(&mut self, clips: &[u32], effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let ids = Self::clip_ids(clips);
        let Ok(kept) = arrange::copy_clips(self.doc(), &ids) else {
            return;
        };
        let there: Vec<ClipId> = kept.iter().map(|k| k.clip.id).collect();
        if let Ok(e) = edit::delete_clips(self.doc(), &there, true) {
            self.clip_board = kept;
            let pick = ClipPick {
                clips: self
                    .clips_sel
                    .iter()
                    .copied()
                    .filter(|c| !there.contains(c))
                    .collect(),
                clicked: self.selected.filter(|c| !there.contains(c)),
                shown: None,
            };
            self.apply_selecting(e, None, Some(pick), effects);
        }
    }

    /// Mod V (`to: None`: on the selected track, at the head of the bar the play line is in)
    /// and the right-click menu's "貼りつけ" (where it was clicked). Returns the notice to show.
    pub fn paste_clips(
        &mut self,
        to: Option<ClipSpot>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || self.clip_board.is_empty() {
            return None;
        }
        let (track, at) = match to {
            Some(s) => (TrackId(s.track), s.tick),
            None => (
                self.song.session.play_target?,
                self.bar_head(self.play_line()),
            ),
        };
        let pasted = arrange::paste_clips(self.doc(), &self.clip_board, track, at);
        self.apply_placed(pasted, StatusMessage::ClipsNotPasted, effects)
    }

    /// Selects the clips placed. The one clicked stays clicked (its copy when copied), else
    /// a clip placed alone is; that one is shown in the piano roll.
    fn apply_placed(
        &mut self,
        placed: Result<ClipEdited, EditError>,
        overlap: StatusMessage,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let e = match placed {
            Ok(e) => e,
            Err(EditError::Overlap) => return Some(overlap),
            Err(_) => return None,
        };
        let clicked = self
            .selected
            .and_then(|s| e.from.iter().position(|&f| f == s))
            .map(|i| e.clips[i])
            .or_else(|| match e.clips[..] {
                [one] => Some(one),
                _ => None,
            });
        let mut clips = e.clips.clone();
        clips.sort_unstable();
        let pick = ClipPick {
            clips,
            clicked,
            shown: clicked,
        };
        self.apply_selecting(e.edit, None, Some(pick), effects);
        None
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
            track: self.track_ref(track.id)?,
            clip: clip.name.clone(),
            take: to as u32 + 1,
            takes: n as u32,
        })
    }

    pub fn delete_take(&mut self, clip: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::delete_take(self.doc(), ClipId(clip)) {
            self.apply(e, effects);
        }
    }

    fn sel(&self) -> Sel {
        Sel {
            notes: self.note_sel.clone(),
            clips: self.clips_sel.clone(),
            clicked: self.selected,
            timeline: self.tl.selected(),
            points: self.point_sel(),
        }
    }

    /// What was selected before or after a step, as far as it is still there. The piano roll
    /// keeps showing what it shows.
    fn put_sel(&mut self, sel: Sel, effects: &mut Vec<Effect>) {
        self.note_sel = sel.notes;
        self.clips_sel = sel.clips;
        self.selected = sel.clicked;
        self.tl.select(sel.timeline);
        self.put_point_sel(sel.points);
        self.drop_gone_clips(effects);
    }

    fn apply(&mut self, e: Edit, effects: &mut Vec<Effect>) {
        self.apply_selecting(e, None, None, effects);
    }

    /// `selected`: the notes of the shown take to select afterwards, or its control points
    /// with `true`. The other kind stays selected: an edit of one leaves the other in order.
    /// `clips`: the clips to select afterwards; `None` keeps them.
    fn apply_selecting(
        &mut self,
        e: Edit,
        selected: Option<(Vec<usize>, bool)>,
        clips: Option<ClipPick>,
        effects: &mut Vec<Effect>,
    ) {
        let sel_before = self.sel();
        let before = self.doc().clone();
        self.tl.forget_fresh();
        self.forget_undone_steps();
        self.history.apply(&mut self.song.doc, e);
        if let Some(pick) = clips {
            self.clips_sel = pick.clips;
            self.selected = pick.clicked;
            if pick.shown.is_some() {
                self.piano_roll = pick.shown;
            }
        }
        self.after_change(&before, effects);
        if let Some((picked, of_controls)) = selected {
            let notes = &sel_before.notes;
            let same_clip = notes.clip.is_some() && notes.clip == self.piano_roll;
            let kept = |v: &Vec<u32>| if same_clip { v.clone() } else { Vec::new() };
            let picked = picked.into_iter().map(|i| i as u32);
            if of_controls {
                self.set_selection(kept(&notes.notes), picked);
            } else {
                self.set_selection(picked, kept(&notes.controls));
            }
        }
        self.sel_undo.push((sel_before, self.sel()));
        self.sel_redo.clear();
    }

    fn apply_notes(&mut self, edited: Result<NotesEdited, EditError>, effects: &mut Vec<Effect>) {
        if let Ok(n) = edited {
            self.apply_selecting(n.edit, Some((n.selected, n.of_controls)), None, effects);
        }
    }

    /// Sends what the engine must hear again, and lets go of clips that are gone.
    fn after_change(&mut self, before: &Doc, effects: &mut Vec<Effect>) {
        self.changes += 1;
        self.inserts_follow(effects);
        // Before any state is put back into a plugin: that takes in what a drag changed.
        self.song_changed_under_drags(effects);
        self.sync_seats(before, effects);
        self.mixer_changed();
        self.sync_mix(effects);
        self.send_automation(false, effects);
        let doc = &self.song.doc;
        let target = &mut self.song.session.play_target;
        if target.is_none_or(|t| doc.track(t).is_none()) {
            *target = doc.tracks.first().map(|t| t.id);
        }
        self.drop_gone_vocals();
        let doc = &self.song.doc;
        if doc.timeline != before.timeline {
            effects.push(Effect::Engine(self.tempo_op()));
        }
        if doc.loop_range != before.loop_range {
            // Undo took the range away: there is nothing to go round.
            self.song.session.loop_on &= doc.loop_range.is_some();
            self.send_loop(effects);
        }
        if doc.tracks.iter().any(|t| !t.clips.is_empty()) {
            self.had_clip = true;
        }
        self.drop_gone_clips(effects);
    }

    fn drop_gone_clips(&mut self, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        self.clips_sel.retain(|&c| doc.clip(c).is_some());
        let sel = &self.clips_sel;
        self.selected = self.selected.filter(|c| sel.contains(c));
        self.piano_roll = self.piano_roll.filter(|&c| doc.clip(c).is_some());
        self.tl.drop_gone(doc);
        self.fix_selection();
        self.drop_gone_lanes(effects);
    }

    fn tempo_op(&self) -> EngineOp {
        let doc = self.doc();
        EngineOp::Timeline {
            timeline: Arc::clone(&doc.timeline),
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
        for t in &doc.tracks {
            if let Some(seat) = self.seats.of(t.id) {
                effects.push(Effect::Engine(EngineOp::Sequence {
                    track: seat,
                    events: track_events(doc, t.id),
                }));
            }
        }
        self.send_silent(effects);
        self.send_target(effects);
        self.send_automation(true, effects);
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
                    rec.vocal.roll = None;
                    rec.vocal.lap_frames.clear();
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
            RecordReport::Rolling {
                frame,
                tick,
                file_frame,
            } => {
                if let Some(rec) = &mut self.rec
                    && rec.vocal.roll.is_none()
                {
                    rec.vocal.roll = Some((frame, tick, file_frame));
                }
            }
            RecordReport::LapChanged {
                lap,
                frame,
                file_frame,
            } => {
                let mut changed = Vec::new();
                if let Some(rec) = &mut self.rec
                    && rec.loop_range.is_some()
                {
                    rec.lap = lap;
                    rec.enter_lap(lap, &mut changed);
                    if rec.vocal.lap_frames.len() + 1 == lap as usize {
                        rec.vocal.lap_frames.push((frame, file_frame));
                    }
                }
                self.send_live(changed, effects);
            }
            RecordReport::Stopped { tick, lap, .. } => {
                if self.rec.as_ref().is_some_and(|r| r.recorder.is_some()) {
                    self.finish(tick, lap, effects);
                } else if let Some(rec) = self.rec.take()
                    && !rec.vocal.tracks.is_empty()
                {
                    effects.push(Effect::RemoveRecNote);
                    let files = rec.vocal.tracks.into_iter().map(|v| v.file).collect();
                    effects.push(Effect::DropVocalFiles(files));
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

    /// Puts what was recorded in the song as one step, the keys and the vocals together.
    /// Returns the number of the shown take of the keys.
    fn finish(&mut self, tick: f64, lap: u32, effects: &mut Vec<Effect>) -> Option<u32> {
        let mut rec = self.rec.take()?;
        if !rec.vocal.tracks.is_empty() {
            if rec.vocal.written.is_none() {
                // The frames still on their way to the disk belong to the recording.
                if rec.stopped.replace((tick, lap)).is_none() {
                    effects.push(Effect::StopVocalRecording);
                }
                self.rec = Some(rec);
                return None;
            }
            effects.push(Effect::RemoveRecNote);
        }
        let (vocals, unused) = self.vocal_recordings(&rec, tick, lap);
        let recorded = rec
            .recorder
            .take()
            .and_then(|r| r.finish(tick.round() as i64, lap));
        // The files' ids were counted from the `Doc` as it was: the vocals go in first, and
        // the keys on top of them. The step is the keys' when there are keys.
        let mut unused = unused;
        let (mut edit, mut pick, mut shown) = (None, None, None);
        if !vocals.is_empty() {
            let files: Vec<String> = vocals.iter().map(|v| v.audio.file.clone()).collect();
            match meloet_model::add_vocal_recordings(self.doc(), vocals, None) {
                Ok((e, added)) => {
                    pick = added.first().map(|a| a.clip);
                    edit = Some(e);
                }
                Err(_) => unused.extend(files),
            }
        }
        if !unused.is_empty() {
            effects.push(Effect::DropVocalFiles(unused));
        }
        if let Some((track, recorded)) = rec.track.zip(recorded) {
            let base = edit.as_ref().map_or(self.doc(), |e| &e.doc);
            let added = if rec.overdub {
                // What is left of the passes, each on top of the one before, as one step.
                let mut done: Option<(Edit, RecordingAdded)> = None;
                for take in recorded.takes {
                    let on = done.as_ref().map_or(base, |(e, _)| &e.doc);
                    if let Ok(d) = overdub_recording(on, track, take) {
                        done = Some(d);
                    }
                }
                done.ok_or(EditError::NoTakes)
            } else {
                edit::add_recording(base, track, recorded.takes, recorded.active)
            };
            if let Ok((e, added)) = added {
                pick = Some(added.clip);
                shown = Some(added.active as u32 + 1);
                edit = Some(e);
            }
        }
        let (e, pick) = edit.zip(pick)?;
        self.apply_selecting(e, None, Some(ClipPick::one(pick)), effects);
        shown
    }

    // ---- 「重ねる」 ----

    pub fn with_overdub(mut self, on: bool) -> Play {
        self.overdub = on;
        self
    }

    pub fn overdub(&self) -> bool {
        self.overdub
    }

    /// Takes effect at the next pass of a loop recording going on, else at the next recording.
    pub fn set_overdub(&mut self, on: bool) {
        self.overdub = on;
    }

    /// Called now and then while moving. In a loop recording, a pass is over once it is no
    /// longer heard: with 「重ねる」, its notes go into the song then, one step for each pass
    /// with notes. The engine plays ahead of what is heard, so the notes so far are sent to it
    /// a little before the pass ends, for the next pass to sound them from its start.
    pub fn overdub_due(&mut self, effects: &mut Vec<Effect>) {
        let Some(h) = self.heard.filter(|h| h.state == TransportState::Recording) else {
            return;
        };
        let Some(rec) = &self.rec else {
            return;
        };
        let Some(range) = rec.loop_range else {
            return;
        };
        if rec.track.is_none() || rec.recorder.is_none() || rec.stop_us.is_some() {
            return;
        }
        if h.lap > rec.closed {
            self.pass_over(h.lap - 1, effects);
        }
        let Some(rec) = &self.rec else {
            return;
        };
        if !rec.overdub || rec.ahead.as_ref().is_some_and(|(lap, _)| *lap >= h.lap) {
            return;
        }
        let map = self.map();
        let end = map.seconds_at(range.end() as f64);
        if h.tick >= map.tick_at(end - SEND_PASS_AHEAD_S) {
            self.send_ahead(h.lap, effects);
        }
    }

    fn send_ahead(&mut self, lap: u32, effects: &mut Vec<Effect>) {
        let Some(rec) = &self.rec else {
            return;
        };
        let (Some(track), Some(mut recorder)) = (rec.track, rec.recorder.clone()) else {
            return;
        };
        let mut doc: Option<Doc> = None;
        for take in recorder.close_laps(lap) {
            let on = doc.as_ref().unwrap_or(self.doc());
            if let Ok((e, _)) = overdub_recording(on, track, take) {
                doc = Some(e.doc);
            }
        }
        let events = doc.map(|d| track_events(&d, track));
        let rec = self.rec.as_mut().expect("there");
        if let (Some(events), Some(seat)) = (&events, rec.keys_seat) {
            effects.push(Effect::Engine(EngineOp::GrowSequence {
                track: seat,
                events: events.clone(),
            }));
        }
        rec.ahead = Some((lap, events));
    }

    /// The passes up to `upto` are over. Passes recorded with 「重ねる」 go into the song, one
    /// step each. When 「重ねる」 was turned on during passes that make takes, those takes go
    /// in as one step, and the next passes add to the last of them.
    fn pass_over(&mut self, upto: u32, effects: &mut Vec<Effect>) {
        let next = self.overdub;
        let Some(rec) = &mut self.rec else {
            return;
        };
        let Some(track) = rec.track else {
            return;
        };
        rec.closed = upto + 1;
        let was = rec.overdub;
        if !was && !next {
            return;
        }
        let takes = (rec.recorder.as_mut()).map_or_else(Vec::new, |r| r.close_laps(upto));
        let ahead = rec
            .ahead
            .take()
            .and_then(|(lap, events)| events.filter(|_| lap <= upto));
        rec.overdub = next;
        let start = effects.len();
        if was {
            for take in takes {
                if let Ok((e, added)) = overdub_recording(self.doc(), track, take) {
                    self.apply_pass(e, added, effects);
                }
            }
            self.grow_instead(start, ahead, effects);
        } else if !takes.is_empty() {
            let active = takes.len() - 1;
            if let Ok((e, added)) = edit::add_recording(self.doc(), track, takes, active) {
                self.apply_pass(e, added, effects);
            }
        }
        if was != next {
            let doc = &self.song.doc;
            let rec = self.rec.as_mut().expect("there");
            if !next {
                let takes = rec
                    .clip
                    .and_then(|c| doc.clip(c))
                    .map_or(0, |(_, c)| c.takes.len());
                rec.take = takes as u32 + 1;
            }
            effects.push(Effect::Engine(EngineOp::RecordMuted { muted: rec.muted() }));
        }
    }

    fn apply_pass(&mut self, e: Edit, added: RecordingAdded, effects: &mut Vec<Effect>) {
        self.apply_selecting(e, None, Some(ClipPick::one(added.clip)), effects);
        if let Some(rec) = &mut self.rec {
            rec.clip = Some(added.clip);
            rec.take = added.active as u32 + 1;
        }
    }

    /// The steps of passes from `effects[start..]` on send the seat of the keys only what the
    /// engine does not have: nothing when it got those events ahead, else the last events as
    /// [`EngineOp::GrowSequence`], so that what sounds goes on.
    fn grow_instead(&self, start: usize, ahead: Option<Vec<PlayEvent>>, effects: &mut Vec<Effect>) {
        let Some(seat) = self.rec.as_ref().and_then(|r| r.keys_seat) else {
            return;
        };
        let mut last = None;
        let mut i = start;
        while i < effects.len() {
            if matches!(&effects[i], Effect::Engine(EngineOp::Sequence { track, .. }) if *track == seat)
            {
                if let Effect::Engine(EngineOp::Sequence { events, .. }) = effects.remove(i) {
                    last = Some((i, events));
                }
            } else {
                i += 1;
            }
        }
        if let Some((at, events)) = last
            && ahead.as_ref() != Some(&events)
        {
            effects.insert(
                at,
                Effect::Engine(EngineOp::GrowSequence {
                    track: seat,
                    events,
                }),
            );
        }
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
            .flat_map(|c| c.keys())
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
        let map = self.map();
        self.doc().track(track).map_or_else(Vec::new, |t| {
            t.clips
                .iter()
                .map(|c| ClipView {
                    id: c.id.0,
                    name: c.name.clone(),
                    start: c.start,
                    end: c.end,
                    take: c.active as u32 + 1,
                    takes: c.takes.len() as u32,
                    take_id: c.active_take_id().0,
                    take_offset: c.active_zero() - c.start as i64,
                    note_count: c.note_count() as u32,
                    audio: c.active_audio().and_then(|take| {
                        let source = &take.take.source;
                        let (state, resampled) =
                            (self.audio_state(source), self.audio_resampled(source));
                        vocal::audio_clip_view(c, &map, state, resampled)
                    }),
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
        match self.saving.take() {
            Some(version) => self.history.mark_saved_at(version),
            None => self.history.mark_saved(),
        }
        self.unsaved_anyway = false;
        self.saved_too = None;
    }

    /// A save of the song as it is now begins. It may take a while (the plugins are asked
    /// for their states): what changes meanwhile is not what was saved.
    pub fn saving(&mut self) {
        self.saving = Some(self.history.version());
    }

    /// The save did not come about.
    pub fn not_saved(&mut self) {
        self.saving = None;
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
        self.shelf_closed(effects);
        self.song = song;
        self.saving = None;
        self.forget_mixer();
        self.forget_automation(effects);
        self.reseat(effects);
        self.changes += 1;
        self.serial += 1;
        self.history = History::new();
        self.unsaved_anyway = unsaved;
        self.saved_too = None;
        self.state = TransportState::Stopped;
        self.position = 0;
        self.clips_sel.clear();
        // The takes kept have ids of the other song.
        self.clip_board.clear();
        self.selected = None;
        self.piano_roll = None;
        self.note_sel = NoteSel::default();
        self.tl = timeline::Tl::default();
        self.sel_undo.clear();
        self.sel_redo.clear();
        self.step_notes = history::StepNotes::default();
        self.rec = None;
        self.heard = None;
        self.last_heard = None;
        self.had_clip = self.doc().tracks.iter().any(|t| !t.clips.is_empty());
        self.stream_opened(effects);
    }

    /// The song brought back from the recovery file, as the first step over the song as
    /// saved, which is how the history starts (and what counts as saved).
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
    pub fn discard_recovered(&mut self, saved: Doc, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        self.unsaved_anyway = false;
        self.serial += 1;
        self.apply(
            Edit {
                doc: saved,
                step: StepKind::DiscardRecovered,
            },
            effects,
        );
        self.saved_too = Some(self.history.version());
    }

    /// Goes up whenever the `Doc` changes or another song takes its place.
    pub fn changes(&self) -> u64 {
        self.changes
    }

    /// See `SongView::serial`.
    pub fn serial(&self) -> u32 {
        self.serial
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
        self.doc().loop_range.map(|r| TickSpan {
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
                None => {
                    let from = self.loop_on().map_or(self.position, |r| r.start());
                    u32::from(self.count_in_bars) * self.clicks_per_bar_at(from)
                }
            },
            can_play: open,
            can_record: open
                && (self.song.keys_target().is_some()
                    || !self.song.session.armed_vocals.is_empty())
                && !self.auditioning(),
            recording: self.rec.as_ref().and_then(|r| {
                Some(RecordingView {
                    id: r.id,
                    track: r.track?.0,
                    from: r.from,
                    take: r.take,
                    clip: r.clip.map(|c| c.0),
                    lap: r.lap,
                    overdub: r.overdub,
                })
            }),
            recording_vocals: self.recording_vocals_view(),
            recording_faults: self.faults,
            overdub: self.overdub,
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
            selected_clips: self
                .doc()
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .filter(|c| self.clips_sel.contains(&c.id))
                .map(|c| c.id.0)
                .collect(),
            selected_clip: self.selected.map(|c| c.0),
            piano_roll_clip: self.piano_roll.map(|c| c.0),
            clipboard_clips: self.clip_board.len() as u32,
            // The rules above the song have it (`state.rs`).
            tall_track: None,
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
        if let Some(said) = self.vocal_recording_status(rec) {
            return Some(said);
        }
        let track = self.track_ref(rec.track?)?;
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
        let nothing_selected = self.clips_sel.is_empty() && notes.is_none();
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
        let mut on_key = [0u32; 128];
        for &i in sel.iter() {
            if let Some(n) = take.notes.get(i as usize) {
                on_key[usize::from(n.key)] += 1;
            }
        }
        let names = (0u8..=127)
            .map(|key| KeyCount {
                key,
                count: on_key[usize::from(key)],
            })
            .filter(|k| k.count > 0)
            .take(NOTE_SELECTION_KEYS + 1)
            .collect();
        Some(NoteSelection {
            names,
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

    /// The clip the piano roll shows. Never a vocal clip, which has no notes to show.
    fn shown(&self) -> Option<&Arc<Clip>> {
        let (_, clip) = self.doc().clip(self.piano_roll?)?;
        clip.is_keys().then_some(clip)
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
    /// track the keys play, in the clip there or in a new one. Outside the shown clip, the clip
    /// grows by bars (B-82). Returns the notice to show.
    pub fn add_note(
        &mut self,
        take: Option<u32>,
        note: Note,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || note.key > 127 {
            return None;
        }
        let at = self.write_at(take)?;
        let scale = self.song.session.roll.scale;
        let track = match at {
            WriteAt::Clip(clip) => self.doc().clip(clip).map(|(t, _)| t),
            WriteAt::Track(track) => self.doc().track(track),
        };
        // A drum kit has no scale: every key is one of its instruments.

        let drums = track?.instrument.group() == InstrumentGroup::Drums;
        if scale.lock && !drums && !scale.contains(note.key) {
            return Some(StatusMessage::OutOfScale {
                key: note.key,
                root: scale.root(),
                mode: scale_mode(scale.mode),
            });
        }
        self.write_notes(at, &[note], effects)
    }

    /// The chord stamp: a note on each of `keys`, as one step, all of them selected. Keys past
    /// 127 are left out and a key given twice is written once. With the scale locked, keys
    /// outside it are written too (the owner's decision, 2026-11-23). As `add_note` otherwise.
    pub fn add_notes(
        &mut self,
        take: Option<u32>,
        (start, length): (u64, u64),
        keys: &[u8],
        velocity: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let keys: BTreeSet<u8> = keys.iter().copied().filter(|&k| k <= 127).collect();
        if keys.is_empty() {
            return None;
        }
        let at = self.write_at(take)?;
        let notes: Vec<Note> = (keys.into_iter())
            .map(|key| Note {
                from: None,
                start,
                length,
                key,
                velocity,
            })
            .collect();
        self.write_notes(at, &notes, effects)
    }

    /// Where the write tool writes: the shown clip when `take` is its shown take; with `take:
    /// None` while no clip is shown, the track the keys play.
    fn write_at(&self, take: Option<u32>) -> Option<WriteAt> {
        match take {
            Some(take) => Some(WriteAt::Clip(self.aimed(take)?)),
            None if self.shown().is_some() => None,
            None => Some(WriteAt::Track(self.song.session.play_target?)),
        }
    }

    /// Written on a track, the clip of the notes is selected and shown. Returns the notice to
    /// show.
    fn write_notes(
        &mut self,
        at: WriteAt,
        notes: &[Note],
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let notes: Vec<Note> = (notes.iter())
            .map(|n| Note {
                length: n.length.max(1),
                velocity: n.velocity.clamp(1, 127),
                ..*n
            })
            .collect();
        let written = match notes::write_notes(self.doc(), at, &notes) {
            Ok(w) => w,
            Err(EditError::Overlap) => return Some(StatusMessage::NotesNotWritten),
            Err(_) => return None,
        };
        let on_track = matches!(at, WriteAt::Track(_));
        let pick = on_track.then(|| ClipPick::one(written.clip));
        self.apply_selecting(written.edit, Some((written.selected, false)), pick, effects);
        None
    }

    /// Out of the shown clip, the clip grows by bars (B-82). Returns the notice to show.
    pub fn move_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        ticks: i64,
        keys: i32,
        copy: bool,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let clip = self.aimed(take)?;
        let r = notes::move_notes(self.doc(), clip, &indexes(picked), ticks, keys, copy);
        let overlap = if copy {
            StatusMessage::ClipsNotDuplicated
        } else {
            StatusMessage::ClipsNotMoved
        };
        self.apply_growing(r, overlap, effects)
    }

    /// Out of the shown clip, the clip grows by bars (B-82). Returns the notice to show.
    pub fn resize_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        edge: ClipEdge,
        ticks: i64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let edge = match edge {
            ClipEdge::Start => arrange::ClipEdge::Start,
            ClipEdge::End => arrange::ClipEdge::End,
        };
        let clip = self.aimed(take)?;
        let r = notes::resize_notes(self.doc(), clip, &indexes(picked), edge, ticks);
        self.apply_growing(r, StatusMessage::NotesNotResized, effects)
    }

    /// Like `apply_notes`; `overlap` is said when the clip could not grow.
    fn apply_growing(
        &mut self,
        edited: Result<NotesEdited, EditError>,
        overlap: StatusMessage,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if edited == Err(EditError::Overlap) {
            return Some(overlap);
        }
        self.apply_notes(edited, effects);
        None
    }

    /// "そろえる" of the velocity panel. Returns the notice to show.
    pub fn match_velocity(
        &mut self,
        take: u32,
        picked: &[u32],
        value: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let clip = self.aimed(take)?;
        let value = value.clamp(1, 127);
        let picked = indexes(picked);
        let r = notes::set_notes_field(
            self.doc(),
            clip,
            &picked,
            ModelNoteField::Velocity,
            i64::from(value),
        );
        let count = picked.iter().collect::<BTreeSet<_>>().len();
        // Notes already at the value are said to be, too.
        if count == 0 || !matches!(r, Ok(_) | Err(EditError::Unchanged)) {
            return None;
        }
        self.apply_notes(r, effects);
        Some(StatusMessage::VelocityMatched {
            count: count as u32,
            value,
        })
    }

    /// Like `match_velocity`: every picked note to `value` ticks (at least 1, and each kept in
    /// its clip).
    pub fn match_length(
        &mut self,
        take: u32,
        picked: &[u32],
        value: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        let clip = self.aimed(take)?;
        let value = value.clamp(1, i64::MAX as u64);
        let picked = indexes(picked);
        let r = notes::set_notes_field(
            self.doc(),
            clip,
            &picked,
            ModelNoteField::Length,
            value as i64,
        );
        let count = picked.iter().collect::<BTreeSet<_>>().len();
        if count == 0 || !matches!(r, Ok(_) | Err(EditError::Unchanged)) {
            return None;
        }
        self.apply_notes(r, effects);
        Some(StatusMessage::LengthMatched {
            count: count as u32,
            value,
        })
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

    /// Out of the shown clip, the clip grows by bars (B-82). Returns the notice to show.
    pub fn paste_notes(&mut self, at: u64, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() || self.clipboard.is_empty() {
            return None;
        }
        let clip = self.shown()?.id;
        let r = notes::paste_notes(self.doc(), clip, &self.clipboard, at);
        self.apply_growing(r, StatusMessage::ClipsNotPasted, effects)
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

    /// The piano roll sounds keys on the track the keys play, in place of what it sounded: not
    /// keys it may not write (outside the locked scale), and nothing without an output. No keys
    /// only stops; keys all left out change nothing.
    pub fn audition(&self, keys: &[u8], velocity: u8, open: bool, effects: &mut Vec<Effect>) {
        if !open {
            return;
        }
        if keys.is_empty() {
            self.audition_off(open, effects);
            return;
        }
        // Every key sounds, also outside a locked scale: the keys column and the notes are heard
        // as they are (B-62). The write tool does not press outside the scale on its own.
        let keys: Vec<u8> = keys.iter().copied().filter(|&k| k <= 127).collect();
        if keys.is_empty() {
            return;
        }
        if let Some(track) = self.target_seat() {
            effects.push(Effect::Engine(EngineOp::Audition {
                track,
                keys,
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

pub(crate) mod ai;
mod audio;
mod audiosync;
mod automation;
mod autosync;
mod buttons;
mod clips;
mod history;
mod mix;
pub(crate) mod mixer;
mod mixsync;
mod params;
mod plugs;
mod seats;
mod shelf;
mod sounds;
mod timeline;
mod tracks;
pub(crate) mod vocal;
mod windows;

pub use audio::ClipGains;
pub use audiosync::{clip_tables, clip_tables_of};
pub use autosync::SeatLane;
use mixer::Mixer;
pub use mixer::{Preview, Selecting};
use mixsync::MixTold;
use params::Panel;
use plugs::Plugs;
pub use plugs::{PlugState, Spot};
use seats::Seats;
pub use seats::{AUDITION_SEAT, MAX_TRACKS};
use shelf::Shelf;
pub use vocal::{OpenInput, VocalFileAsk, VoiceSpec};
use windows::Windows;

#[cfg(test)]
mod arrange_tests;
#[cfg(test)]
mod buttons_tests;
#[cfg(test)]
mod clips_tests;
#[cfg(test)]
mod overdub_tests;
#[cfg(test)]
mod roll_tests;
#[cfg(test)]
pub(crate) mod tests;
