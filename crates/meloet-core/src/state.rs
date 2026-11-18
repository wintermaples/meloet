//! The pure rules: an input comes in, the state changes, and a list of things to do comes back.
//! Nothing here touches a device, a file or a thread.

mod recovery;
mod songs;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use meloet_devices::{DevicesPicture, Endpoint, sort_for_choice};
use meloet_midi::Keyboard;
use meloet_model::{BuiltinInstrument, InstrumentRef, Note, Song};
use meloet_protocol::{
    AppState, AudioView, Availability, Band, ChooserView, ChosenState, Command, EndpointView,
    Friendly, Hint, InputsView, InstrumentKind, KeyboardView, KeyboardsView, LayoutView, LinkKind,
    LiveNotes, LiveNotesSnapshot, LowerBandView, MeterSize, OutputsView, QuantizeSettings,
    RecoveryCause, SettingsDialogView, ShelfView, SongFile, SongView, StatusMessage, StatusView,
    TrackView, Unusable, VelocityFixed, WorkspaceView,
};

use crate::instruments;

use crate::layout::{self, Bands, BottomBand, FoldWhy, SidePanel, WindowSize};
use crate::play::{Cut, EngineOp, Heard, Play, RecordReport};
use crate::settings::{SCALES, Settings};

pub use recovery::{QUIET_US, RecoveryStart};

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
    /// the instruments and the play target are for the engine at any time.
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
    /// The starred instruments of the shelf. A meloet setting too.
    favorites: Vec<BuiltinInstrument>,
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
            favorites: settings.favorites,
        }
    }

    /// `installed`: the sampled instruments the sound pack of this PC has.
    pub fn with_sounds(mut self, installed: Vec<BuiltinInstrument>) -> Model {
        self.play.set_installed_sounds(installed);
        self
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
        }
        if !self.bands.side_is_open(SidePanel::Shelf) {
            self.play.shelf_closed(&mut effects);
        }
        self.settle(&mut effects);
        if self.open.is_none() {
            let cut = if self.audio_system_down {
                Cut::AudioSystem
            } else {
                Cut::Output
            };
            self.play.stop_sample(&mut effects);
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
        self.play.sync_silent(&mut effects);
        self.play.sync_target(&mut effects);
        self.play.sync_sample(&mut effects);
        self.play.sync_sounds(&mut effects);
        self.close_when_recorded(&mut effects);
        self.recovery_tick(&mut effects);
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
                if let Some(fold) = self.bands.toggle(band_of(band)) {
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
                if !self.cancel_song_dialog()
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
            Command::SelectClip { clip } => self.play.select_clip(clip),
            Command::SelectTrack { track, from_arm } => {
                if let Some(n) = self.play.select_track(track, from_arm) {
                    self.notice = Some(n);
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
                if let Some(n) = self.play.shelf_confirm(effects) {
                    self.notice = Some(n);
                }
            }
            Command::ShelfCancel => {
                self.play.end_audition(effects);
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
            Command::Undo => self.play.undo(effects),
            Command::Redo => self.play.redo(effects),
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
            Command::PasteNotes { at } => self.play.paste_notes(at, effects),
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
        }
        None
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
            velocity_fixed: self.velocity_fixed.on,
            fixed_velocity: self.velocity_fixed.value,
            favorites: self.favorites.clone(),
        }
    }

    fn shelf_view(&self) -> ShelfView {
        self.play.shelf_view(
            InstrumentKind::ALL
                .into_iter()
                .filter(|&k| self.favorites.contains(&instruments::builtin_of(k)))
                .collect(),
        )
    }

    /// A sample plays: [`Input::Tick`] is wanted now and then, to end it.
    pub fn sample_playing(&self) -> bool {
        self.play.sample_playing()
    }

    /// The instruments and the play target, for an engine that has none yet.
    pub fn engine_seats(&self) -> Vec<EngineOp> {
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
                    .or_else(|| self.empty_hint().then_some(Hint::EmptyArrange))
                    .filter(|_| recovery.is_none()),
                song_dialog: self.song_dialog_view(),
                recovery,
            },
            shelf: self.shelf_view(),
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
        if let Some(m) = self.play.moving_status() {
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
        if self.new_song_unsaid {
            let song = &self.play.song().doc;
            return Some(StatusMessage::NewSong {
                tempo_bpm: song.tempo.bpm(),
                meter_num: song.meter.num(),
                meter_den: song.meter.den(),
                track: self.play.record_target(),
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
        serial: play.serial(),
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
            .enumerate()
            .map(|(i, t)| TrackView {
                id: t.id.0,
                number: i as u32 + 1,
                name: t.name.clone(),
                color: t.color.get(),
                instrument: match t.instrument {
                    InstrumentRef::Builtin(b) => instruments::kind_of(b),
                },
                gain_db: t.gain_db as f32,
                muted: t.muted,
                soloed: play.is_soloed(t.id),
                clips: play.clips_of(t.id),
            })
            .collect(),
        play_target: song.session.play_target.map(|t| t.0),
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
mod speed;
#[cfg(test)]
mod tests;
