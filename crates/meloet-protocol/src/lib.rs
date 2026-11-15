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
    TAKE_CONTROL_LEN, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NOTE_LEN, TAKE_VERSION, TakeControl,
    TakeControlValue, TakeError, TakeNote, TakeNotes,
};
pub use telemetry::{
    TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, Telemetry, TelemetryError,
    TransportReading,
};

pub const PROTOCOL_VERSION: u32 = 5;

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
    /// The take's tick 0 is at `start + take_offset` in the song.
    pub take_offset: u64,
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
    /// The number the first new take gets in its clip, counting from 1.
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
    /// The left of the status line when nothing else is to be said: "ループ 5–9" when the loop
    /// is on, "選択なし" when nothing is selected, joined by " ・ ".
    Summary {
        loop_bars: Option<BarSpan>,
        nothing_selected: bool,
    },
}

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
