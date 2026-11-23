//! The pure rules: an input comes in, the state changes, and a list of things to do comes back.
//! Nothing here touches a device, a file or a thread.

mod audio;
mod buttons;
mod export;
mod pack;
mod recovery;
mod songs;
mod vocal;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use meloet_devices::{DevicesPicture, Endpoint, sort_for_choice};
use meloet_midi::Keyboard;
use meloet_model::drums::gm_kit_rows;
use meloet_model::mixer::Missing;
use meloet_model::{
    AudioSource, BuiltinInstrument, GmFamily, InstrumentRef, Note, PluginPlace, Song, StripId,
    TrackKind, VocalInput,
};
use meloet_plugin::ScanCache;
use meloet_plugin::wire::{ParamInfo, ParamNow, TextValue};
use meloet_protocol::{
    AppState, ArrangeView, AudioView, Availability, Band, ChooserView, ChosenState, Command,
    EffectRef, EndpointView, FileDropView, Friendly, Hint, HistoryRow, InstrumentKind,
    KeyboardView, KeyboardsView, LayoutView, LibraryView, LinkKind, LiveNotes, LiveNotesSnapshot,
    LowerBandView, MeterSize, MissingRef, MissingView, OutputsView, ParamTable, ParamValues,
    PluginRole, PluginWindowKind, QuantizeSettings, RecordFormat, RecoveryCause, ScanView,
    SettingsDialogView, ShelfMode, ShelfView, SongFile, SongView, SoundRef, StandIn, StatusMessage,
    StatusView, TrackKindView, TrackView, Unusable, VelocityFixed, WorkspaceView,
};

use crate::added;
use crate::audiofiles;
use crate::audiofiles::keeper::AudioNews;
use crate::catalog::Catalog;
use crate::instruments;
use crate::sf2store::{self, Sf2Info, Sf2Key};

use crate::layout::{self, Bands, BottomBand, FoldWhy, SidePanel, WindowSize};
use crate::play::mixer::gain_view;
use crate::play::{
    ClipGains, Cut, EngineOp, Heard, OpenInput, Play, PlugState, RecordReport, VocalFileAsk,
};
use crate::plugins::{Plugins, ScanJob, ScanNews};
use crate::settings::{SCALES, Settings, SoundSettings};
use crate::vocalrec::{RecNote, Written};

pub use pack::{PackAsk, PackStart};
pub use recovery::{QUIET_US, RecoveryStart};

/// The buffer size meloet asks for. What really runs is `AudioView::actual_frames`.
pub const REQUESTED_FRAMES: u32 = crate::settings::DEFAULT_BUFFER;
/// What 「バッファ ▾」 and 「サンプルレート ▾」 of the advanced sound settings list.
pub use crate::settings::{BUFFER_CHOICES, SAMPLE_RATE_CHOICES};
pub use vocal::Delays;
pub const AUDIO_SYSTEM: &str = "PipeWire";
/// Key to sound is about this many buffers (docs/architecture.md 4).
const LATENCY_BUFFERS: f32 = 2.5;
/// Measured key-to-sound times that make up the shown average.
const LATENCY_SAMPLES: usize = 16;
const START_VIEWPORT: (f32, f32) = (1280.0, 800.0);
/// How many times a stream that ended on its own is opened again before meloet waits for the
/// outputs to change or for the user.
const AUDIO_RETRIES: u32 = 4;

/// The wait before try number `attempt` (1, 2, …) to connect again: 0.5 s, 1 s, 2 s, 4 s, then
/// every 5 s. Used for the stream, PipeWire and the keyboard input alike.
pub fn retry_delay(attempt: u32) -> Duration {
    let ms = 500u64.saturating_mul(1 << attempt.saturating_sub(1).min(4));
    Duration::from_millis(ms.min(5_000))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Command(Command),
    KeyboardAdded(Keyboard),
    KeyboardRemoved {
        id: String,
    },
    /// A key went down on a keyboard other than the one played before.
    KeyboardPlayed {
        id: String,
    },
    /// A button pressed on a keyboard's button port.
    KeyboardButton(meloet_midi::Button),
    /// A keyboard's fader (CC 7) moved to `value`, 0..=127.
    KeyboardFader {
        value: u8,
    },
    Devices(DevicesPicture),
    /// The answer to [`Effect::OpenOutput`].
    AudioOpened {
        node_name: String,
        sample_rate: u32,
    },
    /// The other answer to [`Effect::OpenOutput`].
    AudioOpenFailed {
        node_name: String,
    },
    /// Frames per block of the open stream; `None` until its first block.
    AudioFrames(Option<u32>),
    /// One measured key-to-DAC time of the open stream.
    LatencySample {
        ms: f32,
    },
    /// Played sound went over -60 dBFS.
    SoundHeard,
    /// From the shell: what the screen the window is on can give to the inside of the window,
    /// in logical px (the work area minus the window frame).
    ScreenSpace {
        width: f32,
        height: f32,
    },
    /// A song was opened. The new song at start counts too; [`Model::new`] does it itself.
    SongOpened,
    /// The open stream ended on its own (the output went away, PipeWire broke the connection).
    AudioLost,
    /// Xruns the open stream had since it was opened.
    AudioXruns {
        count: u64,
    },
    /// The time from [`Effect::RetryAudio`] has passed.
    AudioRetry,
    /// The connection to PipeWire broke. The next [`Input::Devices`] means it is back.
    AudioSystemLost,
    /// The keyboard input stopped by itself. Every held key and pedal counts as released.
    MidiStopped,
    /// The keyboard input runs (again); every keyboard there has been reported since.
    MidiBack,
    /// What the engine says about a recording.
    Record(RecordReport),
    /// A key went down while [`Model::wants_loading_key`].
    KeyWhileLoading,
    /// Nothing but the time and the heard position given with [`Model::set_now`].
    Tick,
    /// From the shell: the window's close button, or anything else that asks to end.
    CloseRequested,
    /// The answer to [`Effect::PickSongFile`]; `None` when it was put away.
    SongFilePicked(Option<PathBuf>),
    /// The answer to [`Effect::PickFolder`]; `None` when it was put away.
    FolderPicked(Option<PathBuf>),
    /// The answer to [`Effect::PickSaveAs`]: the folder and name typed, as one path. `None`
    /// when it was put away.
    SaveAsPicked(Option<PathBuf>),
    /// An answer to [`Effect::SaveSong`]: the song as it is now is in `path`, named `name`.
    SongSaved {
        path: PathBuf,
        name: String,
    },
    /// The other answer to [`Effect::SaveSong`].
    SongNotSaved,
    /// An answer to [`Effect::ReadSong`]. `path` is the song's file, also when the song came
    /// from the copy kept beside it.
    SongRead {
        path: PathBuf,
        song: Box<Song>,
        from_previous: bool,
    },
    /// The other answer to [`Effect::ReadSong`]. `previous`: the copy kept beside it reads.
    SongUnreadable {
        path: PathBuf,
        previous: bool,
    },
    /// At the start: the recovery folder had a file.
    RecoveryFound(Box<RecoveryStart>),
    /// The PC is ending (a signal, logind). Answered with [`Effect::WriteRecovery`] with
    /// `ending`, or [`Effect::EndingReady`] when there is nothing to write.
    PcEnding,
    /// An answer to [`Effect::LoadSound`]: the samples of the instrument are in memory.
    SoundLoaded(BuiltinInstrument),
    /// The other answer to [`Effect::LoadSound`]: they do not read.
    SoundUnreadable(BuiltinInstrument),
    /// There is a scanner. `cache`: what the scan file held. `first`: there was no scan file,
    /// so the scan that starts now shows its count.
    PluginsStart {
        cache: Box<ScanCache>,
        first: bool,
    },
    /// What the scanner says about a scan asked for with [`Effect::ScanPlugins`].
    PluginScan(ScanNews),
    /// The answer to [`Effect::PickPluginPlace`]; `None` when the window was put away.
    PluginPlacePicked(Option<PathBuf>),
    /// An answer to [`Effect::LoadPlugin`] and to [`Effect::ReloadPlugin`]: the plugin is
    /// ready to be put into the engine (`EngineOp::Plug`).
    PluginLoaded {
        ticket: u64,
    },
    /// The other answer to them: it did not load.
    PluginUnreadable {
        ticket: u64,
    },
    /// The process of the plugin loaded under `ticket` answers no more. A recording goes on.
    PluginStopped {
        ticket: u64,
    },
    /// The engine was made anew, without the plugins the old one held.
    PluginsLost,
    /// From the plugins' thread: something the plugin loaded under `ticket` did by itself,
    /// or its answer to an [`Effect::AskPlugin`].
    PluginSaid {
        ticket: u64,
        said: PluginSaid,
    },
    /// An answer to [`Effect::LoadSf2`]: the samples of the preset are in memory.
    Sf2Loaded(Sf2Key),
    /// The other answer to [`Effect::LoadSf2`]: they do not read.
    Sf2Unreadable(Sf2Key),
    /// The answer to [`Effect::PickSf2`]; `None` when the window was put away.
    Sf2Picked(Option<PathBuf>),
    /// An answer to [`Effect::AddSf2`]: the file is in meloet's folder, and this is what it
    /// holds.
    Sf2Added(Box<Sf2Info>),
    /// The other answer to [`Effect::AddSf2`]: the file of this name does not read.
    Sf2NotAdded {
        file: String,
    },
    /// From the shell: files are held over the window.
    FilesOver(Vec<PathBuf>),
    /// From the shell: they were taken out of the window again, or the drag was given up.
    FilesLeft,
    /// From the shell: they were let go of over the window.
    FilesDropped(Vec<PathBuf>),
    /// From the shell: where the pointer is while files are held over the window, in px of
    /// the page. Comes after `FilesOver`, with every move, and before `FilesDropped`.
    FilesAt {
        x: i32,
        y: i32,
    },
    /// An answer to [`Effect::OpenInput`]: the input runs, with this many channels, at the
    /// output's rate.
    InputOpened {
        node_name: String,
        channels: u16,
        sample_rate: u32,
    },
    /// The other answer to [`Effect::OpenInput`].
    InputOpenFailed {
        node_name: String,
    },
    /// The open input ended on its own (the microphone was pulled out).
    InputLost,
    /// Frames of vocal clips were not read from the disk in time, and were silence.
    DiskTooSlow,
    /// How far a save that puts audio files into the song's folder is: `done` of `total`
    /// files, `done_bytes` of `total_bytes`. At most 4 times a second, while it goes on.
    SavingAudio {
        done: u32,
        total: u32,
        done_bytes: u64,
        total_bytes: u64,
    },
    /// What the PC says of its delays now; `None` while the input is not open.
    AudioDelays(Option<Delays>),
    /// The input of a metered vocal track reached 0 dB.
    MicClipped,
    /// An answer to [`Effect::StartVocalRecording`]: the files are there, under these names
    /// (in the order asked; another name where one was taken).
    VocalsStarted {
        names: Vec<String>,
    },
    /// The other answer to [`Effect::StartVocalRecording`]: no file could be made.
    VocalsNotStarted,
    /// The answer to [`Effect::StopVocalRecording`]: what the disk writer wrote.
    VocalsWritten(Box<Written>),
    /// While vocals are recorded: how many times the input skipped, and how many blocks
    /// the ring to the disk had no room for, so far.
    VocalFaults {
        gaps: u32,
        dropped: u32,
    },
    /// The files reached the size a wav file can have, or the disk refused: the recording
    /// is to stop.
    VocalsCannotGoOn,
    /// The answer to [`Effect::LatencyClick`]: what the round trip of the click took more
    /// than the PC says, in microseconds; `None`: it did not come back.
    LatencyAnswer {
        x_us: Option<f64>,
    },
    /// The answer to [`Effect::PickAudio`]; empty when the window was put away.
    AudioPicked(Vec<PathBuf>),
    /// From the thread that reads audio files in: the answers to [`Effect::BringAudio`],
    /// [`Effect::WatchAudio`] and [`Effect::FindAudio`].
    Audio(AudioNews),
    /// From the thread that makes the audio of stretched and pitched clips: every clip
    /// whose audio is being made or waits to be, for 「音を作っています」.
    AudioMaking(Vec<crate::maker::MakerJob>),
    /// From the export's worker and from the shell: the answers to [`Effect::Export`].
    Export(crate::export::ExportNews),
    /// From the thread that fetches the sound pack: the answers to [`Effect::Pack`].
    Pack(crate::sounds::fetch::PackNews),
    /// The page subscribed and is up.
    ScreenUp,
}

/// What a loaded plugin tells: its window, its parameters, its state.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginSaid {
    /// What window it has, as the plugin itself says once it is loaded. Comes before its
    /// [`Input::PluginLoaded`].
    Editor(PluginWindowKind),
    /// The window asked for with [`PluginAsk::OpenWindow`] is on the screen.
    WindowOpened {
        width: u32,
        height: u32,
        larger_than_screen: bool,
    },
    /// It could not be made, or the plugin did not hear the ask.
    WindowFailed,
    WindowResized {
        width: u32,
        height: u32,
        larger_than_screen: bool,
    },
    /// The person closed the window with its own button, or the plugin did.
    WindowClosed,
    /// The keyboard came to the window: the person is in the plugin.
    WindowEntered,
    WindowLeft,
    /// A parameter has this value now: moved in the plugin's window, or the answer to
    /// [`PluginAsk::SetParam`].
    Param(ParamNow),
    /// Any of its parameters may have another value now.
    ParamsChanged,
    /// The answer to [`PluginAsk::Params`].
    Params {
        ask: u64,
        params: Vec<ParamInfo>,
    },
    /// The answer to [`PluginAsk::State`]. `None`: it could not say.
    State {
        ask: StateAsk,
        state: Option<Arc<[u8]>>,
    },
    /// The answer to [`PluginAsk::ReadText`]. `None`: the text does not read as a value.
    TextRead {
        param: u32,
        read: Option<TextValue>,
    },
    /// The person took hold of a parameter in the plugin's window (`on`), or let go of it
    /// (VST3 `beginEdit` / `endEdit`, LV2 `ui:touch`, else 0.5 s after the last change).
    /// While held the plugin passes over the automation of it by itself. `t_us`: when the
    /// plugin said it, CLOCK_MONOTONIC in microseconds (docs/architecture.md 21-9, 21-10).
    ParamTouch {
        param: u32,
        on: bool,
        t_us: u64,
    },
    /// What a [`PluginSaid::Param`] moved in the window was, for writing it into a lane.
    /// Comes right after that `Param`. `heard`: what the listener heard when the plugin said
    /// it (its `t_us` on the engine's clock), as for a key (docs/architecture.md 21-9).
    ParamMoved {
        param: u32,
        value: f64,
        heard: Option<crate::play::Heard>,
    },
    /// How often the person took hold of a parameter or loaded a program while in the
    /// window (every `ParamTouch` with `on` counts). Comes right before its `WindowLeft`.
    WindowTouches {
        touches: u32,
    },
}

/// What is asked of a loaded plugin. Asks of one plugin are answered in the order they
/// were made.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginAsk {
    /// Open its window, or bring it to the front. [`PluginSaid::WindowOpened`] or
    /// [`PluginSaid::WindowFailed`] follows a window that was not there.
    OpenWindow { title: String },
    /// Nothing follows.
    CloseWindow,
    /// List its parameters. Answer with [`PluginSaid::Params`] and the same `ask`.
    Params { ask: u64 },
    /// `value` is 0..=1. Answer with [`PluginSaid::Param`]: the value as the plugin took it.
    SetParam { param: u32, value: f64 },
    /// Its whole state. Answer with [`PluginSaid::State`].
    State(StateAsk),
    /// What value the typed text stands for. Answer with [`PluginSaid::TextRead`].
    ReadText { param: u32, text: String },
    /// The parameters panel shows the plugin now (`on`), or no longer does. While it does,
    /// a [`PluginSaid::Param`] comes for what the lanes do to its parameters
    /// (docs/architecture.md 21-8).
    Watch { on: bool },
}

/// What a state is asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateAsk {
    /// Before anything is changed in a plugin the song has "as it starts": what undo puts
    /// back.
    Base,
    /// As the person comes into the plugin's window; the number of the visit.
    Entered(u64),
    /// As the visit ends.
    Left(u64),
    /// After this parameter was set from the parameters panel, for the step. `puts`: how
    /// many states had been put back into the plugin by then; an answer that comes after
    /// another one was put back is of no use.
    Param { param: u32, puts: u32 },
}

/// Where [`Effect::SaveSong`] writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveTo {
    /// Over the song's file.
    File(PathBuf),
    /// A new song folder in this folder; `None`: in the songs folder.
    NewFolder(Option<PathBuf>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Open the stream on this node (closing the one that is open). Answer with
    /// [`Input::AudioOpened`] or [`Input::AudioOpenFailed`].
    OpenOutput {
        node_name: String,
    },
    CloseOutput,
    /// Play the test tone on the open stream.
    TestTone,
    /// Release every sounding note and forget the held keys.
    AllNotesOff,
    SaveSettings(Settings),
    /// Answer with [`Input::AudioRetry`] after this long.
    RetryAudio {
        after: Duration,
    },
    /// Only while a stream is open; [`Input::AudioOpened`] sends everything again. The ops of
    /// the instruments, the play target and the mixer are for the engine at any time.
    Engine(EngineOp),
    LiveNotes(LiveNotes),
    /// Answer with [`Input::SongSaved`] or [`Input::SongNotSaved`].
    SaveSong {
        song: Box<Song>,
        to: SaveTo,
    },
    /// The PC's open window. Answer with [`Input::SongFilePicked`].
    PickSongFile,
    /// The PC's folder window. Answer with [`Input::FolderPicked`].
    PickFolder,
    /// The PC's save window, with `name` filled in. Answer with [`Input::SaveAsPicked`].
    PickSaveAs {
        name: String,
    },
    /// Read the song at `path`, or with `previous` the copy kept beside it. Answer with
    /// [`Input::SongRead`] or [`Input::SongUnreadable`].
    ReadSong {
        path: PathBuf,
        previous: bool,
    },
    /// meloet may end now.
    Exit,
    /// Write the recovery file. `ending`: the PC is ending and waits for it.
    WriteRecovery {
        song: Box<Song>,
        path: Option<PathBuf>,
        cause: RecoveryCause,
        ending: bool,
    },
    RemoveRecovery,
    /// The PC is ending and there is nothing to write.
    EndingReady,
    /// Show the file in the PC's file manager.
    ShowFile(PathBuf),
    /// Read the samples of the instrument into memory, off the core thread. Answer with
    /// [`Input::SoundLoaded`] or [`Input::SoundUnreadable`].
    LoadSound(BuiltinInstrument),
    /// Let go of the samples of the instrument, read or still to be read. Instruments that
    /// sound with them keep them until they are gone themselves.
    DropSound(BuiltinInstrument),
    /// Run this scan off the core thread. Answer with [`Input::PluginScan`], ending with
    /// `ScanNews::Done`.
    ScanPlugins(Box<ScanJob>),
    /// Write the scan file, off the core thread.
    SavePluginScan(Box<ScanCache>),
    /// The PC's folder window for a place to look for plugins in. Answer with
    /// [`Input::PluginPlacePicked`].
    PickPluginPlace,
    /// Load the plugin of `key` from `bundle` into a process, off the core thread, with
    /// `state`, for a stream of `sample_rate`. Answer with [`Input::PluginLoaded`] or
    /// [`Input::PluginUnreadable`]. The loaded plugin waits for `EngineOp::Plug` or
    /// [`Effect::DropPlugin`].
    LoadPlugin {
        ticket: u64,
        key: Arc<str>,
        bundle: PathBuf,
        state: PlugState,
        sample_rate: u32,
    },
    /// The plugin loaded under `ticket` is not put into the engine after all: let go of it.
    DropPlugin {
        ticket: u64,
    },
    /// Give the plugin loaded under `ticket` this state.
    SetPluginState {
        ticket: u64,
        state: Arc<[u8]>,
    },
    /// Start a new process for the stopped plugin loaded under `ticket`, with the state
    /// remembered for it. Answer as for [`Effect::LoadPlugin`], with the same ticket.
    ReloadPlugin {
        ticket: u64,
    },
    /// Ask the plugin loaded under `ticket`, off the core thread. Answers come as
    /// [`Input::PluginSaid`].
    AskPlugin {
        ticket: u64,
        ask: PluginAsk,
    },
    /// Values of the table the parameters panel shows, for the messages Channel.
    ParamValues(ParamValues),
    /// Read the samples of the preset into memory, off the core thread. Answer with
    /// [`Input::Sf2Loaded`] or [`Input::Sf2Unreadable`].
    LoadSf2(Sf2Key),
    /// Let go of the samples of the preset, as [`Effect::DropSound`].
    DropSf2(Sf2Key),
    /// The PC's open window for an SF2 file. Answer with [`Input::Sf2Picked`].
    PickSf2,
    /// Where files with audio among them are held or were let go of
    /// (`Event::FileDrag`, `Event::FileDrop`), for the messages Channel.
    FileEvent(meloet_protocol::Event),
    /// Read the SF2 file and copy it into meloet's folder, off the core thread. Answer with
    /// [`Input::Sf2Added`] or [`Input::Sf2NotAdded`].
    AddSf2(PathBuf),
    /// Open the PC's input on this node, at the rate of the open output (closing the one
    /// that is open). `sink_monitor`: the node is a sink, and its monitor is the input.
    /// Answer with [`Input::InputOpened`] or [`Input::InputOpenFailed`].
    OpenInput {
        node_name: String,
        sink_monitor: bool,
    },
    CloseInput,
    /// Make the files of a recording in the work folder and start writing the input into
    /// them, before the engine is told to record. Answer with [`Input::VocalsStarted`] or
    /// [`Input::VocalsNotStarted`]. `x_us`: what the round trip takes more than the PC says.
    StartVocalRecording {
        files: Vec<VocalFileAsk>,
        format: RecordFormat,
        x_us: i64,
    },
    /// Write what is still on its way and close the files. Answer with
    /// [`Input::VocalsWritten`].
    StopVocalRecording,
    /// Write the note about the recording going on (`work/rec.json`).
    WriteRecNote(Box<RecNote>),
    RemoveRecNote,
    /// Files of a recording that holds nothing to keep: take them out of the work folder.
    DropVocalFiles(Vec<String>),
    /// Start or stop keeping what the input hears, for the latency measurement.
    LatencyListen(bool),
    /// Play one click and listen for it. Answer with [`Input::LatencyAnswer`].
    LatencyClick,
    /// The PC's open window for audio files (`many`: more than one may be chosen). Answer
    /// with [`Input::AudioPicked`].
    PickAudio {
        many: bool,
    },
    /// Copy the files into the work place and find out what they are, off the core
    /// thread. `taken`: names a new file must not get. Answer with `AudioNews::Brought`.
    BringAudio {
        ticket: u64,
        paths: Vec<PathBuf>,
        taken: Vec<String>,
    },
    /// The audio table of the song with this serial, and the rate of the open stream: say
    /// which files are there (`AudioNews::States`), and make ready what is not.
    WatchAudio {
        serial: u32,
        sources: Vec<Arc<AudioSource>>,
        sample_rate: Option<u32>,
    },
    /// 「さがす…」: take the file at `path` for the missing `source`. Answer with
    /// `AudioNews::Found`.
    FindAudio {
        source: Arc<AudioSource>,
        path: PathBuf,
    },
    /// The song is let go of: empty the work place of audio (docs/architecture.md 22-4).
    ClearWorkAudio,
    /// For the export's worker and its window for a place. Answer with [`Input::Export`].
    Export(crate::export::ExportAsk),
    /// For the thread that fetches the sound pack. Answers come as [`Input::Pack`].
    Pack(PackAsk),
}

pub struct Model {
    play: Play,
    /// A song was opened and nothing has been done since: the status line says what it is.
    new_song_unsaid: bool,
    /// The empty arrange hint was put away.
    empty_dismissed: bool,
    scale: u16,
    viewport: (f32, f32),
    /// `None` until the shell tells.
    screen: Option<(f32, f32)>,
    bands: Bands,
    /// Something that just happened, for the status line. Gone at the next thing the user does.
    notice: Option<StatusMessage>,
    /// `StatusMessage::DiskTooSlow` was said since the song started to move.
    disk_slow_said: bool,
    /// `StatusMessage::SavingAudio`, while the save that said it goes on.
    saving_audio: Option<StatusMessage>,
    /// A key was played on the track with this instrument while it was being read: the
    /// status line says so until it is read or the keys play something else.
    loading_said: Option<InstrumentRef>,
    dialog: Option<SettingsDialogView>,
    highlighted: Option<String>,
    hint: Option<Hint>,
    /// The chooser decided an output; the next played sound shows the hint.
    hint_armed: bool,
    hint_dismissed: bool,
    /// Oldest first.
    keyboards: Vec<Keyboard>,
    /// The id of the keyboard played last, while it is plugged in.
    played: Option<String>,
    lost: Option<String>,
    /// `None` until the first picture. Outputs are kept in chooser order.
    devices: Option<DevicesPicture>,
    chosen: Option<String>,
    chosen_name: Option<String>,
    /// The chosen output was ready at some point of this run: not ready is then `Missing`,
    /// before that `NotPlugged`.
    chosen_was_ready: bool,
    /// The output a test tone opened, while the chooser or the settings dialog is up.
    preview: Option<String>,
    /// The node the last `OpenOutput` asked for and no `CloseOutput` took back.
    wanted: Option<String>,
    /// A node that failed to open. Not tried again until the devices change or the user asks.
    failed: Option<String>,
    open: Option<OpenStream>,
    actual_frames: Option<u32>,
    latencies: VecDeque<f32>,
    xruns: u64,
    /// Opens in a row of the stream that ended on its own; 0 once a stream runs.
    audio_retries: u32,
    /// The stream ended on its own and every try to open it again failed. Until the outputs
    /// change or a stream runs, the status line says the output went away.
    audio_gave_up: bool,
    /// PipeWire cannot be reached.
    audio_system_down: bool,
    /// While the keyboard input starts again: the ids it has reported so far. `None` while the
    /// input runs.
    midi_restarting: Option<Vec<String>>,
    /// A song was opened and the running buffer has not been looked at since.
    buffer_unsaid: bool,
    /// The fixed buffer size as last known; `None` when it was not fixed. Kept while the stream
    /// is reopened, so the same size on the next stream is not said again.
    buffer_fixed_seen: Option<u32>,
    files: songs::Files,
    recovery: recovery::Recovery,
    /// The time given with `set_now`, µs.
    now_us: u64,
    /// A meloet setting, not part of the song.
    velocity_fixed: VelocityFixed,
    fader: buttons::Fader,
    /// The starred instruments of the shelf. A meloet setting too.
    favorites: Vec<BuiltinInstrument>,
    plugins: Plugins,
    catalog: Catalog,
    /// `Plugins::rows_rev` when the catalog last took in what the scan knows.
    rows_seen: u64,
    /// The files held over the window.
    file_drop: Option<FileDropView>,
    /// Esc while files were held over the window: letting go of them adds nothing.
    drop_cancelled: bool,
    /// Where the pointer was last while files were held over the window.
    drop_at: (i32, i32),
    /// Counts the drops with audio files; `Event::FileDrop::drop`.
    drop_serial: u32,
    /// Audio files that were let go of over the window and wait for the screen to say where
    /// that is in the song (`Command::DropAudio`).
    pending_drop: Option<PendingDrop>,
    /// `ArrangeView::tall_track`, with the serial of the song the track is in.
    tall_track: Option<(u32, u32)>,
    /// Several files were let go of over the window and their SF2 files are being added.
    sf2_batch: Option<Sf2Batch>,
    /// The advanced sound settings.
    sound: SoundSettings,
    input: vocal::InputSide,
    delays: Option<Delays>,
    latency: Option<vocal::LatencyDialog>,
    /// 「モニター」 was turned on once in this run: the status line said how late the voice is.
    monitor_said: bool,
    audio: audio::AudioSide,
    export: crate::export::Exporting,
    pack: pack::PackSide,
}

/// Audio files let go of over the window (`Model::pending_drop`).
#[derive(Debug)]
struct PendingDrop {
    id: u32,
    /// As they were held.
    paths: Vec<PathBuf>,
}

/// The SF2 files of one drop of several files, said as one once the last is answered.
#[derive(Debug, Default)]
struct Sf2Batch {
    /// Answers still to come.
    waiting: u32,
    /// The files added, in the order they were held, with what `Sf2Added` says of each.
    added: Vec<(String, u32, u32)>,
    /// The files not added: no SF2 file, or one that did not read.
    skipped: u32,
    /// The first file that did not read.
    unreadable: Option<String>,
}

struct OpenStream {
    node_name: String,
    sample_rate: u32,
}

impl Model {
    #[cfg(test)]
    pub(crate) fn play(&self) -> &Play {
        &self.play
    }

    #[cfg(test)]
    pub(crate) fn play_mut(&mut self) -> &mut Play {
        &mut self.play
    }

    #[cfg(feature = "dev-hooks")]
    pub(crate) fn memory_counts(&self) -> serde_json::Value {
        self.play.memory_counts()
    }

    pub fn new(settings: Settings) -> Model {
        let scale = if SCALES.contains(&settings.scale) {
            settings.scale
        } else {
            crate::settings::DEFAULT_SCALE
        };
        Model {
            play: Play::new(settings.metronome, settings.count_in_bars),
            new_song_unsaid: true,
            empty_dismissed: false,
            scale,
            viewport: START_VIEWPORT,
            screen: None,
            bands: Bands::new(window(START_VIEWPORT, scale)),
            notice: None,
            disk_slow_said: false,
            saving_audio: None,
            loading_said: None,
            dialog: None,
            highlighted: None,
            hint: None,
            hint_armed: false,
            hint_dismissed: false,
            keyboards: Vec::new(),
            played: None,
            lost: None,
            devices: None,
            chosen_name: settings.output_name.filter(|_| settings.output.is_some()),
            chosen: settings.output,
            chosen_was_ready: false,
            preview: None,
            wanted: None,
            failed: None,
            open: None,
            actual_frames: None,
            latencies: VecDeque::with_capacity(LATENCY_SAMPLES),
            buffer_unsaid: true,
            buffer_fixed_seen: None,
            xruns: 0,
            audio_retries: 0,
            audio_gave_up: false,
            audio_system_down: false,
            midi_restarting: None,
            files: songs::Files::default(),
            recovery: recovery::Recovery::default(),
            now_us: 0,
            velocity_fixed: VelocityFixed {
                on: settings.velocity_fixed,
                value: settings.fixed_velocity.clamp(1, 127),
            },
            fader: buttons::Fader::default(),
            favorites: settings.favorites,
            plugins: Plugins::new(|name| std::env::var_os(name), settings.plugin_places),
            catalog: Catalog::new(settings.shelf),
            rows_seen: 0,
            file_drop: None,
            drop_cancelled: false,
            drop_at: (0, 0),
            drop_serial: 0,
            pending_drop: None,
            tall_track: None,
            sf2_batch: None,
            sound: settings.sound,
            input: vocal::InputSide::default(),
            delays: None,
            latency: None,
            monitor_said: false,
            audio: audio::AudioSide::default(),
            export: crate::export::Exporting::default(),
            pack: pack::PackSide::new(settings.pack_fetch),
        }
    }

    /// `files`: the SF2 files that were added before, as meloet's folder has them.
    pub fn with_sf2(mut self, files: Vec<Sf2Info>) -> Model {
        self.play
            .set_sf2_presets(files.iter().flat_map(sf2_keys).collect());
        self.catalog.set_sf2(files);
        self
    }

    /// `installed`: the sampled instruments the sound pack of this PC has.
    pub fn with_sounds(mut self, installed: Vec<BuiltinInstrument>) -> Model {
        self.play.set_installed_sounds(installed);
        self
    }

    /// `usual`: the places that are always looked in for plugins, in place of the ones the
    /// environment gives.
    pub fn with_plugin_places(mut self, usual: Vec<PathBuf>) -> Model {
        self.plugins.set_usual(usual);
        self
    }

    /// Whether [`Input::SoundHeard`] would change anything now.
    pub fn wants_sound(&self) -> bool {
        self.hint_armed && !self.hint_dismissed
    }

    /// Whether [`Input::KeyWhileLoading`] would change anything now: a key played now is
    /// silent, and the status line does not say why yet.
    pub fn wants_loading_key(&self) -> bool {
        let loading = self.play.target_loading();
        loading.is_some() && loading != self.loading_said
    }

    pub fn handle(&mut self, input: Input) -> Vec<Effect> {
        let mut effects = Vec::new();
        let mut tone_on = None;
        match input {
            Input::Command(command) => tone_on = self.command(command, &mut effects),
            Input::KeyboardAdded(keyboard) => {
                if let Some(seen) = &mut self.midi_restarting {
                    seen.push(keyboard.id.clone());
                }
                self.keyboards.retain(|k| k.id != keyboard.id);
                self.keyboards.push(keyboard);
                self.lost = None;
            }
            Input::KeyboardRemoved { id } => {
                if self.remove_keyboard(&id) {
                    effects.push(Effect::AllNotesOff);
                }
            }
            Input::KeyboardPlayed { id } => {
                if self.keyboards.iter().any(|k| k.id == id) {
                    self.played = Some(id);
                }
            }
            Input::KeyboardButton(button) => {
                if let Some(command) = self.button_command(button) {
                    tone_on = self.command(command, &mut effects);
                }
            }
            Input::KeyboardFader { value } => self.keyboard_fader(value, &mut effects),
            Input::Devices(mut picture) => {
                sort_for_choice(&mut picture.outputs);
                self.devices = Some(picture);
                self.devices_changed_for_input();
                self.failed = None;
                self.audio_retries = 0;
                self.audio_gave_up = false;
                self.audio_system_down = false;
            }
            Input::AudioOpened {
                node_name,
                sample_rate,
            } => {
                if self.wanted.as_deref() == Some(&node_name) {
                    self.audio_gave_up = false;
                    self.open = Some(OpenStream {
                        node_name,
                        sample_rate,
                    });
                    self.play.stream_opened(&mut effects);
                    self.play.set_plug_rate(sample_rate, &mut effects);
                }
            }
            Input::AudioOpenFailed { node_name } => {
                if self.wanted.as_deref() == Some(&node_name) {
                    self.wanted = None;
                    self.open = None;
                    self.failed = Some(node_name);
                    if self.audio_retries > 0 {
                        self.retry_audio(&mut effects);
                    }
                }
            }
            Input::AudioFrames(frames) => {
                if self.open.is_some() {
                    self.actual_frames = frames.filter(|f| *f > 0);
                    if self.actual_frames.is_some() {
                        self.audio_retries = 0;
                    }
                }
            }
            Input::AudioLost => {
                if self.open.is_some() {
                    self.failed = self.wanted.clone();
                    self.retry_audio(&mut effects);
                }
            }
            Input::AudioXruns { count } => {
                if self.open.is_some() {
                    self.xruns = count;
                }
            }
            Input::AudioRetry => {
                if self.audio_retries > 0 {
                    self.failed = None;
                }
            }
            Input::AudioSystemLost => self.audio_system_down = true,
            Input::MidiStopped => {
                if self.midi_restarting.is_none() {
                    self.midi_restarting = Some(Vec::new());
                    self.play.keys_lost(&mut effects);
                    effects.push(Effect::AllNotesOff);
                }
            }
            Input::MidiBack => {
                if let Some(seen) = self.midi_restarting.take() {
                    let gone: Vec<String> = self
                        .keyboards
                        .iter()
                        .filter(|k| !seen.contains(&k.id))
                        .map(|k| k.id.clone())
                        .collect();
                    for id in gone {
                        self.remove_keyboard(&id);
                    }
                }
            }
            Input::LatencySample { ms } => {
                if self.open.is_some() && ms.is_finite() {
                    if self.latencies.len() == LATENCY_SAMPLES {
                        self.latencies.pop_front();
                    }
                    self.latencies.push_back(ms);
                }
            }
            Input::SoundHeard => {
                if self.wants_sound() {
                    self.hint = Some(Hint::SoundCameOut);
                    self.hint_armed = false;
                }
            }
            Input::ScreenSpace { width, height } => {
                if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
                    self.screen = Some((width, height));
                    self.lower_scale_for_screen(&mut effects);
                }
            }
            Input::SongOpened => {
                self.buffer_unsaid = true;
                self.new_song_unsaid = true;
            }
            Input::Record(report) => self.play.report(report, &mut effects),
            Input::KeyWhileLoading => self.loading_said = self.play.target_loading(),
            Input::Tick => self.fader_due(&mut effects),
            Input::CloseRequested
            | Input::SongFilePicked(_)
            | Input::FolderPicked(_)
            | Input::SaveAsPicked(_)
            | Input::SongSaved { .. }
            | Input::SongNotSaved
            | Input::SongRead { .. }
            | Input::SongUnreadable { .. } => self.song_input(input, &mut effects),
            Input::RecoveryFound(start) => self.recovery_start(*start, &mut effects),
            Input::PcEnding => self.pc_ending(&mut effects),
            Input::SoundLoaded(instrument) => self.play.sound_loaded(instrument, &mut effects),
            Input::SoundUnreadable(instrument) => {
                self.play.sound_unreadable(instrument, &mut effects);
            }
            Input::PluginsStart { cache, first } => {
                self.plugins.start(*cache, first, &mut effects);
            }
            Input::PluginScan(news) => {
                if let Some(n) = self.plugins.news(news, self.now_us, &mut effects) {
                    self.notice = Some(n);
                }
            }
            Input::PluginPlacePicked(place) => {
                if let Some(place) = place
                    && self.plugins.place_picked(place, &mut effects)
                {
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Input::PluginLoaded { ticket } => self.play.plugin_loaded(ticket, &mut effects),
            Input::PluginUnreadable { ticket } => {
                if let Some(n) = self.play.plugin_unreadable(ticket, &mut effects) {
                    self.notice = Some(n);
                }
            }
            Input::PluginStopped { ticket } => {
                self.play.plugin_stopped(ticket, &mut effects);
                // The stop is the newer news.
                if self.notice.as_ref().is_some_and(asked_to_see) {
                    self.notice = None;
                }
            }
            Input::PluginsLost => self.play.plugs_lost(&mut effects),
            Input::PluginSaid { ticket, said } => {
                if let Some(n) = self.play.plugin_said(ticket, said, &mut effects) {
                    self.notice = Some(n);
                }
                if let Some(n) = self.play.take_window_failed() {
                    self.notice = Some(n);
                }
            }
            Input::Sf2Loaded(key) => self.play.sf2_loaded(&key, &mut effects),
            Input::Sf2Unreadable(key) => {
                (self.catalog).sf2_unreadable(&key.file, key.bank, key.program);
                self.play.sf2_unreadable(&key, &mut effects);
            }
            Input::Sf2Picked(path) => {
                if let Some(path) = path {
                    effects.push(Effect::AddSf2(path));
                }
            }
            Input::Sf2Added(info) => self.sf2_added(*info, &mut effects),
            Input::Sf2NotAdded { file } => match &mut self.sf2_batch {
                Some(batch) => {
                    batch.skipped += 1;
                    batch.unreadable.get_or_insert(file);
                    self.sf2_answered();
                }
                None => self.notice = Some(StatusMessage::Sf2Unreadable { file }),
            },
            Input::FilesOver(paths) => {
                self.drop_cancelled = false;
                self.pending_drop = None;
                let sf2 = paths.iter().filter(|p| sf2store::looks_like_sf2(p)).count() as u32;
                let first_audio = paths.iter().find(|p| audiofiles::looks_like_audio(p));
                let audio = (paths.iter())
                    .filter(|p| audiofiles::looks_like_audio(p))
                    .count() as u32;
                self.file_drop = first_audio.or(paths.first()).map(|path| FileDropView {
                    name: (path.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    addable: sf2 + audio > 0,
                    files: paths.len() as u32,
                    sf2,
                    audio,
                });
            }
            Input::FilesAt { x, y } => {
                self.drop_at = (x, y);
                if self.file_drop.as_ref().is_some_and(|d| d.audio > 0) {
                    effects.push(Effect::FileEvent(meloet_protocol::Event::FileDrag { x, y }));
                }
            }
            Input::FilesLeft => {
                self.file_drop = None;
                self.drop_cancelled = false;
            }
            Input::FilesDropped(paths) => {
                self.file_drop = None;
                let cancelled = std::mem::take(&mut self.drop_cancelled);
                // Every SF2 file, in the order they were held; the others are passed over
                // (spec 3, 89-10).
                if !cancelled && !self.play.busy() {
                    // The audio files wait for the screen to say where they were let go of.
                    let (audio, paths): (Vec<PathBuf>, Vec<PathBuf>) =
                        (paths.into_iter()).partition(|p| audiofiles::looks_like_audio(p));
                    if !audio.is_empty() {
                        self.drop_serial = self.drop_serial.wrapping_add(1);
                        let (x, y) = self.drop_at;
                        effects.push(Effect::FileEvent(meloet_protocol::Event::FileDrop {
                            drop: self.drop_serial,
                            x,
                            y,
                        }));
                        self.pending_drop = Some(PendingDrop {
                            id: self.drop_serial,
                            paths: audio,
                        });
                    }
                    let (sf2, others): (Vec<PathBuf>, Vec<PathBuf>) =
                        paths.into_iter().partition(|p| sf2store::looks_like_sf2(p));
                    if sf2.len() + others.len() > 1 && !sf2.is_empty() {
                        self.sf2_batch = Some(Sf2Batch {
                            waiting: sf2.len() as u32,
                            skipped: others.len() as u32,
                            ..Sf2Batch::default()
                        });
                    }
                    effects.extend(sf2.into_iter().map(Effect::AddSf2));
                }
            }
            Input::InputOpened {
                node_name,
                channels,
                sample_rate,
            } => {
                let open = OpenInput {
                    channels,
                    sample_rate,
                };
                self.input_opened(&node_name, open);
            }
            Input::InputOpenFailed { node_name } => {
                self.input_gone(Some(&node_name), &mut effects);
            }
            Input::InputLost => {
                let lost = self.input_lost_notice();
                self.input_gone(None, &mut effects);
                if self.play.recording_vocals() {
                    self.play.stop_recording(&mut effects);
                    if lost.is_some() {
                        self.notice = lost;
                    }
                }
            }
            Input::AudioDelays(delays) => self.delays = delays,
            Input::DiskTooSlow => {
                if self.play.is_moving() && !self.disk_slow_said {
                    self.disk_slow_said = true;
                    self.notice = Some(StatusMessage::DiskTooSlow);
                }
            }
            Input::SavingAudio {
                done,
                total,
                done_bytes,
                total_bytes,
            } => {
                if self.saving() {
                    self.saving_audio = Some(StatusMessage::SavingAudio {
                        done,
                        total,
                        done_bytes,
                        total_bytes,
                    });
                }
            }
            Input::MicClipped => {
                if !self.play.busy() {
                    self.notice = Some(StatusMessage::MicTooLoud);
                }
            }
            Input::VocalsStarted { names } => {
                let path = self.files.path.clone();
                self.play
                    .vocals_started(names, path.as_deref(), &mut effects);
            }
            Input::VocalsNotStarted => self.play.vocals_not_started(),
            Input::VocalsWritten(written) => {
                let ours = self.play.recording_vocals();
                let skipped = written.gap_count.saturating_add(written.dropped_blocks);
                // A recording the stream went away under says so now that its take is in.
                if let Some((cut, take)) = self.play.vocals_written(*written, &mut effects) {
                    self.notice = Some(self.cut_notice(cut, take));
                } else if ours
                    && skipped > 0
                    && !matches!(self.notice, Some(StatusMessage::InputLost { .. }))
                {
                    self.notice = Some(StatusMessage::RecordingGaps { count: skipped });
                }
            }
            Input::VocalFaults { gaps, dropped } => self.play.vocal_faults(gaps, dropped),
            Input::VocalsCannotGoOn => {
                if self.play.recording_vocals() {
                    self.play.stop_recording(&mut effects);
                }
            }
            Input::LatencyAnswer { x_us } => self.latency_answer(x_us, &mut effects),
            Input::AudioPicked(paths) => self.audio_picked(paths, &mut effects),
            Input::Audio(news) => self.audio_news(news),
            Input::AudioMaking(jobs) => self.play.set_audio_making(jobs),
            Input::Export(news) => self.export_news(news),
            Input::Pack(news) => self.pack_news(news, &mut effects),
            Input::ScreenUp => self.pack_screen_up(&mut effects),
        }
        self.settle_export(&mut effects);
        if !self.play.is_moving() {
            self.disk_slow_said = false;
        }
        if !self.saving() {
            self.saving_audio = None;
        }
        self.refresh_catalog();
        if !self.bands.side_is_open(SidePanel::Shelf) {
            self.play.shelf_closed(&mut effects);
        }
        self.settle(&mut effects);
        self.settle_input(&mut effects);
        let input = self.open_input();
        self.play.sync_voices(input, &mut effects);
        let path = self.files.path.clone();
        self.play.note_vocals(path.as_deref(), &mut effects);
        if self.open.is_none() {
            let cut = if self.audio_system_down {
                Cut::AudioSystem
            } else {
                Cut::Output
            };
            self.play.stop_sample(&mut effects);
            let was_recording = self.play.is_recording();
            let take = self.play.stream_gone(&mut effects);
            match take {
                Some(take) if was_recording => self.notice = Some(self.cut_notice(cut, take)),
                // Vocals wait for the disk writer: said when their take is in the song.
                _ => self.play.vocals_cut(cut),
            }
        }
        self.watch_buffer();
        self.play.sync_silent(&mut effects);
        self.play.sync_target(&mut effects);
        self.play.sync_mix(&mut effects);
        self.play.sync_plugs(&mut effects);
        self.play.sync_windows(&mut effects);
        self.play.sync_panel(&mut effects);
        self.play.sync_sample(&mut effects);
        self.play.sync_sounds(&mut effects);
        self.play.sync_automation(&mut effects);
        if let Some(n) = self.play.take_auto_notice() {
            self.notice = Some(n);
        }
        if self.loading_said != self.play.target_loading() {
            self.loading_said = None;
        }
        self.audio_follow(&mut effects);
        self.close_when_recorded(&mut effects);
        self.recovery_tick(&mut effects);
        if tone_on.is_some() && tone_on == self.wanted {
            effects.push(Effect::TestTone);
        }
        effects
    }

    /// The vocal tracks whose input is metered now, in the order of the song.
    pub fn metered_vocals(&self) -> Vec<(meloet_model::TrackId, VocalInput)> {
        self.play.metered_vocals()
    }

    pub fn set_uid_seed(&mut self, seed: u64) {
        self.play.set_uid_seed(seed);
    }

    /// Xruns of the open stream since it was opened.
    pub fn xruns(&self) -> u64 {
        self.xruns
    }

    /// The time and what the listener hears, before each input. `heard` only while a stream
    /// is open.
    pub fn set_now(&mut self, t_us: u64, heard: Option<Heard>) {
        self.now_us = t_us;
        self.play.set_now(t_us, heard);
    }

    /// Whether the transport moves or a recording is under way: the engine's reports are then
    /// read often.
    pub fn is_moving(&self) -> bool {
        self.play.is_moving()
    }

    pub fn live_notes(&self) -> LiveNotesSnapshot {
        self.play.live_snapshot()
    }

    pub fn take_notes(&self, id: u32) -> Option<Vec<u8>> {
        self.play.take_notes(id)
    }

    /// Where the peaks of an audio file of the song are; `None` for an `audio` that is no
    /// file's. A file that is being read has them in memory (`PeaksPlace::Growing`).
    pub fn peaks_place(&self, audio: u32) -> Option<crate::peaks::PeaksPlace> {
        self.play.peaks_place(audio)
    }

    /// The points of a lane of the song as it is now; `None` for a `lane_id` that is no
    /// lane's.
    pub fn lane(&self, lane_id: u32) -> Option<Vec<u8>> {
        self.play.lane(lane_id)
    }

    /// The rows `from ..= to` of the history band that there are (rows count from 1).
    pub fn history_rows(&self, from: u32, to: u32) -> Vec<HistoryRow> {
        self.play.history_rows(from, to)
    }

    fn output_name(&self) -> String {
        self.chosen_name
            .clone()
            .or_else(|| self.chosen.clone())
            .unwrap_or_default()
    }

    /// Forgets a keyboard that went away. Returns whether it was there.
    fn remove_keyboard(&mut self, id: &str) -> bool {
        let Some(i) = self.keyboards.iter().position(|k| k.id == id) else {
            return false;
        };
        let gone = self.keyboards.remove(i);
        if self.played.as_ref() == Some(&gone.id) {
            self.played = None;
        }
        if self.keyboards.is_empty() {
            self.lost = Some(gone.name);
        }
        true
    }

    /// Asks to open the stream again later, unless it has been tried enough.
    fn retry_audio(&mut self, effects: &mut Vec<Effect>) {
        if self.audio_retries < AUDIO_RETRIES {
            self.audio_retries += 1;
            effects.push(Effect::RetryAudio {
                after: retry_delay(self.audio_retries),
            });
        } else {
            self.audio_retries = 0;
            self.audio_gave_up = true;
        }
    }

    /// Returns the node a test tone was asked on.
    fn command(&mut self, command: Command, effects: &mut Vec<Effect>) -> Option<String> {
        // A resize is not something the user does to answer a notice, and neither is the sound
        // of the write tool, which comes and goes around the write that may give a notice.
        if !matches!(
            command,
            Command::SetViewport { .. } | Command::Audition { .. } | Command::AuditionOff
        ) {
            self.notice = None;
            self.new_song_unsaid = false;
        }
        self.audio_saw_command(&command);
        self.export.saw_command(&command);
        let can_play = self.open.is_some() && !self.measuring();
        let command = self.vocal_command(command, effects)?;
        match command {
            Command::SetViewport { width, height } => {
                if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
                    self.viewport = (width, height);
                    self.fit_window();
                }
            }
            Command::SetScale { percent } if !SCALES.contains(&percent) => {}
            Command::SetScale { percent } if percent == self.scale => {}
            Command::SetScale { percent } => {
                if !self.scale_fits(percent) {
                    self.notice = Some(StatusMessage::ScaleDoesNotFit {
                        requested: percent,
                        kept: self.scale,
                    });
                } else {
                    self.scale = percent;
                    // The shell grows a window that is smaller than the smallest one at this
                    // zoom; the bands are laid out for that size right away.
                    let min = layout::min_window(percent);
                    let grown = (
                        self.viewport.0.max(min.width as f32),
                        self.viewport.1.max(min.height as f32),
                    );
                    if grown != self.viewport {
                        self.viewport = grown;
                        self.notice = Some(StatusMessage::ScaleWindowGrown { scale: percent });
                    }
                    if let Some(band) = self.fit_bands() {
                        self.notice = Some(StatusMessage::BandFoldedScale {
                            scale: percent,
                            band,
                        });
                    }
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::ToggleBand { band } => {
                let mixer_was_open = self.mixer_is_open();
                let fold = self.bands.toggle(band_of(band));
                // Said once as the mixer opens; a band folded for it is said instead.
                if !mixer_was_open && self.mixer_is_open() {
                    self.notice =
                        (self.play.mixer_passes_by()).or_else(|| self.play.automation_moving());
                }
                if let Some(fold) = fold {
                    let folded = bottom_band(fold.band);
                    self.notice = Some(match fold.why {
                        FoldWhy::Third => StatusMessage::BandFoldedThird {
                            opened: band,
                            folded,
                        },
                        FoldWhy::TooLow => StatusMessage::BandFoldedToOpen {
                            opened: band,
                            folded,
                        },
                    });
                }
            }
            Command::GoArrange => self.bands.go_arrange(),
            Command::Back => {
                if !self.plugins.close_stopped()
                    && !self.cancel_song_dialog()
                    && self.dialog.take().is_none()
                    && !self.play.end_audition(effects)
                {
                    self.bands.back();
                }
            }
            Command::OpenSettings { advanced } => match &mut self.dialog {
                Some(dialog) => dialog.advanced_open |= advanced,
                None => {
                    self.dialog = Some(SettingsDialogView {
                        draft_output: self.chosen.clone(),
                        advanced_open: advanced,
                    });
                }
            },
            Command::SettingsSelectOutput { output } => {
                let choosable = self.choosable(&output).is_some();
                if let Some(dialog) = &mut self.dialog
                    && choosable
                {
                    dialog.draft_output = Some(output);
                }
            }
            Command::SettingsToggleAdvanced => {
                if let Some(dialog) = &mut self.dialog {
                    dialog.advanced_open = !dialog.advanced_open;
                }
            }
            Command::SettingsApply => {
                if let Some(dialog) = self.dialog.take()
                    && let Some(draft) = dialog.draft_output
                    && self.chosen.as_ref() != Some(&draft)
                {
                    self.choose(draft, effects);
                }
            }
            Command::SettingsCancel => self.dialog = None,
            Command::ChooserMove { delta } => {
                if self.chooser_shown() {
                    let ids: Vec<&String> = self
                        .outputs()
                        .iter()
                        .filter(|e| is_choosable(e))
                        .map(|e| &e.id)
                        .collect();
                    let at = ids
                        .iter()
                        .position(|id| Some(*id) == self.highlighted.as_ref());
                    if let (Some(at), false) = (at, ids.is_empty()) {
                        let last = ids.len() as i64 - 1;
                        let to = (at as i64 + i64::from(delta)).clamp(0, last) as usize;
                        self.highlighted = Some(ids[to].clone());
                    }
                }
            }
            Command::ChooserSelect { output } => {
                if self.chooser_shown() && self.choosable(&output).is_some() {
                    self.highlighted = Some(output);
                }
            }
            Command::ChooserConfirm => {
                if self.chooser_shown()
                    && let Some(output) = self.highlighted.clone()
                    && self.choosable(&output).is_some()
                {
                    self.choose(output.clone(), effects);
                    self.hint_armed = true;
                    let output = self.endpoint(&output).map_or(output, |e| e.name.clone());
                    self.notice = Some(StatusMessage::OutputChosen {
                        output,
                        track: self.play.record_target(),
                    });
                }
            }
            Command::TestTone { output } => {
                let node = self.choosable(&output)?.node_name.clone()?;
                if self.wanted.as_ref() != Some(&node) {
                    if !(self.chooser_shown() || self.dialog.is_some()) {
                        return None;
                    }
                    self.preview = Some(output);
                    if self.failed.as_ref() == Some(&node) {
                        self.failed = None;
                        self.audio_gave_up = false;
                    }
                }
                return Some(node);
            }
            Command::DismissHint => {
                if !self.dismiss_recovery() {
                    if self.hint.take().is_some() {
                        self.hint_dismissed = true;
                    } else if self.empty_hint() {
                        self.empty_dismissed = true;
                    }
                }
            }
            Command::PlayStop => self.play.play_stop(can_play, effects),
            Command::Record => {
                let input = self.open_input();
                let setup = input.map(|i| (i, self.sound.record_format, self.latency_x_us()));
                self.notice = self.play.record(can_play, setup, effects);
            }
            Command::GoToStart => self.play.go_to_start(effects),
            Command::Locate { tick } => self.play.locate(tick, effects),
            Command::SetTempo { bpm } => self.say(|p| p.set_tempo(bpm, effects)),
            Command::SetMeter { num, den } => self.say(|p| p.set_meter(num, den, effects)),
            Command::SetTimelineOpen { open } => self.play.set_timeline_open(open),
            Command::AddTempoPoint { tick, bpm } => {
                self.say(|p| p.add_tempo_point(tick, bpm, effects));
            }
            Command::MoveTempoPoint { tick, to_tick, bpm } => {
                self.say(|p| p.move_tempo_point(tick, to_tick, bpm, effects));
            }
            Command::SetTempoRamp { tick, on } => self.play.set_tempo_ramp(tick, on, effects),
            Command::AddMeterChange { bar, num, den } => {
                self.say(|p| p.add_meter_change(bar, num, den, effects));
            }
            Command::MoveMeterChange { bar, to_bar } => {
                self.say(|p| p.move_meter_change(bar, to_bar, effects));
            }
            Command::SetMeterChange { bar, num, den } => {
                self.say(|p| p.set_meter_change(bar, num, den, effects));
            }
            Command::SelectTimeline { target } => self.say(|p| p.select_timeline(target)),
            Command::DeleteTimeline { target } => self.play.delete_timeline(target, effects),
            Command::AddMarker { name, tick: None } => {
                self.say(|p| p.add_marker(&name, effects));
            }
            Command::AddMarker { tick: Some(_), .. } => {}
            Command::OpenMarkerName { id } => self.play.open_marker_name(id),
            Command::RenameMarker { id, name } => self.play.rename_marker(id, &name, effects),
            Command::CloseMarkerName => self.play.close_marker_name(),
            Command::MoveMarker { id, tick } => self.play.move_marker(id, tick, effects),
            Command::GoToMarker { id } => self.play.go_to_marker(id, effects),
            Command::StepMarker { forward } => self.play.step_marker(forward, effects),
            Command::LoopFromMarker { id } => self.play.loop_from_marker(id, effects),
            Command::SetCountIn { bars } => {
                if self.play.set_count_in(bars) {
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::ToggleMetronome => {
                self.play.toggle_metronome(effects);
                effects.push(Effect::SaveSettings(self.settings()));
            }
            Command::ToggleLoop => self.play.toggle_loop(effects),
            Command::SetLoop { from_bar, to_bar } => self.play.set_loop(from_bar, to_bar, effects),
            Command::SelectClip { clip } => self.play.select_clip(clip),
            Command::SelectTrack { track, from_arm } => {
                if let Some(n) = self.play.select_track(track, from_arm) {
                    self.notice = Some(n);
                }
                if self.play.selected_vocal().is_some() && !self.has_input() {
                    self.notice = Some(StatusMessage::NoInput);
                }
            }
            Command::DeleteTrack { track } => {
                if let Some(n) = self.play.delete_track(track, effects) {
                    self.notice = Some(n);
                }
            }
            Command::RenameTrack { track, name } => self.play.rename_track(track, &name, effects),
            Command::MoveTrack { track, to } => self.play.move_track(track, to, effects),
            Command::SetTrackColor { track, color } => {
                self.play.set_track_color(track, color, effects);
            }
            Command::ToggleFavorite { instrument } => {
                let favorite = instruments::builtin_of(instrument);
                if !self.favorites.contains(&favorite) {
                    self.favorites.push(favorite);
                } else {
                    self.favorites.retain(|&f| f != favorite);
                }
                effects.push(Effect::SaveSettings(self.settings()));
            }
            Command::OpenShelf { mode } => {
                if self.play.open_shelf(mode, effects) {
                    self.bands.show_side(SidePanel::Shelf);
                }
            }
            Command::ShelfSelect { instrument } => {
                if self.bands.side_is_open(SidePanel::Shelf) {
                    self.play.shelf_select(instrument, effects);
                }
            }
            Command::ShelfConfirm => {
                let lists_effects = matches!(
                    self.shelf_view(&self.library_view()).mode,
                    ShelfMode::InsertEffect { .. } | ShelfMode::ReplaceEffect { .. }
                );
                if let Some(n) = self.play.shelf_confirm(effects) {
                    self.notice = Some(n);
                    // An effect went in and is the selected insert now.
                    if lists_effects && self.play.selection_wants_panel() {
                        self.bands.show_side(SidePanel::Params);
                    }
                }
            }
            Command::ShelfSelectEffect { effect } => {
                if self.bands.side_is_open(SidePanel::Shelf) {
                    self.play.shelf_select_effect(effect);
                }
            }
            Command::SetGain { strip, db, commit } => {
                self.play.set_gain(strip, db, commit, effects);
            }
            Command::SetPan { strip, pan, commit } => {
                self.play.set_pan(strip, pan, commit, effects);
            }
            Command::SetSend {
                track,
                bus,
                db,
                commit,
            } => self.play.set_send(track, bus, db, commit, effects),
            Command::SetParam {
                insert,
                effect,
                param,
                value,
                commit,
            } => self
                .play
                .set_param(insert, effect, param, value, commit, effects),
            Command::CancelMixPreview => self.play.cancel_mix_preview(effects),
            Command::ToggleReturnMute { bus } => self.play.toggle_return_mute(bus, effects),
            Command::RemoveEffect { insert } => {
                self.notice = self.play.remove_effect(insert, effects);
            }
            Command::ToggleBypass { insert } => {
                self.notice = self.play.toggle_bypass(insert, effects);
            }
            Command::MoveInsert { insert, to } => {
                self.notice = self.play.move_insert(insert, to, effects);
            }
            Command::TypeParam { param, text } => {
                self.notice = self.play.type_param(param, &text, effects);
            }
            Command::SelectInsert { insert } => {
                let selecting = self.play.select_insert(insert);
                self.selected(selecting, effects);
            }
            // Vocal tracks, the input and the latency: accepted, and nothing yet
            // (docs/architecture.md 22-15).
            // Taken by `vocal_command` above.
            Command::AddVocalTrack
            | Command::SetVocalInput { .. }
            | Command::ToggleVocalArm { .. }
            | Command::ToggleMonitor { .. }
            | Command::SettingsSelectInput { .. }
            | Command::SettingsSetBuffer { .. }
            | Command::SettingsSetSampleRate { .. }
            | Command::SettingsSetRecordFormat { .. }
            | Command::SettingsSetManualLatency { .. }
            | Command::OpenLatencyMeasure
            | Command::StartLatencyMeasure
            | Command::StopLatencyMeasure
            | Command::UseMeasuredLatency
            | Command::CloseLatencyMeasure => {}
            Command::SetTallTrack { track } => {
                let serial = self.play.serial();
                let vocal = |id: u32| {
                    (self.play.song().doc.track(meloet_model::TrackId(id)))
                        .is_some_and(|t| t.kind.is_vocal())
                };
                self.tall_track = track
                    .filter(|&id| vocal(id) && self.tall_track != Some((serial, id)))
                    .map(|id| (serial, id));
            }
            Command::DropAudio {
                drop,
                track,
                tick,
                mode,
            } => self.drop_audio(drop, track, tick, mode, effects),
            Command::SplitClips => self.split_clips(effects),
            Command::SplitClipAt { .. }
            | Command::JoinClips { .. }
            | Command::RenameClip { .. }
            | Command::CreateClip { .. } => {}
            Command::SetClipField { clip, field, value } => {
                if let Some(n) = self.play.set_clip_field(clip, field, value, effects) {
                    self.notice = Some(n);
                }
            }
            Command::OpenExport
            | Command::CloseExport
            | Command::SetExport { .. }
            | Command::ChooseExportPlace
            | Command::StartExport
            | Command::CancelExport
            | Command::ShowExported => self.export_command(&command, effects),
            Command::ShiftClipGains { clips, by_db } => {
                (self.play).set_clip_gains(&clips, ClipGains::By(by_db), effects);
            }
            Command::SetClipGains { clips, db } => {
                (self.play).set_clip_gains(&clips, ClipGains::To(db), effects);
            }
            Command::SetClipsStretch { clips, change } => {
                self.play.set_clips_stretch(&clips, change, effects);
            }
            // Protocol 27: accepted, and nothing yet (docs/architecture.md 23-7).
            Command::FetchPack | Command::PackLater | Command::StopPackFetch => {
                self.pack_command(&command, effects);
            }
            Command::ImportAudio { track } => self.import_audio(track, effects),
            Command::FindAudio { audio } => self.find_audio(audio, effects),
            Command::SetClipStretch { clip, change } => {
                self.play.set_clip_stretch(clip, change, effects);
            }
            Command::FindSourceTempo { clip } => {
                if let Some(n) = self.play.find_source_tempo(clip, effects) {
                    self.notice = Some(n);
                }
            }
            Command::SelectInstrument { track } => {
                let selecting = self.play.select_instrument(track);
                self.selected(selecting, effects);
            }
            Command::SelectParam { param } => self.play.select_param(param),
            Command::ShelfCancel => {
                self.play.end_audition(effects);
                self.play.forget_unreadable();
            }
            Command::PlaySample { instrument } => {
                if self.bands.side_is_open(SidePanel::Shelf) {
                    let ms = self.audio_view().latency_ms.unwrap_or(0.0);
                    let latency_us = (ms * 1000.0) as u64;
                    self.play
                        .play_sample(instrument, can_play, latency_us, effects);
                }
            }
            Command::StopSample => self.play.stop_sample(effects),
            Command::SelectClips { clips, mode } => self.play.select_clips(clips, mode),
            Command::SelectAllClips => self.play.select_all_clips(),
            Command::CopyClips { clips } => self.play.copy_clips(&clips),
            Command::CutClips { clips } => self.play.cut_clips(&clips, effects),
            Command::PasteClips { to } => {
                if let Some(n) = self.play.paste_clips(to, effects) {
                    self.notice = Some(n);
                }
            }
            Command::ToggleMute { track } => self.play.toggle_mute(track, effects),
            Command::ToggleSolo { track } => self.play.toggle_solo(track),
            Command::SetOverdub { .. } => {}
            Command::SetVelocityFixed { on, value } => {
                let fixed = VelocityFixed {
                    on,
                    value: value.clamp(1, 127),
                };
                if fixed != self.velocity_fixed {
                    self.velocity_fixed = fixed;
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::SaveSongAs => self.save_song_as(effects),
            Command::Quit => self.close_requested(effects),
            Command::DeleteSelected => self.play.delete_selected(effects),
            Command::StepTake { delta } => self.notice = self.play.step_take(delta, effects),
            Command::DeleteTake { clip } => self.play.delete_take(clip, effects),
            Command::AddLane { track, target } => self.play.add_lane(track, target, effects),
            Command::DeleteLane { track, target } => {
                self.play.delete_lane(track, target, effects);
            }
            Command::FoldLane {
                track,
                target,
                folded,
            } => self.play.fold_lane(track, target, folded),
            Command::AddAutoPoint { lane, tick, value } => {
                self.play.add_auto_point(lane, tick, value, effects);
            }
            Command::MoveAutoPoints {
                lane,
                points,
                d_tick,
                d_value,
            } => self
                .play
                .move_auto_points(lane, &points, d_tick, d_value, effects),
            Command::DeleteAutoPoints { lane, points } => {
                self.play.delete_auto_points(lane, &points, effects);
            }
            Command::SelectAutoPoints { lane, points } => {
                self.play.select_auto_points(lane, points);
            }
            Command::SetLaneWrite { track, target, on } => {
                self.play.set_lane_write(track, target, on);
            }
            Command::RestoreLane { track, target } => {
                self.say(|p| p.restore_lane(track, target, effects));
            }
            Command::HistoryJump { index, step } => {
                self.notice = self.play.history_jump(index, step, effects);
            }
            Command::Undo => self.notice = self.play.undo(effects),
            Command::Redo => self.notice = self.play.redo(effects),
            Command::NewSong => self.new_song(effects),
            Command::OpenSong => self.open_song(effects),
            Command::SaveSong => self.save_song(effects),
            Command::AnswerSongDialog { dialog, answer } => {
                self.answer_song_dialog(dialog, answer, effects);
            }
            Command::SelectNotes { take, notes, mode } => {
                self.play.select_notes(take, notes, mode);
            }
            Command::SelectAllNotes => self.play.select_all_notes(),
            Command::ClearNoteSelection => self.play.clear_note_selection(),
            Command::AddNote {
                take,
                start,
                length,
                key,
                velocity,
            } => {
                let note = Note {
                    from: None,
                    start,
                    length,
                    key,
                    velocity,
                };
                self.notice = self.play.add_note(take, note, effects);
            }
            Command::MoveNotes {
                take,
                notes,
                ticks,
                keys,
                copy,
            } => {
                self.notice = self
                    .play
                    .move_notes(take, &notes, ticks, keys, copy, effects);
            }
            Command::ResizeNotes {
                take,
                notes,
                edge,
                ticks,
            } => {
                self.notice = self.play.resize_notes(take, &notes, edge, ticks, effects);
            }
            Command::DeleteNotes { take, notes } => self.play.delete_notes(take, &notes, effects),
            Command::ChangeVelocity { take, notes, delta } => {
                self.play.change_velocity(take, &notes, delta, effects);
            }
            Command::CopyNotes { take, notes } => self.play.copy_notes(take, &notes),
            Command::CutNotes { take, notes } => self.play.cut_notes(take, &notes, effects),
            Command::PasteNotes { at } => self.notice = self.play.paste_notes(at, effects),
            Command::AddNotes { .. } => {}
            Command::SetGrid { division, triplet } => self.play.set_grid(division, triplet),
            Command::SetMusicalScale { root, mode, lock } => {
                self.play.set_musical_scale(root, mode, lock);
            }
            Command::SetQuantize {
                division,
                triplet,
                strength,
                swing,
                align,
            } => self.play.set_quantize(QuantizeSettings {
                division,
                triplet,
                strength,
                swing,
                align,
            }),
            Command::Audition { keys, velocity } => {
                if can_play && !keys.is_empty() {
                    self.loading_said = self.play.target_loading();
                }
                self.play.audition(&keys, velocity, can_play, effects);
            }
            Command::AuditionOff => self.play.audition_off(can_play, effects),
            Command::Quantize { take, notes } => {
                if let Some(n) = self.play.quantize(take, &notes, effects) {
                    self.notice = Some(n);
                }
            }
            Command::Unquantize { take, notes } => self.play.unquantize(take, &notes, effects),
            Command::SetNotesField {
                take,
                notes,
                field,
                value,
            } => self
                .play
                .set_notes_field(take, &notes, field, value, effects),
            Command::MatchVelocity { take, notes, value } => {
                if let Some(n) = self.play.match_velocity(take, &notes, value, effects) {
                    self.notice = Some(n);
                }
            }
            Command::MatchLength { take, notes, value } => {
                if let Some(n) = self.play.match_length(take, &notes, value, effects) {
                    self.notice = Some(n);
                }
            }
            Command::SelectControls { take, points, mode } => {
                self.play.select_controls(take, points, mode);
            }
            Command::AddControl {
                take,
                tick,
                control,
                value,
            } => self.play.add_control(take, tick, control, value, effects),
            Command::MoveControls {
                take,
                points,
                ticks,
                value,
            } => self
                .play
                .move_controls(take, &points, ticks, value, effects),
            Command::DeleteControls { take, points } => {
                self.play.delete_controls(take, &points, effects);
            }
            Command::MoveClips {
                clips,
                ticks,
                tracks,
                copy,
            } => {
                if let Some(n) = self.play.move_clips(&clips, ticks, tracks, copy, effects) {
                    self.notice = Some(n);
                }
            }
            Command::DuplicateClips { clips } => {
                if let Some(n) = self.play.duplicate_clips(&clips, effects) {
                    self.notice = Some(n);
                }
            }
            Command::TrimClip { clip, edge, ticks } => {
                self.play.trim_clip(clip, edge, ticks, effects);
            }
            Command::RejectValue { field, kept, range } => {
                self.notice = Some(StatusMessage::ValueRejected { field, kept, range });
            }
            Command::ClampValue {
                field,
                value,
                range,
            } => {
                self.notice = Some(StatusMessage::ValueClamped {
                    field,
                    value,
                    range,
                });
            }
            Command::DiscardRecovered => self.discard_recovered(effects),
            Command::ShowRecoveryFile => self.show_recovery_file(effects),
            Command::RescanPlugins => self.plugins.rescan(effects),
            Command::RetryStoppedPlugin { path, id } => self.plugins.retry(&path, id, effects),
            Command::AddPluginPlace => effects.push(Effect::PickPluginPlace),
            Command::RemovePluginPlace { path } => {
                if self.plugins.remove_place(&path, self.now_us, effects) {
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::OpenScanStopped => self.plugins.open_stopped(),
            Command::CloseScanStopped => {
                self.plugins.close_stopped();
            }
            Command::OpenPluginPlaces => {
                self.plugins.close_stopped();
                self.plugins.unfold_places();
                match &mut self.dialog {
                    Some(dialog) => dialog.advanced_open = true,
                    None => {
                        self.dialog = Some(SettingsDialogView {
                            draft_output: self.chosen.clone(),
                            advanced_open: true,
                        });
                    }
                }
            }
            Command::TogglePluginPlaces => self.plugins.toggle_places(),
            Command::ReloadPlugin { slot } => self.play.reload_plugin(slot, effects),
            // The engine hands the plugin back when the step takes it out of the song, and
            // it is let go of then.
            Command::RemovePlugin { slot } => {
                if self.play.plugin_is_down(slot)
                    && let Some(n) = self.play.remove_plugin(slot, effects)
                {
                    self.notice = Some(n);
                }
            }
            Command::ShelfSelectSound { sound } => match sound {
                Some(SoundRef::Builtin { instrument }) => {
                    let instrument = Some(instrument);
                    return self.command(Command::ShelfSelect { instrument }, effects);
                }
                // A sound the shelf does not have is no row: as `ShelfCancel`.
                sound => {
                    if self.bands.side_is_open(SidePanel::Shelf) {
                        let found = sound.and_then(|s| self.catalog.instrument(&s));
                        (self.play).shelf_select_sound(found.map(|(i, _)| i), effects);
                    }
                }
            },
            Command::PlaySampleOf { sound } => match sound {
                SoundRef::Builtin { instrument } => {
                    return self.command(Command::PlaySample { instrument }, effects);
                }
                sound => {
                    if self.bands.side_is_open(SidePanel::Shelf)
                        && let Some((instrument, group)) = self.catalog.instrument(&sound)
                    {
                        let ms = self.audio_view().latency_ms.unwrap_or(0.0);
                        let latency_us = (ms * 1000.0) as u64;
                        (self.play)
                            .play_sample_of(instrument, group, can_play, latency_us, effects);
                    }
                }
            },
            Command::ToggleFavoriteSound { sound } => match sound {
                SoundRef::Builtin { instrument } => {
                    return self.command(Command::ToggleFavorite { instrument }, effects);
                }
                sound => {
                    if self.catalog.toggle_sound(sound) {
                        effects.push(Effect::SaveSettings(self.settings()));
                    }
                }
            },
            Command::ToggleFavoriteEffect { effect } => {
                if self.catalog.toggle_effect(effect) {
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::ShelfChooseEffect { effect } => match effect {
                Some(EffectRef::Plugin { key }) => {
                    if self.bands.side_is_open(SidePanel::Shelf) {
                        let found = self.catalog.plugin(&key).filter(|f| !f.instrument);
                        self.play.shelf_choose_plugin(found.map(|f| f.plugin));
                    }
                }
                Some(EffectRef::Builtin { effect }) => {
                    let effect = Some(effect);
                    return self.command(Command::ShelfSelectEffect { effect }, effects);
                }
                None => return self.command(Command::ShelfSelectEffect { effect: None }, effects),
            },
            Command::UsePluginFormat { key } => {
                if self.catalog.use_format(&key) {
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::AddSf2 => {
                if !self.play.busy() {
                    effects.push(Effect::PickSf2);
                }
            }
            Command::CancelFileDrop => {
                if self.file_drop.take().is_some() {
                    self.drop_cancelled = true;
                }
            }
            Command::RemoveMissing { missing } => {
                // Only what a notice names: what this PC has is taken out in the usual ways.
                let doc = &self.play.song().doc;
                let notices = added::missing(doc, &self.library_view(), self.plugins.home());
                let named = notices.iter().any(|notice| match (notice, &missing) {
                    (MissingView::Sf2 { file, .. }, MissingRef::Sf2 { file: f }) => file == f,
                    (MissingView::Plugin { key, .. }, MissingRef::Plugin { key: k }) => key == k,
                    _ => false,
                });
                let missing = match &missing {
                    MissingRef::Sf2 { file } => Missing::Sf2(file),
                    MissingRef::Plugin { key } => Missing::Plugin(key),
                    MissingRef::Audio { audio } => {
                        self.remove_missing_audio(*audio, effects);
                        return None;
                    }
                };
                if named {
                    self.play.remove_missing(missing, effects);
                }
            }
            Command::OpenPluginWindow { slot } => self.play.open_plugin_window(slot, effects),
            Command::ClosePluginWindow { slot } => self.play.close_plugin_window(slot, effects),
            Command::RaisePluginWindows => self.play.raise_plugin_windows(effects),
            Command::SetPluginParam {
                slot,
                table,
                param,
                value,
                commit,
            } => self
                .play
                .set_plugin_param(slot, table, param, value, commit, effects),
        }
        None
    }

    fn mixer_is_open(&self) -> bool {
        let open = self.bands.layout().bottom;
        open.iter().any(|b| b.band == BottomBand::Mixer)
    }

    /// An insert or an instrument was selected: its panel or its window, and what the
    /// status line says.
    fn selected(&mut self, selecting: crate::play::Selecting, effects: &mut Vec<Effect>) {
        if selecting.panel {
            self.bands.show_side(SidePanel::Params);
        }
        if let Some(slot) = selecting.window {
            self.play.open_plugin_window(Some(slot), effects);
        }
        if selecting.notice.is_some() {
            self.notice = selecting.notice;
        }
    }

    /// Takes in what the scan knows, when that changed since the catalog last looked: the
    /// rows of the shelf, and which plugins of the song this PC has.
    fn refresh_catalog(&mut self) {
        let rev = self.plugins.rows_rev();
        if rev != self.rows_seen {
            self.rows_seen = rev;
            self.catalog.set_plugins(self.plugins.cache());
            self.play.set_known_plugins(self.catalog.bundles());
        }
    }

    /// The file is in meloet's folder: the shelf opens on its instruments with the file's
    /// heading unfolded, and the tracks that name it sound with it.
    fn sf2_added(&mut self, info: Sf2Info, effects: &mut Vec<Effect>) {
        let kits = (info.presets.iter())
            .filter(|p| p.family() == GmFamily::DrumKit)
            .count() as u32;
        let presets = info.presets.len() as u32 - kits;
        match &mut self.sf2_batch {
            Some(batch) => {
                batch.added.push((info.file.clone(), presets, kits));
                self.sf2_answered();
            }
            None => {
                self.notice = Some(StatusMessage::Sf2Added {
                    file: info.file.clone(),
                    presets,
                    kits,
                    skipped: 0,
                });
            }
        }
        self.play
            .sf2_added(&info.file, sf2_keys(&info).collect(), effects);
        self.catalog.sf2_added(info);
        let lists_instruments = self.bands.side_is_open(SidePanel::Shelf)
            && !matches!(
                self.shelf_view(&self.library_view()).mode,
                ShelfMode::InsertEffect { .. } | ShelfMode::ReplaceEffect { .. }
            );
        if !lists_instruments && self.play.open_shelf(ShelfMode::NewTrack, effects) {
            self.bands.show_side(SidePanel::Shelf);
        }
    }

    /// One file of `sf2_batch` was answered: after the last, the status line says what came
    /// of them all.
    fn sf2_answered(&mut self) {
        let Some(batch) = &mut self.sf2_batch else {
            return;
        };
        batch.waiting = batch.waiting.saturating_sub(1);
        if batch.waiting > 0 {
            return;
        }
        let Some(batch) = self.sf2_batch.take() else {
            return;
        };
        let skipped = batch.skipped;
        self.notice = match (batch.added.len(), batch.added.into_iter().next()) {
            (1, Some((file, presets, kits))) => Some(StatusMessage::Sf2Added {
                file,
                presets,
                kits,
                skipped,
            }),
            (added, Some((first, _, _))) => Some(StatusMessage::Sf2AddedMany {
                added: added as u32,
                first,
                skipped,
            }),
            (_, None) => batch
                .unreadable
                .map(|file| StatusMessage::Sf2Unreadable { file }),
        };
    }

    /// A song was opened: says so when it uses sounds this PC does not have (spec 6).
    /// Puts what `play` gives into the status line; `None` leaves the line as it is.
    fn say(&mut self, play: impl FnOnce(&mut Play) -> Option<StatusMessage>) {
        if let Some(n) = play(&mut self.play) {
            self.notice = Some(n);
        }
    }

    pub(super) fn say_missing(&mut self) {
        let library = self.library_view();
        let sounds = self.play.available_sounds();
        let doc = &self.play.song().doc;
        let pack = (doc.tracks.iter())
            .filter(|t| {
                added::stand_in(&t.instrument, &library, &sounds) == Some(StandIn::PackMissing)
            })
            .count() as u32;
        let missing = added::missing(doc, &library, self.plugins.home());
        let scanning = matches!(self.plugins.view().scan, ScanView::Running { .. });
        let plugin = (missing.iter()).find_map(|m| match m {
            MissingView::Plugin {
                key,
                name,
                role: PluginRole::Instrument,
                ..
            } if !scanning => Some((key.clone(), name.clone())),
            _ => None,
        });
        if pack > 0 {
            self.notice = Some(StatusMessage::SoundsMissing { tracks: pack });
        } else if let Some(MissingView::Sf2 { file, tracks }) = missing.into_iter().next() {
            self.notice = Some(StatusMessage::Sf2Missing { file, tracks });
        } else if let Some((key, name)) = plugin {
            let tracks = (doc.tracks.iter())
                .filter(|t| matches!(&t.instrument, InstrumentRef::Plugin(p) if *p.key == *key))
                .count() as u32;
            self.notice = Some(StatusMessage::PluginMissing { name, tracks });
        }
    }

    /// Lays the bands out for the window as it is now. A band folded here was folded because
    /// the window, in px at 100 %, got lower.
    fn fit_window(&mut self) {
        if let Some(band) = self.fit_bands() {
            self.notice = Some(StatusMessage::BandFoldedWindowLow { band });
        }
    }

    /// Lays the bands out for the window and the zoom as they are now. Returns the band folded.
    fn fit_bands(&mut self) -> Option<Band> {
        self.bands
            .set_window(window(self.viewport, self.scale))
            .map(bottom_band)
    }

    /// The smallest window at this zoom fits the screen. 100 is the smallest zoom there is, so
    /// it always counts as fitting; so does everything while the screen is not known.
    fn scale_fits(&self, percent: u16) -> bool {
        let min = layout::min_window(percent);
        percent == SCALES[0]
            || self.screen.is_none_or(|(width, height)| {
                min.width <= f64::from(width) && min.height <= f64::from(height)
            })
    }

    fn lower_scale_for_screen(&mut self, effects: &mut Vec<Effect>) {
        if self.scale_fits(self.scale) {
            return;
        }
        let from = self.scale;
        let fitting = SCALES
            .iter()
            .rev()
            .find(|&&s| s < from && self.scale_fits(s));
        self.scale = fitting.copied().unwrap_or(SCALES[0]);
        self.notice = Some(StatusMessage::ScaleLoweredForScreen {
            from,
            to: self.scale,
        });
        self.fit_window();
        effects.push(Effect::SaveSettings(self.settings()));
    }

    fn choose(&mut self, output: String, effects: &mut Vec<Effect>) {
        self.chosen_name = self.endpoint(&output).map(|e| e.name.clone());
        self.chosen = Some(output);
        self.chosen_was_ready = false;
        self.failed = None;
        self.audio_gave_up = false;
        self.preview = None;
        effects.push(Effect::SaveSettings(self.settings()));
    }

    /// Brings everything that follows from the rest back in line, and asks for the stream the
    /// state now calls for.
    fn settle(&mut self, effects: &mut Vec<Effect>) {
        if !self.chooser_shown() {
            self.highlighted = None;
        } else if self
            .highlighted
            .as_ref()
            .is_none_or(|id| self.choosable(id).is_none())
        {
            self.highlighted = self
                .outputs()
                .iter()
                .find(|e| is_choosable(e))
                .map(|e| e.id.clone());
        }

        let previewing = self.chooser_shown() || self.dialog.is_some();
        if self
            .preview
            .as_ref()
            .is_some_and(|id| !previewing || self.choosable(id).is_none())
        {
            self.preview = None;
        }

        if let Some(name) = self.chosen_endpoint().map(|e| e.name.clone()) {
            self.chosen_name = Some(name);
        }
        if self.chosen_ready() {
            self.chosen_was_ready = true;
        }

        let target = self
            .preview
            .as_ref()
            .and_then(|id| self.choosable(id))
            .or_else(|| self.chosen_endpoint().filter(|_| self.chosen_ready()))
            .and_then(|e| e.node_name.clone())
            .filter(|node| self.failed.as_ref() != Some(node))
            .filter(|_| !self.audio_system_down);
        if target != self.wanted {
            // The sound goes somewhere else, or nowhere: nothing is auditioned across that.
            self.play.end_audition(effects);
            match &target {
                Some(node_name) => effects.push(Effect::OpenOutput {
                    node_name: node_name.clone(),
                }),
                None => effects.push(Effect::CloseOutput),
            }
            self.wanted = target;
            self.open = None;
            self.actual_frames = None;
            self.latencies.clear();
            self.xruns = 0;
            self.input_reset(effects);
        }
    }

    /// Says that the PC fixes the buffer when a song was opened and when the fixed size changed
    /// (spec 6). Only for the chosen output's stream, not for a test tone on another one.
    fn watch_buffer(&mut self) {
        if self.chosen.is_none() || self.preview.is_some() {
            return;
        }
        let (Some(actual), Some(rate)) = (
            self.actual_frames,
            self.open.as_ref().map(|o| o.sample_rate),
        ) else {
            return;
        };
        let forced = self.devices.as_ref().and_then(|d| d.clock.force_quantum);
        let requested = self.sound.buffer;
        let fixed = (actual != requested && forced == Some(actual)).then_some(actual);
        if fixed.is_some() && (self.buffer_unsaid || fixed != self.buffer_fixed_seen) {
            self.notice = Some(StatusMessage::BufferFixed {
                actual,
                requested,
                latency_ms: estimate_ms(actual, rate),
                latency_ms_at_requested: estimate_ms(requested, rate),
            });
        }
        self.buffer_unsaid = false;
        self.buffer_fixed_seen = fixed;
    }

    fn settings(&self) -> Settings {
        Settings {
            output: self.chosen.clone(),
            output_name: self.chosen_name.clone(),
            scale: self.scale,
            metronome: self.play.metronome(),
            count_in_bars: self.play.count_in_bars(),
            velocity_fixed: self.velocity_fixed.on,
            fixed_velocity: self.velocity_fixed.value,
            favorites: self.favorites.clone(),
            plugin_places: self.plugins.added_places().to_vec(),
            shelf: self.catalog.settings().clone(),
            sound: self.sound.clone(),
            pack_fetch: self.pack.wanted,
        }
    }

    fn shelf_view(&self, library: &LibraryView) -> ShelfView {
        let used = added::plugin_keys(&self.play.song().doc);
        self.play.shelf_view(
            InstrumentKind::ALL
                .into_iter()
                .filter(|&k| self.favorites.contains(&instruments::builtin_of(k)))
                .collect(),
            (
                self.catalog.favorite_sounds(&used),
                self.catalog.favorite_effects(&used),
            ),
            library,
            self.plugins.home(),
        )
    }

    /// 「あとから入れたもの」: the SF2 files that were added, and the scanned plugins as the
    /// catalog last took them in.
    fn library_view(&self) -> LibraryView {
        let used = added::plugin_keys(&self.play.song().doc);
        self.catalog.view(&used, self.play.pack_missing())
    }

    /// The plugins of the song that sound, by the ticket each was loaded under.
    pub fn plugin_tickets(&self) -> Vec<(PluginPlace, u64)> {
        self.play.plugin_tickets()
    }

    /// The parameters of the plugin the parameters panel shows, when `table` is still the
    /// number of its table.
    pub fn plugin_params(&self, table: u32) -> Option<ParamTable> {
        self.play.plugin_params(table)
    }

    /// When an [`Input::Tick`] is wanted next, as the time given with `set_now`: a window
    /// that is slow to open is said then, or parameter values held back are sent.
    pub fn wake_at_us(&self) -> Option<u64> {
        [self.play.wake_at_us(), self.fader_wake_us()]
            .into_iter()
            .flatten()
            .min()
    }

    /// A sample plays: [`Input::Tick`] is wanted now and then, to end it.
    pub fn sample_playing(&self) -> bool {
        self.play.sample_playing()
    }

    /// The instruments, the play target and the mixer, for an engine that has none yet.
    pub fn engine_seats(&mut self) -> Vec<EngineOp> {
        self.play.engine_seats()
    }

    /// The engine's track of each track of the song, in the order of the song.
    pub fn track_seats(&self) -> Vec<Option<usize>> {
        self.play.track_seats()
    }

    /// The velocity every key played takes now, if it is fixed.
    pub fn fixed_velocity(&self) -> Option<u8> {
        self.velocity_fixed.on.then_some(self.velocity_fixed.value)
    }

    /// No clip has been in the song yet, the chosen output plays, nothing is being recorded, and
    /// no other hint is up.
    fn empty_hint(&self) -> bool {
        self.hint.is_none()
            && !self.empty_dismissed
            && self.open.is_some()
            && self.preview.is_none()
            && !self.chooser_shown()
            && !self.play.is_recording()
            && !self.play.had_clip()
    }

    fn outputs(&self) -> &[Endpoint] {
        self.devices.as_ref().map_or(&[], |d| &d.outputs)
    }

    fn endpoint(&self, id: &str) -> Option<&Endpoint> {
        self.outputs().iter().find(|e| e.id == id)
    }

    fn choosable(&self, id: &str) -> Option<&Endpoint> {
        self.endpoint(id).filter(|e| is_choosable(e))
    }

    fn chosen_endpoint(&self) -> Option<&Endpoint> {
        self.endpoint(self.chosen.as_deref()?)
    }

    fn chosen_ready(&self) -> bool {
        self.chosen_endpoint()
            .is_some_and(|e| is_choosable(e) && e.availability != meloet_devices::Availability::No)
    }

    fn chosen_state(&self) -> Option<ChosenState> {
        self.chosen.as_ref()?;
        self.devices.as_ref()?;
        Some(if self.chosen_ready() {
            ChosenState::Ready
        } else if self.chosen_was_ready {
            ChosenState::Missing
        } else {
            ChosenState::NotPlugged
        })
    }

    /// The keyboard played last; before any key, the one plugged in last.
    fn active_keyboard(&self) -> Option<&Keyboard> {
        self.played
            .as_ref()
            .and_then(|id| self.keyboards.iter().find(|k| &k.id == id))
            .or_else(|| self.keyboards.last())
    }

    fn chooser_shown(&self) -> bool {
        self.chosen.is_none() && !self.keyboards.is_empty()
    }

    pub fn view(&self) -> AppState {
        let audio = self.audio_view();
        let chosen_state = self.chosen_state();
        let status = self.status(chosen_state);
        // Choosing the output comes first; the recovery hint waits behind it, in the same place.
        let recovery = self.recovery_view().filter(|_| !self.chooser_shown());
        let library = self.library_view();
        let timeline = self.play.timeline_view();
        let layout = self.bands.layout_with(timeline.open);
        AppState {
            song: song_view(
                &self.play,
                self.song_file(),
                &library,
                &self.catalog,
                self.input_channels(),
            ),
            timeline,
            automation: self.play.automation_view(),
            history: self.play.history_view(),
            transport: {
                let mut transport = self.play.transport_view(self.open.is_some());
                transport.can_record &= !self.measuring();
                transport
            },
            arrange: ArrangeView {
                tall_track: self.tall_track_view(),
                ..self.play.arrange_view()
            },
            piano_roll: self.play.piano_roll_view(),
            workspace: WorkspaceView {
                scale: self.scale,
                viewport_width: self.viewport.0,
                viewport_height: self.viewport.1,
                unfit_scales: SCALES
                    .iter()
                    .copied()
                    .filter(|&s| !self.scale_fits(s))
                    .collect(),
                layout: layout_view(&layout),
                settings: self.dialog.clone(),
                chooser: self.chooser_shown().then(|| ChooserView {
                    highlighted: self.highlighted.clone(),
                }),
                hint: self
                    .hint
                    .or_else(|| self.empty_hint().then_some(Hint::EmptyArrange))
                    .filter(|_| recovery.is_none()),
                song_dialog: self.song_dialog_view(),
                recovery,
                file_drop: self.file_drop.clone(),
                latency_dialog: self.latency_dialog_view(),
            },
            shelf: self.shelf_view(&library),
            library,
            params: self.params_view(),
            plugins: self.plugins.view(),
            windows: self.play.windows_view(),
            keyboards: KeyboardsView {
                list: self
                    .keyboards
                    .iter()
                    .map(|k| KeyboardView {
                        id: k.id.clone(),
                        name: k.name.clone(),
                        port_name: k.port_name.clone(),
                        address: k.address(),
                    })
                    .collect(),
                active: self.active_keyboard().map(|k| k.id.clone()),
                lost: self.lost.clone(),
                input_stopped: self.midi_restarting.is_some(),
                velocity_fixed: self.velocity_fixed,
            },
            outputs: OutputsView {
                list: self.outputs().iter().map(endpoint_view).collect(),
                chosen: self.chosen.clone(),
                chosen_state,
            },
            inputs: self.inputs_view(),
            audio,
            audio_work: self.play.audio_work_view(),
            export: self.export_view(),
            sounds: self.sounds_view(),
            status: StatusView { message: status },
        }
    }

    /// The row that is 160 high, while its track is still a vocal track of this song.
    fn tall_track_view(&self) -> Option<u32> {
        let (serial, id) = self.tall_track?;
        let track = self.play.song().doc.track(meloet_model::TrackId(id))?;
        (serial == self.play.serial() && track.kind.is_vocal()).then_some(id)
    }

    fn audio_view(&self) -> AudioView {
        let rate = self.open.as_ref().map(|o| o.sample_rate);
        let measured = (!self.latencies.is_empty()).then(|| {
            let mean = self.latencies.iter().sum::<f32>() / self.latencies.len() as f32;
            (mean * 10.0).round() / 10.0
        });
        AudioView {
            running: self.open.is_some(),
            system: AUDIO_SYSTEM.into(),
            sample_rate: rate,
            requested_frames: self.sound.buffer,
            actual_frames: self.actual_frames,
            forced_quantum: self.devices.as_ref().and_then(|d| d.clock.force_quantum),
            latency_ms: measured.or_else(|| Some(estimate_ms(self.actual_frames?, rate?))),
            latency_ms_at_requested: rate.map(|r| estimate_ms(self.sound.buffer, r)),
            output_device: self.open.as_ref().map(|o| o.node_name.clone()),
            midi_input: self.active_keyboard().map(Keyboard::label),
            buffer_choices: BUFFER_CHOICES.to_vec(),
            requested_sample_rate: self.sound.sample_rate,
            sample_rate_choices: SAMPLE_RATE_CHOICES.to_vec(),
            record_format: self.sound.record_format,
            latency: self.latency_view(),
        }
    }

    /// The first line of the table that applies. A notice comes after the lines that ask the
    /// user to do something about a device, and before the rest.
    fn status(&self, chosen_state: Option<ChosenState>) -> Option<StatusMessage> {
        let output = || self.output_name();
        if self.latency.is_some() {
            return Some(StatusMessage::LatencyMeasureOpen);
        }
        if self.dialog.is_some() {
            return Some(StatusMessage::SettingsOpen);
        }
        if self.export.dialog_open() {
            return Some(StatusMessage::ExportDialogOpen);
        }
        if self.plugins.stopped_open() {
            // What "もう一度ためす" came to is said over the open dialog.
            return match &self.notice {
                Some(
                    m @ (StatusMessage::ScanRetryAdded { .. }
                    | StatusMessage::ScanRetryStopped { .. }),
                ) => Some(m.clone()),
                _ => Some(StatusMessage::ScanStoppedOpen),
            };
        }
        if let Some(
            m @ (StatusMessage::RecordingStoppedAudioSystem { .. }
            | StatusMessage::RecordingStoppedOutputMissing { .. }),
        ) = &self.notice
        {
            return Some(m.clone());
        }
        if self.audio_system_down {
            return Some(StatusMessage::AudioSystemStopped);
        }
        let gave_up = self.audio_gave_up && self.failed.is_some() && self.preview.is_none();
        match chosen_state {
            Some(ChosenState::Missing) => {
                return Some(StatusMessage::OutputMissing { output: output() });
            }
            Some(ChosenState::Ready) if gave_up => {
                return Some(StatusMessage::OutputMissing { output: output() });
            }
            Some(ChosenState::NotPlugged) => {
                return Some(StatusMessage::OutputNotPlugged { output: output() });
            }
            Some(ChosenState::Ready) | None => {}
        }
        if self.midi_restarting.is_some() {
            return Some(StatusMessage::KeyboardInputStopped);
        }
        let active = self.active_keyboard();
        if let (None, Some(keyboard)) = (active, &self.lost) {
            return Some(StatusMessage::KeyboardLost {
                keyboard: keyboard.clone(),
            });
        }
        // Said until the plugin is loaded again or removed, also over a recording: it goes
        // on, and the person should know why the track is silent. What the person just
        // asked to see is said once over it; the next operation brings it back.
        if !self.notice.as_ref().is_some_and(asked_to_see)
            && let Some((slot, name)) = self.play.stopped_plugin()
        {
            return Some(StatusMessage::PluginStopped { name, slot });
        }
        if let Some(drop) = &self.file_drop {
            return Some(match drop {
                FileDropView {
                    audio: 1.., sf2, ..
                } => StatusMessage::AudioDragging {
                    audio: drop.audio,
                    sf2: *sf2,
                },
                FileDropView { addable: true, .. } => StatusMessage::Sf2Dragging,
                _ => StatusMessage::NotSf2Dragging,
            });
        }
        if let Some(m) = self.play.moving_status() {
            return Some(m);
        }
        if let Some(m) = self.play.writing_status() {
            return Some(m);
        }
        if let Some(m) = &self.saving_audio {
            return Some(m.clone());
        }
        // Before a notice: it answers the key just played, and no command puts it away.
        if let Some(instrument) = &self.loading_said {
            return Some(match instrument {
                InstrumentRef::Builtin(b) => StatusMessage::InstrumentLoading {
                    instrument: instruments::kind_of(*b),
                },
                other => StatusMessage::SoundLoading {
                    sound: added::sound_name(other),
                },
            });
        }
        if let Some(m) = self.play.window_status() {
            return Some(m);
        }
        if self.notice.is_some() {
            return self.notice.clone();
        }
        if let Some(m) = self.play.audition_status() {
            return Some(m);
        }
        if let Some(m) = self.play.sample_status() {
            return Some(m);
        }
        let Some(active) = active else {
            return Some(StatusMessage::SearchingKeyboard);
        };
        if self.chosen.is_none() {
            return Some(StatusMessage::KeyboardFound {
                keyboard: active.name.clone(),
            });
        }
        if self.pack_asking() {
            return Some(StatusMessage::PackAsking);
        }
        if self.new_song_unsaid {
            let song = &self.play.song().doc;
            return Some(StatusMessage::NewSong {
                tempo_bpm: song.timeline.tempo.bpm(),
                meter_num: song.timeline.meter.num(),
                meter_den: song.timeline.meter.den(),
                track: self.play.record_target(),
            });
        }
        let redo = self.play.redo_count();
        if redo > 0 && !self.play.busy() {
            return Some(StatusMessage::UndoPending { count: redo });
        }
        if self.bands.side_is_open(SidePanel::Params)
            && let Some(m) = self.play.param_status()
        {
            return Some(m);
        }
        self.play.summary()
    }
}

fn estimate_ms(frames: u32, sample_rate: u32) -> f32 {
    LATENCY_BUFFERS * frames as f32 / sample_rate.max(1) as f32 * 1000.0
}

/// meloet can play through it without changing a PC-wide setting: it has a node, and the PC has
/// its Route on now (an endpoint without a Route counts as on).
fn is_choosable(endpoint: &Endpoint) -> bool {
    endpoint.unusable().is_none()
}

/// The viewport is CSS px; the band rules work in px at 100 %.
fn window(viewport: (f32, f32), scale: u16) -> WindowSize {
    let factor = 100.0 / f64::from(scale);
    WindowSize {
        width: f64::from(viewport.0) * factor,
        height: f64::from(viewport.1) * factor,
    }
}

fn band_of(band: Band) -> layout::Band {
    match band {
        Band::PianoRoll => BottomBand::PianoRoll.into(),
        Band::Mixer => BottomBand::Mixer.into(),
        Band::History => BottomBand::History.into(),
        Band::Shelf => SidePanel::Shelf.into(),
        Band::Params => SidePanel::Params.into(),
    }
}

fn bottom_band(band: BottomBand) -> Band {
    match band {
        BottomBand::PianoRoll => Band::PianoRoll,
        BottomBand::Mixer => Band::Mixer,
        BottomBand::History => Band::History,
    }
}

fn layout_view(layout: &layout::Layout) -> LayoutView {
    let side = |panel: &SidePanel| match panel {
        SidePanel::Shelf => Band::Shelf,
        SidePanel::Params => Band::Params,
    };
    LayoutView {
        meter: match layout.meter {
            layout::MeterSize::Full => MeterSize::Full,
            layout::MeterSize::Slim => MeterSize::Slim,
        },
        arrange_height: layout.arrange_height as f32,
        arrange_overview: layout.arrange_overview,
        lower: layout
            .bottom
            .iter()
            .map(|open| LowerBandView {
                band: bottom_band(open.band),
                height: open.height as f32,
            })
            .collect(),
        right_open: layout.side_open.iter().map(side).collect(),
        right_front: layout.side_front.as_ref().map(side),
        show_back_hint: layout.show_back_hint,
    }
}

fn sf2_keys(info: &Sf2Info) -> impl Iterator<Item = Sf2Key> + '_ {
    let file: Arc<str> = info.file.as_str().into();
    info.presets.iter().map(move |p| Sf2Key {
        file: file.clone(),
        bank: p.bank,
        program: p.program,
    })
}

fn song_view(
    play: &Play,
    file: SongFile,
    library: &LibraryView,
    catalog: &Catalog,
    input_channels: Option<u16>,
) -> SongView {
    let song = play.song();
    let doc = &song.doc;
    let sounds = play.available_sounds();
    SongView {
        serial: play.serial(),
        name: doc.name.clone(),
        undo_count: play.undo_count(),
        redo_count: play.redo_count(),
        note_count: play.piano_roll_notes(),
        tracks: doc
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| TrackView {
                id: t.id.0,
                number: i as u32 + 1,
                name: t.name.clone(),
                color: t.color.get(),
                kind: match t.kind {
                    TrackKind::Keys => TrackKindView::Keys,
                    TrackKind::Vocal { input } => TrackKindView::Vocal {
                        input: vocal_input_view(input),
                        input_ready: input_channels.is_some_and(|n| input.fits(n)),
                        armed: song.session.armed_vocals.contains(&t.id),
                        monitor: song.session.monitored.contains(&t.id),
                    },
                },
                sound: added::sound_view(
                    &t.instrument,
                    play.plugin_state(PluginPlace::Instrument(t.id)),
                ),
                stand_in: added::stand_in(&t.instrument, library, &sounds).or_else(|| {
                    (play.plugin_failed(PluginPlace::Instrument(t.id)))
                        .then_some(StandIn::PluginUnreadable)
                }),
                drum_keys: match &t.instrument {
                    InstrumentRef::Sf2(s) if s.family() == GmFamily::DrumKit => catalog
                        .sf2_preset(&s.file, s.bank, s.program)
                        .map(|(_, preset)| gm_kit_rows(&preset.keys)),
                    _ => None,
                },
                gain_db: gain_view(t.channel.gain),
                pan: t.channel.pan.get(),
                sends: t.sends.map(gain_view),
                send_defaults: meloet_model::mixer::new_track_sends(&t.instrument).map(gain_view),
                slots: play.slots_of(StripId::Track(t.id), &t.channel.inserts),
                muted: t.muted,
                soloed: play.is_soloed(t.id),
                lanes: play.lanes_of(t.id),
                clips: play.clips_of(t.id),
            })
            .collect(),
        returns: play.returns_view(),
        master: play.master_view(),
        play_target: song.session.play_target.map(|t| t.0),
        keys_target: song.keys_target().map(|t| t.0),
        loop_range: play.loop_span(),
        loop_on: song.session.loop_on,
        file,
    }
}

pub(crate) fn model_vocal_input(input: meloet_protocol::VocalInput) -> VocalInput {
    match input {
        meloet_protocol::VocalInput::Mono1 => VocalInput::Mono1,
        meloet_protocol::VocalInput::Mono2 => VocalInput::Mono2,
        meloet_protocol::VocalInput::Stereo12 => VocalInput::Stereo12,
    }
}

pub(crate) fn vocal_input_view(input: VocalInput) -> meloet_protocol::VocalInput {
    match input {
        VocalInput::Mono1 => meloet_protocol::VocalInput::Mono1,
        VocalInput::Mono2 => meloet_protocol::VocalInput::Mono2,
        VocalInput::Stereo12 => meloet_protocol::VocalInput::Stereo12,
    }
}

fn endpoint_view(endpoint: &Endpoint) -> EndpointView {
    use meloet_devices as d;
    EndpointView {
        id: endpoint.id.clone(),
        name: endpoint.name.clone(),
        friendly: endpoint.friendly.map(|f| match f {
            d::Friendly::Headphones => Friendly::Headphones,
            d::Friendly::Hdmi => Friendly::Hdmi,
            d::Friendly::LineOut => Friendly::LineOut,
            d::Friendly::RearMic => Friendly::RearMic,
        }),
        availability: match endpoint.availability {
            d::Availability::Yes => Availability::Yes,
            d::Availability::No => Availability::No,
            d::Availability::Unknown => Availability::Unknown,
        },
        link: match endpoint.link {
            d::LinkKind::Jack => LinkKind::Jack,
            d::LinkKind::Plug => LinkKind::Plug,
        },
        channels: endpoint.channels.map(u16::from),
        node_name: endpoint.node_name.clone(),
        device: endpoint.device_description.clone(),
        unusable: endpoint.unusable().map(|u| match u {
            d::Unusable::SwitchedByPc => Unusable::SwitchedByPc,
            d::Unusable::StoppedByPc => Unusable::StoppedByPc,
        }),
    }
}

/// The notices that answer "show me": the summary of a mixer just opened (it names the
/// stopped plugins itself) and the word of a plugin's window just opened.
fn asked_to_see(notice: &StatusMessage) -> bool {
    matches!(
        notice,
        StatusMessage::MixerPassesBy { .. }
            | StatusMessage::PluginWindowsOpen { .. }
            | StatusMessage::PluginWindowLarge { .. }
    )
}

#[cfg(test)]
pub(crate) mod added_tests;
#[cfg(test)]
mod audio_tests;
#[cfg(test)]
mod pack_tests;
#[cfg(test)]
mod plugin_tests;
#[cfg(test)]
mod speed;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod vocal_tests;
