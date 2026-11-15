//! The contract between Rust and the screen (docs/architecture.md 3).
//!
//! JSON shapes are camelCase. The TypeScript types in `ui/src/bridge/generated/` are written
//! from these types by `cargo test -p meloet-protocol`.
//! All times are CLOCK_MONOTONIC. JSON carries microseconds (`tUs`); the telemetry binary
//! carries nanoseconds. Song positions are ticks, 480 to a quarter note (docs/architecture.md 11).

mod take;
mod telemetry;

use serde::{Deserialize, Serialize};

pub use take::{
    TAKE_CONTROL_LEN, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NO_FROM, TAKE_NOTE_LEN, TAKE_VERSION,
    TakeControl, TakeControlValue, TakeError, TakeNote, TakeNotes,
};
pub use telemetry::{
    TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, Telemetry, TelemetryError,
    TransportReading,
};

pub const PROTOCOL_VERSION: u32 = 7;

/// Screen → Rust. Intent only; Rust decides what the state becomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// CSS px inside the WebView.
    SetViewport {
        width: f32,
        height: f32,
    },
    /// 100 | 125 | 150
    SetScale {
        percent: u16,
    },
    ToggleBand {
        band: Band,
    },
    GoArrange,
    Back,
    /// Mod+, opens the front; "くわしく" in the status line opens it with the advanced part.
    OpenSettings {
        advanced: bool,
    },
    SettingsSelectOutput {
        output: String,
    },
    SettingsToggleAdvanced,
    SettingsApply,
    SettingsCancel,
    ChooserMove {
        delta: i32,
    },
    ChooserSelect {
        output: String,
    },
    ChooserConfirm,
    TestTone {
        output: String,
    },
    DismissHint,
    /// Space: play when stopped, stop otherwise.
    PlayStop,
    /// R and the record button: stopped → count-in then record; playing → record from here;
    /// counting in or recording → stop.
    Record,
    /// Home.
    GoToStart,
    /// A click on the ruler. `tick` is where it was clicked; Rust moves to a bar head.
    Locate {
        tick: u64,
    },
    /// Beats per minute as typed; Rust keeps 0.01 steps.
    SetTempo {
        bpm: f64,
    },
    SetMeter {
        num: u8,
        den: u8,
    },
    /// 0, 1 or 2.
    SetCountIn {
        bars: u8,
    },
    /// C.
    ToggleMetronome,
    /// L and the loop button. The range stays.
    ToggleLoop,
    /// A drag on the ruler. Bar lines, counting from 1, in either order: 5 and 9 are the heads
    /// of bars 5 and 9 ("ループ 5–9").
    SetLoop {
        from_bar: u32,
        to_bar: u32,
    },
    /// The ● of a track. `track` is `TrackView::number`.
    ToggleArm {
        track: u32,
    },
    /// `None`: a click where there is no clip.
    SelectClip {
        clip: Option<u32>,
    },
    /// Delete.
    DeleteSelected,
    /// ◂ ▸, T, Shift T on the selected clip. -1 is the take before, 1 the take after.
    StepTake {
        delta: i32,
    },
    Undo,
    Redo,
    /// Mod N and the song menu. Asks first when there are unsaved changes.
    NewSong,
    /// Mod O and the song menu. Asks first when there are unsaved changes, then Rust shows the
    /// PC's open window.
    OpenSong,
    /// Mod S and the song menu. The first save goes to the song folder without asking.
    SaveSong,
    /// A button of the song dialog. `dialog` is `SongDialogView::id`; an answer to a dialog that
    /// is no longer up, or that the dialog has no button for, is dropped. Esc stays `Back`,
    /// which is "やめる" while a song dialog is up.
    AnswerSongDialog {
        dialog: u32,
        answer: SongAnswer,
    },
    // The piano roll and the clips (round 3). A note is named by `take`, the id of the take the
    // piano roll shows (`PianoRollView::take`), and its index in that take's notes (in order of
    // start, from 0). A command whose `take` is not the take shown now is dropped: it was aimed
    // at notes that have moved since. Fields named `start`, `at` or `tick` are song positions;
    // fields named `ticks` are how far to move.
    //
    // Every command that changes notes makes a new take (a new id) and puts it in place of the
    // shown one in its clip, so the number of takes stays the same; undo brings the old id back.
    /// A click, a box drawn with the select tool, Shift + box (`Add`), and Mod + click (`Toggle`).
    SelectNotes {
        take: u32,
        notes: Vec<u32>,
        mode: SelectMode,
    },
    /// Mod A in the piano roll.
    SelectAllNotes,
    ClearNoteSelection,
    /// The write tool: one command when the mouse button is let go. `take: None` when the piano
    /// roll shows no clip: Rust makes a clip on the track the keys play, from the bar of `start`,
    /// 4 bars long or up to the next clip, named after the track; making it and writing the note
    /// are one step (the user's decision, 2026-11-15).
    AddNote {
        take: Option<u32>,
        start: u64,
        length: u64,
        key: u8,
        velocity: u8,
    },
    /// A drag, ← → (a grid step), ↑ ↓ (1 or 12 keys). `copy`: Alt + drag and Mod D (the copies
    /// are selected afterwards).
    MoveNotes {
        take: u32,
        notes: Vec<u32>,
        ticks: i64,
        keys: i32,
        copy: bool,
    },
    /// A drag on the right edge: every note grows or shrinks by the same amount.
    ResizeNotes {
        take: u32,
        notes: Vec<u32>,
        ticks: i64,
    },
    /// The erase tool, and Delete while the piano roll has the keys. Deleting clips is
    /// `DeleteSelected`.
    DeleteNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// The velocity lane. The result stays in 1..=127.
    ChangeVelocity {
        take: u32,
        notes: Vec<u32>,
        delta: i32,
    },
    /// Mod C.
    CopyNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// Mod X.
    CutNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// Mod V: into the clip of the piano roll, the earliest note at `at`.
    PasteNotes {
        at: u64,
    },
    /// `division`: how many to a whole note (4 is a quarter, 16 a sixteenth). `triplet` makes
    /// each step 2/3 as long.
    SetGrid {
        division: u16,
        triplet: bool,
    },
    /// "スケール固定" and "音階". `root` 0..=11 is C..B.
    SetMusicalScale {
        root: u8,
        mode: ScaleMode,
        lock: bool,
    },
    /// The write tool sounds the key the moment it is pressed (mock R3-PRMouse).
    Audition {
        key: u8,
        velocity: u8,
    },
    AuditionOff,
    /// The quantize panel: the settings the next `Quantize` uses. `strength` and `swing` are
    /// percent, 0..=100.
    SetQuantize {
        division: u16,
        triplet: bool,
        strength: u8,
        swing: u8,
        align: QuantizeAlign,
    },
    /// Q and "かけなおす". Empty `notes`: every note of the take. The new take points to the
    /// recorded take it came from (`TakeNotes::origin`).
    Quantize {
        take: u32,
        notes: Vec<u32>,
    },
    /// "もとの位置へ戻す": the notes go back to where they are in the origin take.
    Unquantize {
        take: u32,
        notes: Vec<u32>,
    },
    /// The note list and the "選んだノート" panel. `Length` and `Velocity` set every note to
    /// `value`. `Start` puts the earliest note at `value` (a song position) and keeps the
    /// distances of the others; `Key` puts the lowest note on `value` and keeps the intervals.
    SetNotesField {
        take: u32,
        notes: Vec<u32>,
        field: NoteField,
        value: i64,
    },
    /// Control points are named by their index in the take's controls (in order of tick).
    SelectControls {
        take: u32,
        points: Vec<u32>,
        mode: SelectMode,
    },
    /// CC 0..=127 (an on/off CC such as sustain only 0 or 127), pitch bend -8192..=8191.
    AddControl {
        take: u32,
        tick: u64,
        control: ControlKind,
        value: i32,
    },
    /// `value` is how much the values change.
    MoveControls {
        take: u32,
        points: Vec<u32>,
        ticks: i64,
        value: i32,
    },
    DeleteControls {
        take: u32,
        points: Vec<u32>,
    },
    /// A drag in the arrange view; `track` is `TrackView::number` of the track it is dropped
    /// on. `copy`: Alt + drag.
    MoveClip {
        clip: u32,
        ticks: i64,
        track: u32,
        copy: bool,
    },
    /// Mod D in the arrange view: a copy right after the clip.
    DuplicateClip {
        clip: u32,
    },
    /// A drag on an edge. What is trimmed off stays in the take and comes back when the edge
    /// is pulled out again.
    TrimClip {
        clip: u32,
        edge: ClipEdge,
        ticks: i64,
    },
    /// Enter on a number field that holds a value out of range. `kept` is the value it still
    /// has, as shown. Rust only says so in the status line; the field stays "broken".
    RejectValue {
        field: ValueField,
        kept: String,
    },
    /// "戻さない" on the recovery hint: back to the song as last saved, as one step.
    DiscardRecovered,
    /// "ファイルの場所をひらく" on the hint about a recovery file that did not read.
    ShowRecoveryFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SelectMode {
    /// The selection becomes these.
    Replace,
    /// These are added to the selection.
    Add,
    /// Each of these flips between selected and not.
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ScaleMode {
    Major,
    Minor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum QuantizeAlign {
    /// "はじまり".
    Start,
    /// Both the start and the end of each note.
    StartAndEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum NoteField {
    Start,
    Key,
    Length,
    Velocity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ControlKind {
    Cc { number: u8 },
    PitchBend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ClipEdge {
    Start,
    End,
}

/// The number fields whose range the status line names (spec 6 「使えない値を打った」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ValueField {
    Tempo,
    MeterNum,
    MeterDen,
    Velocity,
}

/// The buttons of the song dialogs, one for one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongAnswer {
    /// "保存する" (`Unsaved`).
    Save,
    /// "保存しない" (`Unsaved`).
    DontSave,
    /// "場所をえらぶ" (`CannotSave`): Rust shows the PC's folder window.
    ChooseLocation,
    /// "1 つ前を開く" (`Broken` with `previous`).
    OpenPrevious,
    /// "やめる" (every dialog).
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Band {
    PianoRoll,
    Mixer,
    History,
    Shelf,
    Params,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub rev: u64,
    pub state: AppState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub song: SongView,
    pub transport: TransportView,
    pub arrange: ArrangeView,
    pub piano_roll: PianoRollView,
    pub workspace: WorkspaceView,
    pub keyboards: KeyboardsView,
    pub outputs: OutputsView,
    pub inputs: InputsView,
    pub audio: AudioView,
    pub status: StatusView,
}

impl AppState {
    /// The sections that differ between `old` and `new`, each one whole.
    pub fn changes_from(old: &AppState, new: &AppState) -> Vec<Change> {
        let mut out = Vec::new();
        if old.song != new.song {
            out.push(Change::Song(new.song.clone()));
        }
        if old.transport != new.transport {
            out.push(Change::Transport(new.transport.clone()));
        }
        if old.arrange != new.arrange {
            out.push(Change::Arrange(new.arrange.clone()));
        }
        if old.piano_roll != new.piano_roll {
            out.push(Change::PianoRoll(new.piano_roll.clone()));
        }
        if old.workspace != new.workspace {
            out.push(Change::Workspace(new.workspace.clone()));
        }
        if old.keyboards != new.keyboards {
            out.push(Change::Keyboards(new.keyboards.clone()));
        }
        if old.outputs != new.outputs {
            out.push(Change::Outputs(new.outputs.clone()));
        }
        if old.inputs != new.inputs {
            out.push(Change::Inputs(new.inputs.clone()));
        }
        if old.audio != new.audio {
            out.push(Change::Audio(new.audio.clone()));
        }
        if old.status != new.status {
            out.push(Change::Status(new.status.clone()));
        }
        out
    }

    /// Replaces the section a change names.
    pub fn apply(&mut self, change: Change) {
        match change {
            Change::Song(v) => self.song = v,
            Change::Transport(v) => self.transport = v,
            Change::Arrange(v) => self.arrange = v,
            Change::PianoRoll(v) => self.piano_roll = v,
            Change::Workspace(v) => self.workspace = v,
            Change::Keyboards(v) => self.keyboards = v,
            Change::Outputs(v) => self.outputs = v,
            Change::Inputs(v) => self.inputs = v,
            Change::Audio(v) => self.audio = v,
            Change::Status(v) => self.status = v,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SongView {
    pub name: String,
    pub tempo_bpm: f64,
    pub meter_num: u8,
    pub meter_den: u8,
    pub tempo_changes: u32,
    pub undo_count: u32,
    pub redo_count: u32,
    /// For the piano roll band label: the notes of the shown take of `ArrangeView::piano_roll_clip`.
    pub note_count: u32,
    pub tracks: Vec<TrackView>,
    pub play_target: Option<u32>,
    /// Kept while the loop is off.
    pub loop_range: Option<TickSpan>,
    pub loop_on: bool,
    pub file: SongFile,
}

/// The song against its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongFile {
    /// Never saved, and nothing to save: a new song as it was made.
    NeverSaved,
    /// The file holds the song as it is.
    Saved,
    /// There are unsaved changes (also a new song with steps, and a song opened from the copy
    /// kept before the last save).
    Changed,
}

/// `[start, end)` in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TickSpan {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub number: u32,
    pub name: String,
    pub color: u8,
    pub instrument: InstrumentKind,
    pub gain_db: f32,
    /// Recording goes to this track.
    pub armed: bool,
    /// In order of start, never overlapping.
    pub clips: Vec<ClipView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipView {
    pub id: u32,
    pub name: String,
    pub start: u64,
    pub end: u64,
    /// The take shown and played, counting from 1 ("2/3").
    pub take: u32,
    pub takes: u32,
    /// Fetch its notes with `take_notes`; they never change, so keep them by this id.
    pub take_id: u32,
    /// The take's tick 0 is at `start + take_offset` in the song; below 0 when the start of
    /// the clip has been trimmed.
    pub take_offset: i64,
    pub note_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum InstrumentKind {
    Placeholder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum TransportState {
    #[default]
    Stopped,
    CountIn,
    Playing,
    Recording,
}

/// Where it moves between patches comes from the telemetry (position, count-in beats left, lap).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TransportView {
    pub state: TransportState,
    /// Where play and record start from; while moving, where it started.
    pub position: u64,
    pub metronome: bool,
    /// 0, 1 or 2.
    pub count_in_bars: u8,
    /// Beats of the count-in: of the one running, or else of the next one.
    pub count_in_beats: u32,
    /// False while the output is not playing.
    pub can_play: bool,
    /// Also false with no armed track. The record button still takes a click then, to say why.
    pub can_record: bool,
    /// From the record command until the take is in the song (also during the count-in).
    pub recording: Option<RecordingView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RecordingView {
    /// Counts up with each recording; `LiveNotes::recording` names it.
    pub id: u32,
    pub track: u32,
    /// Where recording starts in the song.
    pub from: u64,
    /// The number the take of the current lap gets in its clip, counting from 1. A loop lap
    /// with no notes makes no take, so it goes up only after a lap with notes.
    pub take: u32,
    /// The clip the takes go into, when the recording overlaps one already.
    pub clip: Option<u32>,
    /// Pass of the loop, 0 for the first; 0 without a loop.
    pub lap: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ArrangeView {
    pub selected_clip: Option<u32>,
    /// The clip the piano roll shows: the one selected last, kept after it is unselected.
    pub piano_roll_clip: Option<u32>,
}

/// The piano roll's own state, kept apart from `song` so that a new selection does not send
/// the song again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PianoRollView {
    /// The shown take of `ArrangeView::piano_roll_clip`. The selections are indexes into it.
    pub take: Option<u32>,
    /// Indexes of the take's notes, in order, ranges that touch joined into one.
    pub selected_notes: Vec<IndexRange>,
    /// Indexes of the take's controls, the same way.
    pub selected_controls: Vec<IndexRange>,
    pub grid: Grid,
    pub scale: MusicalScale,
    /// What the quantize panel shows: the settings the next `Quantize` uses.
    pub quantize: QuantizeSettings,
    /// How many notes `PasteNotes` would put in; 0 when there is nothing to paste.
    pub clipboard_notes: u32,
}

/// `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct IndexRange {
    pub start: u32,
    pub end: u32,
}

/// `division`: how many to a whole note (16 is "1/16"); 0 is one bar ("1 小節", as long as the
/// meter makes it). `triplet` makes each step 2/3 as long.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub division: u16,
    pub triplet: bool,
}

/// `root` 0..=11 is C..B. `lock`: "スケール固定 ・ オン", only keys of the scale can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct MusicalScale {
    pub root: u8,
    pub mode: ScaleMode,
    pub lock: bool,
}

/// `strength` and `swing` are percent, 0..=100.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct QuantizeSettings {
    pub division: u16,
    pub triplet: bool,
    pub strength: u8,
    pub swing: u8,
    pub align: QuantizeAlign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub scale: u16,
    pub viewport_width: f32,
    pub viewport_height: f32,
    /// Zooms whose smallest window does not fit the screen the window is on. Empty until the
    /// screen is known; never holds 100.
    pub unfit_scales: Vec<u16>,
    pub layout: LayoutView,
    pub settings: Option<SettingsDialogView>,
    pub chooser: Option<ChooserView>,
    pub hint: Option<Hint>,
    pub song_dialog: Option<SongDialogView>,
    /// The hint about the recovery file. While it is up, `hint` is `None`. It stays until the
    /// song is saved, "戻さない" or "閉じる" (`DismissHint`), or the song is let go.
    pub recovery: Option<RecoveryView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RecoveryView {
    /// The song was brought back: "保存する / 戻さない".
    Recovered {
        /// When the recovery file was written, Unix ms. The screen shows it in local time.
        written_ms: u64,
        cause: RecoveryCause,
        /// The song's name; `None` for a song never saved.
        song: Option<String>,
        /// The takes of every clip, all together.
        takes: u32,
    },
    /// The recovery file did not read and was kept: "ファイルの場所をひらく / 閉じる".
    Unreadable {
        /// The last saved song, opened instead; `None`: a new song.
        song: Option<String>,
    },
}

/// When the recovery file was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum RecoveryCause {
    /// The PC was ending (a signal, or logind's PrepareForShutdown).
    PcEnding,
    /// Every now and then, a little after the song changed: meloet ended without warning.
    Sudden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SongDialogView {
    /// New for every dialog put up; `Command::AnswerSongDialog` names it.
    pub id: u32,
    pub kind: SongDialogKind,
    /// The heading: the song's name, or for `Broken` the name of the file that could not be
    /// read. `Unsaved` puts the same name in its sentence ("{song}").
    pub title: String,
    /// `Broken` only: the copy kept before the last save can be read. Without it the dialog
    /// has only the first sentence and "やめる".
    pub previous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum SongDialogKind {
    /// "保存する / 保存しない / やめる". Before closing, a new song, or opening another.
    Unsaved,
    /// "場所をえらぶ / やめる".
    CannotSave,
    /// "1 つ前を開く / やめる", or "やめる" alone.
    Broken,
}

/// Heights are px at 100 %; the screen multiplies by the scale. `lower` is top to bottom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LayoutView {
    pub meter: MeterSize,
    pub arrange_height: f32,
    pub arrange_overview: bool,
    pub lower: Vec<LowerBandView>,
    pub right_open: Vec<Band>,
    pub right_front: Option<Band>,
    pub show_back_hint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum MeterSize {
    Full,
    Slim,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LowerBandView {
    pub band: Band,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SettingsDialogView {
    pub draft_output: Option<String>,
    pub advanced_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ChooserView {
    pub highlighted: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Hint {
    SoundCameOut,
    EmptyArrange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyboardsView {
    pub list: Vec<KeyboardView>,
    /// The keyboard that was played last; before any key, the one plugged in last.
    pub active: Option<String>,
    pub lost: Option<String>,
    /// The keyboard input itself stopped (the PC's MIDI system, not one keyboard), and meloet
    /// is starting it again. The list is empty meanwhile.
    pub input_stopped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyboardView {
    pub id: String,
    pub name: String,
    pub port_name: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct OutputsView {
    pub list: Vec<EndpointView>,
    pub chosen: Option<String>,
    pub chosen_state: Option<ChosenState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct InputsView {
    pub list: Vec<EndpointView>,
    pub chosen: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EndpointView {
    pub id: String,
    pub name: String,
    pub friendly: Option<Friendly>,
    pub availability: Availability,
    pub link: LinkKind,
    pub node_name: Option<String>,
    /// The PC's name of the device this endpoint is on, as it is.
    pub device: Option<String>,
    /// `None`: meloet can play through it without changing any PC-wide setting. Others are
    /// listed but cannot be chosen.
    pub unusable: Option<Unusable>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Unusable {
    /// A hole on a device that plays, but the PC has the device on another hole now.
    SwitchedByPc,
    /// Nothing plays through it: a PC setting has it off.
    StoppedByPc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Friendly {
    Headphones,
    Hdmi,
    LineOut,
    RearMic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    Jack,
    Plug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ChosenState {
    Ready,
    NotPlugged,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    pub running: bool,
    pub system: String,
    pub sample_rate: Option<u32>,
    pub requested_frames: u32,
    pub actual_frames: Option<u32>,
    pub forced_quantum: Option<u32>,
    pub latency_ms: Option<f32>,
    pub latency_ms_at_requested: Option<f32>,
    pub output_device: Option<String>,
    pub midi_input: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct StatusView {
    pub message: Option<StatusMessage>,
}

/// Which sentence, and the values to put in it. The wording lives in the screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "id", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StatusMessage {
    SearchingKeyboard,
    KeyboardFound {
        keyboard: String,
    },
    KeyboardLost {
        keyboard: String,
    },
    /// "出口を{output}にしました。{track} が録音待機になっています。" `track` is the armed one.
    OutputChosen {
        output: String,
        track: Option<TrackRef>,
    },
    OutputNotPlugged {
        output: String,
    },
    OutputMissing {
        output: String,
    },
    BufferFixed {
        actual: u32,
        requested: u32,
        latency_ms: f32,
        latency_ms_at_requested: f32,
    },
    SettingsOpen,
    /// "新しい曲 ・ テンポ {tempo} ・ {signature} ・ {track} が録音待機"
    NewSong {
        tempo_bpm: f64,
        meter_num: u8,
        meter_den: u8,
        track: Option<TrackRef>,
    },
    BandFoldedWindowLow {
        band: Band,
    },
    BandFoldedToOpen {
        opened: Band,
        folded: Band,
    },
    /// A third bottom band was opened; the one opened first was folded (two at most).
    BandFoldedThird {
        opened: Band,
        folded: Band,
    },
    /// The zoom went up to `scale` and the window, in px at 100 %, got too low for `band`.
    BandFoldedScale {
        scale: u16,
        band: Band,
    },
    ScaleDoesNotFit {
        requested: u16,
        kept: u16,
    },
    ScaleWindowGrown {
        scale: u16,
    },
    ScaleLoweredForScreen {
        from: u16,
        to: u16,
    },
    /// PipeWire cannot be reached. meloet connects again by itself when it is back.
    AudioSystemStopped,
    /// A recording was stopped because PipeWire stopped; what was played is in `take`.
    RecordingStoppedAudioSystem {
        take: u32,
    },
    /// A recording was stopped because the chosen output went away.
    RecordingStoppedOutputMissing {
        output: String,
        take: u32,
    },
    /// The keyboard input stopped; meloet is starting it again.
    KeyboardInputStopped,
    /// `beats` is the whole count-in; how many are left comes from the telemetry
    /// (`countInBeatsLeft`), in step with the clicks heard.
    CountingIn {
        beats: u32,
    },
    Recording {
        track: TrackRef,
        take: u32,
    },
    /// `lap` counts from 1 ("2 周目").
    LoopRecording {
        track: TrackRef,
        from_bar: u32,
        to_bar: u32,
        lap: u32,
    },
    TakeSwitched {
        track: TrackRef,
        clip: String,
        take: u32,
        takes: u32,
    },
    /// The record button was pressed with no armed track.
    NoArmedTrack,
    UndoPending {
        count: u32,
    },
    /// A number field refused what was typed: "{field}は {range} です。もとの {kept} のままです。"
    ValueRejected {
        field: ValueField,
        kept: String,
    },
    /// The left of the status line when nothing else is to be said: "ループ 5–9" when the loop
    /// is on, "選択なし" when nothing is selected, joined by " ・ ". `notes` while notes of the
    /// piano roll are selected ("選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み…", mock
    /// A-PianoRoll); `nothing_selected` is false then.
    Summary {
        loop_bars: Option<BarSpan>,
        nothing_selected: bool,
        notes: Option<NoteSelection>,
    },
    /// The write tool on a key outside the scale while the scale is locked: "D# は C メジャー
    /// の外の音です（スケール固定 ・ オン）" (spec 6). The screen names the key without its
    /// octave.
    OutOfScale {
        key: u8,
        root: u8,
        mode: ScaleMode,
    },
    /// "なおすためのファイルから戻しました ・ まだ保存していません".
    Recovered,
    /// "戻した変更をすてて、最後に保存した形にしました。取り消せます（Mod Z）".
    RecoveryDiscarded,
    /// "なおすためのファイルが読めませんでした ・ 最後に保存した曲をひらいています", or with
    /// `saved_song` false "… ・ 新しい曲をひらいています".
    RecoveryUnreadable {
        saved_song: bool,
    },
}

/// The selected notes of the piano roll, for the status line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct NoteSelection {
    pub count: u32,
    /// The keys of the first `NOTE_SELECTION_KEYS` selected notes, in the order of the take.
    /// The mocks show at most 4 note names in the status line ("D4 E4 F4" in A-PianoRoll,
    /// "C3 E3 G3 C4" in F-PRPlay and F-PRRec); none shows more selected notes than names.
    pub keys: Vec<u8>,
    /// The take is quantized (it has an origin take), so it can go back ("もとの位置へ戻せます").
    pub quantized: bool,
}

/// How many keys `NoteSelection::keys` holds at most.
pub const NOTE_SELECTION_KEYS: usize = 4;

/// "{number} {name}" (`track.label`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TrackRef {
    pub number: u32,
    pub name: String,
}

/// Bar lines counting from 1: `{from: 5, to: 9}` is "5–9".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BarSpan {
    pub from: u32,
    pub to: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Patch {
    pub rev: u64,
    pub changes: Vec<Change>,
}

/// One whole section. `section` is the key of that section in `AppState`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "section", content = "value", rename_all = "camelCase")]
pub enum Change {
    Song(SongView),
    Transport(TransportView),
    Arrange(ArrangeView),
    PianoRoll(PianoRollView),
    Workspace(WorkspaceView),
    Keyboards(KeyboardsView),
    Outputs(OutputsView),
    Inputs(InputsView),
    Audio(AudioView),
    Status(StatusView),
}

/// Things sent the moment they arrive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Event {
    Note {
        on: bool,
        note: u8,
        velocity: u8,
        t_us: u64,
    },
    Control {
        controller: u8,
        value: u8,
        t_us: u64,
    },
    Bend {
        value: i16,
        t_us: u64,
    },
}

/// What the messages Channel carries, in arrival order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ServerMessage {
    Patch(Patch),
    Event(Event),
    LiveNotes(LiveNotes),
}

/// Notes of the recording going on, added a little at a time. `seq` goes up by one with every
/// message for the whole run of the app; the screen fetches `live_notes` when it skips one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNotes {
    pub seq: u64,
    /// `RecordingView::id`.
    pub recording: u32,
    /// New notes, and notes seen before that now have their length. The same `(lap, index)`
    /// replaces what the screen holds.
    pub notes: Vec<LiveNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNote {
    /// Pass of the loop, 0 for the first.
    pub lap: u32,
    /// Order of the key press within the lap, from 0.
    pub index: u32,
    pub key: u8,
    pub velocity: u8,
    /// Song position.
    pub start: u64,
    /// `None` while the key is down: draw it up to the play line.
    pub length: Option<u64>,
}

/// The reply of `live_notes`: every note of the recording going on, as of message `seq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LiveNotesSnapshot {
    /// The last `LiveNotes::seq` already in `notes` (0 before any).
    pub seq: u64,
    /// `None` when nothing is being recorded.
    pub recording: Option<u32>,
    pub notes: Vec<LiveNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct HelloReply {
    pub protocol_version: u32,
    pub ok: bool,
    pub mono_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct SendReply {
    pub rev: u64,
}

#[cfg(test)]
mod tests;
