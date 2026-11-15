//! The pure rules: an input comes in, the state changes, and a list of things to do comes back.
//! Nothing here touches a device, a file or a thread.

mod songs;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use meloet_devices::{DevicesPicture, Endpoint, sort_for_choice};
use meloet_midi::Keyboard;
use meloet_model::{InstrumentRef, Song};
use meloet_protocol::{
    AppState, AudioView, Availability, Band, ChooserView, ChosenState, Command, EndpointView,
    Friendly, Hint, InputsView, InstrumentKind, KeyboardView, KeyboardsView, LayoutView, LinkKind,
    LiveNotes, LiveNotesSnapshot, LowerBandView, MeterSize, OutputsView, SettingsDialogView,
    SongFile, SongView, StatusMessage, StatusView, TrackView, Unusable, WorkspaceView,
};

use crate::layout::{self, Bands, BottomBand, SidePanel, WindowSize};
use crate::play::{Cut, EngineOp, Heard, Play, RecordReport};
use crate::settings::{SCALES, Settings};

/// The buffer size meloet asks for. What really runs is `AudioView::actual_frames`.
pub const REQUESTED_FRAMES: u32 = 256;
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
    /// Nothing but the time and the heard position given with [`Model::set_now`].
    Tick,
    /// From the shell: the window's close button, or anything else that asks to end.
    CloseRequested,
    /// The answer to [`Effect::PickSongFile`]; `None` when it was put away.
    SongFilePicked(Option<PathBuf>),
    /// The answer to [`Effect::PickFolder`]; `None` when it was put away.
    FolderPicked(Option<PathBuf>),
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
    /// Only while a stream is open; [`Input::AudioOpened`] sends everything again.
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
    /// Read the song at `path`, or with `previous` the copy kept beside it. Answer with
    /// [`Input::SongRead`] or [`Input::SongUnreadable`].
    ReadSong {
        path: PathBuf,
        previous: bool,
    },
    /// meloet may end now.
    Exit,
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
}

struct OpenStream {
    node_name: String,
    sample_rate: u32,
}

impl Model {
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
            audio_system_down: false,
            midi_restarting: None,
            files: songs::Files::default(),
        }
    }

    /// Whether [`Input::SoundHeard`] would change anything now.
    pub fn wants_sound(&self) -> bool {
        self.hint_armed && !self.hint_dismissed
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
            Input::Devices(mut picture) => {
                sort_for_choice(&mut picture.outputs);
                self.devices = Some(picture);
                self.failed = None;
                self.audio_retries = 0;
                self.audio_system_down = false;
            }
            Input::AudioOpened {
                node_name,
                sample_rate,
            } => {
                if self.wanted.as_deref() == Some(&node_name) {
                    self.open = Some(OpenStream {
                        node_name,
                        sample_rate,
                    });
                    self.play.stream_opened(&mut effects);
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
            Input::Tick => {}
            Input::CloseRequested
            | Input::SongFilePicked(_)
            | Input::FolderPicked(_)
            | Input::SongSaved { .. }
            | Input::SongNotSaved
            | Input::SongRead { .. }
            | Input::SongUnreadable { .. } => self.song_input(input, &mut effects),
        }
        self.settle(&mut effects);
        if self.open.is_none() {
            let cut = if self.audio_system_down {
                Cut::AudioSystem
            } else {
                Cut::Output
            };
            let was_recording = self.play.is_recording();
            let take = self.play.stream_gone(&mut effects);
            if let (true, Some(take)) = (was_recording, take) {
                self.notice = Some(match cut {
                    Cut::AudioSystem => StatusMessage::RecordingStoppedAudioSystem { take },
                    Cut::Output => StatusMessage::RecordingStoppedOutputMissing {
                        output: self.output_name(),
                        take,
                    },
                });
            }
        }
        self.watch_buffer();
        self.close_when_recorded(&mut effects);
        if tone_on.is_some() && tone_on == self.wanted {
            effects.push(Effect::TestTone);
        }
        effects
    }

    /// Xruns of the open stream since it was opened.
    pub fn xruns(&self) -> u64 {
        self.xruns
    }

    /// The time and what the listener hears, before each input. `heard` only while a stream
    /// is open.
    pub fn set_now(&mut self, t_us: u64, heard: Option<Heard>) {
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
        }
    }

    /// Returns the node a test tone was asked on.
    fn command(&mut self, command: Command, effects: &mut Vec<Effect>) -> Option<String> {
        // A resize is not something the user does to answer a notice.
        if !matches!(command, Command::SetViewport { .. }) {
            self.notice = None;
            self.new_song_unsaid = false;
        }
        let can_play = self.open.is_some();
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
                    self.fit_window();
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::ToggleBand { band } => {
                if let Some(folded) = self.bands.toggle(band_of(band)) {
                    self.notice = Some(StatusMessage::BandFoldedToOpen {
                        opened: band,
                        folded: bottom_band(folded),
                    });
                }
            }
            Command::GoArrange => self.bands.go_arrange(),
            Command::Back => {
                if !self.cancel_song_dialog() && self.dialog.take().is_none() {
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
                        track: self.play.armed(),
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
                    }
                }
                return Some(node);
            }
            Command::DismissHint => {
                if self.hint.take().is_some() {
                    self.hint_dismissed = true;
                } else if self.empty_hint() {
                    self.empty_dismissed = true;
                }
            }
            Command::PlayStop => self.play.play_stop(can_play, effects),
            Command::Record => self.notice = self.play.record(can_play, effects),
            Command::GoToStart => self.play.locate(0, effects),
            Command::Locate { tick } => self.play.locate(tick, effects),
            Command::SetTempo { bpm } => self.play.set_tempo(bpm, effects),
            Command::SetMeter { num, den } => self.play.set_meter(num, den, effects),
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
            Command::ToggleArm { track } => self.play.toggle_arm(track),
            Command::SelectClip { clip } => self.play.select_clip(clip),
            Command::DeleteSelected => self.play.delete_selected(effects),
            Command::StepTake { delta } => self.notice = self.play.step_take(delta, effects),
            Command::Undo => self.play.undo(effects),
            Command::Redo => self.play.redo(effects),
            Command::NewSong => self.new_song(effects),
            Command::OpenSong => self.open_song(effects),
            Command::SaveSong => self.save_song(effects),
            Command::AnswerSongDialog { dialog, answer } => {
                self.answer_song_dialog(dialog, answer, effects);
            }
            Command::SelectNotes { .. }
            | Command::SelectAllNotes
            | Command::ClearNoteSelection
            | Command::AddNote { .. }
            | Command::MoveNotes { .. }
            | Command::ResizeNotes { .. }
            | Command::DeleteNotes { .. }
            | Command::ChangeVelocity { .. }
            | Command::CopyNotes { .. }
            | Command::CutNotes { .. }
            | Command::PasteNotes { .. }
            | Command::SetGrid { .. }
            | Command::SetMusicalScale { .. }
            | Command::Audition { .. }
            | Command::AuditionOff
            | Command::SetQuantize { .. }
            | Command::Quantize { .. }
            | Command::Unquantize { .. }
            | Command::SetNotesField { .. }
            | Command::SelectControls { .. }
            | Command::AddControl { .. }
            | Command::MoveControls { .. }
            | Command::DeleteControls { .. }
            | Command::MoveClip { .. }
            | Command::DuplicateClip { .. }
            | Command::TrimClip { .. } => {}
        }
        None
    }

    /// Lays the bands out for the window as it is now. A band folded here was folded because
    /// the window, in px at 100 %, got lower.
    fn fit_window(&mut self) {
        if let Some(folded) = self.bands.set_window(window(self.viewport, self.scale)) {
            self.notice = Some(StatusMessage::BandFoldedWindowLow {
                band: bottom_band(folded),
            });
        }
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
        let fixed = (actual != REQUESTED_FRAMES && forced == Some(actual)).then_some(actual);
        if fixed.is_some() && (self.buffer_unsaid || fixed != self.buffer_fixed_seen) {
            self.notice = Some(StatusMessage::BufferFixed {
                actual,
                requested: REQUESTED_FRAMES,
                latency_ms: estimate_ms(actual, rate),
                latency_ms_at_requested: estimate_ms(REQUESTED_FRAMES, rate),
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
        }
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
        AppState {
            song: song_view(&self.play, self.song_file()),
            transport: self.play.transport_view(self.open.is_some()),
            arrange: self.play.arrange_view(),
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
                layout: layout_view(&self.bands.layout()),
                settings: self.dialog.clone(),
                chooser: self.chooser_shown().then(|| ChooserView {
                    highlighted: self.highlighted.clone(),
                }),
                hint: self
                    .hint
                    .or_else(|| self.empty_hint().then_some(Hint::EmptyArrange)),
                song_dialog: self.song_dialog_view(),
            },
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
            },
            outputs: OutputsView {
                list: self.outputs().iter().map(endpoint_view).collect(),
                chosen: self.chosen.clone(),
                chosen_state,
            },
            inputs: InputsView {
                list: self
                    .devices
                    .iter()
                    .flat_map(|d| &d.inputs)
                    .map(endpoint_view)
                    .collect(),
                chosen: None,
            },
            audio,
            status: StatusView { message: status },
        }
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
            requested_frames: REQUESTED_FRAMES,
            actual_frames: self.actual_frames,
            forced_quantum: self.devices.as_ref().and_then(|d| d.clock.force_quantum),
            latency_ms: measured.or_else(|| Some(estimate_ms(self.actual_frames?, rate?))),
            latency_ms_at_requested: rate.map(|r| estimate_ms(REQUESTED_FRAMES, r)),
            output_device: self.open.as_ref().map(|o| o.node_name.clone()),
            midi_input: self.active_keyboard().map(Keyboard::label),
        }
    }

    /// The first line of the table that applies. A notice comes after the lines that ask the
    /// user to do something about a device, and before the rest.
    fn status(&self, chosen_state: Option<ChosenState>) -> Option<StatusMessage> {
        let output = || self.output_name();
        if self.dialog.is_some() {
            return Some(StatusMessage::SettingsOpen);
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
        match chosen_state {
            Some(ChosenState::Missing) => {
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
        if let Some(m) = self.play.moving_status() {
            return Some(m);
        }
        if self.notice.is_some() {
            return self.notice.clone();
        }
        let Some(active) = active else {
            return Some(StatusMessage::SearchingKeyboard);
        };
        if self.chosen.is_none() {
            return Some(StatusMessage::KeyboardFound {
                keyboard: active.name.clone(),
            });
        }
        if self.new_song_unsaid {
            let song = &self.play.song().doc;
            return Some(StatusMessage::NewSong {
                tempo_bpm: song.tempo.bpm(),
                meter_num: song.meter.num(),
                meter_den: song.meter.den(),
                track: self.play.armed(),
            });
        }
        let redo = self.play.redo_count();
        if redo > 0 && !self.play.busy() {
            return Some(StatusMessage::UndoPending { count: redo });
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

fn song_view(play: &Play, file: SongFile) -> SongView {
    let song = play.song();
    let doc = &song.doc;
    SongView {
        name: doc.name.clone(),
        tempo_bpm: doc.tempo.bpm(),
        meter_num: doc.meter.num(),
        meter_den: doc.meter.den(),
        tempo_changes: doc.tempo_changes.len() as u32,
        undo_count: play.undo_count(),
        redo_count: play.redo_count(),
        note_count: play.piano_roll_notes(),
        tracks: doc
            .tracks
            .iter()
            .map(|t| TrackView {
                number: t.id.0,
                name: t.name.clone(),
                color: t.color.get(),
                instrument: match t.instrument {
                    InstrumentRef::Placeholder => InstrumentKind::Placeholder,
                },
                gain_db: t.gain_db as f32,
                armed: song.session.armed == Some(t.id),
                clips: play.clips_of(t.id),
            })
            .collect(),
        play_target: Some(song.session.play_target.0),
        loop_range: play.loop_span(),
        loop_on: song.session.loop_on,
        file,
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
        node_name: endpoint.node_name.clone(),
        device: endpoint.device_description.clone(),
        unusable: endpoint.unusable().map(|u| match u {
            d::Unusable::SwitchedByPc => Unusable::SwitchedByPc,
            d::Unusable::StoppedByPc => Unusable::StoppedByPc,
        }),
    }
}

#[cfg(test)]
mod tests;
