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
            serial: 1,
            name: "はじめての曲".into(),
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            tempo_changes: 0,
            undo_count: 2,
            redo_count: 1,
            note_count: 23,
            tracks: vec![TrackView {
                id: 7,
                number: 1,
                name: "ピアノ".into(),
                color: 1,
                instrument: InstrumentKind::GrandPiano,
                gain_db: Some(-3.0),
                pan: -20,
                sends: [Some(-15.0), None],
                inserts: [None, Some(EffectKind::Compressor), None, None],
                muted: true,
                soloed: false,
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
            returns: [
                ReturnView {
                    gain_db: Some(0.0),
                    pan: 0,
                    muted: false,
                    inserts: [Some(EffectKind::Reverb), None, None, None],
                },
                ReturnView {
                    gain_db: None,
                    pan: 100,
                    muted: true,
                    inserts: [Some(EffectKind::Delay), None, None, None],
                },
            ],
            master: MasterView {
                gain_db: Some(0.0),
                inserts: [Some(EffectKind::Limiter), None, None, None],
            },
            play_target: Some(7),
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
                track: 7,
                from: 7680,
                take: 4,
                clip: Some(4),
                lap: 1,
            }),
        },
        arrange: ArrangeView {
            selected_clips: vec![4, 9],
            selected_clip: None,
            piano_roll_clip: Some(4),
            clipboard_clips: 2,
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
        shelf: ShelfView {
            mode: ShelfMode::Replace { track: 7 },
            available: vec![InstrumentKind::GrandPiano, InstrumentKind::Sine],
            favorites: vec![InstrumentKind::GrandPiano, InstrumentKind::StudioKit],
            selected: Some(InstrumentKind::Sine),
            current: Some(InstrumentKind::GrandPiano),
            loading: None,
            sample: Some(SampleView {
                instrument: InstrumentKind::GrandPiano,
                started_us: 1_000_000,
                length_us: 4_000_000,
            }),
            selected_effect: None,
        },
        params: ParamsView {
            selected: Some(SelectedInsert {
                insert: InsertRef {
                    strip: StripRef::Return { bus: SendBus::A },
                    slot: 0,
                },
                effect: EffectKind::Reverb,
                values: vec![2.4, 60.0, 20.0, 6000.0, 120.0, 100.0, 100.0],
                row: Some(0),
            }),
        },
        keyboards: KeyboardsView {
            list: vec![],
            active: None,
            lost: None,
            input_stopped: false,
            velocity_fixed: VelocityFixed {
                on: true,
                value: DEFAULT_FIXED_VELOCITY,
            },
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
        return_peaks: [[0.125, 0.0625], [0.0, 2.0]],
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
    InstrumentGroup::export_all(&cfg).unwrap();
    let groups: Vec<serde_json::Value> = InstrumentGroup::ALL
        .iter()
        .map(|&group| {
            let instruments: Vec<InstrumentKind> = InstrumentKind::ALL
                .into_iter()
                .filter(|i| i.group() == group)
                .collect();
            json!({"group": group, "instruments": instruments})
        })
        .collect();
    std::fs::write(
        dir.join("instruments.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ InstrumentGroup }} from \"./InstrumentGroup\";\n\
             import type {{ InstrumentKind }} from \"./InstrumentKind\";\n\n\
             /** The shelf's groups and their instruments, in the order of the shelf. */\n\
             export const INSTRUMENT_GROUPS: readonly {{ readonly group: InstrumentGroup; \
             readonly instruments: readonly InstrumentKind[] }}[] = {};\n",
            serde_json::to_string_pretty(&groups).unwrap()
        ),
    )
    .unwrap();
    let drums: Vec<serde_json::Value> = DRUM_ROWS
        .iter()
        .map(|&(id, key)| json!({"id": id, "key": key}))
        .collect();
    std::fs::write(
        dir.join("drums.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\n\
             /** A drum kit's sounds, top row of the drum piano roll first, and their keys (General MIDI). */\n\
             export const DRUM_ROWS = {} as const;\n\n\
             export type DrumPieceId = (typeof DRUM_ROWS)[number][\"id\"];\n",
            serde_json::to_string_pretty(&drums).unwrap()
        ),
    )
    .unwrap();
    std::fs::write(dir.join("effects.ts"), effects_ts()).unwrap();
    std::fs::write(
        dir.join("protocolVersion.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n"
        ),
    )
    .unwrap();
}

/// The model's kind for a kind of the contract: the same effects in the same order.
fn model_effect(kind: EffectKind) -> meloet_model::effects::EffectKind {
    let at = EffectKind::ALL.iter().position(|&k| k == kind).unwrap();
    meloet_model::effects::EffectKind::ALL[at]
}

/// The parameter tables of `meloet_model::effects`, for the screen.
fn effects_ts() -> String {
    use meloet_model::effects::{ParamShape, Unit};
    // The shortest decimal of the f32, not the f64 next to it (0.2, not 0.20000000298).
    let num = |v: f32| v.to_string().parse::<f64>().unwrap();
    let mut table = serde_json::Map::new();
    for kind in EffectKind::ALL {
        let params: Vec<serde_json::Value> = model_effect(kind)
            .params()
            .iter()
            .map(|p| {
                let (shape, steps): (&str, &[&str]) = match p.shape {
                    ParamShape::Continuous => ("continuous", &[]),
                    ParamShape::Steps(names) => ("steps", names),
                    ParamShape::Switch => ("switch", &[]),
                };
                let unit = match p.unit {
                    Unit::Hz => "hz",
                    Unit::Db => "db",
                    Unit::Ms => "ms",
                    Unit::Seconds => "s",
                    Unit::Percent => "percent",
                    Unit::Octaves => "oct",
                    Unit::Ratio => "ratio",
                    Unit::None => "none",
                };
                json!({"id": p.name, "group": p.group, "shape": shape, "steps": steps,
                       "unit": unit, "min": num(p.min), "max": num(p.max), "default": num(p.default)})
            })
            .collect();
        let name = serde_json::to_value(kind).unwrap();
        table.insert(name.as_str().unwrap().to_owned(), params.into());
    }
    format!(
        "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
         import type {{ EffectKind }} from \"./EffectKind\";\n\n\
         /** The effects that come with meloet, in the order of the shelf. */\n\
         export const EFFECT_KINDS: readonly EffectKind[] = {};\n\n\
         /** The insert slots of every strip. */\n\
         export const INSERT_SLOTS = {INSERT_SLOTS};\n\n\
         /**\n \
         * One parameter of an effect. A value is a number in the parameter's own unit: for\n \
         * `steps` the step's number from 0 (`steps` holds their ids), for `switch` 0 or 1.\n \
         * `group`: the heading it sits under (the EQ's four bands).\n \
         */\n\
         export interface EffectParam {{\n  \
         readonly id: string;\n  \
         readonly group: string | null;\n  \
         readonly shape: \"continuous\" | \"steps\" | \"switch\";\n  \
         readonly steps: readonly string[];\n  \
         readonly unit: \"hz\" | \"db\" | \"ms\" | \"s\" | \"percent\" | \"oct\" | \"ratio\" | \"none\";\n  \
         readonly min: number;\n  \
         readonly max: number;\n  \
         readonly default: number;\n\
         }}\n\n\
         /** The parameters of each effect, in the order of the parameters panel; a parameter's number is its place here. */\n\
         export const EFFECT_PARAMS: {{ readonly [K in EffectKind]: readonly EffectParam[] }} = {};\n",
        serde_json::to_string(&EffectKind::ALL).unwrap(),
        serde_json::to_string_pretty(&table).unwrap()
    )
}

#[test]
fn effects_are_the_models_in_the_same_order() {
    assert_eq!(
        EffectKind::ALL.len(),
        meloet_model::effects::EffectKind::ALL.len()
    );
    for kind in EffectKind::ALL {
        assert_eq!(
            serde_json::to_value(kind).unwrap(),
            model_effect(kind).name(),
            "the contract and the song file use one name"
        );
    }
    assert_eq!(INSERT_SLOTS, meloet_model::INSERT_SLOTS);
    let text = effects_ts();
    assert!(text.contains("\"id\": \"decay\""));
    assert!(text.contains("\"eighthDotted\""));
}

#[test]
fn mixer_command_json_shape() {
    let a = StripRef::Return { bus: SendBus::A };
    let shape = |c: Command, want: serde_json::Value| {
        assert_eq!(serde_json::to_value(&c).unwrap(), want);
        assert_eq!(serde_json::from_value::<Command>(want).unwrap(), c);
    };
    shape(
        Command::SetGain {
            strip: StripRef::Track { id: 4 },
            db: Some(-3.0),
            commit: false,
        },
        json!({"type": "setGain", "strip": {"kind": "track", "id": 4}, "db": -3.0, "commit": false}),
    );
    shape(
        Command::SetGain {
            strip: StripRef::Master,
            db: None,
            commit: true,
        },
        json!({"type": "setGain", "strip": {"kind": "master"}, "db": null, "commit": true}),
    );
    shape(
        Command::SetPan {
            strip: a,
            pan: -20,
            commit: true,
        },
        json!({"type": "setPan", "strip": {"kind": "return", "bus": "a"}, "pan": -20, "commit": true}),
    );
    shape(
        Command::SetSend {
            track: 5,
            bus: SendBus::B,
            db: Some(-18.0),
            commit: true,
        },
        json!({"type": "setSend", "track": 5, "bus": "b", "db": -18.0, "commit": true}),
    );
    let insert = InsertRef { strip: a, slot: 0 };
    let insert_json = json!({"strip": {"kind": "return", "bus": "a"}, "slot": 0});
    shape(
        Command::SetParam {
            insert,
            effect: EffectKind::Reverb,
            param: 0,
            value: 2.5,
            commit: false,
        },
        json!({"type": "setParam", "insert": insert_json, "effect": "reverb", "param": 0,
               "value": 2.5, "commit": false}),
    );
    shape(
        Command::CancelMixPreview,
        json!({"type": "cancelMixPreview"}),
    );
    shape(
        Command::ToggleReturnMute { bus: SendBus::B },
        json!({"type": "toggleReturnMute", "bus": "b"}),
    );
    shape(
        Command::RemoveEffect { insert },
        json!({"type": "removeEffect", "insert": insert_json}),
    );
    shape(
        Command::SelectInsert {
            insert: Some(insert),
        },
        json!({"type": "selectInsert", "insert": insert_json}),
    );
    shape(
        Command::SelectInsert { insert: None },
        json!({"type": "selectInsert", "insert": null}),
    );
    shape(
        Command::SelectParam { param: Some(3) },
        json!({"type": "selectParam", "param": 3}),
    );
    shape(
        Command::ShelfSelectEffect {
            effect: Some(EffectKind::Eq4),
        },
        json!({"type": "shelfSelectEffect", "effect": "eq4"}),
    );
    shape(
        Command::OpenShelf {
            mode: ShelfMode::InsertEffect {
                strip: StripRef::Track { id: 2 },
                slot: 3,
            },
        },
        json!({"type": "openShelf", "mode": {"kind": "insertEffect",
               "strip": {"kind": "track", "id": 2}, "slot": 3}}),
    );
    assert_eq!(
        serde_json::to_value(EffectKind::ALL).unwrap(),
        json!(["eq4", "compressor", "reverb", "delay", "chorus", "limiter"])
    );
    assert_eq!(
        serde_json::to_value(StatusMessage::ParamSelected {
            loop_bars: Some(BarSpan { from: 5, to: 9 }),
            insert,
            effect: EffectKind::Reverb,
            param: 0,
        })
        .unwrap(),
        json!({"id": "paramSelected", "loopBars": {"from": 5, "to": 9}, "insert": insert_json,
               "effect": "reverb", "param": 0})
    );
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
            Command::DeleteTrack { track: 3 },
            json!({"type": "deleteTrack", "track": 3}),
        ),
        (
            Command::RenameTrack {
                track: 3,
                name: "うら".into(),
            },
            json!({"type": "renameTrack", "track": 3, "name": "うら"}),
        ),
        (
            Command::MoveTrack { track: 3, to: 0 },
            json!({"type": "moveTrack", "track": 3, "to": 0}),
        ),
        (
            Command::SetTrackColor { track: 3, color: 5 },
            json!({"type": "setTrackColor", "track": 3, "color": 5}),
        ),
        (
            Command::OpenShelf {
                mode: ShelfMode::NewTrack,
            },
            json!({"type": "openShelf", "mode": {"kind": "newTrack"}}),
        ),
        (
            Command::OpenShelf {
                mode: ShelfMode::Replace { track: 3 },
            },
            json!({"type": "openShelf", "mode": {"kind": "replace", "track": 3}}),
        ),
        (
            Command::ShelfSelect {
                instrument: Some(InstrumentKind::SynthBass),
            },
            json!({"type": "shelfSelect", "instrument": "synthBass"}),
        ),
        (
            Command::ShelfSelect { instrument: None },
            json!({"type": "shelfSelect", "instrument": null}),
        ),
        (Command::ShelfConfirm, json!({"type": "shelfConfirm"})),
        (Command::ShelfCancel, json!({"type": "shelfCancel"})),
        (
            Command::ToggleFavorite {
                instrument: InstrumentKind::StudioKit,
            },
            json!({"type": "toggleFavorite", "instrument": "studioKit"}),
        ),
        (
            Command::PlaySample {
                instrument: InstrumentKind::WarmPad,
            },
            json!({"type": "playSample", "instrument": "warmPad"}),
        ),
        (Command::StopSample, json!({"type": "stopSample"})),
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
        (
            Command::DeleteTake { clip: 4 },
            json!({"type": "deleteTake", "clip": 4}),
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
                range: None,
            },
            json!({"type": "rejectValue", "field": "meterDen", "kept": "4", "range": null}),
        ),
        (
            Command::RejectValue {
                field: ValueField::Position,
                kept: "3.1.000".into(),
                range: Some("1.1.000〜4.4.479".into()),
            },
            json!({"type": "rejectValue", "field": "position", "kept": "3.1.000",
                   "range": "1.1.000〜4.4.479"}),
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
                edge: ClipEdge::Start,
                ticks: 240,
            },
            json!({"type": "resizeNotes", "take": 6, "notes": [14], "edge": "start",
                   "ticks": 240}),
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
                keys: vec![48, 52, 55],
                velocity: 100,
            },
            json!({"type": "audition", "keys": [48, 52, 55], "velocity": 100}),
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
            Command::MoveClips {
                clips: vec![3, 8],
                ticks: 1920,
                tracks: -1,
                copy: false,
            },
            json!({"type": "moveClips", "clips": [3, 8], "ticks": 1920, "tracks": -1,
                   "copy": false}),
        ),
        (
            Command::DuplicateClips { clips: vec![3] },
            json!({"type": "duplicateClips", "clips": [3]}),
        ),
        (
            Command::CopyClips { clips: vec![3, 8] },
            json!({"type": "copyClips", "clips": [3, 8]}),
        ),
        (
            Command::CutClips { clips: vec![3] },
            json!({"type": "cutClips", "clips": [3]}),
        ),
        (
            Command::PasteClips { to: None },
            json!({"type": "pasteClips", "to": null}),
        ),
        (
            Command::PasteClips {
                to: Some(ClipSpot {
                    track: 2,
                    tick: 1000,
                }),
            },
            json!({"type": "pasteClips", "to": {"track": 2, "tick": 1000}}),
        ),
        (
            Command::SelectClips {
                clips: vec![3, 8],
                mode: SelectMode::Add,
            },
            json!({"type": "selectClips", "clips": [3, 8], "mode": "add"}),
        ),
        (Command::SelectAllClips, json!({"type": "selectAllClips"})),
        (
            Command::SelectTrack {
                track: 2,
                from_arm: true,
            },
            json!({"type": "selectTrack", "track": 2, "fromArm": true}),
        ),
        (
            Command::ToggleMute { track: 2 },
            json!({"type": "toggleMute", "track": 2}),
        ),
        (
            Command::ToggleSolo { track: 2 },
            json!({"type": "toggleSolo", "track": 2}),
        ),
        (
            Command::MatchVelocity {
                take: 6,
                notes: vec![1, 2, 3],
                value: 100,
            },
            json!({"type": "matchVelocity", "take": 6, "notes": [1, 2, 3], "value": 100}),
        ),
        (
            Command::MatchLength {
                take: 6,
                notes: vec![1, 2, 3],
                value: 360,
            },
            json!({"type": "matchLength", "take": 6, "notes": [1, 2, 3], "value": 360}),
        ),
        (
            Command::ClampValue {
                field: ValueField::ControlValue,
                value: "+8191".into(),
                range: Some("−8192〜+8191".into()),
            },
            json!({"type": "clampValue", "field": "controlValue", "value": "+8191",
                   "range": "−8192〜+8191"}),
        ),
        (
            Command::SetVelocityFixed {
                on: true,
                value: 90,
            },
            json!({"type": "setVelocityFixed", "on": true, "value": 90}),
        ),
        (
            Command::ClampValue {
                field: ValueField::MeterDen,
                value: "4".into(),
                range: None,
            },
            json!({"type": "clampValue", "field": "meterDen", "value": "4", "range": null}),
        ),
        (
            Command::ClampValue {
                field: ValueField::PlayPosition,
                value: "9. 4. 479".into(),
                range: Some("1. 1. 000〜999. 4. 479".into()),
            },
            json!({"type": "clampValue", "field": "playPosition", "value": "9. 4. 479",
                   "range": "1. 1. 000〜999. 4. 479"}),
        ),
        (Command::SaveSongAs, json!({"type": "saveSongAs"})),
        (Command::Quit, json!({"type": "quit"})),
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
    assert_eq!(v["state"]["song"]["tracks"][0]["instrument"], "grandPiano");
    assert_eq!(v["state"]["song"]["tracks"][0]["id"], 7);
    assert_eq!(v["state"]["song"]["tracks"][0]["number"], 1);
    assert_eq!(v["state"]["song"]["playTarget"], 7);
    assert_eq!(
        v["state"]["shelf"],
        json!({"mode": {"kind": "replace", "track": 7},
               "available": ["grandPiano", "sine"],
               "favorites": ["grandPiano", "studioKit"],
               "selected": "sine", "current": "grandPiano", "loading": null,
               "sample": {"instrument": "grandPiano", "startedUs": 1_000_000,
                          "lengthUs": 4_000_000},
               "selectedEffect": null})
    );
    let track = &v["state"]["song"]["tracks"][0];
    assert_eq!(track["gainDb"], -3.0);
    assert_eq!(track["pan"], -20);
    assert_eq!(track["sends"], json!([-15.0, null]), "null is −∞");
    assert_eq!(track["inserts"], json!([null, "compressor", null, null]));
    assert_eq!(
        v["state"]["song"]["returns"],
        json!([{"gainDb": 0.0, "pan": 0, "muted": false, "inserts": ["reverb", null, null, null]},
               {"gainDb": null, "pan": 100, "muted": true, "inserts": ["delay", null, null, null]}])
    );
    assert_eq!(
        v["state"]["song"]["master"],
        json!({"gainDb": 0.0, "inserts": ["limiter", null, null, null]})
    );
    assert_eq!(
        v["state"]["params"]["selected"]["insert"],
        json!({"strip": {"kind": "return", "bus": "a"}, "slot": 0})
    );
    assert_eq!(v["state"]["params"]["selected"]["effect"], "reverb");
    assert_eq!(v["state"]["params"]["selected"]["values"][3], 6000.0);
    assert_eq!(v["state"]["params"]["selected"]["row"], 0);
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
        json!({"id": 3, "track": 7, "from": 7680, "take": 4, "clip": 4, "lap": 1})
    );
    assert_eq!(
        v["state"]["arrange"],
        json!({"selectedClips": [4, 9], "selectedClip": null, "pianoRollClip": 4,
               "clipboardClips": 2})
    );
    assert_eq!(v["state"]["song"]["tracks"][0]["muted"], true);
    assert_eq!(v["state"]["song"]["tracks"][0]["soloed"], false);
    assert_eq!(
        v["state"]["keyboards"]["velocityFixed"],
        json!({"on": true, "value": 100})
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
                range: None,
            },
            json!({"id": "valueRejected", "field": "velocity", "kept": "77", "range": null}),
        ),
        (
            StatusMessage::ValueClamped {
                field: ValueField::Velocity,
                value: "127".into(),
                range: None,
            },
            json!({"id": "valueClamped", "field": "velocity", "value": "127", "range": null}),
        ),
        (
            StatusMessage::VelocityMatched {
                count: 3,
                value: 100,
            },
            json!({"id": "velocityMatched", "count": 3, "value": 100}),
        ),
        (
            StatusMessage::LengthMatched {
                count: 3,
                value: 360,
            },
            json!({"id": "lengthMatched", "count": 3, "value": 360}),
        ),
        (
            StatusMessage::ValueClamped {
                field: ValueField::Beat,
                value: "3. 4. 000".into(),
                range: Some("1〜4".into()),
            },
            json!({"id": "valueClamped", "field": "beat", "value": "3. 4. 000", "range": "1〜4"}),
        ),
        (
            StatusMessage::ValueClamped {
                field: ValueField::Bar,
                value: "1. 1. 000".into(),
                range: None,
            },
            json!({"id": "valueClamped", "field": "bar", "value": "1. 1. 000", "range": null}),
        ),
        (
            StatusMessage::ValueClamped {
                field: ValueField::Tick,
                value: "3. 4. 479".into(),
                range: Some("0〜479".into()),
            },
            json!({"id": "valueClamped", "field": "tick", "value": "3. 4. 479", "range": "0〜479"}),
        ),
        (StatusMessage::ClipsNotMoved, json!({"id": "clipsNotMoved"})),
        (
            StatusMessage::ClipsNotPasted,
            json!({"id": "clipsNotPasted"}),
        ),
        (
            StatusMessage::ClipsNotDuplicated,
            json!({"id": "clipsNotDuplicated"}),
        ),
        (
            StatusMessage::NotesNotWritten,
            json!({"id": "notesNotWritten"}),
        ),
        (
            StatusMessage::NotesNotResized,
            json!({"id": "notesNotResized"}),
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
        (
            StatusMessage::ArmMoved { track: piano() },
            json!({"id": "armMoved", "track": {"number": 1, "name": "ピアノ"}}),
        ),
        (
            StatusMessage::TrackAdded { track: piano() },
            json!({"id": "trackAdded", "track": {"number": 1, "name": "ピアノ"}}),
        ),
        (
            StatusMessage::TrackDeleted { track: piano() },
            json!({"id": "trackDeleted", "track": {"number": 1, "name": "ピアノ"}}),
        ),
        (
            StatusMessage::InstrumentReplaced {
                track: piano(),
                instrument: InstrumentKind::ElectricPiano,
            },
            json!({"id": "instrumentReplaced", "track": {"number": 1, "name": "ピアノ"},
                   "instrument": "electricPiano"}),
        ),
        (
            StatusMessage::AuditionNewTrack {
                instrument: InstrumentKind::SynthBass,
            },
            json!({"id": "auditionNewTrack", "instrument": "synthBass"}),
        ),
        (
            StatusMessage::AuditionReplace {
                instrument: InstrumentKind::ElectricPiano,
                track: piano(),
                current: InstrumentKind::GrandPiano,
            },
            json!({"id": "auditionReplace", "instrument": "electricPiano",
                   "track": {"number": 1, "name": "ピアノ"}, "current": "grandPiano"}),
        ),
        (
            StatusMessage::SamplePlaying {
                instrument: InstrumentKind::GrandPiano,
            },
            json!({"id": "samplePlaying", "instrument": "grandPiano"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }
}

#[test]
fn instruments_are_in_shelf_order_with_their_groups() {
    let names: Vec<serde_json::Value> = InstrumentKind::ALL
        .iter()
        .map(|i| serde_json::to_value(i).unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "grandPiano",
            "electricPiano",
            "organ",
            "fingerBass",
            "synthBass",
            "studioKit",
            "electroKit",
            "strings",
            "warmPad",
            "synthLead",
            "sine"
        ]
    );
    let mut groups: Vec<InstrumentGroup> = InstrumentKind::ALL.iter().map(|i| i.group()).collect();
    groups.dedup();
    assert_eq!(groups, InstrumentGroup::ALL);
    assert_eq!(
        serde_json::to_value(InstrumentGroup::ALL).unwrap(),
        json!(["pianoKeys", "bass", "drums", "padStrings", "synth"])
    );
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
        Change::Shelf(state.shelf.clone()),
        Change::Params(state.params.clone()),
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
    new.shelf.selected = None;
    new.params.selected = None;
    new.status.message = Some(StatusMessage::NoArmedTrack);
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(
        changes,
        vec![
            Change::Transport(new.transport.clone()),
            Change::PianoRoll(new.piano_roll.clone()),
            Change::Workspace(new.workspace.clone()),
            Change::Shelf(new.shelf.clone()),
            Change::Params(new.params.clone()),
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

    assert_eq!(b.len(), 88 + 8 * 2);
    assert_eq!(b.len(), t.encoded_len());
    assert_eq!(b[0], 1, "kind");
    assert_eq!(b[1], 3, "version");
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
    assert_eq!(b[72..76], 0.125f32.to_le_bytes(), "return A left");
    assert_eq!(b[76..80], 0.0625f32.to_le_bytes(), "return A right");
    assert_eq!(b[80..84], 0.0f32.to_le_bytes(), "return B left");
    assert_eq!(b[84..88], 2.0f32.to_le_bytes(), "return B right");
    assert_eq!(b[88..92], 0.25f32.to_le_bytes(), "track 1 left");
    assert_eq!(b[92..96], 0.75f32.to_le_bytes(), "track 1 right");
    assert_eq!(b[96..100], 1.5f32.to_le_bytes(), "track 2 left");
    assert_eq!(b[100..104], 0.0f32.to_le_bytes(), "track 2 right");

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
    assert_eq!(b.len(), 88);
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
        Telemetry::decode(&b[..87]),
        Err(TelemetryError::TooShort { len: 87 })
    );
    assert_eq!(
        Telemetry::decode(&b[..103]),
        Err(TelemetryError::WrongLength {
            len: 103,
            expected: 104
        })
    );
    let mut wrong = b.clone();
    wrong[0] = 2;
    assert_eq!(Telemetry::decode(&wrong), Err(TelemetryError::WrongKind(2)));
    let mut wrong = b.clone();
    wrong[1] = 2;
    assert_eq!(
        Telemetry::decode(&wrong),
        Err(TelemetryError::WrongVersion(2))
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
    let mut d = a.clone();
    d.return_peaks[1][0] = 0.5;
    assert!(!a.same_readings(&d));
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
        "telemetry-v3",
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
            "returnPeaks": t.return_peaks.iter().flatten().copied().map(f64::from).collect::<Vec<_>>(),
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
