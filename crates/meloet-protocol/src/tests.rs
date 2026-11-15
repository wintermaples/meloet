use std::path::PathBuf;

use serde_json::json;
use ts_rs::TS;

use super::*;

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui")
}

fn sample_state() -> AppState {
    AppState {
        song: SongView {
            name: "はじめての曲".into(),
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            tempo_changes: 0,
            undo_count: 2,
            redo_count: 1,
            note_count: 23,
            tracks: vec![TrackView {
                number: 1,
                name: "ピアノ".into(),
                color: 1,
                instrument: InstrumentKind::Placeholder,
                gain_db: 0.0,
                armed: true,
                clips: vec![ClipView {
                    id: 4,
                    name: "ピアノ".into(),
                    start: 7680,
                    end: 15360,
                    take: 2,
                    takes: 3,
                    take_id: 6,
                    take_offset: -480,
                    note_count: 23,
                }],
            }],
            play_target: Some(1),
            loop_range: Some(TickSpan {
                start: 7680,
                end: 15360,
            }),
            loop_on: true,
            file: SongFile::Changed,
        },
        transport: TransportView {
            state: TransportState::Recording,
            position: 7680,
            metronome: true,
            count_in_bars: 1,
            count_in_beats: 4,
            can_play: true,
            can_record: true,
            recording: Some(RecordingView {
                id: 3,
                track: 1,
                from: 7680,
                take: 4,
                clip: Some(4),
                lap: 1,
            }),
        },
        arrange: ArrangeView {
            selected_clip: None,
            piano_roll_clip: Some(4),
        },
        piano_roll: PianoRollView {
            take: Some(6),
            selected_notes: vec![IndexRange { start: 14, end: 17 }],
            selected_controls: vec![],
            grid: Grid {
                division: 16,
                triplet: false,
            },
            scale: MusicalScale {
                root: 0,
                mode: ScaleMode::Major,
                lock: true,
            },
            quantize: QuantizeSettings {
                division: 16,
                triplet: false,
                strength: 75,
                swing: 0,
                align: QuantizeAlign::Start,
            },
            clipboard_notes: 0,
        },
        workspace: WorkspaceView {
            scale: 100,
            viewport_width: 1280.0,
            viewport_height: 800.0,
            unfit_scales: vec![150],
            layout: LayoutView {
                meter: MeterSize::Full,
                arrange_height: 600.0,
                arrange_overview: false,
                lower: vec![LowerBandView {
                    band: Band::PianoRoll,
                    height: 240.0,
                }],
                right_open: vec![Band::Shelf],
                right_front: Some(Band::Shelf),
                show_back_hint: false,
            },
            settings: None,
            chooser: None,
            hint: Some(Hint::EmptyArrange),
            song_dialog: Some(SongDialogView {
                id: 2,
                kind: SongDialogKind::Broken,
                title: "はじめての曲.meloet".into(),
                previous: true,
            }),
            recovery: Some(RecoveryView::Recovered {
                written_ms: 1_794_750_060_000,
                cause: RecoveryCause::PcEnding,
                song: None,
                takes: 2,
            }),
        },
        keyboards: KeyboardsView {
            list: vec![],
            active: None,
            lost: None,
            input_stopped: false,
        },
        outputs: OutputsView {
            list: vec![EndpointView {
                id: "alsa_card.pci:analog-output-headphones".into(),
                name: "アナログヘッドフォン".into(),
                friendly: Some(Friendly::Headphones),
                availability: Availability::Yes,
                link: LinkKind::Jack,
                node_name: Some("alsa_output.pci.analog-stereo".into()),
                device: Some("Ryzen HD Audio Controller".into()),
                unusable: None,
            }],
            chosen: None,
            chosen_state: None,
        },
        inputs: InputsView {
            list: vec![],
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
            message: Some(StatusMessage::SearchingKeyboard),
        },
    }
}

fn sample_telemetry() -> Telemetry {
    Telemetry {
        seq: 0x0102_0304,
        sent_ns: 123_456_789_012_345,
        keys_low: (1 << 0) | (1 << 36) | (1 << 60),
        keys_high: (1 << 3) | (1 << 63),
        last_velocity: 101,
        sustain: 127,
        modulation: 64,
        running: true,
        master_clipped: true,
        pitch_bend: -8192,
        load_permille: 137,
        master_peak: [0.5, 0.1],
        transport: Some(TransportReading {
            tick: -479.75,
            ticks_per_us: 0.000_768,
            state: TransportState::CountIn,
            count_in_beats_left: 2,
            lap: 0x0a0b_0c0d,
        }),
        track_peaks: vec![[0.25, 0.75], [1.5, 0.0]],
    }
}

fn sample_take() -> TakeNotes {
    TakeNotes {
        id: 0x0102_0304,
        length: 15_360,
        origin: Some(0x0A0B_0C0D),
        notes: vec![
            TakeNote {
                start: 0,
                length: 240,
                key: 60,
                velocity: 100,
                from: Some(0x0E0F_1011),
            },
            TakeNote {
                start: 0x0506_0708,
                length: 1,
                key: 127,
                velocity: 1,
                from: None,
            },
        ],
        controls: vec![
            TakeControl {
                tick: 0,
                value: TakeControlValue::Cc {
                    number: 64,
                    value: 127,
                },
            },
            TakeControl {
                tick: 480,
                value: TakeControlValue::PitchBend(-8192),
            },
        ],
    }
}

/// Writes `ui/src/bridge/generated/*.ts`. u64 becomes `number`: every u64 here is a rev, a
/// time in microseconds or Unix milliseconds, a song position in ticks or a message number,
/// far below 2^53.
#[test]
fn export_typescript() {
    let dir = ui_dir().join("src/bridge/generated");
    let cfg = ts_rs::Config::new()
        .with_out_dir(&dir)
        .with_large_int("number");
    Command::export_all(&cfg).unwrap();
    Snapshot::export_all(&cfg).unwrap();
    ServerMessage::export_all(&cfg).unwrap();
    HelloReply::export_all(&cfg).unwrap();
    SendReply::export_all(&cfg).unwrap();
    LiveNotesSnapshot::export_all(&cfg).unwrap();
    std::fs::write(
        dir.join("protocolVersion.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n"
        ),
    )
    .unwrap();
}

#[test]
fn command_json_shape() {
    let v = serde_json::to_value(Command::SetViewport {
        width: 1280.0,
        height: 800.0,
    })
    .unwrap();
    assert_eq!(
        v,
        json!({"type": "setViewport", "width": 1280.0, "height": 800.0})
    );
    assert_eq!(
        serde_json::to_value(Command::ToggleBand {
            band: Band::PianoRoll
        })
        .unwrap(),
        json!({"type": "toggleBand", "band": "pianoRoll"})
    );
    let back: Command = serde_json::from_value(json!({"type": "goArrange"})).unwrap();
    assert_eq!(back, Command::GoArrange);
    let c: Command =
        serde_json::from_value(json!({"type": "settingsSelectOutput", "output": "x"})).unwrap();
    assert_eq!(c, Command::SettingsSelectOutput { output: "x".into() });

    for (command, json) in [
        (Command::PlayStop, json!({"type": "playStop"})),
        (Command::Record, json!({"type": "record"})),
        (Command::GoToStart, json!({"type": "goToStart"})),
        (
            Command::Locate { tick: 7680 },
            json!({"type": "locate", "tick": 7680}),
        ),
        (
            Command::SetTempo { bpm: 96.5 },
            json!({"type": "setTempo", "bpm": 96.5}),
        ),
        (
            Command::SetMeter { num: 6, den: 8 },
            json!({"type": "setMeter", "num": 6, "den": 8}),
        ),
        (
            Command::SetCountIn { bars: 2 },
            json!({"type": "setCountIn", "bars": 2}),
        ),
        (Command::ToggleMetronome, json!({"type": "toggleMetronome"})),
        (Command::ToggleLoop, json!({"type": "toggleLoop"})),
        (
            Command::SetLoop {
                from_bar: 5,
                to_bar: 9,
            },
            json!({"type": "setLoop", "fromBar": 5, "toBar": 9}),
        ),
        (
            Command::ToggleArm { track: 1 },
            json!({"type": "toggleArm", "track": 1}),
        ),
        (
            Command::SelectClip { clip: Some(4) },
            json!({"type": "selectClip", "clip": 4}),
        ),
        (
            Command::SelectClip { clip: None },
            json!({"type": "selectClip", "clip": null}),
        ),
        (Command::DeleteSelected, json!({"type": "deleteSelected"})),
        (
            Command::StepTake { delta: -1 },
            json!({"type": "stepTake", "delta": -1}),
        ),
        (Command::Undo, json!({"type": "undo"})),
        (Command::Redo, json!({"type": "redo"})),
        (Command::NewSong, json!({"type": "newSong"})),
        (Command::OpenSong, json!({"type": "openSong"})),
        (Command::SaveSong, json!({"type": "saveSong"})),
        (
            Command::AnswerSongDialog {
                dialog: 3,
                answer: SongAnswer::DontSave,
            },
            json!({"type": "answerSongDialog", "dialog": 3, "answer": "dontSave"}),
        ),
        (
            Command::AnswerSongDialog {
                dialog: 4,
                answer: SongAnswer::ChooseLocation,
            },
            json!({"type": "answerSongDialog", "dialog": 4, "answer": "chooseLocation"}),
        ),
        (
            Command::AnswerSongDialog {
                dialog: 5,
                answer: SongAnswer::OpenPrevious,
            },
            json!({"type": "answerSongDialog", "dialog": 5, "answer": "openPrevious"}),
        ),
        (
            Command::RejectValue {
                field: ValueField::MeterDen,
                kept: "4".into(),
            },
            json!({"type": "rejectValue", "field": "meterDen", "kept": "4"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }
}

#[test]
fn piano_roll_command_json_shape() {
    for (command, json) in [
        (
            Command::SelectNotes {
                take: 6,
                notes: vec![14, 15, 16],
                mode: SelectMode::Replace,
            },
            json!({"type": "selectNotes", "take": 6, "notes": [14, 15, 16], "mode": "replace"}),
        ),
        (Command::SelectAllNotes, json!({"type": "selectAllNotes"})),
        (
            Command::ClearNoteSelection,
            json!({"type": "clearNoteSelection"}),
        ),
        (
            Command::AddNote {
                take: Some(6),
                start: 960,
                length: 120,
                key: 50,
                velocity: 100,
            },
            json!({"type": "addNote", "take": 6, "start": 960, "length": 120, "key": 50,
                   "velocity": 100}),
        ),
        (
            Command::MoveNotes {
                take: 6,
                notes: vec![14],
                ticks: -120,
                keys: 2,
                copy: true,
            },
            json!({"type": "moveNotes", "take": 6, "notes": [14], "ticks": -120, "keys": 2,
                   "copy": true}),
        ),
        (
            Command::ResizeNotes {
                take: 6,
                notes: vec![14],
                ticks: 240,
            },
            json!({"type": "resizeNotes", "take": 6, "notes": [14], "ticks": 240}),
        ),
        (
            Command::DeleteNotes {
                take: 6,
                notes: vec![1, 2],
            },
            json!({"type": "deleteNotes", "take": 6, "notes": [1, 2]}),
        ),
        (
            Command::ChangeVelocity {
                take: 6,
                notes: vec![14, 15, 16],
                delta: 12,
            },
            json!({"type": "changeVelocity", "take": 6, "notes": [14, 15, 16], "delta": 12}),
        ),
        (
            Command::CopyNotes {
                take: 6,
                notes: vec![14],
            },
            json!({"type": "copyNotes", "take": 6, "notes": [14]}),
        ),
        (
            Command::CutNotes {
                take: 6,
                notes: vec![14],
            },
            json!({"type": "cutNotes", "take": 6, "notes": [14]}),
        ),
        (
            Command::PasteNotes { at: 3840 },
            json!({"type": "pasteNotes", "at": 3840}),
        ),
        (
            Command::SetGrid {
                division: 8,
                triplet: true,
            },
            json!({"type": "setGrid", "division": 8, "triplet": true}),
        ),
        (
            Command::SetMusicalScale {
                root: 9,
                mode: ScaleMode::Minor,
                lock: false,
            },
            json!({"type": "setMusicalScale", "root": 9, "mode": "minor", "lock": false}),
        ),
        (
            Command::Audition {
                key: 50,
                velocity: 100,
            },
            json!({"type": "audition", "key": 50, "velocity": 100}),
        ),
        (Command::AuditionOff, json!({"type": "auditionOff"})),
        (
            Command::SetQuantize {
                division: 16,
                triplet: false,
                strength: 75,
                swing: 0,
                align: QuantizeAlign::StartAndEnd,
            },
            json!({"type": "setQuantize", "division": 16, "triplet": false, "strength": 75,
                   "swing": 0, "align": "startAndEnd"}),
        ),
        (
            Command::Quantize {
                take: 6,
                notes: vec![],
            },
            json!({"type": "quantize", "take": 6, "notes": []}),
        ),
        (
            Command::Unquantize {
                take: 6,
                notes: vec![14],
            },
            json!({"type": "unquantize", "take": 6, "notes": [14]}),
        ),
        (
            Command::SetNotesField {
                take: 6,
                notes: vec![14],
                field: NoteField::Velocity,
                value: 77,
            },
            json!({"type": "setNotesField", "take": 6, "notes": [14], "field": "velocity",
                   "value": 77}),
        ),
        (
            Command::SelectControls {
                take: 6,
                points: vec![0],
                mode: SelectMode::Toggle,
            },
            json!({"type": "selectControls", "take": 6, "points": [0], "mode": "toggle"}),
        ),
        (
            Command::AddControl {
                take: 6,
                tick: 960,
                control: ControlKind::Cc { number: 64 },
                value: 127,
            },
            json!({"type": "addControl", "take": 6, "tick": 960,
                   "control": {"kind": "cc", "number": 64}, "value": 127}),
        ),
        (
            Command::AddControl {
                take: 6,
                tick: 960,
                control: ControlKind::PitchBend,
                value: -8192,
            },
            json!({"type": "addControl", "take": 6, "tick": 960,
                   "control": {"kind": "pitchBend"}, "value": -8192}),
        ),
        (
            Command::MoveControls {
                take: 6,
                points: vec![1, 2],
                ticks: 120,
                value: -512,
            },
            json!({"type": "moveControls", "take": 6, "points": [1, 2], "ticks": 120,
                   "value": -512}),
        ),
        (
            Command::DeleteControls {
                take: 6,
                points: vec![1],
            },
            json!({"type": "deleteControls", "take": 6, "points": [1]}),
        ),
        (
            Command::MoveClip {
                clip: 3,
                ticks: 1920,
                track: 2,
                copy: false,
            },
            json!({"type": "moveClip", "clip": 3, "ticks": 1920, "track": 2, "copy": false}),
        ),
        (
            Command::DuplicateClip { clip: 3 },
            json!({"type": "duplicateClip", "clip": 3}),
        ),
        (
            Command::TrimClip {
                clip: 3,
                edge: ClipEdge::Start,
                ticks: -480,
            },
            json!({"type": "trimClip", "clip": 3, "edge": "start", "ticks": -480}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }
}

#[test]
fn state_json_is_camel_case_and_round_trips() {
    let snap = Snapshot {
        rev: 7,
        state: sample_state(),
    };
    let v = serde_json::to_value(&snap).unwrap();
    assert_eq!(v["rev"], 7);
    assert_eq!(v["state"]["song"]["tempoBpm"], 96.0);
    assert_eq!(v["state"]["song"]["tracks"][0]["instrument"], "placeholder");
    assert_eq!(v["state"]["song"]["tracks"][0]["gainDb"], 0.0);
    assert_eq!(
        v["state"]["song"]["tracks"][0]["clips"][0],
        json!({"id": 4, "name": "ピアノ", "start": 7680, "end": 15360, "take": 2, "takes": 3,
               "takeId": 6, "takeOffset": -480, "noteCount": 23})
    );
    assert_eq!(
        v["state"]["song"]["loopRange"],
        json!({"start": 7680, "end": 15360})
    );
    assert_eq!(v["state"]["transport"]["state"], "recording");
    assert_eq!(v["state"]["transport"]["countInBars"], 1);
    assert_eq!(
        v["state"]["transport"]["recording"],
        json!({"id": 3, "track": 1, "from": 7680, "take": 4, "clip": 4, "lap": 1})
    );
    assert_eq!(
        v["state"]["arrange"],
        json!({"selectedClip": null, "pianoRollClip": 4})
    );
    assert_eq!(v["state"]["workspace"]["viewportWidth"], 1280.0);
    assert_eq!(v["state"]["workspace"]["unfitScales"], json!([150]));
    assert_eq!(v["state"]["workspace"]["layout"]["rightFront"], "shelf");
    assert_eq!(v["state"]["workspace"]["settings"], json!(null));
    assert_eq!(v["state"]["song"]["file"], "changed");
    assert_eq!(
        v["state"]["pianoRoll"],
        json!({"take": 6, "selectedNotes": [{"start": 14, "end": 17}], "selectedControls": [],
               "grid": {"division": 16, "triplet": false},
               "scale": {"root": 0, "mode": "major", "lock": true},
               "quantize": {"division": 16, "triplet": false, "strength": 75, "swing": 0,
                            "align": "start"},
               "clipboardNotes": 0})
    );
    assert_eq!(
        v["state"]["workspace"]["songDialog"],
        json!({"id": 2, "kind": "broken", "title": "はじめての曲.meloet", "previous": true})
    );
    assert_eq!(v["state"]["outputs"]["list"][0]["friendly"], "headphones");
    assert_eq!(
        v["state"]["outputs"]["list"][0]["nodeName"],
        "alsa_output.pci.analog-stereo"
    );
    assert_eq!(v["state"]["audio"]["requestedFrames"], 256);
    assert_eq!(
        v["state"]["status"]["message"],
        json!({"id": "searchingKeyboard"})
    );
    let back: Snapshot = serde_json::from_value(v).unwrap();
    assert_eq!(back, snap);
}

#[test]
fn status_message_json_shape() {
    let v = serde_json::to_value(StatusMessage::BufferFixed {
        actual: 1024,
        requested: 256,
        latency_ms: 53.5,
        latency_ms_at_requested: 13.25,
    })
    .unwrap();
    assert_eq!(
        v,
        json!({"id": "bufferFixed", "actual": 1024, "requested": 256,
               "latencyMs": 53.5, "latencyMsAtRequested": 13.25})
    );

    for (message, json) in [
        (
            StatusMessage::BandFoldedWindowLow { band: Band::Mixer },
            json!({"id": "bandFoldedWindowLow", "band": "mixer"}),
        ),
        (
            StatusMessage::BandFoldedToOpen {
                opened: Band::PianoRoll,
                folded: Band::Mixer,
            },
            json!({"id": "bandFoldedToOpen", "opened": "pianoRoll", "folded": "mixer"}),
        ),
        (
            StatusMessage::BandFoldedThird {
                opened: Band::History,
                folded: Band::Mixer,
            },
            json!({"id": "bandFoldedThird", "opened": "history", "folded": "mixer"}),
        ),
        (
            StatusMessage::BandFoldedScale {
                scale: 150,
                band: Band::Mixer,
            },
            json!({"id": "bandFoldedScale", "scale": 150, "band": "mixer"}),
        ),
        (
            StatusMessage::ValueRejected {
                field: ValueField::Velocity,
                kept: "77".into(),
            },
            json!({"id": "valueRejected", "field": "velocity", "kept": "77"}),
        ),
        (
            StatusMessage::ScaleDoesNotFit {
                requested: 150,
                kept: 125,
            },
            json!({"id": "scaleDoesNotFit", "requested": 150, "kept": 125}),
        ),
        (
            StatusMessage::ScaleWindowGrown { scale: 125 },
            json!({"id": "scaleWindowGrown", "scale": 125}),
        ),
        (
            StatusMessage::ScaleLoweredForScreen { from: 150, to: 125 },
            json!({"id": "scaleLoweredForScreen", "from": 150, "to": 125}),
        ),
        (
            StatusMessage::AudioSystemStopped,
            json!({"id": "audioSystemStopped"}),
        ),
        (
            StatusMessage::RecordingStoppedAudioSystem { take: 2 },
            json!({"id": "recordingStoppedAudioSystem", "take": 2}),
        ),
        (
            StatusMessage::RecordingStoppedOutputMissing {
                output: "ヘッドホン".into(),
                take: 2,
            },
            json!({"id": "recordingStoppedOutputMissing", "output": "ヘッドホン", "take": 2}),
        ),
        (
            StatusMessage::KeyboardInputStopped,
            json!({"id": "keyboardInputStopped"}),
        ),
        (
            StatusMessage::OutputChosen {
                output: "アナログヘッドフォン".into(),
                track: Some(piano()),
            },
            json!({"id": "outputChosen", "output": "アナログヘッドフォン",
                   "track": {"number": 1, "name": "ピアノ"}}),
        ),
        (
            StatusMessage::NewSong {
                tempo_bpm: 96.0,
                meter_num: 4,
                meter_den: 4,
                track: None,
            },
            json!({"id": "newSong", "tempoBpm": 96.0, "meterNum": 4, "meterDen": 4,
                   "track": null}),
        ),
        (
            StatusMessage::CountingIn { beats: 4 },
            json!({"id": "countingIn", "beats": 4}),
        ),
        (
            StatusMessage::Recording {
                track: piano(),
                take: 2,
            },
            json!({"id": "recording", "track": {"number": 1, "name": "ピアノ"}, "take": 2}),
        ),
        (
            StatusMessage::LoopRecording {
                track: piano(),
                from_bar: 5,
                to_bar: 9,
                lap: 2,
            },
            json!({"id": "loopRecording", "track": {"number": 1, "name": "ピアノ"},
                   "fromBar": 5, "toBar": 9, "lap": 2}),
        ),
        (
            StatusMessage::TakeSwitched {
                track: piano(),
                clip: "Aメロ".into(),
                take: 2,
                takes: 3,
            },
            json!({"id": "takeSwitched", "track": {"number": 1, "name": "ピアノ"},
                   "clip": "Aメロ", "take": 2, "takes": 3}),
        ),
        (StatusMessage::NoArmedTrack, json!({"id": "noArmedTrack"})),
        (
            StatusMessage::UndoPending { count: 2 },
            json!({"id": "undoPending", "count": 2}),
        ),
        (
            StatusMessage::Summary {
                loop_bars: Some(BarSpan { from: 5, to: 9 }),
                nothing_selected: true,
                notes: None,
            },
            json!({"id": "summary", "loopBars": {"from": 5, "to": 9}, "nothingSelected": true,
                   "notes": null}),
        ),
        (
            StatusMessage::Summary {
                loop_bars: None,
                nothing_selected: false,
                notes: Some(NoteSelection {
                    count: 3,
                    keys: vec![62, 64, 65],
                    quantized: true,
                }),
            },
            json!({"id": "summary", "loopBars": null, "nothingSelected": false,
                   "notes": {"count": 3, "keys": [62, 64, 65], "quantized": true}}),
        ),
        (
            StatusMessage::OutOfScale {
                key: 63,
                root: 0,
                mode: ScaleMode::Major,
            },
            json!({"id": "outOfScale", "key": 63, "root": 0, "mode": "major"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }
}

fn piano() -> TrackRef {
    TrackRef {
        number: 1,
        name: "ピアノ".into(),
    }
}

#[test]
fn server_message_json_shape() {
    let patch = ServerMessage::Patch(Patch {
        rev: 3,
        changes: vec![Change::Status(StatusView {
            message: Some(StatusMessage::KeyboardFound {
                keyboard: "Keystation 49 MK3".into(),
            }),
        })],
    });
    let v = serde_json::to_value(&patch).unwrap();
    assert_eq!(
        v,
        json!({"kind": "patch", "rev": 3, "changes": [
            {"section": "status",
             "value": {"message": {"id": "keyboardFound", "keyboard": "Keystation 49 MK3"}}}
        ]})
    );
    assert_eq!(serde_json::from_value::<ServerMessage>(v).unwrap(), patch);

    let event = ServerMessage::Event(Event::Note {
        on: true,
        note: 60,
        velocity: 100,
        t_us: 1_234_567,
    });
    let v = serde_json::to_value(&event).unwrap();
    assert_eq!(
        v,
        json!({"kind": "event", "type": "note", "on": true, "note": 60, "velocity": 100,
               "tUs": 1_234_567})
    );
    assert_eq!(serde_json::from_value::<ServerMessage>(v).unwrap(), event);

    let live = ServerMessage::LiveNotes(LiveNotes {
        seq: 12,
        recording: 3,
        notes: vec![
            LiveNote {
                lap: 0,
                index: 0,
                key: 60,
                velocity: 90,
                start: 7680,
                length: Some(240),
            },
            LiveNote {
                lap: 0,
                index: 1,
                key: 64,
                velocity: 80,
                start: 7920,
                length: None,
            },
        ],
    });
    let v = serde_json::to_value(&live).unwrap();
    assert_eq!(
        v,
        json!({"kind": "liveNotes", "seq": 12, "recording": 3, "notes": [
            {"lap": 0, "index": 0, "key": 60, "velocity": 90, "start": 7680, "length": 240},
            {"lap": 0, "index": 1, "key": 64, "velocity": 80, "start": 7920, "length": null}
        ]})
    );
    assert_eq!(serde_json::from_value::<ServerMessage>(v).unwrap(), live);

    let snap = LiveNotesSnapshot {
        seq: 0,
        recording: None,
        notes: vec![],
    };
    assert_eq!(
        serde_json::to_value(&snap).unwrap(),
        json!({"seq": 0, "recording": null, "notes": []})
    );
}

#[test]
fn recovery_json_shape() {
    assert_eq!(
        serde_json::to_value(RecoveryView::Recovered {
            written_ms: 1_794_750_060_000,
            cause: RecoveryCause::Sudden,
            song: Some("はじめての曲".into()),
            takes: 2,
        })
        .unwrap(),
        json!({"kind": "recovered", "writtenMs": 1_794_750_060_000u64, "cause": "sudden",
               "song": "はじめての曲", "takes": 2})
    );
    assert_eq!(
        serde_json::to_value(RecoveryView::Unreadable { song: None }).unwrap(),
        json!({"kind": "unreadable", "song": null})
    );
    for (message, json) in [
        (StatusMessage::Recovered, json!({"id": "recovered"})),
        (
            StatusMessage::RecoveryDiscarded,
            json!({"id": "recoveryDiscarded"}),
        ),
        (
            StatusMessage::RecoveryUnreadable { saved_song: true },
            json!({"id": "recoveryUnreadable", "savedSong": true}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
    }
    for (command, json) in [
        (
            Command::DiscardRecovered,
            json!({"type": "discardRecovered"}),
        ),
        (
            Command::ShowRecoveryFile,
            json!({"type": "showRecoveryFile"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }
}

/// The `section` of a change is the key of that section in the state JSON.
#[test]
fn change_section_names_match_state_keys() {
    let state = sample_state();
    let v = serde_json::to_value(&state).unwrap();
    let all = [
        Change::Song(state.song.clone()),
        Change::Transport(state.transport.clone()),
        Change::Arrange(state.arrange.clone()),
        Change::PianoRoll(state.piano_roll.clone()),
        Change::Workspace(state.workspace.clone()),
        Change::Keyboards(state.keyboards.clone()),
        Change::Outputs(state.outputs.clone()),
        Change::Inputs(state.inputs.clone()),
        Change::Audio(state.audio.clone()),
        Change::Status(state.status.clone()),
    ];
    assert_eq!(all.len(), v.as_object().unwrap().len());
    for change in all {
        let c = serde_json::to_value(&change).unwrap();
        let section = c["section"].as_str().unwrap();
        assert_eq!(c["value"], v[section], "section {section}");
    }
}

#[test]
fn changes_from_gives_only_changed_sections() {
    let old = sample_state();
    assert!(AppState::changes_from(&old, &old).is_empty());

    let mut new = old.clone();
    new.transport.state = TransportState::Stopped;
    new.piano_roll.selected_notes.clear();
    new.workspace.scale = 125;
    new.status.message = Some(StatusMessage::NoArmedTrack);
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(
        changes,
        vec![
            Change::Transport(new.transport.clone()),
            Change::PianoRoll(new.piano_roll.clone()),
            Change::Workspace(new.workspace.clone()),
            Change::Status(new.status.clone())
        ]
    );

    let mut applied = old.clone();
    for c in changes {
        applied.apply(c);
    }
    assert_eq!(applied, new);
}

/// Every position of the table in docs/architecture.md 3-3.
#[test]
fn telemetry_layout_matches_the_table() {
    let t = sample_telemetry();
    let mut b = vec![0xAA; 3];
    t.encode_into(&mut b);

    assert_eq!(b.len(), 72 + 8 * 2);
    assert_eq!(b.len(), t.encoded_len());
    assert_eq!(b[0], 1, "kind");
    assert_eq!(b[1], 2, "version");
    assert_eq!(b[2..4], 2u16.to_le_bytes(), "track count");
    assert_eq!(b[4..8], [0x04, 0x03, 0x02, 0x01], "seq, little-endian");
    assert_eq!(b[8..16], 123_456_789_012_345u64.to_le_bytes(), "sent time");
    assert_eq!(b[16..24], t.keys_low.to_le_bytes(), "keys 0..=63");
    assert_eq!(b[16] & 1, 1, "note 0 is bit 0 of byte 16");
    assert_eq!(b[24..32], t.keys_high.to_le_bytes(), "keys 64..=127");
    assert_eq!(b[31] & 0x80, 0x80, "note 127 is the top bit of byte 31");
    assert_eq!(b[32], 101, "last velocity");
    assert_eq!(b[33], 127, "sustain");
    assert_eq!(b[34], 64, "modulation");
    assert_eq!(b[35], 0b111, "flags");
    assert_eq!(b[36..38], (-8192i16).to_le_bytes(), "pitch bend");
    assert_eq!(b[38..40], 137u16.to_le_bytes(), "load");
    assert_eq!(b[40..44], 0.5f32.to_le_bytes(), "master left");
    assert_eq!(b[44..48], 0.1f32.to_le_bytes(), "master right");
    assert_eq!(b[48..56], (-479.75f64).to_le_bytes(), "heard tick");
    assert_eq!(b[56..64], 0.000_768f64.to_le_bytes(), "ticks per µs");
    assert_eq!(b[64], 1, "transport state: count-in");
    assert_eq!(b[65], 0, "reserved");
    assert_eq!(b[66..68], 2u16.to_le_bytes(), "count-in beats left");
    assert_eq!(b[68..72], [0x0d, 0x0c, 0x0b, 0x0a], "lap");
    assert_eq!(b[72..76], 0.25f32.to_le_bytes(), "track 1 left");
    assert_eq!(b[76..80], 0.75f32.to_le_bytes(), "track 1 right");
    assert_eq!(b[80..84], 1.5f32.to_le_bytes(), "track 2 left");
    assert_eq!(b[84..88], 0.0f32.to_le_bytes(), "track 2 right");

    for (state, byte) in [
        (TransportState::Stopped, 0),
        (TransportState::CountIn, 1),
        (TransportState::Playing, 2),
        (TransportState::Recording, 3),
    ] {
        let t = Telemetry {
            transport: Some(TransportReading {
                state,
                ..TransportReading::default()
            }),
            ..Telemetry::default()
        };
        t.encode_into(&mut b);
        assert_eq!(b[64], byte);
        assert_eq!(b[35], 0b100);
        assert_eq!(Telemetry::decode(&b).unwrap(), t);
    }

    let only_running = Telemetry {
        running: true,
        ..Telemetry::default()
    };
    only_running.encode_into(&mut b);
    assert_eq!(b.len(), 72);
    assert_eq!(b[35], 0b001);
    assert!(b[48..72].iter().all(|&x| x == 0), "no position: zeros");
    assert_eq!(Telemetry::decode(&b).unwrap().transport, None);
    let only_clipped = Telemetry {
        master_clipped: true,
        ..Telemetry::default()
    };
    only_clipped.encode_into(&mut b);
    assert_eq!(b[35], 0b010);
}

#[test]
fn telemetry_round_trips_and_reuses_the_buffer() {
    let t = sample_telemetry();
    let mut b = Vec::new();
    t.encode_into(&mut b);
    assert_eq!(Telemetry::decode(&b).unwrap(), t);

    let cap = b.capacity();
    let ptr = b.as_ptr();
    t.encode_into(&mut b);
    assert_eq!((b.capacity(), b.as_ptr()), (cap, ptr));
}

#[test]
fn telemetry_decode_rejects_bad_input() {
    let mut b = Vec::new();
    sample_telemetry().encode_into(&mut b);

    assert_eq!(
        Telemetry::decode(&b[..71]),
        Err(TelemetryError::TooShort { len: 71 })
    );
    assert_eq!(
        Telemetry::decode(&b[..87]),
        Err(TelemetryError::WrongLength {
            len: 87,
            expected: 88
        })
    );
    let mut wrong = b.clone();
    wrong[0] = 2;
    assert_eq!(Telemetry::decode(&wrong), Err(TelemetryError::WrongKind(2)));
    let mut wrong = b.clone();
    wrong[1] = 1;
    assert_eq!(
        Telemetry::decode(&wrong),
        Err(TelemetryError::WrongVersion(1))
    );
    let mut wrong = b.clone();
    wrong[64] = 4;
    assert_eq!(
        Telemetry::decode(&wrong),
        Err(TelemetryError::WrongTransportState(4))
    );
}

#[test]
fn same_readings_ignores_seq_and_time() {
    let a = sample_telemetry();
    let mut b = a.clone();
    b.seq += 1;
    b.sent_ns += 16_666_667;
    assert!(a.same_readings(&b));
    b.master_peak[0] = 0.6;
    assert!(!a.same_readings(&b));
    let mut c = a.clone();
    c.transport.as_mut().unwrap().tick += 12.8;
    assert!(!a.same_readings(&c));
}

/// Every position of the table in docs/architecture.md 3-4.
#[test]
fn take_layout_matches_the_table() {
    let t = sample_take();
    let b = t.encode();
    assert_eq!(b.len(), 24 + 16 * 2 + 8 * 2);
    assert_eq!(b.len(), t.encoded_len());
    assert_eq!(b[0], 2, "kind");
    assert_eq!(b[1], 2, "version");
    assert_eq!(b[2..4], [0, 0], "reserved");
    assert_eq!(b[4..8], [0x04, 0x03, 0x02, 0x01], "take id");
    assert_eq!(b[8..12], 15_360u32.to_le_bytes(), "length");
    assert_eq!(b[12..16], 2u32.to_le_bytes(), "note count");
    assert_eq!(b[16..20], 2u32.to_le_bytes(), "control count");
    assert_eq!(b[20..24], [0x0D, 0x0C, 0x0B, 0x0A], "origin take id");
    assert_eq!(b[24..28], 0u32.to_le_bytes(), "note 1 start");
    assert_eq!(b[28..32], 240u32.to_le_bytes(), "note 1 length");
    assert_eq!(b[32..36], [60, 100, 0, 0], "note 1 key, velocity, reserved");
    assert_eq!(b[36..40], [0x11, 0x10, 0x0F, 0x0E], "note 1 from");
    assert_eq!(b[40..44], [0x08, 0x07, 0x06, 0x05], "note 2 start");
    assert_eq!(b[48..52], [127, 1, 0, 0], "note 2 key, velocity");
    assert_eq!(b[52..56], [0xFF; 4], "note 2 from: none");
    assert_eq!(b[56..60], 0u32.to_le_bytes(), "control 1 tick");
    assert_eq!(b[60..64], [0, 64, 127, 0], "control 1: CC 64 = 127");
    assert_eq!(b[64..68], 480u32.to_le_bytes(), "control 2 tick");
    assert_eq!(b[68], 1, "control 2: pitch bend");
    assert_eq!(b[70..72], (-8192i16).to_le_bytes(), "control 2 value");

    let plain = TakeNotes {
        origin: None,
        ..TakeNotes::default()
    }
    .encode();
    assert_eq!(plain[20..24], [0; 4], "no origin");
}

#[test]
fn take_round_trips_and_rejects_bad_input() {
    let t = sample_take();
    let b = t.encode();
    assert_eq!(TakeNotes::decode(&b).unwrap(), t);
    assert_eq!(
        TakeNotes::decode(&TakeNotes::default().encode()).unwrap(),
        TakeNotes::default()
    );

    assert_eq!(
        TakeNotes::decode(&b[..23]),
        Err(TakeError::TooShort { len: 23 })
    );
    assert_eq!(
        TakeNotes::decode(&b[..71]),
        Err(TakeError::WrongLength {
            len: 71,
            expected: 72
        })
    );
    let mut wrong = b.clone();
    wrong[0] = 1;
    assert_eq!(TakeNotes::decode(&wrong), Err(TakeError::WrongKind(1)));
    let mut wrong = b.clone();
    wrong[1] = 1;
    assert_eq!(TakeNotes::decode(&wrong), Err(TakeError::WrongVersion(1)));
    let mut wrong = b.clone();
    wrong[60] = 7;
    assert_eq!(
        TakeNotes::decode(&wrong),
        Err(TakeError::WrongControlKind(7))
    );
    let mut huge = b.clone();
    huge[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        TakeNotes::decode(&huge),
        Err(TakeError::WrongLength { .. })
    ));
}

/// Writes the bytes and what `ui/src/bridge/telemetry.ts` and `ui/src/bridge/take.ts` must read
/// from them. `ui/tests/bridge.telemetry.test.ts` and `ui/tests/bridge.take.test.ts` compare.
#[test]
fn write_fixtures_for_the_ui() {
    let dir = ui_dir().join("tests/fixtures");
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str, bytes: &[u8], expected: serde_json::Value| {
        std::fs::write(dir.join(format!("{name}.bin")), bytes).unwrap();
        let mut text = serde_json::to_string_pretty(&expected).unwrap();
        text.push('\n');
        std::fs::write(dir.join(format!("{name}.json")), text).unwrap();
    };

    let t = sample_telemetry();
    let mut bytes = Vec::new();
    t.encode_into(&mut bytes);
    let word = |v: u64, hi: bool| if hi { (v >> 32) as u32 } else { v as u32 };
    let tr = t.transport.unwrap();
    write(
        "telemetry-v2",
        &bytes,
        json!({
            "seq": t.seq,
            "sentUs": t.sent_ns / 1000,
            "keys": [word(t.keys_low, false), word(t.keys_low, true),
                     word(t.keys_high, false), word(t.keys_high, true)],
            "keysDown": [0, 36, 60, 67, 127],
            "lastVelocity": t.last_velocity,
            "sustain": t.sustain,
            "modulation": t.modulation,
            "running": t.running,
            "masterClipped": t.master_clipped,
            "pitchBend": t.pitch_bend,
            "loadPermille": t.load_permille,
            "masterPeak": t.master_peak.map(f64::from),
            "position": {
                "tick": tr.tick,
                "ticksPerUs": tr.ticks_per_us,
                "state": "countIn",
                "countInBeatsLeft": tr.count_in_beats_left,
                "lap": tr.lap,
            },
            "trackPeaks": t.track_peaks.iter().flatten().copied().map(f64::from).collect::<Vec<_>>(),
        }),
    );

    let take = sample_take();
    write(
        "take-v2",
        &take.encode(),
        json!({
            "id": take.id,
            "length": take.length,
            "origin": take.origin.unwrap_or(0),
            "notes": take.notes.iter().map(|n| json!({
                "start": n.start, "length": n.length, "key": n.key, "velocity": n.velocity,
                "from": n.from.unwrap_or(TAKE_NO_FROM),
            })).collect::<Vec<_>>(),
            "controls": [
                {"tick": 0, "kind": "cc", "number": 64, "value": 127},
                {"tick": 480, "kind": "pitchBend", "number": 0, "value": -8192},
            ],
        }),
    );
}
