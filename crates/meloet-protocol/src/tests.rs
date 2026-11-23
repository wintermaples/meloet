use std::path::PathBuf;

use serde_json::json;
use ts_rs::TS;

use super::*;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

const SURGE_VST3: &str = "vst3:535247455375726758545379000000AA";
const SURGE_LV2: &str = "lv2:https://surge-synthesizer.github.io/lv2/surge-xt";
const MVERB: &str = "lv2:http://distrho.sf.net/plugins/MVerb";

fn ui_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui")
}

/// Mock F6-Tempo, with the marker of R10-Markers just added.
fn sample_timeline() -> TimelineView {
    TimelineView {
        tempo_bpm: 96.0,
        meter_num: 4,
        meter_den: 4,
        head_ramp: false,
        tempo_points: vec![
            TempoPointView {
                tick: 4 * BAR,
                tempo_bpm: 104.0,
                ramp: true,
            },
            TempoPointView {
                tick: 8 * BAR,
                tempo_bpm: 120.0,
                ramp: false,
            },
        ],
        meter_changes: vec![MeterChangeView {
            bar: 8,
            meter_num: 3,
            meter_den: 4,
        }],
        markers: vec![
            MarkerView {
                id: 1,
                tick: 0,
                name: "イントロ".into(),
            },
            MarkerView {
                id: 4,
                tick: 6 * BAR,
                name: "マーカー 4".into(),
            },
        ],
        selected: Some(TimelineRef::Marker { id: 4 }),
        renaming: Some(4),
        open: true,
    }
}

fn volume_lane() -> LaneRef {
    LaneRef {
        track: 7,
        target: AutoTarget::Volume,
    }
}

pub(super) fn sample_state() -> AppState {
    AppState {
        automation: AutomationView {
            folded: vec![LaneRef {
                track: 7,
                target: AutoTarget::Insert {
                    insert: 12,
                    param: 0,
                },
            }],
            armed: vec![volume_lane()],
            writing: vec![WritingLane {
                lane: volume_lane(),
                from_tick: 6 * BAR,
            }],
            held: vec![],
            selected: Some(AutoSelection {
                lane_id: 31,
                points: vec![IndexRange { start: 1, end: 4 }],
            }),
        },
        history: HistoryView {
            total: 3,
            redo: 1,
            current: 2,
            newest: 9,
        },
        song: SongView {
            keys_target: None,
            serial: 1,
            name: "はじめての曲".into(),
            undo_count: 2,
            redo_count: 1,
            note_count: 23,
            tracks: vec![TrackView {
                kind: TrackKindView::Keys,
                id: 7,
                number: 1,
                name: "ピアノ".into(),
                color: 1,
                sound: SoundView::Builtin {
                    instrument: InstrumentKind::GrandPiano,
                },
                stand_in: None,
                drum_keys: None,
                gain_db: Some(-3.0),
                pan: -20,
                sends: [Some(-15.0), None],
                send_defaults: [Some(-15.0), None],
                slots: [
                    None,
                    Some(InsertView::builtin(12, EffectKind::Compressor)),
                    None,
                    None,
                ],
                muted: true,
                soloed: false,
                lanes: vec![
                    LaneView {
                        target: AutoTarget::Volume,
                        name: LaneName::Volume,
                        lane_id: 31,
                        points: 5,
                        step: false,
                        resting: false,
                    },
                    LaneView {
                        target: AutoTarget::Insert {
                            insert: 12,
                            param: 0,
                        },
                        name: LaneName::Builtin {
                            effect: EffectKind::Compressor,
                            param: 0,
                        },
                        lane_id: 34,
                        points: 2,
                        step: false,
                        resting: true,
                    },
                ],
                clips: vec![ClipView {
                    audio: None,
                    id: 4,
                    name: "ピアノ".into(),
                    start: 4 * BAR,
                    end: 8 * BAR,
                    take: 2,
                    takes: 3,
                    take_id: 6,
                    take_offset: -(Q as i64),
                    note_count: 23,
                }],
            }],
            returns: [
                ReturnView {
                    gain_db: Some(0.0),
                    pan: 0,
                    muted: false,
                    slots: [
                        Some(InsertView::builtin(1, EffectKind::Reverb)),
                        None,
                        None,
                        None,
                    ],
                },
                ReturnView {
                    gain_db: None,
                    pan: 100,
                    muted: true,
                    slots: [
                        Some(InsertView::builtin(2, EffectKind::Delay)),
                        None,
                        None,
                        None,
                    ],
                },
            ],
            master: MasterView {
                gain_db: Some(0.0),
                pan: -5,
                slots: [
                    Some(InsertView::builtin(3, EffectKind::Limiter)),
                    None,
                    None,
                    None,
                ],
            },
            play_target: Some(7),
            loop_range: Some(TickSpan {
                start: 4 * BAR,
                end: 8 * BAR,
            }),
            loop_on: true,
            file: SongFile::Changed,
        },
        timeline: sample_timeline(),
        transport: TransportView {
            recording_vocals: Vec::new(),
            recording_faults: None,
            state: TransportState::Recording,
            position: 4 * BAR,
            metronome: true,
            count_in_bars: 1,
            count_in_beats: 4,
            can_play: true,
            can_record: true,
            recording: Some(RecordingView {
                id: 3,
                track: 7,
                from: 4 * BAR,
                take: 4,
                clip: Some(4),
                lap: 1,
            }),
            overdub: false,
        },
        arrange: ArrangeView {
            selected_clips: vec![4, 9],
            selected_clip: None,
            piano_roll_clip: Some(4),
            clipboard_clips: 2,
            tall_track: None,
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
            latency_dialog: None,
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
                vocals: None,
                written_ms: 1_794_750_060_000,
                cause: RecoveryCause::PcEnding,
                song: None,
                takes: 2,
            }),
            file_drop: Some(FileDropView {
                name: "FluidR3_GM.sf2".into(),
                addable: true,
                files: 3,
                sf2: 2,
                audio: 0,
            }),
        },
        library: LibraryView {
            sf2: vec![Sf2FileView {
                file: "FluidR3_GM.sf2".into(),
                presets: vec![
                    Sf2PresetView {
                        bank: 8,
                        program: 27,
                        name: "Clean Guitar".into(),
                        family: GmFamily::Guitar,
                    },
                    Sf2PresetView {
                        bank: 128,
                        program: 0,
                        name: "Standard".into(),
                        family: GmFamily::DrumKit,
                    },
                ],
            }],
            plugins: vec![PluginRowView {
                name: "Surge XT".into(),
                vendor: "Surge Synth Team".into(),
                kind: PluginKindView::Instrument(PluginInstrumentCategory::Synth),
                key: SURGE_LV2.into(),
                formats: vec![
                    PluginFormatView {
                        format: PluginFormat::Vst3,
                        key: SURGE_VST3.into(),
                    },
                    PluginFormatView {
                        format: PluginFormat::Lv2,
                        key: SURGE_LV2.into(),
                    },
                ],
            }],
            opened_sf2: Some("FluidR3_GM.sf2".into()),
            pack_missing: false,
        },
        shelf: ShelfView {
            mode: ShelfMode::Replace { track: 7 },
            available: vec![InstrumentKind::GrandPiano, InstrumentKind::Sine],
            selected_sound: Some(SoundRef::Builtin {
                instrument: InstrumentKind::Sine,
            }),
            current_sound: Some(SoundRef::Plugin {
                key: SURGE_LV2.into(),
            }),
            loading_sound: None,
            sound_sample: Some(SoundSampleView {
                sound: SoundRef::Sf2 {
                    file: "FluidR3_GM.sf2".into(),
                    bank: 8,
                    program: 27,
                },
                playing: None,
            }),
            chosen_effect: Some(EffectRef::Plugin { key: MVERB.into() }),
            current_effect: Some(EffectRef::Builtin {
                effect: EffectKind::Compressor,
            }),
            unreadable: None,
            favorite_sounds: vec![SoundRef::Builtin {
                instrument: InstrumentKind::GrandPiano,
            }],
            favorite_effects: vec![
                EffectRef::Builtin {
                    effect: EffectKind::Reverb,
                },
                EffectRef::Builtin {
                    effect: EffectKind::Delay,
                },
            ],
            missing: vec![MissingView::Sf2 {
                file: "Old Keys.sf2".into(),
                tracks: 2,
            }],
            not_yet: Vec::new(),
        },
        params: ParamsView {
            selected: Some(SelectedParams::Builtin(SelectedInsert {
                insert: InsertRef {
                    strip: StripRef::Return { bus: SendBus::A },
                    slot: 0,
                },
                effect: EffectKind::Reverb,
                values: vec![2.4, 60.0, 20.0, 6000.0, 120.0, 100.0, 100.0],
                row: Some(0),
            })),
            clips: None,
        },
        windows: WindowsView {
            list: vec![PluginWindowView {
                slot: Some(PluginSlotRef::Instrument { track: 7 }),
                name: "Surge XT".into(),
                width: 1597,
                height: 995,
                larger_than_screen: true,
            }],
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
                channels: None,
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
        plugins: PluginsView {
            scan: ScanView::Running {
                done: 37,
                total: 112,
            },
            stopped: vec![StoppedPluginView {
                name: "Granular Fog".into(),
                format: PluginFormat::Lv2,
                reason: ScanStopReason::TimedOut,
                path: "/usr/lib/lv2/granfog.lv2".into(),
                id: Some("urn:granfog:fog".into()),
                shown_path: "/usr/lib/lv2/granfog.lv2".into(),
                retrying: false,
            }],
            places: vec![
                PluginPlaceView {
                    path: "/home/yachiyo/.lv2".into(),
                    shown_path: "~/.lv2".into(),
                    added: false,
                },
                PluginPlaceView {
                    path: "/opt/surge-xt/lib/vst3".into(),
                    shown_path: "/opt/surge-xt/lib/vst3".into(),
                    added: true,
                },
            ],
            stopped_open: false,
            places_open: false,
        },
        inputs: InputsView {
            open: false,
            list: vec![],
            chosen: None,
        },
        audio: AudioView {
            requested_sample_rate: 48_000,
            sample_rate_choices: vec![44_100, 48_000, 96_000],
            buffer_choices: vec![64, 128, 256, 512, 1024],
            latency: LatencyView {
                reported_ms: None,
                measured_ms: None,
                manual_ms: None,
                used: LatencySource::Reported,
                used_ms: None,
            },
            record_format: RecordFormat::Bits24,
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
        audio_work: AudioWorkView::default(),
        export: ExportView::default(),
        sounds: SoundsView::default(),
        status: StatusView {
            message: Some(StatusMessage::SearchingKeyboard),
        },
    }
}

fn sample_telemetry() -> Telemetry {
    Telemetry {
        mics: vec![
            MicMeter {
                track: 6,
                peak: [0.375, 0.375],
                clipped: false,
                stereo: false,
            },
            MicMeter {
                track: 0x0a0b_0c0d,
                peak: [1.25, 0.03125],
                clipped: true,
                stereo: true,
            },
        ],
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
        length: (8 * BAR) as u32,
        origin: Some(0x0A0B_0C0D),
        notes: vec![
            TakeNote {
                start: 0,
                length: (Q / 2) as u32,
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
                tick: Q as u32,
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
    ParamTable::export_all(&cfg).unwrap();
    SendReply::export_all(&cfg).unwrap();
    LiveNotesSnapshot::export_all(&cfg).unwrap();
    HistoryRow::export_all(&cfg).unwrap();
    InstrumentGroup::export_all(&cfg).unwrap();
    let step_names: Vec<serde_json::Value> = StepName::every_kind()
        .iter()
        .map(|name| serde_json::to_value(name).unwrap()["kind"].clone())
        .collect();
    std::fs::write(
        dir.join("stepNames.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ StepName }} from \"./StepName\";\n\n\
             /** Every kind of step name, in the order of the contract. */\n\
             export const STEP_NAME_KINDS: readonly StepName[\"kind\"][] = {};\n",
            serde_json::to_string_pretty(&step_names).unwrap()
        ),
    )
    .unwrap();
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
    let list = |v: serde_json::Value| serde_json::to_string(&v).unwrap();
    std::fs::write(
        dir.join("added.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ GmFamily }} from \"./GmFamily\";\n\
             import type {{ PluginEffectCategory }} from \"./PluginEffectCategory\";\n\
             import type {{ PluginFormat }} from \"./PluginFormat\";\n\
             import type {{ PluginInstrumentCategory }} from \"./PluginInstrumentCategory\";\n\n\
             /** The headings under an SF2 file, in the order of the shelf. */\n\
             export const GM_FAMILIES: readonly GmFamily[] = {};\n\n\
             /** The headings of the plugin instruments, in the order of the shelf. */\n\
             export const PLUGIN_INSTRUMENT_CATEGORIES: readonly PluginInstrumentCategory[] = {};\n\n\
             /** The headings of the plugin effects, in the order of the shelf. */\n\
             export const PLUGIN_EFFECT_CATEGORIES: readonly PluginEffectCategory[] = {};\n\n\
             /** The formats of plugins, in the order a row lists them. */\n\
             export const PLUGIN_FORMATS: readonly PluginFormat[] = {};\n\n\
             /** The bank an SF2 file keeps its drum kits in. */\n\
             export const SF2_DRUM_BANK = {};\n",
            list(json!(GmFamily::ALL)),
            list(json!(PluginInstrumentCategory::ALL)),
            list(json!(PluginEffectCategory::ALL)),
            list(json!([PluginFormat::Vst3, PluginFormat::Lv2])),
            meloet_model::SF2_DRUM_BANK,
        ),
    )
    .unwrap();
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
    let names = |all: &[String]| all.join(" ");
    assert_eq!(
        names(&GmFamily::ALL.map(|f| format!("{f:?}"))),
        names(&meloet_model::GmFamily::ALL.map(|f| format!("{f:?}"))),
        "the families of an SF2 file are the model's, in the same order"
    );
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
            Command::Locate { tick: 4 * BAR },
            json!({"type": "locate", "tick": 4 * BAR}),
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
                start: 2 * Q,
                length: Q / 4,
                key: 50,
                velocity: 100,
            },
            json!({"type": "addNote", "take": 6, "start": 2 * Q, "length": Q / 4, "key": 50,
                   "velocity": 100}),
        ),
        (
            Command::MoveNotes {
                take: 6,
                notes: vec![14],
                ticks: -(Q as i64) / 4,
                keys: 2,
                copy: true,
            },
            json!({"type": "moveNotes", "take": 6, "notes": [14], "ticks": -(Q as i64) / 4, "keys": 2,
                   "copy": true}),
        ),
        (
            Command::ResizeNotes {
                take: 6,
                notes: vec![14],
                edge: ClipEdge::Start,
                ticks: Q as i64 / 2,
            },
            json!({"type": "resizeNotes", "take": 6, "notes": [14], "edge": "start",
                   "ticks": Q / 2}),
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
            Command::PasteNotes { at: 2 * BAR },
            json!({"type": "pasteNotes", "at": 2 * BAR}),
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
                tick: 2 * Q,
                control: ControlKind::Cc { number: 64 },
                value: 127,
            },
            json!({"type": "addControl", "take": 6, "tick": 2 * Q,
                   "control": {"kind": "cc", "number": 64}, "value": 127}),
        ),
        (
            Command::AddControl {
                take: 6,
                tick: 2 * Q,
                control: ControlKind::PitchBend,
                value: -8192,
            },
            json!({"type": "addControl", "take": 6, "tick": 2 * Q,
                   "control": {"kind": "pitchBend"}, "value": -8192}),
        ),
        (
            Command::MoveControls {
                take: 6,
                points: vec![1, 2],
                ticks: Q as i64 / 4,
                value: -512,
            },
            json!({"type": "moveControls", "take": 6, "points": [1, 2], "ticks": Q / 4,
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
                ticks: BAR as i64,
                tracks: -1,
                copy: false,
            },
            json!({"type": "moveClips", "clips": [3, 8], "ticks": BAR, "tracks": -1,
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
                value: 3 * Q / 4,
            },
            json!({"type": "matchLength", "take": 6, "notes": [1, 2, 3], "value": 3 * Q / 4}),
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
                ticks: -(Q as i64),
            },
            json!({"type": "trimClip", "clip": 3, "edge": "start", "ticks": -(Q as i64)}),
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
    assert_eq!(v["state"]["song"].get("tempoBpm"), None);
    assert_eq!(
        v["state"]["timeline"],
        json!({"tempoBpm": 96.0, "meterNum": 4, "meterDen": 4, "headRamp": false,
               "tempoPoints": [{"tick": 4 * BAR, "tempoBpm": 104.0, "ramp": true},
                               {"tick": 8 * BAR, "tempoBpm": 120.0, "ramp": false}],
               "meterChanges": [{"bar": 8, "meterNum": 3, "meterDen": 4}],
               "markers": [{"id": 1, "tick": 0, "name": "イントロ"},
                           {"id": 4, "tick": 6 * BAR, "name": "マーカー 4"}],
               "selected": {"kind": "marker", "id": 4},
               "renaming": 4,
               "open": true})
    );
    assert_eq!(v["state"]["song"]["tracks"][0]["id"], 7);
    assert_eq!(v["state"]["song"]["tracks"][0]["number"], 1);
    assert_eq!(v["state"]["song"]["playTarget"], 7);
    assert_eq!(
        v["state"]["shelf"],
        json!({"mode": {"kind": "replace", "track": 7},
               "available": ["grandPiano", "sine"],
               "selectedSound": {"kind": "builtin", "instrument": "sine"},
               "currentSound": {"kind": "plugin", "key": SURGE_LV2},
               "loadingSound": null,
               "soundSample": {"sound": {"kind": "sf2", "file": "FluidR3_GM.sf2", "bank": 8,
                                         "program": 27},
                               "playing": null},
               "chosenEffect": {"kind": "plugin", "key": MVERB},
               "currentEffect": {"kind": "builtin", "effect": "compressor"},
               "unreadable": null,
               "favoriteSounds": [{"kind": "builtin", "instrument": "grandPiano"}],
               "favoriteEffects": [{"kind": "builtin", "effect": "reverb"},
                                   {"kind": "builtin", "effect": "delay"}],
               "missing": [{"kind": "sf2", "file": "Old Keys.sf2", "tracks": 2}],
               "notYet": []})
    );
    assert_eq!(
        v["state"]["library"],
        json!({"sf2": [{"file": "FluidR3_GM.sf2", "presets": [
                   {"bank": 8, "program": 27, "name": "Clean Guitar", "family": "guitar"},
                   {"bank": 128, "program": 0, "name": "Standard", "family": "drumKit"}]}],
               "plugins": [{"name": "Surge XT", "vendor": "Surge Synth Team",
                            "kind": {"role": "instrument", "category": "synth"},
                            "key": SURGE_LV2,
                            "formats": [{"format": "vst3", "key": SURGE_VST3},
                                        {"format": "lv2", "key": SURGE_LV2}]}],
               "openedSf2": "FluidR3_GM.sf2", "packMissing": false})
    );
    assert_eq!(
        v["state"]["workspace"]["fileDrop"],
        json!({"name": "FluidR3_GM.sf2", "addable": true, "files": 3, "sf2": 2, "audio": 0})
    );
    let track = &v["state"]["song"]["tracks"][0];
    assert_eq!(track["gainDb"], -3.0);
    assert_eq!(track["pan"], -20);
    assert_eq!(track["sends"], json!([-15.0, null]), "null is −∞");
    assert_eq!(track["sendDefaults"], json!([-15.0, null]));
    assert!(track.get("inserts").is_none() && track.get("instrument").is_none());
    assert_eq!(
        track["slots"],
        json!([null, {"id": 12, "effect": {"kind": "builtin", "effect": "compressor"},
                      "bypassed": false},
               null, null])
    );
    assert_eq!(
        track["sound"],
        json!({"kind": "builtin", "instrument": "grandPiano"})
    );
    assert_eq!(track["standIn"], json!(null));
    assert_eq!(track["drumKeys"], json!(null));
    let slot = |id: u32, effect: &str| json!({"id": id, "effect": {"kind": "builtin", "effect": effect}, "bypassed": false});
    assert_eq!(
        v["state"]["song"]["returns"],
        json!([{"gainDb": 0.0, "pan": 0, "muted": false,
                "slots": [slot(1, "reverb"), null, null, null]},
               {"gainDb": null, "pan": 100, "muted": true,
                "slots": [slot(2, "delay"), null, null, null]}])
    );
    assert_eq!(
        v["state"]["song"]["master"],
        json!({"gainDb": 0.0, "pan": -5,
               "slots": [slot(3, "limiter"), null, null, null]})
    );
    assert_eq!(
        v["state"]["params"]["selected"]["insert"],
        json!({"strip": {"kind": "return", "bus": "a"}, "slot": 0})
    );
    assert_eq!(v["state"]["params"]["selected"]["kind"], "builtin");
    assert_eq!(v["state"]["params"]["selected"]["effect"], "reverb");
    assert_eq!(
        v["state"]["windows"],
        json!({"list": [{"slot": {"kind": "instrument", "track": 7}, "name": "Surge XT",
                         "width": 1597, "height": 995, "largerThanScreen": true}]})
    );
    assert_eq!(v["state"]["params"]["selected"]["values"][3], 6000.0);
    assert_eq!(v["state"]["params"]["selected"]["row"], 0);
    assert_eq!(
        v["state"]["song"]["tracks"][0]["clips"][0],
        json!({"id": 4, "name": "ピアノ", "start": 4 * BAR, "end": 8 * BAR, "take": 2, "takes": 3,
               "takeId": 6, "takeOffset": -(Q as i64), "noteCount": 23, "audio": null})
    );
    assert_eq!(
        v["state"]["song"]["loopRange"],
        json!({"start": 4 * BAR, "end": 8 * BAR})
    );
    assert_eq!(v["state"]["transport"]["state"], "recording");
    assert_eq!(v["state"]["transport"]["countInBars"], 1);
    assert_eq!(
        v["state"]["transport"]["recording"],
        json!({"id": 3, "track": 7, "from": 4 * BAR, "take": 4, "clip": 4, "lap": 1})
    );
    assert_eq!(
        v["state"]["arrange"],
        json!({"selectedClips": [4, 9], "selectedClip": null, "pianoRollClip": 4,
               "clipboardClips": 2, "tallTrack": null})
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
                value: 3 * Q / 4,
            },
            json!({"id": "lengthMatched", "count": 3, "value": 3 * Q / 4}),
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
                    names: vec![
                        KeyCount { key: 62, count: 2 },
                        KeyCount { key: 64, count: 1 },
                    ],
                }),
            },
            json!({"id": "summary", "loopBars": null, "nothingSelected": false,
                   "notes": {"count": 3, "keys": [62, 64, 65], "quantized": true,
                             "names": [{"key": 62, "count": 2}, {"key": 64, "count": 1}]}}),
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
                lanes_removed: 0,
                track: piano(),
                instrument: InstrumentKind::ElectricPiano,
            },
            json!({"id": "instrumentReplaced", "track": {"number": 1, "name": "ピアノ"},
                   "instrument": "electricPiano", "lanesRemoved": 0}),
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
        (
            StatusMessage::InstrumentLoading {
                instrument: InstrumentKind::GrandPiano,
            },
            json!({"id": "instrumentLoading", "instrument": "grandPiano"}),
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
            "organDrawbar",
            "organPercussive",
            "organRock",
            "organ",
            "organQuiet",
            "organChurch",
            "organReed",
            "vibraphone",
            "marimba",
            "guitarNylon",
            "guitarSteel",
            "guitarElectric",
            "guitarDistortion",
            "fingerBass",
            "pickBass",
            "woodBass",
            "synthBass",
            "studioKit",
            "electroKit",
            "brushKit",
            "strings",
            "pizzicato",
            "warmPad",
            "trumpet",
            "brassSection",
            "altoSax",
            "flute",
            "synthLead",
            "sine",
        ]
    );
    // Every name goes there and back, and they are all different.
    for (i, kind) in InstrumentKind::ALL.into_iter().enumerate() {
        let back: InstrumentKind = serde_json::from_value(names[i].clone()).unwrap();
        assert_eq!(back, kind);
        assert_eq!(names.iter().filter(|n| **n == names[i]).count(), 1);
    }
    assert!(serde_json::from_value::<InstrumentKind>(json!("kazoo")).is_err());
    let mut groups: Vec<InstrumentGroup> = InstrumentKind::ALL.iter().map(|i| i.group()).collect();
    groups.dedup();
    assert_eq!(groups, InstrumentGroup::ALL);
    assert_eq!(
        serde_json::to_value(InstrumentGroup::ALL).unwrap(),
        json!([
            "pianoKeys",
            "guitar",
            "bass",
            "drums",
            "padStrings",
            "wind",
            "synth"
        ])
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
                start: 4 * BAR,
                length: Some(Q / 2),
            },
            LiveNote {
                lap: 0,
                index: 1,
                key: 64,
                velocity: 80,
                start: 4 * BAR + Q / 2,
                length: None,
            },
        ],
    });
    let v = serde_json::to_value(&live).unwrap();
    assert_eq!(
        v,
        json!({"kind": "liveNotes", "seq": 12, "recording": 3, "notes": [
            {"lap": 0, "index": 0, "key": 60, "velocity": 90, "start": 4 * BAR, "length": Q / 2},
            {"lap": 0, "index": 1, "key": 64, "velocity": 80, "start": 4 * BAR + Q / 2,
             "length": null}
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
            vocals: None,
            written_ms: 1_794_750_060_000,
            cause: RecoveryCause::Sudden,
            song: Some("はじめての曲".into()),
            takes: 2,
        })
        .unwrap(),
        json!({"kind": "recovered", "writtenMs": 1_794_750_060_000u64, "cause": "sudden",
               "song": "はじめての曲", "takes": 2, "vocals": null})
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

/// Protocol 17: SF2 files and plugins in the shelf, on tracks and in insert slots.
#[test]
fn added_things_json_shape() {
    let guitar = SoundRef::Sf2 {
        file: "FluidR3_GM.sf2".into(),
        bank: 0,
        program: 27,
    };
    let guitar_json = json!({"kind": "sf2", "file": "FluidR3_GM.sf2", "bank": 0, "program": 27});
    let surge = SoundRef::Plugin {
        key: SURGE_LV2.into(),
    };
    let surge_json = json!({"kind": "plugin", "key": SURGE_LV2});
    for (command, json) in [
        (
            Command::ShelfSelectSound {
                sound: Some(surge.clone()),
            },
            json!({"type": "shelfSelectSound", "sound": surge_json}),
        ),
        (
            Command::ShelfSelectSound { sound: None },
            json!({"type": "shelfSelectSound", "sound": null}),
        ),
        (
            Command::PlaySampleOf {
                sound: guitar.clone(),
            },
            json!({"type": "playSampleOf", "sound": guitar_json}),
        ),
        (
            Command::ToggleFavoriteSound {
                sound: SoundRef::Builtin {
                    instrument: InstrumentKind::Marimba,
                },
            },
            json!({"type": "toggleFavoriteSound",
                   "sound": {"kind": "builtin", "instrument": "marimba"}}),
        ),
        (
            Command::ToggleFavoriteEffect {
                effect: EffectRef::Builtin {
                    effect: EffectKind::Chorus,
                },
            },
            json!({"type": "toggleFavoriteEffect",
                   "effect": {"kind": "builtin", "effect": "chorus"}}),
        ),
        (
            Command::ShelfChooseEffect {
                effect: Some(EffectRef::Plugin { key: MVERB.into() }),
            },
            json!({"type": "shelfChooseEffect", "effect": {"kind": "plugin", "key": MVERB}}),
        ),
        (
            Command::ShelfChooseEffect { effect: None },
            json!({"type": "shelfChooseEffect", "effect": null}),
        ),
        (
            Command::UsePluginFormat {
                key: SURGE_VST3.into(),
            },
            json!({"type": "usePluginFormat", "key": SURGE_VST3}),
        ),
        (
            Command::OpenPluginWindow { slot: None },
            json!({"type": "openPluginWindow", "slot": null}),
        ),
        (
            Command::OpenPluginWindow {
                slot: Some(PluginSlotRef::Instrument { track: 5 }),
            },
            json!({"type": "openPluginWindow", "slot": {"kind": "instrument", "track": 5}}),
        ),
        (Command::AddSf2, json!({"type": "addSf2"})),
        (Command::CancelFileDrop, json!({"type": "cancelFileDrop"})),
        (
            Command::RemoveMissing {
                missing: MissingRef::Sf2 {
                    file: "Old Keys.sf2".into(),
                },
            },
            json!({"type": "removeMissing", "missing": {"kind": "sf2", "file": "Old Keys.sf2"}}),
        ),
        (
            Command::RemoveMissing {
                missing: MissingRef::Plugin { key: MVERB.into() },
            },
            json!({"type": "removeMissing", "missing": {"kind": "plugin", "key": MVERB}}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }

    let plugin = |state| PluginView {
        key: SURGE_LV2.into(),
        name: "Surge XT".into(),
        format: Some(PluginFormat::Lv2),
        state,
    };
    let check = |value: serde_json::Value, json: serde_json::Value| assert_eq!(value, json);
    check(
        serde_json::to_value(SoundView::Plugin(plugin(PluginState::Loading))).unwrap(),
        json!({"kind": "plugin", "key": SURGE_LV2, "name": "Surge XT", "format": "lv2",
               "state": "loading"}),
    );
    check(
        serde_json::to_value(SoundView::Sf2 {
            file: "FluidR3_GM.sf2".into(),
            bank: 128,
            program: 40,
            name: "Brush".into(),
            family: GmFamily::DrumKit,
        })
        .unwrap(),
        json!({"kind": "sf2", "file": "FluidR3_GM.sf2", "bank": 128, "program": 40,
               "name": "Brush", "family": "drumKit"}),
    );
    check(
        serde_json::to_value(InsertView {
            id: 9,
            effect: InsertEffectView::Plugin(PluginView {
                key: "clap:org.example.verb".into(),
                name: "Some Verb".into(),
                format: None,
                state: PluginState::Missing,
            }),
            bypassed: true,
        })
        .unwrap(),
        json!({"id": 9,
               "effect": {"kind": "plugin", "key": "clap:org.example.verb", "name": "Some Verb",
                          "format": null, "state": "missing"},
               "bypassed": true}),
    );
    check(
        serde_json::to_value([
            PluginState::Ready,
            PluginState::Loading,
            PluginState::Stopped,
            PluginState::Missing,
        ])
        .unwrap(),
        json!(["ready", "loading", "stopped", "missing"]),
    );
    check(
        serde_json::to_value([
            StandIn::PackMissing,
            StandIn::Sf2Missing,
            StandIn::PluginMissing,
            StandIn::PluginUnreadable,
        ])
        .unwrap(),
        json!([
            "packMissing",
            "sf2Missing",
            "pluginMissing",
            "pluginUnreadable"
        ]),
    );
    check(
        serde_json::to_value(GmFamily::ALL).unwrap(),
        json!([
            "piano",
            "mallet",
            "organ",
            "guitar",
            "bass",
            "strings",
            "ensemble",
            "brass",
            "reed",
            "pipe",
            "synthLead",
            "synthPad",
            "synthEffects",
            "ethnic",
            "percussive",
            "soundEffects",
            "drumKit"
        ]),
    );
    check(
        serde_json::to_value(PluginInstrumentCategory::ALL).unwrap(),
        json!(["synth", "sampler", "drum", "keys", "other"]),
    );
    check(
        serde_json::to_value(PluginEffectCategory::ALL).unwrap(),
        json!([
            "eq",
            "dynamics",
            "reverb",
            "delay",
            "modulation",
            "distortion",
            "filter",
            "pitch",
            "spatial",
            "analysis",
            "other"
        ]),
    );
    check(
        serde_json::to_value(PluginKindView::Effect(PluginEffectCategory::Reverb)).unwrap(),
        json!({"role": "effect", "category": "reverb"}),
    );
    check(
        serde_json::to_value(MissingView::Plugin {
            key: MVERB.into(),
            name: "MVerb".into(),
            vendor: "DISTRHO".into(),
            format: Some(PluginFormat::Lv2),
            role: PluginRole::Effect,
            shown_path: Some("~/.lv2/MVerb.lv2".into()),
        })
        .unwrap(),
        json!({"kind": "plugin", "key": MVERB, "name": "MVerb", "vendor": "DISTRHO",
               "format": "lv2", "role": "effect", "shownPath": "~/.lv2/MVerb.lv2"}),
    );
    check(
        serde_json::to_value(SoundSampleView {
            sound: surge,
            playing: Some(SamplePlaying {
                started_us: 5,
                length_us: 6_000_000,
            }),
        })
        .unwrap(),
        json!({"sound": surge_json, "playing": {"startedUs": 5, "lengthUs": 6_000_000}}),
    );

    let track = TrackRef {
        number: 5,
        name: "Surge XT".into(),
    };
    let track_json = json!({"number": 5, "name": "Surge XT"});
    let surge_name = SoundName::Plugin {
        name: "Surge XT".into(),
    };
    let guitar_name = SoundName::Sf2 {
        name: "Clean Guitar".into(),
        file: "FluidR3_GM.sf2".into(),
    };
    let guitar_name_json = json!({"kind": "sf2", "name": "Clean Guitar", "file": "FluidR3_GM.sf2"});
    for (message, json) in [
        (
            StatusMessage::AuditionNewTrackSound {
                sound: surge_name.clone(),
            },
            json!({"id": "auditionNewTrackSound", "sound": {"kind": "plugin", "name": "Surge XT"}}),
        ),
        (
            StatusMessage::AuditionReplaceSound {
                sound: guitar_name.clone(),
                track: track.clone(),
                current: SoundName::Builtin {
                    instrument: InstrumentKind::GrandPiano,
                },
            },
            json!({"id": "auditionReplaceSound", "sound": guitar_name_json, "track": track_json,
                   "current": {"kind": "builtin", "instrument": "grandPiano"}}),
        ),
        (
            StatusMessage::SamplePlayingSound {
                sound: guitar_name.clone(),
            },
            json!({"id": "samplePlayingSound", "sound": guitar_name_json}),
        ),
        (
            StatusMessage::InstrumentReplacedSound {
                lanes_removed: 0,
                track: track.clone(),
                sound: surge_name.clone(),
            },
            json!({"id": "instrumentReplacedSound", "track": track_json,
                   "sound": {"kind": "plugin", "name": "Surge XT"}, "lanesRemoved": 0}),
        ),
        (
            StatusMessage::SoundLoading {
                sound: guitar_name.clone(),
            },
            json!({"id": "soundLoading", "sound": guitar_name_json}),
        ),
        (
            StatusMessage::PluginLoading {
                name: "Surge XT".into(),
            },
            json!({"id": "pluginLoading", "name": "Surge XT"}),
        ),
        (StatusMessage::Sf2Dragging, json!({"id": "sf2Dragging"})),
        (
            StatusMessage::NotSf2Dragging,
            json!({"id": "notSf2Dragging"}),
        ),
        (
            StatusMessage::Sf2Added {
                file: "FluidR3_GM.sf2".into(),
                presets: 128,
                kits: 9,
                skipped: 0,
            },
            json!({"id": "sf2Added", "file": "FluidR3_GM.sf2", "presets": 128, "kits": 9,
                   "skipped": 0}),
        ),
        (
            StatusMessage::Sf2AddedMany {
                added: 2,
                first: "FluidR3_GM.sf2".into(),
                skipped: 1,
            },
            json!({"id": "sf2AddedMany", "added": 2, "first": "FluidR3_GM.sf2", "skipped": 1}),
        ),
        (
            StatusMessage::PluginMissing {
                name: "Surge XT".into(),
                tracks: 1,
            },
            json!({"id": "pluginMissing", "name": "Surge XT", "tracks": 1}),
        ),
        (
            StatusMessage::PluginUnreadable {
                name: "Surge XT".into(),
                tracks: 1,
            },
            json!({"id": "pluginUnreadable", "name": "Surge XT", "tracks": 1}),
        ),
        (
            StatusMessage::InstrumentPluginRemoved {
                track: track.clone(),
            },
            json!({"id": "instrumentPluginRemoved", "track": track_json}),
        ),
        (
            StatusMessage::Sf2Unreadable {
                file: "FluidR3_GM.sf2".into(),
            },
            json!({"id": "sf2Unreadable", "file": "FluidR3_GM.sf2"}),
        ),
        (
            StatusMessage::Sf2Missing {
                file: "FluidR3_GM.sf2".into(),
                tracks: 2,
            },
            json!({"id": "sf2Missing", "file": "FluidR3_GM.sf2", "tracks": 2}),
        ),
        (
            StatusMessage::SoundsMissing { tracks: 4 },
            json!({"id": "soundsMissing", "tracks": 4}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }

    // A change of the library is a section of its own.
    let old = sample_state();
    let mut new = old.clone();
    new.library.opened_sf2 = None;
    new.library.pack_missing = true;
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(changes, [Change::Library(new.library.clone())]);
    let json = serde_json::to_value(&changes[0]).unwrap();
    assert_eq!(json["section"], "library");
    assert_eq!(json["value"]["packMissing"], true);
    let mut applied = old.clone();
    applied.apply(changes[0].clone());
    assert_eq!(applied, new);
}

#[test]
fn plugins_json_shape() {
    let state = sample_state();
    assert_eq!(
        serde_json::to_value(&state.plugins).unwrap(),
        json!({
            "scan": {"state": "running", "done": 37, "total": 112},
            "stopped": [{"name": "Granular Fog", "format": "lv2", "reason": "timedOut",
                         "path": "/usr/lib/lv2/granfog.lv2", "shownPath": "/usr/lib/lv2/granfog.lv2",
                         "id": "urn:granfog:fog", "retrying": false}],
            "places": [
                {"path": "/home/yachiyo/.lv2", "shownPath": "~/.lv2", "added": false},
                {"path": "/opt/surge-xt/lib/vst3", "shownPath": "/opt/surge-xt/lib/vst3", "added": true},
            ],
            "stoppedOpen": false,
            "placesOpen": false,
        })
    );
    for (scan, json) in [
        (ScanView::Idle, json!({"state": "idle"})),
        (
            ScanView::Finished {
                added: 667,
                stopped: 4,
            },
            json!({"state": "finished", "added": 667, "stopped": 4}),
        ),
    ] {
        assert_eq!(serde_json::to_value(scan).unwrap(), json);
    }
    for (reason, json) in [
        (ScanStopReason::Crashed, json!("crashed")),
        (ScanStopReason::TimedOut, json!("timedOut")),
        (ScanStopReason::Broken, json!("broken")),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json);
    }
    assert_eq!(
        serde_json::to_value(PluginFormat::Vst3).unwrap(),
        json!("vst3")
    );

    let instrument = PluginSlotRef::Instrument { track: 5 };
    let insert = PluginSlotRef::Insert {
        insert: InsertRef {
            strip: StripRef::Return { bus: SendBus::A },
            slot: 1,
        },
    };
    let insert_json =
        json!({"kind": "insert", "insert": {"strip": {"kind": "return", "bus": "a"}, "slot": 1}});
    for (message, json) in [
        (
            StatusMessage::PluginsFound { count: 3 },
            json!({"id": "pluginsFound", "count": 3}),
        ),
        (
            StatusMessage::ScanFinished {
                added: 667,
                stopped: 4,
            },
            json!({"id": "scanFinished", "added": 667, "stopped": 4}),
        ),
        (
            StatusMessage::ScanRetryAdded {
                name: "Granular Fog".into(),
            },
            json!({"id": "scanRetryAdded", "name": "Granular Fog"}),
        ),
        (
            StatusMessage::ScanRetryStopped {
                name: "Granular Fog".into(),
            },
            json!({"id": "scanRetryStopped", "name": "Granular Fog"}),
        ),
        (
            StatusMessage::ScanStoppedOpen,
            json!({"id": "scanStoppedOpen"}),
        ),
        (
            StatusMessage::PluginStopped {
                name: "Surge XT".into(),
                slot: instrument,
            },
            json!({"id": "pluginStopped", "name": "Surge XT",
                   "slot": {"kind": "instrument", "track": 5}}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }
    for (command, json) in [
        (Command::RescanPlugins, json!({"type": "rescanPlugins"})),
        (
            Command::RetryStoppedPlugin {
                path: "/usr/lib/lv2/granfog.lv2".into(),
                id: Some("urn:granfog:fog".into()),
            },
            json!({"type": "retryStoppedPlugin", "path": "/usr/lib/lv2/granfog.lv2", "id": "urn:granfog:fog"}),
        ),
        (
            Command::RetryStoppedPlugin {
                path: "/home/me/.vst3/OldChorus.vst3".into(),
                id: None,
            },
            json!({"type": "retryStoppedPlugin", "path": "/home/me/.vst3/OldChorus.vst3", "id": null}),
        ),
        (Command::AddPluginPlace, json!({"type": "addPluginPlace"})),
        (
            Command::RemovePluginPlace {
                path: "/opt/surge-xt/lib/vst3".into(),
            },
            json!({"type": "removePluginPlace", "path": "/opt/surge-xt/lib/vst3"}),
        ),
        (Command::OpenScanStopped, json!({"type": "openScanStopped"})),
        (
            Command::CloseScanStopped,
            json!({"type": "closeScanStopped"}),
        ),
        (
            Command::OpenPluginPlaces,
            json!({"type": "openPluginPlaces"}),
        ),
        (
            Command::TogglePluginPlaces,
            json!({"type": "togglePluginPlaces"}),
        ),
        (
            Command::ReloadPlugin { slot: instrument },
            json!({"type": "reloadPlugin", "slot": {"kind": "instrument", "track": 5}}),
        ),
        (
            Command::RemovePlugin { slot: insert },
            json!({"type": "removePlugin", "slot": insert_json}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }
}

/// Protocol 18: the plugin windows, a plugin in the parameters panel, and what is left of 86.
#[test]
fn windows_and_plugin_params_json_shape() {
    let surge = PluginSlotRef::Instrument { track: 7 };
    let insert = InsertRef {
        strip: StripRef::Track { id: 8 },
        slot: 0,
    };
    let both = |command: Command, json: serde_json::Value| {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    };
    both(
        Command::SelectInstrument { track: 7 },
        json!({"type": "selectInstrument", "track": 7}),
    );
    both(
        Command::SetPluginParam {
            slot: surge,
            table: 3,
            param: 1024,
            value: 0.5,
            commit: false,
        },
        json!({"type": "setPluginParam", "slot": {"kind": "instrument", "track": 7},
               "table": 3, "param": 1024, "value": 0.5, "commit": false}),
    );
    both(
        Command::TypeParam {
            param: ParamRef::Builtin {
                insert,
                effect: EffectKind::Compressor,
                param: 1,
            },
            text: "40".into(),
        },
        json!({"type": "typeParam",
               "param": {"kind": "builtin",
                         "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                         "effect": "compressor", "param": 1},
               "text": "40"}),
    );
    both(
        Command::TypeParam {
            param: ParamRef::Plugin {
                slot: surge,
                table: 3,
                param: 1024,
            },
            text: "440 Hz".into(),
        },
        json!({"type": "typeParam",
               "param": {"kind": "plugin", "slot": {"kind": "instrument", "track": 7},
                         "table": 3, "param": 1024},
               "text": "440 Hz"}),
    );
    both(
        Command::ToggleBypass { insert },
        json!({"type": "toggleBypass",
               "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0}}),
    );
    both(
        Command::MoveInsert { insert, to: 3 },
        json!({"type": "moveInsert",
               "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0}, "to": 3}),
    );
    both(
        Command::ClosePluginWindow { slot: None },
        json!({"type": "closePluginWindow", "slot": null}),
    );
    both(
        Command::RaisePluginWindows,
        json!({"type": "raisePluginWindows"}),
    );
    both(
        Command::OpenShelf {
            mode: ShelfMode::ReplaceEffect {
                strip: StripRef::Track { id: 8 },
                slot: 0,
            },
        },
        json!({"type": "openShelf",
               "mode": {"kind": "replaceEffect", "strip": {"kind": "track", "id": 8},
                        "slot": 0}}),
    );

    let selected = SelectedParams::Plugin(SelectedPlugin {
        slot: surge,
        window: PluginWindowKind::Opens,
        table: Some(3),
        count: 2855,
        row: Some(1024),
        automated: vec![],
    });
    let json = json!({"kind": "plugin", "slot": {"kind": "instrument", "track": 7},
                      "window": "opens", "table": 3, "count": 2855, "row": 1024,
                      "automated": []});
    assert_eq!(serde_json::to_value(&selected).unwrap(), json);
    assert_eq!(
        serde_json::from_value::<SelectedParams>(json).unwrap(),
        selected
    );
    assert_eq!(
        serde_json::to_value([PluginWindowKind::NotYet, PluginWindowKind::None]).unwrap(),
        json!(["notYet", "none"])
    );

    let table = ParamTable {
        table: 3,
        seq: 0,
        groups: vec![
            ParamGroup {
                name: Some("Macros".into()),
                params: vec![ParamRow {
                    id: 1024,
                    name: "Macro 1".into(),
                    kind: ParamKind::Smooth,
                    default: 0.0,
                    value: Some(0.35),
                    text: Some("35.00 %".into()),
                }],
            },
            ParamGroup {
                name: None,
                params: vec![
                    ParamRow {
                        id: 7,
                        name: "Type".into(),
                        kind: ParamKind::Steps {
                            steps: 3,
                            labels: vec!["Room".into(), "Hall".into(), "Plate".into()],
                        },
                        default: 0.0,
                        value: Some(0.5),
                        text: Some("Hall".into()),
                    },
                    ParamRow {
                        id: 8,
                        name: "Freeze".into(),
                        kind: ParamKind::Toggle,
                        default: 0.0,
                        value: None,
                        text: None,
                    },
                ],
            },
        ],
        keyboard_knobs: vec![],
    };
    let json = json!({"table": 3, "seq": 0, "groups": [
        {"name": "Macros", "params": [
            {"id": 1024, "name": "Macro 1", "kind": {"kind": "smooth"}, "default": 0.0,
             "value": 0.35, "text": "35.00 %"}]},
        {"name": null, "params": [
            {"id": 7, "name": "Type",
             "kind": {"kind": "steps", "steps": 3, "labels": ["Room", "Hall", "Plate"]},
             "default": 0.0, "value": 0.5, "text": "Hall"},
            {"id": 8, "name": "Freeze", "kind": {"kind": "toggle"}, "default": 0.0,
             "value": null, "text": null}]}], "keyboardKnobs": []});
    assert_eq!(serde_json::to_value(&table).unwrap(), json);
    assert_eq!(serde_json::from_value::<ParamTable>(json).unwrap(), table);

    let message = ServerMessage::ParamValues(ParamValues {
        table: 3,
        seq: 1,
        values: vec![ParamValue {
            id: 1024,
            value: Some(0.4),
            text: Some("40.00 %".into()),
        }],
    });
    let json = json!({"kind": "paramValues", "table": 3, "seq": 1,
                      "values": [{"id": 1024, "value": 0.4, "text": "40.00 %"}]});
    assert_eq!(serde_json::to_value(&message).unwrap(), json);
    assert_eq!(
        serde_json::from_value::<ServerMessage>(json).unwrap(),
        message
    );

    let compressor = EffectName::Builtin {
        effect: EffectKind::Compressor,
    };
    for (message, json) in [
        (
            StatusMessage::PluginParamSelected {
                loop_bars: None,
                slot: surge,
                plugin: "Surge XT".into(),
                param: "Macro 1".into(),
            },
            json!({"id": "pluginParamSelected", "loopBars": null,
                   "slot": {"kind": "instrument", "track": 7},
                   "plugin": "Surge XT", "param": "Macro 1"}),
        ),
        (
            StatusMessage::ParamClamped {
                to: ParamEnd::Builtin {
                    effect: EffectKind::Compressor,
                    param: 1,
                    value: 20.0,
                },
            },
            json!({"id": "paramClamped",
                   "to": {"kind": "builtin", "effect": "compressor", "param": 1,
                          "value": 20.0}}),
        ),
        (
            StatusMessage::ParamClamped {
                to: ParamEnd::Plugin {
                    name: "Decay".into(),
                    value: "10.0 s".into(),
                    range: "0.1 s〜10.0 s".into(),
                },
            },
            json!({"id": "paramClamped",
                   "to": {"kind": "plugin", "name": "Decay", "value": "10.0 s",
                          "range": "0.1 s〜10.0 s"}}),
        ),
        (
            StatusMessage::EffectInserted {
                insert,
                effect: EffectName::Plugin {
                    name: "MVerb".into(),
                },
            },
            json!({"id": "effectInserted",
                   "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                   "effect": {"kind": "plugin", "name": "MVerb"}}),
        ),
        (
            StatusMessage::EffectRemoved {
                lanes_removed: 0,
                insert,
                effect: compressor.clone(),
            },
            json!({"id": "effectRemoved",
                   "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                   "effect": {"kind": "builtin", "effect": "compressor"}, "lanesRemoved": 0}),
        ),
        (
            StatusMessage::InsertsReordered {
                strip: StripRef::Track { id: 8 },
                order: vec![
                    EffectName::Builtin {
                        effect: EffectKind::Eq4,
                    },
                    compressor.clone(),
                ],
            },
            json!({"id": "insertsReordered", "strip": {"kind": "track", "id": 8},
                   "order": [{"kind": "builtin", "effect": "eq4"},
                             {"kind": "builtin", "effect": "compressor"}]}),
        ),
        (
            StatusMessage::InsertReplaced {
                lanes_removed: 0,
                insert,
                effect: compressor.clone(),
            },
            json!({"id": "insertReplaced",
                   "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                   "effect": {"kind": "builtin", "effect": "compressor"}, "lanesRemoved": 0}),
        ),
        (
            StatusMessage::EffectBypassed {
                insert,
                effect: compressor,
                bypassed: true,
            },
            json!({"id": "effectBypassed",
                   "insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                   "effect": {"kind": "builtin", "effect": "compressor"}, "bypassed": true}),
        ),
        (
            StatusMessage::PluginWindowOpening {
                name: "Surge XT".into(),
                slot: None,
            },
            json!({"id": "pluginWindowOpening", "name": "Surge XT", "slot": null}),
        ),
        (
            StatusMessage::PluginWindowLarge {
                name: "Surge XT".into(),
                slot: Some(surge),
                width: 1597,
                height: 995,
            },
            json!({"id": "pluginWindowLarge", "name": "Surge XT",
                   "slot": {"kind": "instrument", "track": 7}, "width": 1597, "height": 995}),
        ),
        (
            StatusMessage::PluginWindowOpen {
                name: "Surge XT".into(),
            },
            json!({"id": "pluginWindowOpen", "name": "Surge XT"}),
        ),
        (
            StatusMessage::PluginWindowsOpen { count: 2 },
            json!({"id": "pluginWindowsOpen", "count": 2}),
        ),
        (
            StatusMessage::PluginWindowFailed {
                name: "Surge XT".into(),
            },
            json!({"id": "pluginWindowFailed", "name": "Surge XT"}),
        ),
        (
            StatusMessage::MixerPassesBy {
                bypassed: vec![NamedInsert {
                    insert,
                    effect: EffectName::Plugin {
                        name: "MVerb".into(),
                    },
                }],
                stopped: Vec::new(),
            },
            json!({"id": "mixerPassesBy",
                   "bypassed": [{"insert": {"strip": {"kind": "track", "id": 8}, "slot": 0},
                                 "effect": {"kind": "plugin", "name": "MVerb"}}],
                   "stopped": []}),
        ),
        (
            StatusMessage::ParamRejected {
                param: ParamRef::Plugin {
                    slot: surge,
                    table: 3,
                    param: 12,
                },
                kept: "523.25 Hz".into(),
            },
            json!({"id": "paramRejected",
                   "param": {"kind": "plugin", "slot": {"kind": "instrument", "track": 7},
                             "table": 3, "param": 12},
                   "kept": "523.25 Hz"}),
        ),
        (
            StatusMessage::PluginWindowNotYet {
                loop_bars: Some(BarSpan { from: 5, to: 9 }),
                name: "Mossy Verb".into(),
            },
            json!({"id": "pluginWindowNotYet", "loopBars": {"from": 5, "to": 9},
                   "name": "Mossy Verb"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }
}

/// The `section` of a change is the key of that section in the state JSON.
#[test]
fn timeline_json_shape() {
    let commands = [
        (
            Command::SetTimelineOpen { open: true },
            json!({"type": "setTimelineOpen", "open": true}),
        ),
        (
            Command::AddTempoPoint {
                tick: 4 * BAR,
                bpm: 104.0,
            },
            json!({"type": "addTempoPoint", "tick": 4 * BAR, "bpm": 104.0}),
        ),
        (
            Command::MoveTempoPoint {
                tick: 4 * BAR,
                to_tick: 4 * BAR + Q,
                bpm: 104.5,
            },
            json!({"type": "moveTempoPoint", "tick": 4 * BAR, "toTick": 4 * BAR + Q,
                   "bpm": 104.5}),
        ),
        (
            Command::SetTempoRamp { tick: 0, on: true },
            json!({"type": "setTempoRamp", "tick": 0, "on": true}),
        ),
        (
            Command::AddMeterChange {
                bar: 8,
                num: 3,
                den: 4,
            },
            json!({"type": "addMeterChange", "bar": 8, "num": 3, "den": 4}),
        ),
        (
            Command::MoveMeterChange { bar: 8, to_bar: 10 },
            json!({"type": "moveMeterChange", "bar": 8, "toBar": 10}),
        ),
        (
            Command::SetMeterChange {
                bar: 0,
                num: 6,
                den: 8,
            },
            json!({"type": "setMeterChange", "bar": 0, "num": 6, "den": 8}),
        ),
        (
            Command::SelectTimeline {
                target: Some(TimelineRef::TempoPoint { tick: 4 * BAR }),
            },
            json!({"type": "selectTimeline", "target": {"kind": "tempoPoint", "tick": 4 * BAR}}),
        ),
        (
            Command::SelectTimeline { target: None },
            json!({"type": "selectTimeline", "target": null}),
        ),
        (
            Command::DeleteTimeline {
                target: TimelineRef::MeterChange { bar: 8 },
            },
            json!({"type": "deleteTimeline", "target": {"kind": "meterChange", "bar": 8}}),
        ),
        (
            Command::AddMarker {
                name: "マーカー 4".into(),
                tick: None,
            },
            json!({"type": "addMarker", "name": "マーカー 4", "tick": null}),
        ),
        (
            Command::OpenMarkerName { id: 2 },
            json!({"type": "openMarkerName", "id": 2}),
        ),
        (
            Command::RenameMarker {
                id: 2,
                name: "Aメロ ・ 1 回目".into(),
            },
            json!({"type": "renameMarker", "id": 2, "name": "Aメロ ・ 1 回目"}),
        ),
        (Command::CloseMarkerName, json!({"type": "closeMarkerName"})),
        (
            Command::MoveMarker {
                id: 2,
                tick: 5 * BAR,
            },
            json!({"type": "moveMarker", "id": 2, "tick": 5 * BAR}),
        ),
        (
            Command::GoToMarker { id: 2 },
            json!({"type": "goToMarker", "id": 2}),
        ),
        (
            Command::StepMarker { forward: false },
            json!({"type": "stepMarker", "forward": false}),
        ),
        (
            Command::LoopFromMarker { id: 2 },
            json!({"type": "loopFromMarker", "id": 2}),
        ),
    ];
    for (command, want) in commands {
        assert_eq!(serde_json::to_value(&command).unwrap(), want);
        assert_eq!(serde_json::from_value::<Command>(want).unwrap(), command);
    }

    let messages = [
        (
            StatusMessage::TempoRampSelected {
                bar: 5,
                beat: 1,
                tempo_bpm: 104.0,
                to_bar: 9,
                to_beat: 1,
                to_tempo_bpm: 120.0,
            },
            json!({"id": "tempoRampSelected", "bar": 5, "beat": 1, "tempoBpm": 104.0,
                   "toBar": 9, "toBeat": 1, "toTempoBpm": 120.0}),
        ),
        (
            StatusMessage::TempoPointSelected {
                bar: 5,
                beat: 3,
                tempo_bpm: 120.0,
            },
            json!({"id": "tempoPointSelected", "bar": 5, "beat": 3, "tempoBpm": 120.0}),
        ),
        (
            StatusMessage::MeterChangeSelected {
                bar: 9,
                meter_num: 3,
                meter_den: 4,
            },
            json!({"id": "meterChangeSelected", "bar": 9, "meterNum": 3, "meterDen": 4}),
        ),
        (
            StatusMessage::MarkerAdded {
                name: "マーカー 4".into(),
            },
            json!({"id": "markerAdded", "name": "マーカー 4"}),
        ),
        (
            StatusMessage::TempoPointSet {
                bar: 5,
                beat: 1,
                tempo_bpm: 110.0,
            },
            json!({"id": "tempoPointSet", "bar": 5, "beat": 1, "tempoBpm": 110.0}),
        ),
        (
            StatusMessage::MeterChangeSet {
                bar: 1,
                meter_num: 6,
                meter_den: 8,
            },
            json!({"id": "meterChangeSet", "bar": 1, "meterNum": 6, "meterDen": 8}),
        ),
    ];
    for (message, want) in messages {
        assert_eq!(serde_json::to_value(&message).unwrap(), want);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(want).unwrap(),
            message
        );
    }
}

#[test]
fn change_section_names_match_state_keys() {
    let state = sample_state();
    let v = serde_json::to_value(&state).unwrap();
    let all = [
        Change::Song(Box::new(state.song.clone())),
        Change::Timeline(state.timeline.clone()),
        Change::Automation(state.automation.clone()),
        Change::History(state.history),
        Change::Transport(state.transport.clone()),
        Change::Arrange(state.arrange.clone()),
        Change::PianoRoll(state.piano_roll.clone()),
        Change::Workspace(state.workspace.clone()),
        Change::Shelf(state.shelf.clone()),
        Change::Library(state.library.clone()),
        Change::Params(state.params.clone()),
        Change::Plugins(state.plugins.clone()),
        Change::Windows(state.windows.clone()),
        Change::Keyboards(state.keyboards.clone()),
        Change::Outputs(state.outputs.clone()),
        Change::Inputs(state.inputs.clone()),
        Change::Audio(state.audio.clone()),
        Change::AudioWork(state.audio_work.clone()),
        Change::Export(state.export.clone()),
        Change::Sounds(state.sounds.clone()),
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
    new.timeline.selected = None;
    new.automation.writing.clear();
    new.history.current = 3;
    new.transport.state = TransportState::Stopped;
    new.piano_roll.selected_notes.clear();
    new.workspace.scale = 125;
    new.shelf.selected_sound = None;
    new.params.selected = None;
    new.plugins.scan = ScanView::Idle;
    new.windows.list.clear();
    new.status.message = Some(StatusMessage::NoArmedTrack);
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(
        changes,
        vec![
            Change::Timeline(new.timeline.clone()),
            Change::Automation(new.automation.clone()),
            Change::History(new.history),
            Change::Transport(new.transport.clone()),
            Change::PianoRoll(new.piano_roll.clone()),
            Change::Workspace(new.workspace.clone()),
            Change::Shelf(new.shelf.clone()),
            Change::Params(new.params.clone()),
            Change::Plugins(new.plugins.clone()),
            Change::Windows(new.windows.clone()),
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

    assert_eq!(b.len(), 92 + 8 * 2 + 16 * 2);
    assert_eq!(b.len(), t.encoded_len());
    assert_eq!(b[0], 1, "kind");
    assert_eq!(b[1], 4, "version");
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
    assert_eq!(b[88..90], 2u16.to_le_bytes(), "mic count");
    assert_eq!(b[90..92], [0, 0], "spare");
    assert_eq!(b[92..96], 0.25f32.to_le_bytes(), "track 1 left");
    assert_eq!(b[96..100], 0.75f32.to_le_bytes(), "track 1 right");
    assert_eq!(b[100..104], 1.5f32.to_le_bytes(), "track 2 left");
    assert_eq!(b[104..108], 0.0f32.to_le_bytes(), "track 2 right");
    assert_eq!(b[108..112], 6u32.to_le_bytes(), "mic 1 track");
    assert_eq!(b[112..116], 0.375f32.to_le_bytes(), "mic 1 left");
    assert_eq!(b[116..120], 0.375f32.to_le_bytes(), "mic 1 right");
    assert_eq!(b[120..124], [0, 0, 0, 0], "mic 1 flags and spare");
    assert_eq!(b[124..128], [0x0d, 0x0c, 0x0b, 0x0a], "mic 2 track");
    assert_eq!(b[128..132], 1.25f32.to_le_bytes(), "mic 2 left");
    assert_eq!(b[132..136], 0.03125f32.to_le_bytes(), "mic 2 right");
    assert_eq!(b[136..140], [0b11, 0, 0, 0], "mic 2: clipped, stereo");

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
    assert_eq!(b.len(), 92);
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
        Telemetry::decode(&b[..91]),
        Err(TelemetryError::TooShort { len: 91 })
    );
    assert_eq!(
        Telemetry::decode(&b[..139]),
        Err(TelemetryError::WrongLength {
            len: 139,
            expected: 140
        })
    );
    // A mic more in the header than there are bytes for.
    let mut wrong = b.clone();
    wrong[88] = 3;
    assert_eq!(
        Telemetry::decode(&wrong),
        Err(TelemetryError::WrongLength {
            len: 140,
            expected: 156
        })
    );
    let mut wrong = b.clone();
    wrong[0] = 2;
    assert_eq!(Telemetry::decode(&wrong), Err(TelemetryError::WrongKind(2)));
    let mut wrong = b.clone();
    wrong[1] = 3;
    assert_eq!(
        Telemetry::decode(&wrong),
        Err(TelemetryError::WrongVersion(3))
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
    assert_eq!(b[8..12], ((8 * BAR) as u32).to_le_bytes(), "length");
    assert_eq!(b[12..16], 2u32.to_le_bytes(), "note count");
    assert_eq!(b[16..20], 2u32.to_le_bytes(), "control count");
    assert_eq!(b[20..24], [0x0D, 0x0C, 0x0B, 0x0A], "origin take id");
    assert_eq!(b[24..28], 0u32.to_le_bytes(), "note 1 start");
    assert_eq!(b[28..32], ((Q / 2) as u32).to_le_bytes(), "note 1 length");
    assert_eq!(b[32..36], [60, 100, 0, 0], "note 1 key, velocity, reserved");
    assert_eq!(b[36..40], [0x11, 0x10, 0x0F, 0x0E], "note 1 from");
    assert_eq!(b[40..44], [0x08, 0x07, 0x06, 0x05], "note 2 start");
    assert_eq!(b[48..52], [127, 1, 0, 0], "note 2 key, velocity");
    assert_eq!(b[52..56], [0xFF; 4], "note 2 from: none");
    assert_eq!(b[56..60], 0u32.to_le_bytes(), "control 1 tick");
    assert_eq!(b[60..64], [0, 64, 127, 0], "control 1: CC 64 = 127");
    assert_eq!(b[64..68], (Q as u32).to_le_bytes(), "control 2 tick");
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
        "telemetry-v4",
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
            "mics": t.mics.iter().map(|m| json!({
                "track": m.track, "peak": m.peak.map(f64::from),
                "clipped": m.clipped, "stereo": m.stereo,
            })).collect::<Vec<_>>(),
        }),
    );

    let lane = sample_lane();
    write(
        "lane-v1",
        &lane.encode(),
        json!({
            "id": lane.id,
            "step": lane.step,
            "points": lane.points.iter().map(|p| json!({
                "tick": p.tick, "value": f64::from(p.value),
            })).collect::<Vec<_>>(),
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
                {"tick": Q, "kind": "pitchBend", "number": 0, "value": -8192},
            ],
        }),
    );
}

fn sample_lane() -> LanePoints {
    LanePoints {
        id: 0x0102_0304,
        step: true,
        points: vec![
            LanePoint {
                tick: 0,
                value: 0.8,
            },
            LanePoint {
                tick: 0x0000_0001_0000_1E00,
                value: 0.0,
            },
            LanePoint {
                tick: 9_007_199_254_740_991,
                value: 1.0,
            },
        ],
    }
}

/// Every position of the table in docs/architecture.md 21-15.
#[test]
fn lane_layout_matches_the_table() {
    let l = sample_lane();
    let b = l.encode();
    assert_eq!(b.len(), 16 + 16 * 3);
    assert_eq!(b.len(), l.encoded_len());
    assert_eq!(b[0], 3, "kind");
    assert_eq!(b[1], 1, "version");
    assert_eq!(b[2], 1, "flags: step");
    assert_eq!(b[3], 0, "reserved");
    assert_eq!(b[4..8], [0x04, 0x03, 0x02, 0x01], "lane id");
    assert_eq!(b[8..12], 3u32.to_le_bytes(), "point count");
    assert_eq!(b[12..16], [0; 4], "reserved");
    assert_eq!(b[16..24], 0u64.to_le_bytes(), "point 1 tick");
    assert_eq!(b[24..28], 0.8f32.to_le_bytes(), "point 1 value");
    assert_eq!(b[28..32], [0; 4], "point 1 reserved");
    assert_eq!(
        b[32..40],
        [0x00, 0x1E, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00],
        "point 2 tick"
    );
    assert_eq!(b[40..44], 0.0f32.to_le_bytes(), "point 2 value");
    assert_eq!(b[56..60], 1.0f32.to_le_bytes(), "point 3 value");

    let line = LanePoints {
        step: false,
        ..LanePoints::default()
    }
    .encode();
    assert_eq!(line.len(), 16);
    assert_eq!(line[2], 0, "no flags");
}

#[test]
fn lane_round_trips_and_rejects_bad_input() {
    let l = sample_lane();
    let b = l.encode();
    assert_eq!(LanePoints::decode(&b).unwrap(), l);
    assert_eq!(
        LanePoints::decode(&LanePoints::default().encode()).unwrap(),
        LanePoints::default()
    );
    assert_eq!(
        LanePoints::decode(&b[..15]),
        Err(LaneError::TooShort { len: 15 })
    );
    let mut wrong = b.clone();
    wrong[0] = 2;
    assert_eq!(LanePoints::decode(&wrong), Err(LaneError::WrongKind(2)));
    let mut wrong = b.clone();
    wrong[1] = 2;
    assert_eq!(LanePoints::decode(&wrong), Err(LaneError::WrongVersion(2)));
    assert_eq!(
        LanePoints::decode(&b[..b.len() - 1]),
        Err(LaneError::WrongLength {
            len: 63,
            expected: 64
        })
    );
    let mut huge = b.clone();
    huge[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        LanePoints::decode(&huge),
        Err(LaneError::WrongLength { .. })
    ));
}

#[test]
fn automation_json_shape() {
    let v = serde_json::to_value(sample_state()).unwrap();
    assert_eq!(
        v["song"]["tracks"][0]["lanes"],
        json!([{"target": {"kind": "volume"}, "name": {"kind": "volume"}, "laneId": 31,
                "points": 5, "step": false, "resting": false},
               {"target": {"kind": "insert", "insert": 12, "param": 0},
                "name": {"kind": "builtin", "effect": "compressor", "param": 0},
                "laneId": 34, "points": 2, "step": false, "resting": true}])
    );
    assert_eq!(
        v["automation"],
        json!({"folded": [{"track": 7, "target": {"kind": "insert", "insert": 12, "param": 0}}],
               "armed": [{"track": 7, "target": {"kind": "volume"}}],
               "writing": [{"lane": {"track": 7, "target": {"kind": "volume"}},
                            "fromTick": 6 * BAR}],
               "held": [],
               "selected": {"laneId": 31, "points": [{"start": 1, "end": 4}]}})
    );
    assert_eq!(
        serde_json::to_value(AutomationView::default()).unwrap(),
        json!({"folded": [], "armed": [], "writing": [], "held": [], "selected": null})
    );

    let targets = [
        (AutoTarget::Volume, json!({"kind": "volume"})),
        (AutoTarget::Pan, json!({"kind": "pan"})),
        (
            AutoTarget::Send { bus: SendBus::B },
            json!({"kind": "send", "bus": "b"}),
        ),
        (AutoTarget::Mute, json!({"kind": "mute"})),
        (
            AutoTarget::Instrument { param: 1_234_567 },
            json!({"kind": "instrument", "param": 1234567}),
        ),
        (
            AutoTarget::Insert {
                insert: 12,
                param: 3,
            },
            json!({"kind": "insert", "insert": 12, "param": 3}),
        ),
    ];
    for (target, json) in targets {
        assert_eq!(serde_json::to_value(target).unwrap(), json);
        assert_eq!(serde_json::from_value::<AutoTarget>(json).unwrap(), target);
    }
    let names = [
        (
            LaneName::Send { bus: SendBus::A },
            json!({"kind": "send", "bus": "a"}),
        ),
        (LaneName::Mute, json!({"kind": "mute"})),
        (LaneName::Pan, json!({"kind": "pan"})),
        (
            LaneName::Plugin {
                plugin: "Surge XT".into(),
                param: Some("A Filter 1 Cutoff".into()),
            },
            json!({"kind": "plugin", "plugin": "Surge XT", "param": "A Filter 1 Cutoff"}),
        ),
        (
            LaneName::Plugin {
                plugin: "Surge XT".into(),
                param: None,
            },
            json!({"kind": "plugin", "plugin": "Surge XT", "param": null}),
        ),
    ];
    for (name, json) in names {
        assert_eq!(serde_json::to_value(&name).unwrap(), json);
        assert_eq!(serde_json::from_value::<LaneName>(json).unwrap(), name);
    }
}

#[test]
fn automation_and_history_command_json_shape() {
    let volume = AutoTarget::Volume;
    let cases = [
        (
            Command::AddLane {
                track: 7,
                target: AutoTarget::Pan,
            },
            json!({"type": "addLane", "track": 7, "target": {"kind": "pan"}}),
        ),
        (
            Command::DeleteLane {
                track: 7,
                target: volume,
            },
            json!({"type": "deleteLane", "track": 7, "target": {"kind": "volume"}}),
        ),
        (
            Command::FoldLane {
                track: 7,
                target: volume,
                folded: true,
            },
            json!({"type": "foldLane", "track": 7, "target": {"kind": "volume"},
                   "folded": true}),
        ),
        (
            Command::AddAutoPoint {
                lane: 31,
                tick: 19 * Q,
                value: None,
            },
            json!({"type": "addAutoPoint", "lane": 31, "tick": 19 * Q, "value": null}),
        ),
        (
            Command::AddAutoPoint {
                lane: 31,
                tick: 19 * Q,
                value: Some(0.5),
            },
            json!({"type": "addAutoPoint", "lane": 31, "tick": 19 * Q, "value": 0.5}),
        ),
        (
            Command::MoveAutoPoints {
                lane: 31,
                points: vec![1, 2, 3],
                d_tick: -(Q as i64),
                d_value: 0.25,
            },
            json!({"type": "moveAutoPoints", "lane": 31, "points": [1, 2, 3], "dTick": -(Q as i64),
                   "dValue": 0.25}),
        ),
        (
            Command::DeleteAutoPoints {
                lane: 31,
                points: vec![2],
            },
            json!({"type": "deleteAutoPoints", "lane": 31, "points": [2]}),
        ),
        (
            Command::SelectAutoPoints {
                lane: 31,
                points: vec![],
            },
            json!({"type": "selectAutoPoints", "lane": 31, "points": []}),
        ),
        (
            Command::SetLaneWrite {
                track: 7,
                target: volume,
                on: true,
            },
            json!({"type": "setLaneWrite", "track": 7, "target": {"kind": "volume"},
                   "on": true}),
        ),
        (
            Command::RestoreLane {
                track: 7,
                target: AutoTarget::Send { bus: SendBus::A },
            },
            json!({"type": "restoreLane", "track": 7,
                   "target": {"kind": "send", "bus": "a"}}),
        ),
        (
            Command::HistoryJump {
                index: 11,
                step: 14,
            },
            json!({"type": "historyJump", "index": 11, "step": 14}),
        ),
    ];
    for (command, json) in cases {
        assert_eq!(serde_json::to_value(&command).unwrap(), json);
        assert_eq!(serde_json::from_value::<Command>(json).unwrap(), command);
    }
    // `value` may be left out.
    assert_eq!(
        serde_json::from_value::<Command>(json!({"type": "addAutoPoint", "lane": 31, "tick": 0}))
            .unwrap(),
        Command::AddAutoPoint {
            lane: 31,
            tick: 0,
            value: None
        }
    );
}

#[test]
fn history_json_shape() {
    let v = serde_json::to_value(sample_state()).unwrap();
    assert_eq!(
        v["history"],
        json!({"total": 3, "redo": 1, "current": 2, "newest": 9})
    );
    let pad = TrackRef {
        number: 4,
        name: "パッド".into(),
    };
    let rows = [
        (
            HistoryRow {
                index: 16,
                step: 21,
                name: StepName::MoveNotes,
                what: StepWhat::Notes { count: 3 },
                place: Some(StepPlace::PianoRoll),
            },
            json!({"index": 16, "step": 21, "name": {"kind": "moveNotes"},
                   "what": {"kind": "notes", "count": 3}, "place": {"kind": "pianoRoll"}}),
        ),
        (
            HistoryRow {
                index: 13,
                step: 18,
                name: StepName::SetGain,
                what: StepWhat::Gain {
                    strip: StripName::Track { track: pad.clone() },
                    from_db: Some(-10.5),
                    to_db: None,
                },
                place: Some(StepPlace::Mixer),
            },
            json!({"index": 13, "step": 18, "name": {"kind": "setGain"},
                   "what": {"kind": "gain",
                            "strip": {"kind": "track", "track": {"number": 4, "name": "パッド"}},
                            "fromDb": -10.5, "toDb": null},
                   "place": {"kind": "mixer"}}),
        ),
        (
            HistoryRow {
                index: 12,
                step: 17,
                name: StepName::BypassEffect,
                what: StepWhat::Effect {
                    effect: EffectName::Plugin {
                        name: "MVerb".into(),
                    },
                },
                place: Some(StepPlace::Track { track: pad.clone() }),
            },
            json!({"index": 12, "step": 17, "name": {"kind": "bypassEffect"},
                   "what": {"kind": "effect", "effect": {"kind": "plugin", "name": "MVerb"}},
                   "place": {"kind": "track", "track": {"number": 4, "name": "パッド"}}}),
        ),
        (
            HistoryRow {
                index: 10,
                step: 15,
                name: StepName::SetTempo,
                what: StepWhat::Tempo {
                    bar: 5,
                    beat: 1,
                    from_bpm: Some(96.0),
                    to_bpm: Some(120.0),
                },
                place: Some(StepPlace::Arrange),
            },
            json!({"index": 10, "step": 15, "name": {"kind": "setTempo"},
                   "what": {"kind": "tempo", "bar": 5, "beat": 1, "fromBpm": 96.0,
                            "toBpm": 120.0},
                   "place": {"kind": "arrange"}}),
        ),
        (
            HistoryRow {
                index: 2,
                step: 2,
                name: StepName::SetParam {
                    effect: EffectKind::Compressor,
                    param: 0,
                },
                what: StepWhat::Param {
                    from: -18.0,
                    to: -24.0,
                },
                place: Some(StepPlace::Mixer),
            },
            json!({"index": 2, "step": 2,
                   "name": {"kind": "setParam", "effect": "compressor", "param": 0},
                   "what": {"kind": "param", "from": -18.0, "to": -24.0},
                   "place": {"kind": "mixer"}}),
        ),
        (
            HistoryRow {
                index: 3,
                step: 3,
                name: StepName::PluginWindow {
                    plugin: "Surge XT".into(),
                },
                what: StepWhat::None,
                place: Some(StepPlace::PluginWindow {
                    strip: StripName::Return { bus: SendBus::A },
                }),
            },
            json!({"index": 3, "step": 3,
                   "name": {"kind": "pluginWindow", "plugin": "Surge XT"},
                   "what": {"kind": "none"},
                   "place": {"kind": "pluginWindow",
                             "strip": {"kind": "return", "bus": "a"}}}),
        ),
        (
            HistoryRow {
                index: 1,
                step: 1,
                name: StepName::Recovered,
                what: StepWhat::None,
                place: None,
            },
            json!({"index": 1, "step": 1, "name": {"kind": "recovered"},
                   "what": {"kind": "none"}, "place": null}),
        ),
        (
            HistoryRow {
                index: 4,
                step: 4,
                name: StepName::MoveAutoPoints,
                what: StepWhat::Lane {
                    target: LaneName::Volume,
                    points: Some(3),
                },
                place: Some(StepPlace::Track { track: pad.clone() }),
            },
            json!({"index": 4, "step": 4, "name": {"kind": "moveAutoPoints"},
                   "what": {"kind": "lane", "target": {"kind": "volume"}, "points": 3},
                   "place": {"kind": "track", "track": {"number": 4, "name": "パッド"}}}),
        ),
        (
            HistoryRow {
                index: 5,
                step: 5,
                name: StepName::SetMeter,
                what: StepWhat::Meter {
                    bar: 9,
                    from: Some(MeterValue { num: 4, den: 4 }),
                    to: Some(MeterValue { num: 3, den: 4 }),
                },
                place: Some(StepPlace::Arrange),
            },
            json!({"index": 5, "step": 5, "name": {"kind": "setMeter"},
                   "what": {"kind": "meter", "bar": 9, "from": {"num": 4, "den": 4},
                            "to": {"num": 3, "den": 4}},
                   "place": {"kind": "arrange"}}),
        ),
    ];
    for (row, json) in rows {
        assert_eq!(serde_json::to_value(&row).unwrap(), json);
        assert_eq!(serde_json::from_value::<HistoryRow>(json).unwrap(), row);
    }
}

#[test]
fn every_step_name_has_a_kind_of_its_own() {
    let all = StepName::every_kind();
    let mut kinds: Vec<String> = all
        .iter()
        .map(|name| {
            serde_json::to_value(name).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(kinds.len(), 77);
    kinds.sort();
    kinds.dedup();
    assert_eq!(kinds.len(), all.len(), "a kind is listed twice");
}

#[test]
fn history_and_automation_status_json_shape() {
    let piano = piano();
    let cases = [
        (
            StatusMessage::Undone {
                name: StepName::MoveNotes,
                what: StepWhat::Notes { count: 3 },
            },
            json!({"id": "undone", "name": {"kind": "moveNotes"},
                   "what": {"kind": "notes", "count": 3}}),
        ),
        (
            StatusMessage::Redone {
                name: StepName::AddMarker,
                what: StepWhat::Name {
                    name: "サビ".into(),
                },
            },
            json!({"id": "redone", "name": {"kind": "addMarker"},
                   "what": {"kind": "name", "name": "サビ"}}),
        ),
        (
            StatusMessage::HistoryJumped {
                index: 11,
                name: StepName::AddMarker,
                steps: 5,
                forward: false,
            },
            json!({"id": "historyJumped", "index": 11, "name": {"kind": "addMarker"},
                   "steps": 5, "forward": false}),
        ),
        (StatusMessage::NothingToUndo, json!({"id": "nothingToUndo"})),
        (
            StatusMessage::AutomationWriting {
                track: piano.clone(),
                target: LaneName::Volume,
            },
            json!({"id": "automationWriting", "track": {"number": 1, "name": "ピアノ"},
                   "target": {"kind": "volume"}}),
        ),
        (
            StatusMessage::AutomationRested {
                track: piano.clone(),
                target: LaneName::Builtin {
                    effect: EffectKind::Compressor,
                    param: 0,
                },
            },
            json!({"id": "automationRested", "track": {"number": 1, "name": "ピアノ"},
                   "target": {"kind": "builtin", "effect": "compressor", "param": 0}}),
        ),
        (
            StatusMessage::AutomationRestored {
                track: piano.clone(),
                target: LaneName::Volume,
            },
            json!({"id": "automationRestored", "track": {"number": 1, "name": "ピアノ"},
                   "target": {"kind": "volume"}}),
        ),
        (
            StatusMessage::AutomationMoving {
                tracks: vec![piano.clone()],
            },
            json!({"id": "automationMoving", "tracks": [{"number": 1, "name": "ピアノ"}]}),
        ),
        (StatusMessage::NothingToRedo, json!({"id": "nothingToRedo"})),
    ];
    for (message, json) in cases {
        assert_eq!(serde_json::to_value(&message).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<StatusMessage>(json).unwrap(),
            message
        );
    }
}
