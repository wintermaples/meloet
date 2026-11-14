//! A stand-in for meloet-core until that crate owns the state. Nothing here is a product rule:
//! the numbers and the band behaviour only give the screen something to connect to.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use meloet_protocol::{
    AppState, AudioView, Band, Command, Hint, InputsView, InstrumentKind, KeyboardsView,
    LayoutView, LowerBandView, MeterSize, OutputsView, Patch, ServerMessage, SettingsDialogView,
    Snapshot, SongView, StatusMessage, StatusView, Telemetry, TrackView, WorkspaceView,
};

use crate::clock;
use crate::core_handle::{CoreHandle, MessageSink, TelemetrySink};

const TICK: Duration = Duration::from_micros(16_667);
const LOWER_BAND_HEIGHT: f32 = 240.0;
const TONE_LENGTH: Duration = Duration::from_secs(1);

pub struct StubCore {
    inner: Mutex<Inner>,
}

struct Inner {
    rev: u64,
    state: AppState,
    message_sinks: Vec<MessageSink>,
    telemetry_sinks: Vec<TelemetrySink>,
    /// A new telemetry sink has not had the current readings yet.
    telemetry_fresh: bool,
    tone_started: Option<Instant>,
}

impl StubCore {
    pub fn start() -> Arc<StubCore> {
        let core = Arc::new(StubCore {
            inner: Mutex::new(Inner {
                rev: 1,
                state: initial_state(),
                message_sinks: Vec::new(),
                telemetry_sinks: Vec::new(),
                telemetry_fresh: false,
                tone_started: None,
            }),
        });
        let weak = Arc::downgrade(&core);
        std::thread::Builder::new()
            .name("meloet-stub-telemetry".into())
            .spawn(move || {
                let mut seq = 0u32;
                let mut last: Option<Telemetry> = None;
                let mut bytes = Vec::new();
                while let Some(core) = weak.upgrade() {
                    core.tick(&mut seq, &mut last, &mut bytes);
                    drop(core);
                    std::thread::sleep(TICK);
                }
            })
            .expect("failed to start the telemetry thread");
        core
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// One 60 Hz step: sends the readings unless they repeat what was sent last.
    fn tick(&self, seq: &mut u32, last: &mut Option<Telemetry>, bytes: &mut Vec<u8>) {
        let mut inner = self.lock();
        let peak = match inner.tone_started.map(|t| t.elapsed()) {
            Some(elapsed) if elapsed < TONE_LENGTH => {
                0.5 * (1.0 - elapsed.as_secs_f32() / TONE_LENGTH.as_secs_f32())
            }
            _ => {
                inner.tone_started = None;
                0.0
            }
        };
        let now = Telemetry {
            seq: *seq,
            sent_ns: clock::mono_ns(),
            master_peak: [peak, peak],
            track_peaks: vec![[peak, peak]; inner.state.song.tracks.len()],
            ..Telemetry::default()
        };
        let fresh = std::mem::take(&mut inner.telemetry_fresh);
        if !fresh && last.as_ref().is_some_and(|l| l.same_readings(&now)) {
            return;
        }
        now.encode_into(bytes);
        inner.telemetry_sinks.retain(|sink| sink(bytes));
        *seq = seq.wrapping_add(1);
        *last = Some(now);
    }
}

impl CoreHandle for StubCore {
    fn snapshot(&self) -> Snapshot {
        let inner = self.lock();
        Snapshot {
            rev: inner.rev,
            state: inner.state.clone(),
        }
    }

    fn send(&self, command: Command) -> u64 {
        // The lock is held while the patch goes out, so patches leave in rev order and a
        // snapshot never sits between a state change and its patch.
        let mut inner = self.lock();
        let mut next = inner.state.clone();
        if let Command::TestTone { .. } = command {
            inner.tone_started = Some(Instant::now());
        }
        apply(&mut next, command);
        let changes = AppState::changes_from(&inner.state, &next);
        if changes.is_empty() {
            return inner.rev;
        }
        inner.rev += 1;
        inner.state = next;
        let patch = Patch {
            rev: inner.rev,
            changes,
        };
        inner
            .message_sinks
            .retain(|sink| sink(ServerMessage::Patch(patch.clone())));
        inner.rev
    }

    fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink) {
        let mut inner = self.lock();
        inner.message_sinks.push(messages);
        inner.telemetry_sinks.push(telemetry);
        inner.telemetry_fresh = true;
    }
}

fn apply(state: &mut AppState, command: Command) {
    let ws = &mut state.workspace;
    match command {
        Command::SetViewport { width, height } => {
            if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
                ws.viewport_width = width;
                ws.viewport_height = height;
            }
        }
        Command::SetScale { percent } => {
            if matches!(percent, 100 | 125 | 150) {
                ws.scale = percent;
            }
        }
        Command::ToggleBand { band } => match band {
            Band::PianoRoll | Band::Mixer => {
                let lower = &mut ws.layout.lower;
                match lower.iter().position(|b| b.band == band) {
                    Some(i) => {
                        lower.remove(i);
                    }
                    None => lower.push(LowerBandView {
                        band,
                        height: LOWER_BAND_HEIGHT,
                    }),
                }
            }
            Band::History | Band::Shelf | Band::Params => {
                let open = &mut ws.layout.right_open;
                match open.iter().position(|b| *b == band) {
                    Some(i) => {
                        open.remove(i);
                    }
                    None => open.push(band),
                }
                ws.layout.right_front = open.last().copied();
            }
        },
        Command::GoArrange => {
            ws.layout.lower.clear();
        }
        Command::Back => {
            if ws.settings.take().is_none() && ws.layout.right_open.pop().is_none() {
                ws.layout.lower.pop();
            }
            ws.layout.right_front = ws.layout.right_open.last().copied();
        }
        Command::OpenSettings => {
            ws.settings.get_or_insert(SettingsDialogView {
                draft_output: None,
                advanced_open: false,
            });
        }
        Command::SettingsToggleAdvanced => {
            if let Some(s) = &mut ws.settings {
                s.advanced_open = !s.advanced_open;
            }
        }
        Command::SettingsApply | Command::SettingsCancel => ws.settings = None,
        Command::DismissHint => ws.hint = None,
        Command::SettingsSelectOutput { .. }
        | Command::ChooserMove { .. }
        | Command::ChooserSelect { .. }
        | Command::ChooserConfirm
        | Command::TestTone { .. } => {}
    }
    let ws = &mut state.workspace;
    let lower: f32 = ws.layout.lower.iter().map(|b| b.height).sum();
    ws.layout.arrange_height = (ws.viewport_height * 100.0 / f32::from(ws.scale) - lower).max(0.0);
    state.status.message = match ws.settings {
        Some(_) => Some(StatusMessage::SettingsOpen),
        None => Some(StatusMessage::NewSong),
    };
}

fn initial_state() -> AppState {
    AppState {
        song: SongView {
            name: "はじめての曲".into(),
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            tempo_changes: 0,
            undo_count: 0,
            note_count: 0,
            tracks: vec![TrackView {
                number: 1,
                name: "ピアノ".into(),
                color: 1,
                instrument: InstrumentKind::Placeholder,
                gain_db: 0.0,
            }],
            play_target: Some(1),
        },
        workspace: WorkspaceView {
            scale: 100,
            viewport_width: 1280.0,
            viewport_height: 800.0,
            layout: LayoutView {
                meter: MeterSize::Full,
                arrange_height: 800.0,
                arrange_overview: false,
                lower: Vec::new(),
                right_open: Vec::new(),
                right_front: None,
                show_back_hint: false,
            },
            settings: None,
            chooser: None,
            hint: Some(Hint::EmptyArrange),
        },
        keyboards: KeyboardsView {
            list: Vec::new(),
            active: None,
            lost: None,
        },
        outputs: OutputsView {
            list: Vec::new(),
            chosen: None,
            chosen_state: None,
        },
        inputs: InputsView {
            list: Vec::new(),
            chosen: None,
        },
        audio: AudioView {
            running: false,
            system: "PipeWire".into(),
            sample_rate: None,
            requested_frames: 256,
            actual_frames: None,
            forced_quantum: None,
            latency_ms: None,
            latency_ms_at_requested: None,
            output_device: None,
            midi_input: None,
        },
        status: StatusView {
            message: Some(StatusMessage::NewSong),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn core_with_sinks() -> (
        Arc<StubCore>,
        mpsc::Receiver<ServerMessage>,
        mpsc::Receiver<Vec<u8>>,
    ) {
        let core = StubCore::start();
        let (mtx, mrx) = mpsc::channel();
        let (ttx, trx) = mpsc::channel();
        core.subscribe(
            Box::new(move |m| mtx.send(m).is_ok()),
            Box::new(move |b| ttx.send(b.to_vec()).is_ok()),
        );
        (core, mrx, trx)
    }

    #[test]
    fn a_command_that_changes_state_sends_one_patch_with_the_next_rev() {
        let (core, messages, _telemetry) = core_with_sinks();
        let before = core.snapshot();

        let rev = core.send(Command::ToggleBand {
            band: Band::PianoRoll,
        });
        assert_eq!(rev, before.rev + 1);

        let ServerMessage::Patch(patch) = messages.try_recv().unwrap() else {
            panic!("expected a patch");
        };
        assert_eq!(patch.rev, rev);
        let mut state = before.state;
        for change in patch.changes {
            state.apply(change);
        }
        assert_eq!(state, core.snapshot().state);
        assert_eq!(state.workspace.layout.lower[0].band, Band::PianoRoll);
        assert!(messages.try_recv().is_err());
    }

    #[test]
    fn a_command_that_changes_nothing_sends_nothing() {
        let (core, messages, _telemetry) = core_with_sinks();
        let before = core.snapshot().rev;
        assert_eq!(core.send(Command::SetScale { percent: 100 }), before);
        assert_eq!(core.send(Command::SetScale { percent: 110 }), before);
        assert!(messages.try_recv().is_err());
    }

    #[test]
    fn telemetry_goes_once_then_stops_until_something_changes() {
        let (core, _messages, telemetry) = core_with_sinks();

        let first = telemetry.recv_timeout(Duration::from_secs(2)).unwrap();
        let first = Telemetry::decode(&first).unwrap();
        assert_eq!(first.track_peaks.len(), 1);
        assert!(
            telemetry.recv_timeout(Duration::from_millis(200)).is_err(),
            "nothing changed, so nothing more is sent"
        );

        core.send(Command::TestTone { output: "x".into() });
        let next = telemetry.recv_timeout(Duration::from_secs(2)).unwrap();
        let next = Telemetry::decode(&next).unwrap();
        assert!(next.master_peak[0] > 0.0);
        assert_eq!(next.seq, first.seq + 1);
    }
}
