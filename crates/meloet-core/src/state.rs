//! The pure rules: an input comes in, the state changes, and a list of things to do comes back.
//! Nothing here touches a device, a file or a thread.

use std::collections::VecDeque;

use meloet_devices::{DevicesPicture, Endpoint, sort_for_choice};
use meloet_midi::Keyboard;
use meloet_model::{InstrumentRef, Song};
use meloet_protocol::{
    AppState, AudioView, Availability, Band, ChooserView, ChosenState, Command, EndpointView,
    Friendly, Hint, InputsView, InstrumentKind, KeyboardView, KeyboardsView, LayoutView, LinkKind,
    LowerBandView, MeterSize, OutputsView, SettingsDialogView, SongView, StatusMessage, StatusView,
    TrackView, Unusable, WorkspaceView,
};

use crate::layout::{self, Bands, BottomBand, SidePanel, WindowSize};
use crate::settings::{SCALES, Settings};

/// The buffer size meloet asks for. What really runs is `AudioView::actual_frames`.
pub const REQUESTED_FRAMES: u32 = 256;
pub const AUDIO_SYSTEM: &str = "PipeWire";
/// Key to sound is about this many buffers (docs/architecture.md 4).
const LATENCY_BUFFERS: f32 = 2.5;
/// Measured key-to-sound times that make up the shown average.
const LATENCY_SAMPLES: usize = 16;
const START_VIEWPORT: (f32, f32) = (1280.0, 800.0);

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

pub struct Model {
    song: Song,
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
    /// A song was opened and the running buffer has not been looked at since.
    buffer_unsaid: bool,
    /// The fixed buffer size as last known; `None` when it was not fixed. Kept while the stream
    /// is reopened, so the same size on the next stream is not said again.
    buffer_fixed_seen: Option<u32>,
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
            song: Song::new_default(),
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
                self.keyboards.retain(|k| k.id != keyboard.id);
                self.keyboards.push(keyboard);
                self.lost = None;
            }
            Input::KeyboardRemoved { id } => {
                if let Some(i) = self.keyboards.iter().position(|k| k.id == id) {
                    let gone = self.keyboards.remove(i);
                    if self.played.as_ref() == Some(&gone.id) {
                        self.played = None;
                    }
                    if self.keyboards.is_empty() {
                        self.lost = Some(gone.name);
                    }
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
                }
            }
            Input::AudioOpenFailed { node_name } => {
                if self.wanted.as_deref() == Some(&node_name) {
                    self.wanted = None;
                    self.open = None;
                    self.failed = Some(node_name);
                }
            }
            Input::AudioFrames(frames) => {
                if self.open.is_some() {
                    self.actual_frames = frames.filter(|f| *f > 0);
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
            Input::SongOpened => self.buffer_unsaid = true,
        }
        self.settle(&mut effects);
        self.watch_buffer();
        if tone_on.is_some() && tone_on == self.wanted {
            effects.push(Effect::TestTone);
        }
        effects
    }

    /// Returns the node a test tone was asked on.
    fn command(&mut self, command: Command, effects: &mut Vec<Effect>) -> Option<String> {
        // A resize is not something the user does to answer a notice.
        if !matches!(command, Command::SetViewport { .. }) {
            self.notice = None;
        }
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
                if self.dialog.take().is_none() {
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
                    self.choose(output, effects);
                    self.hint_armed = true;
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
                }
            }
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
            .filter(|node| self.failed.as_ref() != Some(node));
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
        }
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
            song: song_view(&self.song),
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
                hint: self.hint,
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
        let output = || {
            self.chosen_name
                .clone()
                .or_else(|| self.chosen.clone())
                .unwrap_or_default()
        };
        if self.dialog.is_some() {
            return Some(StatusMessage::SettingsOpen);
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
        let active = self.active_keyboard();
        if let (None, Some(keyboard)) = (active, &self.lost) {
            return Some(StatusMessage::KeyboardLost {
                keyboard: keyboard.clone(),
            });
        }
        if self.notice.is_some() {
            return self.notice.clone();
        }
        let Some(active) = active else {
            return Some(StatusMessage::SearchingKeyboard);
        };
        self.chosen.is_none().then(|| StatusMessage::KeyboardFound {
            keyboard: active.name.clone(),
        })
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

fn song_view(song: &Song) -> SongView {
    SongView {
        name: song.name.clone(),
        tempo_bpm: song.tempo_bpm,
        meter_num: song.meter.num,
        meter_den: song.meter.den,
        tempo_changes: song.tempo_changes.len() as u32,
        undo_count: 0,
        note_count: 0,
        tracks: song
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
            })
            .collect(),
        play_target: Some(song.play_target.0),
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
