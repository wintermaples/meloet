//! What the AI link (the crate `meloet-ai`, its socket and its MCP tools) asks of the core,
//! and what it gets back (docs/architecture.md 24-11). Every write is one undoable step with
//! a step name the history band already has. Some are the screen's own intents
//! ([`AiAsk::Screen`]); they come through `Input::Ai` all the same.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::effects::EffectKind;
use meloet_model::{
    AutoPoint, AutoTarget, BuiltinInstrument, ClipEdge, InstrumentRef, Note, NoteField,
    OutputChange, PluginPlace, PluginRef, QuantizeSettings, Song, StripId, TickRange, TrackId,
};
use meloet_protocol::{Command, ExportFormat, HistoryRow, LibraryView, RecordFormat};

pub use crate::plugins::hosts::{OutputSet, PresetAsk, PresetInfo, PresetSaid};

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

/// A note to put in, before it is known to fit: it may start before the song or after its
/// end, and its key may be off the keyboard (a file brought in, a moved copy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiNote {
    pub start: i64,
    pub length: u64,
    pub key: i32,
    pub velocity: u8,
}

/// Which notes of a clip's shown take to copy. Only notes that sound (those that start
/// inside the clip) are copied, as long as they sound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiPick {
    All,
    /// Indexes in the take.
    Notes(Vec<u32>),
    /// Notes that start in `[from, to)`.
    Range {
        from: u64,
        to: u64,
    },
}

/// How far copied notes move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiShift {
    Ticks(i64),
    /// Whole bars, counted from the bar of the earliest copied note.
    Bars(i64),
    /// The clip's start (the range's `from` for [`AiPick::Range`]) goes to this tick.
    To(u64),
}

/// Why a note was not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiNoteWhy {
    /// It starts at or after the end of the clip made for the write.
    PastClipEnd,
    PastSongEnd,
    BeforeSongStart,
    KeyOutOfRange,
    /// It does not sound in the clip it would be copied from (the clip was trimmed).
    OutsideSourceClip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiNoteNot {
    /// The note's index in the ask (for a copy: in the take copied from).
    pub index: u32,
    pub note: AiNote,
    pub why: AiNoteWhy,
}

/// A note written shorter than asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiNoteCut {
    pub index: u32,
    pub note: AiNote,
    /// The length it got.
    pub length: u64,
    /// At the end of the clip made for the write; else at the end of the song.
    pub at_clip_end: bool,
}

/// At most this many notes are listed in [`AiNotesDone::not_written`] and in `cut`.
pub const MAX_TOLD: usize = 200;

/// What a write of notes did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiNotesDone {
    pub clip: u32,
    /// The clip's track, as its number from 1.
    pub track: u32,
    pub clip_made: bool,
    /// The clip's start and end before the write; `None` for a clip made.
    pub was: Option<(u64, u64)>,
    pub now: (u64, u64),
    pub written: u32,
    /// The first [`MAX_TOLD`]; `not_written_count` is how many there are.
    pub not_written: Vec<AiNoteNot>,
    pub not_written_count: u32,
    pub cut: Vec<AiNoteCut>,
    pub cut_count: u32,
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
    /// [`AiAsk::CreateClip`] up to `end`, which is brought on to the head of a bar.
    CreateClipTo {
        track: u32,
        tick: u64,
        end: u64,
    },
    /// Many notes as one step. Those that cannot be written are told in the answer. Into a
    /// track: the clip that holds the earliest start, or a new clip as long as the notes
    /// need. Refused when a clip of the track is in the way.
    PutNotes {
        at: AiPlace,
        notes: Vec<AiNote>,
    },
    /// Notes of the take `clip` shows into another place, as [`AiAsk::PutNotes`]. `take`:
    /// refused when the clip shows another take now.
    CopyNotes {
        clip: u32,
        take: Option<u32>,
        pick: AiPick,
        to: AiPlace,
        shift: AiShift,
        transpose: i32,
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
    /// Opens the batch `batch` (a number of the asker's own): the steps of the asks that
    /// follow in [`AiAsk::InBatch`] become one step with [`AiAsk::Commit`], or are taken
    /// back with [`AiAsk::Rollback`]. Until then every other write of the AI link is
    /// refused.
    Begin {
        batch: u64,
    },
    /// An ask of the open batch `batch`. Refused, and nothing is done, once anyone else has
    /// changed the song or its history since the batch's last ask. Not for the asks of
    /// [`batchable`].
    InBatch {
        batch: u64,
        ask: Box<AiAsk>,
    },
    /// Closes the batch. Its steps are one step now; `step` is that step's row. `step` is
    /// `None` when the batch made no step, or when its steps could not be made one (anyone
    /// else changed the song or its history since the batch's last ask).
    Commit {
        batch: u64,
    },
    /// Closes the batch and takes back what it did, the solo marks and the loop's switch
    /// too. Refused when it cannot be taken back (meloet records, or anyone else changed
    /// the song or its history since): the batch is closed and its steps stay.
    Rollback {
        batch: u64,
    },
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
    /// What each key of a track's sound is (`track` a number from 1): answered with
    /// [`AiAnswer::KeyNames`]. Not a write.
    KeyNames {
        track: u32,
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
    /// Parameters of a plugin (id, value 0..=1), all set and then made one step of the
    /// plugin's state. The answer is `pending` with a ticket for [`AiAsk::ParamsSet`].
    SetPluginParams {
        place: PluginPlace,
        values: Vec<(u32, f64)>,
    },
    /// How the set of a ticket of [`AiAsk::SetPluginParams`] (or `SetPluginParam`) ended:
    /// answered with [`AiParams::Set`], or `Waiting` until the plugin has said its state.
    /// Not a write.
    ParamsSet {
        ticket: u64,
    },
    /// The texts a plugin shows for these values of a parameter (each 0..=1), without
    /// setting anything: answered with [`AiParams::Texts`], or `Waiting`. Not a write.
    ParamTexts {
        place: PluginPlace,
        param: u32,
        values: Vec<f64>,
    },
    /// Parameters of the built-in effect in a slot (index, value in its unit), as one step.
    SetEffectParams {
        strip: StripId,
        slot: usize,
        values: Vec<(u32, f32)>,
    },
    /// The parameters of a plugin, read: answered with [`AiAnswer::Params`]. Not a write.
    PluginParams {
        place: PluginPlace,
    },
    /// The whole state of a plugin as it is now, asked of the plugin: answered with
    /// [`AiAnswer::State`]. Not a write.
    PluginState {
        place: PluginPlace,
    },
    /// Puts a whole state into a plugin. With a ticket: once the plugin has taken it, the
    /// song has it as one step. A plugin that refuses it, or whose process stops while taking
    /// it, gets the song's state back, and no step is made.
    SetPluginState {
        place: PluginPlace,
        state: Arc<[u8]>,
    },
    /// How the put of a ticket of [`AiAsk::SetPluginState`] ended: answered with
    /// [`AiState::Put`], or `Waiting` until the plugin has taken the state. Not a write.
    StatePut {
        ticket: u64,
    },
    /// About the presets of the LV2 plugin at `place`, asked of its plugin host and answered
    /// from there with [`AiAnswer::Preset`]. Not a write: a preset saved is a file, not a
    /// change of the song.
    Preset {
        place: PluginPlace,
        ask: PresetAsk,
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
    /// Keeps the mutes now, and the solo, as the mute set `name` (`meloet_model::mute_sets`);
    /// over a set of that name only with `overwrite`. The same as the set: no step, done.
    SaveMuteSet {
        name: String,
        overwrite: bool,
    },
    /// Mutes as the set says (one step) and solos as it says (no step). As the set says
    /// already: no step, done.
    RecallMuteSet {
        name: String,
    },
    DeleteMuteSet {
        name: String,
    },
    /// The set `name` gets the name `to`, in its place. The name it has: no step, done. The
    /// name of another set is refused, as for a save without `overwrite`.
    RenameMuteSet {
        name: String,
        to: String,
    },
    /// An audio file into a vocal track (id; `None`: a new one) from `tick`. With a ticket.
    ImportAudio {
        path: PathBuf,
        track: Option<u32>,
        tick: u64,
    },
    /// The level, the pan or the on/off of one output set of the plugin that is the
    /// instrument of track `track` (id) (docs/architecture.md 20-15). Any id while the plugin
    /// is not loaded; once it is, one of its sets or one the song keeps a setting of.
    SetPluginOutput {
        track: u32,
        output: String,
        change: OutputChange,
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

/// Asks that can be in a batch ([`AiAsk::InBatch`]): not those that end later (a ticket, a
/// plugin's step), not `Undo`, not a read, and no batch inside a batch.
pub fn batchable(ask: &AiAsk) -> bool {
    !matches!(
        ask,
        AiAsk::Undo
            | AiAsk::Begin { .. }
            | AiAsk::InBatch { .. }
            | AiAsk::Commit { .. }
            | AiAsk::Rollback { .. }
            | AiAsk::SetPluginParam { .. }
            | AiAsk::PluginParams { .. }
            | AiAsk::SetPluginParams { .. }
            | AiAsk::ParamsSet { .. }
            | AiAsk::ParamTexts { .. }
            | AiAsk::PluginState { .. }
            | AiAsk::SetPluginState { .. }
            | AiAsk::StatePut { .. }
            | AiAsk::Preset { .. }
            | AiAsk::KeyNames { .. }
            | AiAsk::Save
            | AiAsk::SaveAs { .. }
            | AiAsk::Open { .. }
            | AiAsk::NewSong
            | AiAsk::ImportAudio { .. }
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
            AiAsk::KeyNames { .. } => "read the names of the keys",
            AiAsk::PutEffect { .. } => "put in the effect",
            AiAsk::WriteAutomation { .. } => "write automation",
            AiAsk::MoveAutomation { .. } => "move automation points",
            AiAsk::DeleteAutomation { .. } => "delete automation points",
            AiAsk::SetPluginParam { .. } => "set the plugin parameter",
            AiAsk::SetPluginParams { .. } => "set the plugin parameters",
            AiAsk::ParamsSet { .. } => "learn how the parameters were set",
            AiAsk::ParamTexts { .. } => "read the parameter's texts",
            AiAsk::SetEffectParams { .. } => "set the effect parameters",
            AiAsk::PluginParams { .. } => "read the plugin's parameters",
            AiAsk::PluginState { .. } => "read the plugin's state",
            AiAsk::SetPluginState { .. } => "set the plugin's state",
            AiAsk::StatePut { .. } => "learn how the state was put in",
            AiAsk::Preset {
                ask: PresetAsk::Save { .. },
                ..
            } => "save the preset",
            AiAsk::Preset { .. } => "read the plugin's presets",
            AiAsk::Save => "save the song",
            AiAsk::SaveAs { .. } => "save the song as",
            AiAsk::Open { .. } => "open the song",
            AiAsk::NewSong => "start a new song",
            AiAsk::SaveMuteSet { .. } => "save the mute set",
            AiAsk::RecallMuteSet { .. } => "recall the mute set",
            AiAsk::DeleteMuteSet { .. } => "delete the mute set",
            AiAsk::RenameMuteSet { .. } => "rename the mute set",
            AiAsk::ImportAudio { .. } => "import audio",
            AiAsk::SetPluginOutput { .. } => "set the plugin's output",
            AiAsk::WriteNotes { .. } => "write notes",
            AiAsk::DeleteNotes { .. } => "delete notes",
            AiAsk::CreateClip { .. } => "create a clip",
            AiAsk::CreateClipTo { .. } => "create a clip",
            AiAsk::PutNotes { .. } => "put in the notes",
            AiAsk::CopyNotes { .. } => "copy notes",
            AiAsk::RenameClip { .. } => "rename a clip",
            AiAsk::SetTempo { .. } => "set the tempo",
            AiAsk::AddMarker { .. } => "add a marker",
            AiAsk::AddTrack { .. } => "add a track",
            AiAsk::Begin { .. } => "start the batch",
            AiAsk::InBatch { ask, .. } => ask.name(),
            AiAsk::Commit { .. } => "finish the batch",
            AiAsk::Rollback { .. } => "take back the batch",
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
    /// Song files opened or saved lately, the newest first (`crate::recent`). One may be gone
    /// since.
    pub recent: Vec<PathBuf>,
    /// Where a song's first save makes its folder, and where 「名前をつけて保存」 starts for a
    /// song with no file.
    pub songs_dir: Option<PathBuf>,
    /// The song does not match its file.
    pub unsaved: bool,
    /// The rate the audio runs at (or would).
    pub sample_rate: u32,
    /// How the asks answered with a ticket ended, the newest last (a few are kept).
    pub outcomes: Vec<(u64, AiOutcome)>,
    /// The output sets of each track's instrument plugin that is loaded, as its plugin host
    /// said them; a track whose plugin is not loaded (yet) is not here.
    pub instrument_sets: Vec<(TrackId, Arc<[OutputSet]>)>,
    /// The tracks whose instrument plugin is loaded and names its keys: kits, as the screen
    /// shows them (`midi::midi_file` writes them on channel 10).
    pub plugin_kits: Vec<TrackId>,
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
    /// It is there only to take a keyboard's knobs (MIDI CC).
    pub midi_cc: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiParams {
    /// The plugin was asked; ask again in a moment.
    Waiting,
    /// One for each value of [`AiAsk::ParamTexts`]; `None` where the plugin has no text.
    Texts(Vec<Option<String>>),
    /// How a set of [`AiAsk::SetPluginParams`] ended.
    Set(AiParamsSet),
    Listed(Vec<AiParam>),
}

/// Where the name of a key comes from, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiKeySource {
    /// meloet's own kit: the names of the drum piano roll's rows.
    Builtin,
    /// The name of the sample an SF2 zone plays there (SF2 has no names for keys).
    Sf2SampleName,
    /// The plugin's MIDNAM document (LV2 midnam).
    Lv2Midnam,
    /// The usual General MIDI drum layout, not the sound's own.
    GeneralMidiGuess,
    /// Nothing names the keys.
    None,
}

/// One key that has a name or a sound.
#[derive(Debug, Clone, PartialEq)]
pub struct AiKey {
    pub key: u8,
    pub name: Option<String>,
    /// The id of a built-in kit's drum ("kick"), as the song's model names it.
    pub id: Option<&'static str>,
    /// Whether a note on it makes a sound; `None`: not known.
    pub sounds: Option<bool>,
    pub source: AiKeySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiKeyNote {
    /// The sound pack of this PC does not have the instrument: it plays a sine wave.
    NotInstalled,
    /// Its samples are not in memory yet: silent until they are.
    NotRead,
    /// Names are SF2 sample names.
    SampleNames,
    /// The SF2 file could not be read now: the names are a guess.
    Sf2Unreadable,
    /// No added SF2 file has the preset: it plays a sine wave.
    Sf2Missing,
}

/// The keys of a track's sound. Whatever is not known yet is answered as well as it can be
/// (a General MIDI guess, keys not known to sound), so every map is a whole answer.
#[derive(Debug, Clone, PartialEq)]
pub struct AiKeyMap {
    pub sound: InstrumentRef,
    /// A drum kit, or another sound whose keys are different sounds.
    pub kit: bool,
    /// Where the names come from, as far as one of them does.
    pub source: AiKeySource,
    /// No name is a guess.
    pub certain: bool,
    /// Keys with a name or a sound, lowest first.
    pub keys: Vec<AiKey>,
    /// The lowest and the highest key that sound, when known.
    pub range: Option<(u8, u8)>,
    pub notes: Vec<AiKeyNote>,
    /// The file of an SF2 `sound`, whose zones name its keys better: read off the core thread
    /// with `crate::play::keynames::read_sf2`. Not for showing.
    pub sf2_path: Option<PathBuf>,
}

/// The whole state of a plugin, as the plugin said it.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPluginState {
    /// The plugin as the song has it at that place; its key names the format.
    pub plugin: Arc<PluginRef>,
    pub bytes: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiState {
    /// The plugin was asked; ask again in a moment.
    Waiting,
    Got(AiPluginState),
    /// How a put of [`AiAsk::SetPluginState`] ended.
    Put(AiStatePut),
}

/// How a state the AI link put into a plugin ended, with the undo count then.
#[derive(Debug, Clone, PartialEq)]
pub struct AiStatePut {
    pub end: AiPutEnd,
    pub undo_count: u32,
}

/// Whether the song holds a state the AI link put into a plugin. Only `Step` changed the
/// song; after the others the song and the plugin have the song's state.
#[derive(Debug, Clone, PartialEq)]
pub enum AiPutEnd {
    /// The plugin took it, and what it says its state is since went into the song as this
    /// step.
    Step(Option<Box<HistoryRow>>),
    /// The plugin took it and says the state the song has already (it may have made what it
    /// was given into what it had): no step.
    Same,
    /// The plugin refused it; the text is for the log.
    Refused(String),
    /// The plugin's process stopped while it took it (the state may be broken): it is loaded
    /// again with the song's state.
    Stopped,
    /// An undo or a redo put another state into the plugin first, or it was loaded again with
    /// the song's state (another sample rate).
    Overtaken,
    /// The plugin is not at its place in the song any more.
    Gone,
    /// The plugin's process could not be asked; the text is for the log.
    Failed(String),
}

/// How a set of plugin parameters the AI link asked for ended.
#[derive(Debug, Clone, PartialEq)]
pub struct AiParamsSet {
    /// In the order asked.
    pub params: Vec<AiParamNow>,
    pub end: AiSetEnd,
    pub undo_count: u32,
}

/// A parameter that was set, and what the plugin answered.
#[derive(Debug, Clone, PartialEq)]
pub struct AiParamNow {
    pub id: u32,
    pub asked: f64,
    /// The value the plugin took (0..=1), with its text; `None` when it did not answer or
    /// does not tell.
    pub value: Option<f64>,
    pub text: Option<String>,
}

/// Whether the song holds what a set of plugin parameters did.
#[derive(Debug, Clone, PartialEq)]
pub enum AiSetEnd {
    /// The plugin's new state went into the song as this step.
    Step(Option<Box<HistoryRow>>),
    /// The plugin's state is the one the song has: no step.
    Same,
    /// The plugin did not say its state (it stopped, or did not answer).
    NoState,
    /// The song's state was put back into the plugin before it said its state (undo, redo):
    /// the values are gone from it again.
    Overtaken,
    /// The plugin is not at its place in the song any more.
    Gone,
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
    /// For a write of notes: where they went and which were left out.
    pub notes: Option<Box<AiNotesDone>>,
    /// The track added, as its number from 1.
    pub track: Option<u32>,
    /// The ask goes on: its outcome comes under this ticket in [`AiSong::outcomes`]; with
    /// `pending`, [`AiAsk::ParamsSet`] tells it.
    pub ticket: Option<u64>,
    /// The step comes later (a plugin's parameter), if at all: see `ticket`.
    pub pending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiAnswer {
    Done(Box<AiDone>),
    Params(AiParams),
    /// What the keys of a track's sound are.
    KeyNames(Box<AiKeyMap>),
    State(AiState),
    Preset(PresetSaid),
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
    /// This clip of the track (its start and end) is in the way.
    OverlapsClip {
        clip: u32,
        start: u64,
        end: u64,
    },
    /// None of the notes can be written; why the first cannot.
    NoNoteFits(AiNoteWhy),
    BadTempo,
    /// The song is so already: nothing was to do. Not a failure.
    Unchanged,
    /// It cannot be done; the text says why, for the AI.
    Cannot(String),
    /// A batch is open (another call's): no other write until it is closed.
    BatchOpen,
    /// This ask cannot be a part of a batch.
    NotInBatch,
    /// The batch is not open (any more), or anyone else changed the song or its history
    /// since the batch's last ask.
    BatchLost,
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
    /// The plugin is still taking a state the AI link gave it.
    PluginBusy,
    /// The parameter has an automation lane: write the lane.
    HasLane,
    /// This parameter has an automation lane; nothing was set.
    LaneDrives(u32),
    /// The effect has no parameter of this index.
    NoSuchParam(u32),
    /// No set of parameters has this ticket (it is too old).
    NoSuchSet,
    NoSuchLane,
    NoSuchPoint,
    SlotTaken,
    SlotEmpty,
    /// The song has unsaved changes.
    Unsaved,
    NeverSaved,
    /// No mute set of that name.
    NoSuchMuteSet(String),
    /// A mute set of that name is there, and `overwrite` was not given.
    MuteSetExists(String),
    /// The song holds as many mute sets as it can.
    TooManyMuteSets,
    /// A save or an open goes on, or a dialog asks something.
    FilesBusy,
    /// The file is there and `overwrite` was not given.
    Exists(PathBuf),
    NothingToRender,
    /// The loaded plugin has no output set `asked`, and the song keeps no setting of it;
    /// these are its sets.
    NoSuchOutput {
        asked: String,
        ids: Vec<String>,
    },
    /// The range is longer than the plugin host can keep of `sets` output sets: at most
    /// `longest_s` seconds.
    CaptureTooLong {
        sets: u32,
        longest_s: f64,
    },
}

/// A range to render and measure, through the export's own engine.
#[derive(Debug, Clone, PartialEq)]
pub struct AiMeasure {
    pub span: TickRange,
    /// Track numbers from 1; `None`: every track. Only these tracks are in the master's mix.
    pub tracks: Option<Vec<u32>>,
    /// Track numbers from 1 to render alone; `None`: the tracks of the mix. Each must be in
    /// the mix.
    pub alone: Option<Vec<u32>>,
    /// A track rendered alone sounds even when the song mutes it (a lane that moves the mute
    /// is still followed). The mix keeps the song's mutes.
    pub ignore_mute: bool,
    /// Bands, overlap and sections, besides the levels.
    pub analysis: Option<AiAnalysisAsk>,
}

/// How the sound is cut into frequency bands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBandLayout {
    /// Low (below 120 Hz), low-mid (to 500 Hz), mid (to 2 kHz), high-mid (to 6 kHz), high.
    Five,
    /// Octaves centred on 31.5 Hz to 16 kHz (base 10, as IEC 61260 names them).
    Octave,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiAnalysisAsk {
    pub bands: AiBandLayout,
    /// Tracks rendered alone (numbers from 1) whose every pair is compared band by band.
    pub compare: Vec<u32>,
    /// Where sections begin, in ticks inside the range, rising; the range's start begins the
    /// first. Empty: no sections.
    pub cuts: Vec<u64>,
}

/// Each output set of the instrument plugin of a track, alone, over a range: rendered
/// through the export's own engine.
#[derive(Debug, Clone, PartialEq)]
pub struct AiOutputsMeasure {
    pub span: TickRange,
    /// The track's number from 1.
    pub track: u32,
}

/// One output set as [`AiOutputsMeasure`] measured it.
#[derive(Debug, Clone, PartialEq)]
pub struct AiOutputLevel {
    /// As the plugin of the export said it.
    pub set: OutputSet,
    /// `None`: the plugin host kept nothing of it.
    pub levels: Option<PassLevels>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiOutputLevels {
    pub sample_rate: u32,
    /// The track's number and name when the measuring began.
    pub track: (u32, String),
    /// The sets the plugin of the export said it has, in its order.
    pub sets: Vec<AiOutputLevel>,
}

/// The longest range in seconds the plugin host keeps the sound of `sets` output sets of,
/// at `rate`.
pub fn capture_longest_s(sets: usize, rate: u32) -> f64 {
    crate::plugins::hosts::capture_frames_max(sets) as f64 / f64::from(rate.max(1))
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
    /// Integrated loudness in LUFS (ITU-R BS.1770-4, gated); `None`: too short or silent.
    pub lufs: Option<f32>,
    /// The loudest 3 s (short-term loudness) in LUFS.
    pub lufs_short_max: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiLevels {
    pub sample_rate: u32,
    pub master: PassLevels,
    /// Number from 1, name, levels of the track played alone (with its sends and the master).
    pub tracks: Vec<(u32, String, PassLevels)>,
    /// The numbers of those tracks that the song mutes.
    pub muted: Vec<u32>,
    pub analysis: Option<Box<AiAnalysis>>,
}

/// A frequency band, from `low_hz` up to `high_hz` (at most half the sample rate).
#[derive(Debug, Clone, PartialEq)]
pub struct AiBand {
    pub name: &'static str,
    pub low_hz: f64,
    pub high_hz: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AiBandLevel {
    /// Mean square of the band over the range, both channels averaged, as `rms` squared.
    pub power: f64,
    /// Of the rendering's whole power, 0..=1.
    pub share: f64,
}

/// How much two tracks sound in one band at the same time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AiShared {
    /// Mean over the range of the smaller of the two band powers of each short window.
    pub power: f64,
    /// That, over the band power of the louder track: 0 never together, 1 the same all along.
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiOverlap {
    /// Indexes into [`AiLevels::tracks`].
    pub a: usize,
    pub b: usize,
    /// In the order of [`AiAnalysis::bands`].
    pub bands: Vec<AiShared>,
}

/// The levels of a part of a rendering.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AiLoudness {
    pub peak: f32,
    pub rms: f32,
    pub lufs: Option<f32>,
    pub lufs_short_max: Option<f32>,
    pub frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiSection {
    /// Ticks.
    pub start: u64,
    pub end: u64,
    /// The mix, then each track alone, in the order of [`AiLevels::tracks`].
    pub passes: Vec<AiLoudness>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiAnalysis {
    /// Empty when no bands were asked for.
    pub bands: Vec<AiBand>,
    /// The length of one window of the analysis, in seconds; windows overlap by half.
    pub window_s: f64,
    /// The mix, then each track alone: the level of each band.
    pub levels: Vec<Vec<AiBandLevel>>,
    pub overlaps: Vec<AiOverlap>,
    pub sections: Vec<AiSection>,
}
