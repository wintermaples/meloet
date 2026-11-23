//! What the AI link (the crate `meloet-ai`, its socket and its MCP tools) asks of the core,
//! and what it gets back (docs/architecture.md 24-11). Every write is one undoable step with
//! a step name the history band already has. Some are the screen's own intents
//! ([`AiAsk::Screen`]); they come through `Input::Ai` all the same.

use std::path::PathBuf;

use meloet_model::effects::EffectKind;
use meloet_model::{
    AutoPoint, AutoTarget, BuiltinInstrument, ClipEdge, Note, NoteField, PluginPlace,
    QuantizeSettings, Song, StripId, TickRange,
};
use meloet_protocol::{Command, ExportFormat, HistoryRow, LibraryView, RecordFormat};

/// Called on the core thread with the switch 「AI とつなぐ」, once when it is set and again
/// whenever the switch changes. Must not wait.
pub type AiSwitch = Box<dyn Fn(bool) + Send>;

/// Where written notes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPlace {
    /// The track of this number (from 1, as the arrange shows it): the clip that holds the
    /// earliest start, or else a new clip.
    Track(u32),
    /// This keys clip; it grows when the notes go past it.
    Clip(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiAsk {
    /// Song positions; `from` is ignored.
    WriteNotes {
        at: AiPlace,
        notes: Vec<Note>,
    },
    /// Indexes in the take, which must be the take a clip shows now.
    DeleteNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// `track` is a number from 1.
    CreateClip {
        track: u32,
        tick: u64,
    },
    RenameClip {
        clip: u32,
        name: String,
    },
    /// The tempo from `tick` (snapped to the nearest beat) on; 0 is the song's head.
    SetTempo {
        tick: u64,
        bpm: f64,
    },
    /// `tick` is snapped to the nearest beat.
    AddMarker {
        tick: u64,
        name: String,
    },
    /// A keys track below the last one.
    AddTrack {
        instrument: AiInstrument,
    },
    Undo,
    /// An intent the screen sends too, as the AI's step. Tracks are named by id, as the
    /// screen names them. Only the intents of [`screen_allowed`].
    Screen(Command),
    /// Notes of the take a clip shows now, by their indexes in it.
    EditNotes {
        take: u32,
        notes: Vec<u32>,
        edit: AiNoteEdit,
    },
    DeleteClips {
        clips: Vec<u32>,
    },
    /// The take of number `index` (from 0) of the clip.
    SwitchTake {
        clip: u32,
        index: u32,
    },
    /// `track` by id. A vocal track has no instrument.
    SetInstrument {
        track: u32,
        instrument: AiInstrument,
    },
    /// Into the slot (from 0) of a strip: an empty one, or in place of what it holds with
    /// `replace`.
    PutEffect {
        strip: StripId,
        slot: usize,
        effect: AiEffect,
        replace: bool,
    },
    /// Points (0..=1) into the lane of `target` on track `track` (id), as one step; a point
    /// at the tick of one there takes its place.
    WriteAutomation {
        track: u32,
        target: AutoTarget,
        points: Vec<AutoPoint>,
    },
    MoveAutomation {
        track: u32,
        target: AutoTarget,
        points: Vec<u32>,
        ticks: i64,
        value: f32,
    },
    DeleteAutomation {
        track: u32,
        target: AutoTarget,
        points: Vec<u32>,
    },
    /// A parameter of a plugin (`value` 0..=1). The step comes once the plugin has said its
    /// state: the answer is `pending`.
    SetPluginParam {
        place: PluginPlace,
        param: u32,
        value: f64,
    },
    /// The parameters of a plugin, read: answered with [`AiAnswer::Params`]. Not a write.
    PluginParams {
        place: PluginPlace,
    },
    /// Over the song's file. Answered with a ticket; see [`AiSong::outcomes`].
    Save,
    /// A new song folder `name` in `folder`, holding `name.meloet`. With a ticket.
    SaveAs {
        folder: PathBuf,
        name: String,
    },
    /// Not while the song has unsaved changes. With a ticket.
    Open {
        path: PathBuf,
    },
    /// Not while the song has unsaved changes.
    NewSong,
    /// An audio file into a vocal track (id; `None`: a new one) from `tick`. With a ticket.
    ImportAudio {
        path: PathBuf,
        track: Option<u32>,
        tick: u64,
    },
}

/// The screen's intents an AI may send through [`AiAsk::Screen`]: each changes the song as
/// one step (solo and the loop's switch change only how it plays), and none needs what the
/// screen shows.
pub fn screen_allowed(command: &Command) -> bool {
    use Command as C;
    matches!(
        command,
        C::SetGain { commit: true, .. }
            | C::SetPan { commit: true, .. }
            | C::SetSend { commit: true, .. }
            | C::SetParam { commit: true, .. }
            | C::ToggleMute { .. }
            | C::ToggleSolo { .. }
            | C::ToggleReturnMute { .. }
            | C::RemoveEffect { .. }
            | C::ToggleBypass { .. }
            | C::MoveInsert { .. }
            | C::SetMeter { .. }
            | C::AddMeterChange { .. }
            | C::SetMeterChange { .. }
            | C::MoveMeterChange { .. }
            | C::MoveTempoPoint { .. }
            | C::SetTempoRamp { .. }
            | C::DeleteTimeline { .. }
            | C::RenameMarker { .. }
            | C::MoveMarker { .. }
            | C::SetLoop { .. }
            | C::ToggleLoop
            | C::DeleteTrack { .. }
            | C::RenameTrack { .. }
            | C::MoveTrack { .. }
            | C::AddVocalTrack
            | C::MoveClips { copy: false, .. }
            | C::DuplicateClips { .. }
            | C::SplitClipAt { .. }
            | C::JoinClips { .. }
            | C::SetClipField { .. }
            | C::SetClipStretch { .. }
            | C::AddLane { .. }
            | C::DeleteLane { .. }
    )
}

/// Intents of [`screen_allowed`] that are no step.
pub(crate) fn screen_no_step(command: &Command) -> bool {
    matches!(command, Command::ToggleSolo { .. } | Command::ToggleLoop)
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiNoteEdit {
    Move {
        ticks: i64,
        keys: i32,
        copy: bool,
    },
    Resize {
        edge: ClipEdge,
        ticks: i64,
    },
    Velocity {
        delta: i32,
    },
    /// Each picked note gets the value.
    Set {
        field: NoteField,
        value: i64,
    },
    Quantize(QuantizeSettings),
    /// Back to where they were played.
    Unquantize,
}

/// A sound a track can have, as the AI names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiInstrument {
    Builtin(BuiltinInstrument),
    /// A scanned instrument plugin, by one of its keys.
    Plugin(String),
    Sf2 {
        file: String,
        bank: u16,
        program: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiEffect {
    Builtin(EffectKind),
    /// A scanned effect plugin, by one of its keys.
    Plugin(String),
}

impl AiAsk {
    pub fn name(&self) -> &'static str {
        match self {
            AiAsk::Screen(_) => "change the song",
            AiAsk::EditNotes { .. } => "edit notes",
            AiAsk::DeleteClips { .. } => "delete clips",
            AiAsk::SwitchTake { .. } => "switch the take",
            AiAsk::SetInstrument { .. } => "set the instrument",
            AiAsk::PutEffect { .. } => "put in the effect",
            AiAsk::WriteAutomation { .. } => "write automation",
            AiAsk::MoveAutomation { .. } => "move automation points",
            AiAsk::DeleteAutomation { .. } => "delete automation points",
            AiAsk::SetPluginParam { .. } => "set the plugin parameter",
            AiAsk::PluginParams { .. } => "read the plugin's parameters",
            AiAsk::Save => "save the song",
            AiAsk::SaveAs { .. } => "save the song as",
            AiAsk::Open { .. } => "open the song",
            AiAsk::NewSong => "start a new song",
            AiAsk::ImportAudio { .. } => "import audio",
            AiAsk::WriteNotes { .. } => "write notes",
            AiAsk::DeleteNotes { .. } => "delete notes",
            AiAsk::CreateClip { .. } => "create a clip",
            AiAsk::RenameClip { .. } => "rename a clip",
            AiAsk::SetTempo { .. } => "set the tempo",
            AiAsk::AddMarker { .. } => "add a marker",
            AiAsk::AddTrack { .. } => "add a track",
            AiAsk::Undo => "undo",
        }
    }
}

/// Why the song may not be changed now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBusy {
    CountingIn,
    Recording,
    Exporting,
}

/// The song as it is now.
#[derive(Debug, Clone, PartialEq)]
pub struct AiSong {
    pub song: Song,
    pub undo_count: u32,
    pub busy: Option<AiBusy>,
    /// 「あとから入れたもの」: the added SF2 files and the scanned plugins.
    pub library: LibraryView,
    /// The song's file, once it has one.
    pub file: Option<PathBuf>,
    /// The song does not match its file.
    pub unsaved: bool,
    /// The rate the audio runs at (or would).
    pub sample_rate: u32,
    /// How the asks answered with a ticket ended, the newest last (a few are kept).
    pub outcomes: Vec<(u64, AiOutcome)>,
}

/// How an ask answered with a ticket is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiOutcome {
    Pending,
    /// The song's file (save, open), or nothing (import).
    Done(Option<PathBuf>),
    Failed(String),
}

/// A parameter of a plugin, as the plugin lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct AiParam {
    pub id: u32,
    pub name: String,
    pub group: Option<String>,
    /// 0..=1.
    pub value: Option<f64>,
    pub default: f64,
    pub text: Option<String>,
    /// Steps of a stepped parameter; 0 for a smooth one, 2 for a switch.
    pub steps: u32,
    pub labels: Vec<String>,
    pub automatable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiParams {
    /// The plugin was asked; ask again in a moment.
    Waiting,
    Listed(Vec<AiParam>),
}

/// What a write did.
#[derive(Debug, Clone, PartialEq)]
pub struct AiDone {
    /// The row the step has in the history band; for `Undo`, the row that was undone.
    pub step: Option<HistoryRow>,
    pub undo_count: u32,
    /// The clip written, made or renamed.
    pub clip: Option<u32>,
    /// The take that clip shows afterwards.
    pub take: Option<u32>,
    /// The track added, as its number from 1.
    pub track: Option<u32>,
    /// The ask goes on: its outcome comes under this ticket in [`AiSong::outcomes`].
    pub ticket: Option<u64>,
    /// The step comes later (a plugin's parameter): `undo_count` grows then.
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiAnswer {
    Done(Box<AiDone>),
    Params(AiParams),
    Refused(AiRefusal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiRefusal {
    Busy(AiBusy),
    /// meloet is ending, or the core does not answer.
    Closing,
    NoSuchTrack(u32),
    NoSuchClip(u32),
    /// No clip shows this take now (the song changed since it was read).
    NoSuchTake(u32),
    NoSuchNote(u32),
    /// A vocal track or clip, which holds audio.
    NotKeys,
    /// It would overlap another clip of the track.
    Overlap,
    /// Past the end of the song meloet can hold.
    OffTheSong,
    BadNote,
    BadTempo,
    /// It would leave the song as it is.
    Unchanged,
    TooManyTracks,
    NothingToUndo,
    /// Another measurement is running.
    Measuring,
    /// The export failed; the text is for the log.
    ExportFailed(String),
    /// What the status line says of an intent that did nothing.
    Said(String),
    NoSuchSound,
    NoSuchEffect,
    /// No plugin at that place.
    NoSuchPlugin,
    /// The plugin is not loaded (yet), or it stopped.
    PluginNotLoaded,
    /// The parameter has an automation lane: write the lane.
    HasLane,
    NoSuchLane,
    NoSuchPoint,
    SlotTaken,
    SlotEmpty,
    /// The song has unsaved changes.
    Unsaved,
    NeverSaved,
    /// A save or an open goes on, or a dialog asks something.
    FilesBusy,
    /// The file is there and `overwrite` was not given.
    Exists(PathBuf),
    NothingToRender,
}

/// A range to render and measure, through the export's own engine.
#[derive(Debug, Clone, PartialEq)]
pub struct AiMeasure {
    pub span: TickRange,
    /// Track numbers from 1; `None`: every track. Only these tracks are in the master's mix.
    pub tracks: Option<Vec<u32>>,
}

/// What to render into files, through the export's own engine.
#[derive(Debug, Clone, PartialEq)]
pub enum AiRenderWhat {
    /// A range of the song: every track or these (numbers from 1), as one mix or a file
    /// for each track.
    Song {
        span: TickRange,
        tracks: Option<Vec<u32>>,
        per_track: bool,
        tail: bool,
    },
    /// One note through the instrument of a track (number from 1), with or without its
    /// strip (inserts, fader, sends) and the master.
    Note {
        track: u32,
        key: u8,
        velocity: u8,
        seconds: f64,
        effects: bool,
    },
    /// An audio clip as it plays (its gain, fades and stretch), with or without its strip.
    Clip { clip: u32, effects: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiRender {
    pub what: AiRenderWhat,
    /// The file; with `per_track` the folder its files go into, named as the tracks.
    pub to: PathBuf,
    pub format: ExportFormat,
    pub bits: RecordFormat,
    /// `None`: the rate the audio runs at.
    pub sample_rate: Option<u32>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiRendered {
    pub files: Vec<PathBuf>,
    pub sample_rate: u32,
    /// Of the longest file.
    pub seconds: f64,
    /// The largest sample in dBFS; `None`: silence.
    pub peak_db: Option<f32>,
}

/// The levels of one rendering, stereo, from the range's start to its end (no tail).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PassLevels {
    /// The largest sample, before the output's clip; 1.0 is 0 dBFS.
    pub peak: f32,
    /// Over both channels.
    pub rms: f32,
    /// Places over 0 dB, as 「書き出す前のたしかめ」 counts them.
    pub over: u32,
    pub frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiLevels {
    pub sample_rate: u32,
    pub master: PassLevels,
    /// Number from 1, name, levels of the track played alone (with its sends and the master).
    pub tracks: Vec<(u32, String, PassLevels)>,
}
