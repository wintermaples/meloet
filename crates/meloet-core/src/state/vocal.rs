//! The PC's input for the vocal tracks, the advanced sound settings and the dialog 「遅れを
//! 測る」 (spec 3, 23-A, 24-C, 25-B, 33-A; docs/architecture.md 22-5).

use meloet_devices::Endpoint;
use meloet_engine::{ProbeVerdict, judge_probes};
use meloet_model::TrackId;
use meloet_protocol::{
    Command, InputsView, LATENCY_CLICKS, LatencyDialogView, LatencyFailure, LatencyMeasure,
    LatencySource, LatencyView, StatusMessage,
};

use super::{Effect, Model, endpoint_view};
use crate::play::{Cut, OpenInput};
use crate::settings::{
    BUFFER_CHOICES, MANUAL_LATENCY_MAX_US, MeasuredLatency, SAMPLE_RATE_CHOICES,
};

/// What the rules know of the PC's input.
#[derive(Debug, Default)]
pub(super) struct InputSide {
    /// The node the last `OpenInput` asked for and no `CloseInput` took back.
    wanted: Option<String>,
    /// The PC's name of that input, for when it is gone from the list.
    wanted_name: Option<String>,
    /// A node that failed to open or went away. Not tried again until the devices change.
    failed: Option<String>,
    open: Option<OpenInput>,
    /// An input that is not in the PC's list: the monitor of a sink, for the measurements
    /// and tests that need a loop with no cable.
    stand_in: Option<Endpoint>,
}

/// What the PC says of its delays, as the core thread last read them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delays {
    /// From the output back into the input: the two delays the PC says, and the one block
    /// the input's frames are behind their time (docs/architecture.md 22-16).
    pub round_us: u32,
    /// How late the voice comes out of meloet: the round trip and the block the monitor
    /// waits for.
    pub monitor_us: u32,
}

#[derive(Debug, Default)]
pub(super) struct LatencyDialog {
    measure: Option<Measured>,
    /// The way round of each click heard so far (`X`, microseconds); `None`: not found.
    answers: Vec<Option<f64>>,
    measuring: bool,
}

#[derive(Debug, Clone, Copy)]
enum Measured {
    Done { x_us: f64 },
    Failed(LatencyFailure),
}

fn ms(us: f64) -> f32 {
    ((us / 100.0).round() / 10.0) as f32
}

impl Model {
    /// The status line of a recording that was stopped because the sound went away, or its
    /// files could not be written on, with the number of the take it left.
    pub(super) fn cut_notice(&self, cut: Cut, take: u32) -> StatusMessage {
        match cut {
            Cut::AudioSystem => StatusMessage::RecordingStoppedAudioSystem { take },
            Cut::Output => StatusMessage::RecordingStoppedOutputMissing {
                output: self.output_name(),
                take,
            },
            Cut::Disk(reason) => StatusMessage::RecordingStoppedDisk { reason },
        }
    }

    /// The monitor of this sink is the input, whatever the PC's inputs are.
    pub fn with_input_stand_in(mut self, sink_node: &str) -> Model {
        self.input.stand_in = Some(Endpoint {
            id: format!("monitor:{sink_node}"),
            name: sink_node.to_owned(),
            friendly: None,
            availability: meloet_devices::Availability::Yes,
            link: meloet_devices::LinkKind::Plug,
            active: true,
            node_name: Some(sink_node.to_owned()),
            device_name: None,
            device_description: None,
            channels: Some(2),
        });
        self
    }

    fn inputs(&self) -> impl Iterator<Item = &Endpoint> {
        (self.input.stand_in.iter()).chain(self.devices.iter().flat_map(|d| &d.inputs))
    }

    /// The input the vocal tracks record from: the one of the settings, else the one the
    /// PC uses (the first that is plugged in).
    fn chosen_input(&self) -> Option<&Endpoint> {
        if let Some(stand_in) = &self.input.stand_in {
            return Some(stand_in);
        }
        let usable = |e: &&Endpoint| e.unusable().is_none() && e.node_name.is_some();
        (self.sound.input.as_ref())
            .and_then(|id| self.inputs().filter(usable).find(|e| &e.id == id))
            .or_else(|| {
                use meloet_devices::Availability as A;
                let plugged = |want: A| {
                    self.inputs()
                        .filter(usable)
                        .find(|e| e.availability == want)
                };
                plugged(A::Yes).or_else(|| plugged(A::Unknown))
            })
    }

    pub(super) fn has_input(&self) -> bool {
        self.chosen_input().is_some()
    }

    /// The input as it is open now.
    pub(super) fn open_input(&self) -> Option<OpenInput> {
        self.input.open.filter(|_| self.open.is_some())
    }

    /// How many channels the input has, open or not: what 「入口なし」 goes by.
    pub(super) fn input_channels(&self) -> Option<u16> {
        let chosen = self.chosen_input()?;
        Some(match self.open_input() {
            Some(open) => open.channels,
            // An input that does not say counts as stereo until it is open.
            None => chosen.channels.map_or(2, u16::from),
        })
    }

    pub(super) fn inputs_view(&self) -> InputsView {
        InputsView {
            list: self.inputs().map(endpoint_view).collect(),
            chosen: self.chosen_input().map(|e| e.id.clone()),
            open: self.open_input().is_some(),
        }
    }

    /// The output is opened anew or closed: the input goes with it, to come back at the
    /// new stream's rate.
    pub(super) fn input_reset(&mut self, effects: &mut Vec<Effect>) {
        if self.input.wanted.take().is_some() {
            effects.push(Effect::CloseInput);
        }
        self.input.open = None;
        self.delays = None;
        self.end_measuring(effects);
    }

    pub(super) fn devices_changed_for_input(&mut self) {
        self.input.failed = None;
    }

    /// Opens the input while a vocal track is selected or has its ● on, or the latency is
    /// measured; closes it when none is.
    pub(super) fn settle_input(&mut self, effects: &mut Vec<Effect>) {
        if self.play.busy() {
            return;
        }
        let wants = self.open.is_some() && (self.play.wants_input() || self.latency.is_some());
        let chosen = self.chosen_input().filter(|_| wants);
        let target = (chosen.and_then(|e| e.node_name.clone()))
            .filter(|node| self.input.failed.as_ref() != Some(node));
        if target == self.input.wanted {
            return;
        }
        self.input.wanted_name = chosen.filter(|_| target.is_some()).map(|e| e.name.clone());
        if self.input.wanted.is_some() {
            effects.push(Effect::CloseInput);
            self.end_measuring(effects);
        }
        if let Some(node_name) = &target {
            effects.push(Effect::OpenInput {
                node_name: node_name.clone(),
                sink_monitor: self.input.stand_in.is_some(),
            });
        }
        self.input.wanted = target;
        self.input.open = None;
        self.delays = None;
    }

    pub(super) fn input_opened(&mut self, node_name: &str, open: OpenInput) {
        if self.input.wanted.as_deref() == Some(node_name) {
            self.input.open = Some(open);
        }
    }

    /// The input that was open went away (spec 6 「入口が抜けた」): what a recording it
    /// stopped says. `input`: the channels of the first vocal track that was recording.
    pub(super) fn input_lost_notice(&self) -> Option<StatusMessage> {
        let recording = self.play.transport_view(true).recording_vocals;
        let track = TrackId(recording.first()?.track);
        let input = self.play.song().doc.track(track)?.kind.input()?;
        let device = (self.input.wanted_name.clone())
            .or_else(|| self.input.wanted.clone())
            .unwrap_or_default();
        Some(StatusMessage::InputLost {
            input: super::vocal_input_view(input),
            device,
        })
    }

    pub(super) fn input_gone(&mut self, node_name: Option<&str>, effects: &mut Vec<Effect>) {
        if node_name.is_none_or(|n| self.input.wanted.as_deref() == Some(n)) {
            self.input.failed = self.input.wanted.take();
            self.input.open = None;
            self.delays = None;
            self.end_measuring(effects);
        }
    }

    // ---- the latency ----

    /// What the round trip takes more than the PC says, by what the settings say to use
    /// (docs/architecture.md 22-5, `X`), in microseconds.
    pub(super) fn latency_x_us(&self) -> i64 {
        let reported = self.delays.map(|d| i64::from(d.round_us));
        match (self.sound.manual_latency_us, self.measured_x_us()) {
            (Some(manual), _) => reported.map_or(0, |r| i64::from(manual) - r),
            (None, Some(x)) => i64::from(x),
            (None, None) => 0,
        }
    }

    fn measured_x_us(&self) -> Option<i32> {
        let output = self.chosen.as_deref()?;
        let input = self.chosen_input()?;
        let rate = self.open.as_ref()?.sample_rate;
        self.sound.measured_for(output, &input.id, rate)
    }

    pub(super) fn latency_view(&self) -> LatencyView {
        let reported = self.delays.map(|d| f64::from(d.round_us));
        let reported_ms = reported.map(ms);
        let measured_ms = (reported.zip(self.measured_x_us())).map(|(r, x)| ms(r + f64::from(x)));
        let manual_ms = self.sound.manual_latency_us.map(|us| ms(f64::from(us)));
        let (used, used_ms) = match (manual_ms, measured_ms) {
            (Some(manual), _) => (LatencySource::Manual, Some(manual)),
            (None, Some(measured)) => (LatencySource::Measured, Some(measured)),
            (None, None) => (LatencySource::Reported, reported_ms),
        };
        LatencyView {
            reported_ms,
            measured_ms,
            manual_ms,
            used,
            used_ms,
        }
    }

    pub(super) fn monitor_latency_ms(&self) -> Option<f32> {
        self.delays.map(|d| ms(f64::from(d.monitor_us)))
    }

    pub(super) fn latency_dialog_view(&self) -> Option<LatencyDialogView> {
        let dialog = self.latency.as_ref()?;
        let reported = self.delays.map_or(0.0, |d| f64::from(d.round_us));
        let measure = if dialog.measuring {
            let found: Vec<f64> = dialog.answers.iter().flatten().copied().collect();
            LatencyMeasure::Measuring {
                done: dialog.answers.len() as u8,
                of: LATENCY_CLICKS,
                mean_ms: (!found.is_empty())
                    .then(|| ms(reported + found.iter().sum::<f64>() / found.len() as f64)),
            }
        } else {
            match dialog.measure {
                None => LatencyMeasure::Idle,
                Some(Measured::Done { x_us }) => LatencyMeasure::Done {
                    measured_ms: ms(reported + x_us),
                },
                Some(Measured::Failed(reason)) => LatencyMeasure::Failed { reason },
            }
        };
        Some(LatencyDialogView { measure })
    }

    /// The clicks are played and listened for: play and record wait.
    pub(super) fn measuring(&self) -> bool {
        self.latency.as_ref().is_some_and(|d| d.measuring)
    }

    fn end_measuring(&mut self, effects: &mut Vec<Effect>) {
        if let Some(dialog) = &mut self.latency
            && dialog.measuring
        {
            dialog.measuring = false;
            dialog.answers.clear();
            dialog.measure = Some(Measured::Failed(LatencyFailure::NoInput));
            effects.push(Effect::LatencyListen(false));
        }
    }

    /// The disk writer heard a click, or did not.
    pub(super) fn latency_answer(&mut self, x_us: Option<f64>, effects: &mut Vec<Effect>) {
        let rate = self.open.as_ref().map_or(48_000, |o| o.sample_rate);
        let Some(dialog) = self.latency.as_mut().filter(|d| d.measuring) else {
            return;
        };
        dialog.answers.push(x_us);
        if dialog.answers.len() < usize::from(LATENCY_CLICKS) {
            effects.push(Effect::LatencyClick);
            return;
        }
        dialog.measuring = false;
        dialog.measure = Some(match judge_probes(&dialog.answers, rate) {
            ProbeVerdict::Measured(x_us) => Measured::Done { x_us },
            ProbeVerdict::TooQuiet => Measured::Failed(LatencyFailure::TooQuiet),
            ProbeVerdict::Unsteady => Measured::Failed(LatencyFailure::Unsteady),
        });
        dialog.answers.clear();
        effects.push(Effect::LatencyListen(false));
    }

    /// The commands of protocol 23 about vocal tracks, the input and the latency. Returns
    /// the command back when it is not one of them.
    pub(super) fn vocal_command(
        &mut self,
        command: Command,
        effects: &mut Vec<Effect>,
    ) -> Option<Command> {
        let busy = self.play.busy();
        match command {
            Command::AddVocalTrack => {
                let has_input = self.has_input();
                self.notice = self.play.add_vocal_track(has_input, effects);
            }
            Command::SetVocalInput { track, input } => {
                self.play
                    .set_vocal_input(track, super::model_vocal_input(input), effects);
            }
            Command::ToggleVocalArm { track } => {
                let has_input = self.has_input();
                self.notice = self.play.toggle_vocal_arm(track, has_input);
            }
            Command::ToggleMonitor { track } => {
                if self.play.toggle_monitor(track) == Some(true) && !self.monitor_said {
                    self.monitor_said = true;
                    self.notice = Some(StatusMessage::MonitorFirstOn {
                        latency_ms: self.monitor_latency_ms(),
                    });
                }
            }
            Command::SettingsSelectInput { input } => {
                let known = (input.as_ref())
                    .is_none_or(|id| self.inputs().any(|e| &e.id == id && e.unusable().is_none()));
                if !busy && known && self.sound.input != input {
                    self.sound.input = input;
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::SettingsSetBuffer { frames } => {
                if !busy && BUFFER_CHOICES.contains(&frames) && self.sound.buffer != frames {
                    self.sound.buffer = frames;
                    self.reopen_output(effects);
                }
            }
            Command::SettingsSetSampleRate { hz } => {
                if !busy && SAMPLE_RATE_CHOICES.contains(&hz) && self.sound.sample_rate != hz {
                    self.sound.sample_rate = hz;
                    self.reopen_output(effects);
                }
            }
            Command::SettingsSetRecordFormat { format } => {
                if !busy && self.sound.record_format != format {
                    self.sound.record_format = format;
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::SettingsSetManualLatency { ms } => {
                let us = ms
                    .filter(|ms| !ms.is_nan())
                    .map(|ms| (ms.clamp(0.0, 500.0) * 1_000.0).round() as u32)
                    .map(|us| us.min(MANUAL_LATENCY_MAX_US));
                if !busy && self.sound.manual_latency_us != us {
                    self.sound.manual_latency_us = us;
                    effects.push(Effect::SaveSettings(self.settings()));
                }
            }
            Command::OpenLatencyMeasure => {
                if !self.play.is_moving() && self.latency.is_none() {
                    self.latency = Some(LatencyDialog::default());
                }
            }
            Command::StartLatencyMeasure => {
                let ready = self.open_input().is_some();
                let moving = self.play.is_moving();
                if let Some(dialog) = self.latency.as_mut().filter(|d| !d.measuring && !moving) {
                    dialog.answers.clear();
                    if ready {
                        dialog.measuring = true;
                        dialog.measure = None;
                        effects.push(Effect::LatencyListen(true));
                        effects.push(Effect::LatencyClick);
                    } else {
                        dialog.measure = Some(Measured::Failed(LatencyFailure::NoInput));
                    }
                }
            }
            Command::StopLatencyMeasure => {
                if let Some(dialog) = self.latency.as_mut().filter(|d| d.measuring) {
                    dialog.measuring = false;
                    dialog.measure = None;
                    dialog.answers.clear();
                    effects.push(Effect::LatencyListen(false));
                }
            }
            Command::UseMeasuredLatency => {
                let done = (self.latency.as_ref()).and_then(|d| match (d.measuring, d.measure) {
                    (false, Some(Measured::Done { x_us })) => Some(x_us),
                    _ => None,
                });
                let key = (self.chosen.clone())
                    .zip(self.chosen_input().map(|e| e.id.clone()))
                    .zip(self.open.as_ref().map(|o| o.sample_rate));
                if let (Some(x_us), Some(((output, input), sample_rate))) = (done, key) {
                    self.sound.keep_measured(MeasuredLatency {
                        output,
                        input,
                        sample_rate,
                        x_us: x_us.round() as i32,
                        at_ms: crate::recovery::now_ms(),
                    });
                    effects.push(Effect::SaveSettings(self.settings()));
                    self.latency = None;
                }
            }
            Command::CloseLatencyMeasure => {
                if self.latency.take().is_some_and(|d| d.measuring) {
                    effects.push(Effect::LatencyListen(false));
                }
            }
            Command::ToggleBand {
                band: meloet_protocol::Band::PianoRoll,
            } if self.play.selected_vocal().is_some()
                && !self
                    .bands
                    .bottom_is_open(crate::layout::BottomBand::PianoRoll) =>
            {
                self.notice = Some(StatusMessage::VocalHasNoPianoRoll);
            }
            other => return Some(other),
        }
        None
    }

    /// The buffer or the rate asked for changed: the stream is opened anew with them.
    fn reopen_output(&mut self, effects: &mut Vec<Effect>) {
        effects.push(Effect::SaveSettings(self.settings()));
        if let Some(node_name) = self.wanted.clone() {
            self.play.end_audition(effects);
            self.input_reset(effects);
            effects.push(Effect::OpenOutput { node_name });
            self.open = None;
            self.actual_frames = None;
            self.latencies.clear();
            self.xruns = 0;
        }
    }
}
