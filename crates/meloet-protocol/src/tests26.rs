//! The shapes protocol 26 added: writing the song out as an audio file.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

/// `v` writes `json`, and `json` reads back as `v`.
fn both_ways<T>(v: T, json: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(to_json(&v), json);
    assert_eq!(serde_json::from_value::<T>(json).unwrap(), v);
}

/// Mock F7-Export.
fn dialog() -> ExportDialogView {
    ExportDialogView {
        range: ExportRange::Song,
        loop_range: Some(TickSpan {
            start: 30_720,
            end: 61_440,
        }),
        tracks: 4,
        span: TickSpan {
            start: 0,
            end: 122_880,
        },
        format: ExportFormat::Wav,
        formats: vec![ExportFormat::Wav, ExportFormat::Flac],
        place: "~/音楽/はじめての曲.wav".to_owned(),
        check: ExportCheck::Done {
            peak_db: Some(-0.5),
            clips: false,
            over: 0,
            lufs: Some(-18.5),
            true_peak_db: Some(-3.25),
        },
        solo: None,
        stopped_plugins: Vec::new(),
        advanced_open: false,
        sample_rate: 48_000,
        sample_rate_choices: vec![44_100, 48_000, 96_000],
        bits: RecordFormat::Bits24,
        bits_choices: RecordFormat::ALL.to_vec(),
        tail: ExportTail::UntilSilent,
        realtime: false,
        realtime_forced: false,
        can_export: true,
        loudness: ExportLoudness::Match,
        can_match: true,
        loudness_result: Some(ExportLoudnessResult {
            gain_db: 4.5,
            limited: false,
            lufs: -14.0,
        }),
        mp3_kbps: 192,
        mp3_kbps_choices: Vec::new(),
        title: "はじめての曲".to_owned(),
        artist: String::new(),
        picture: None,
        picture_usable: false,
        midi_tracks: ExportMidiTracks::All,
        midi_numbers: String::new(),
        midi_selected_usable: true,
        midi_taken: Vec::new(),
        midi_vocal: Vec::new(),
        midi_problem: None,
    }
}

#[test]
fn the_formats() {
    assert_eq!(
        to_json(&ExportFormat::ALL),
        json!(["wav", "flac", "mp3", "midi"])
    );
    assert_eq!(ExportFormat::Flac.extension(), "flac");
    // 32 bits are floats, which a flac file does not hold.
    assert_eq!(
        ExportFormat::Flac.bits_choices(),
        [RecordFormat::Bits16, RecordFormat::Bits24]
    );
    assert_eq!(ExportFormat::Wav.bits_choices(), RecordFormat::ALL);
    // An mp3 has a bitrate instead (protocol 32).
    assert_eq!(ExportFormat::Mp3.bits_choices(), []);
    for format in [ExportFormat::Wav, ExportFormat::Flac] {
        assert!(format.bits_choices().contains(&RecordFormat::default()));
    }
}

#[test]
fn the_dialog_json_shape() {
    both_ways(
        ExportView {
            dialog: Some(dialog()),
            run: None,
        },
        json!({"dialog": {
            "range": "song",
            "loopRange": {"start": 30_720, "end": 61_440},
            "tracks": 4,
            "span": {"start": 0, "end": 122_880},
            "format": "wav",
            "formats": ["wav", "flac"],
            "place": "~/音楽/はじめての曲.wav",
            "check": {"state": "done", "peakDb": -0.5, "clips": false, "over": 0,
                      "lufs": -18.5, "truePeakDb": -3.25},
            "solo": null,
            "stoppedPlugins": [],
            "advancedOpen": false,
            "sampleRate": 48_000,
            "sampleRateChoices": [44_100, 48_000, 96_000],
            "bits": "bits24",
            "bitsChoices": ["bits16", "bits24", "bits32"],
            "tail": "untilSilent",
            "realtime": false,
            "realtimeForced": false,
            "canExport": true,
            "loudness": "match",
            "canMatch": true,
            "loudnessResult": {"gainDb": 4.5, "limited": false, "lufs": -14.0},
            "mp3Kbps": 192,
            "mp3KbpsChoices": [],
            "title": "はじめての曲",
            "artist": "",
            "picture": null,
            "pictureUsable": false,
            "midiTracks": "all",
            "midiNumbers": "",
            "midiSelectedUsable": true,
            "midiTaken": [],
            "midiVocal": [],
            "midiProblem": null,
        }, "run": null}),
    );
    both_ways(ExportView::default(), json!({"dialog": null, "run": null}));
    both_ways(
        [ExportRange::Song, ExportRange::Loop, ExportRange::Tracks],
        json!(["song", "loop", "tracks"]),
    );
    both_ways(
        [ExportTail::UntilSilent, ExportTail::Off],
        json!(["untilSilent", "off"]),
    );
}

#[test]
fn the_check_json_shape() {
    both_ways(ExportCheck::Waiting, json!({"state": "waiting"}));
    both_ways(
        ExportCheck::Counting {
            percent: 38,
            peak_db: Some(-3.5),
            clips: false,
            over: 0,
            lufs: Some(-20.5),
            true_peak_db: Some(-3.0),
        },
        json!({"state": "counting", "percent": 38, "peakDb": -3.5, "clips": false, "over": 0,
               "lufs": -20.5, "truePeakDb": -3.0}),
    );
    // Nothing but silence so far.
    both_ways(
        ExportCheck::Counting {
            percent: 0,
            peak_db: None,
            clips: false,
            over: 0,
            lufs: None,
            true_peak_db: None,
        },
        json!({"state": "counting", "percent": 0, "peakDb": null, "clips": false, "over": 0,
               "lufs": null, "truePeakDb": null}),
    );
    both_ways(
        ExportCheck::Done {
            peak_db: Some(1.5),
            clips: true,
            over: 3,
            lufs: Some(-9.5),
            true_peak_db: Some(2.0),
        },
        json!({"state": "done", "peakDb": 1.5, "clips": true, "over": 3, "lufs": -9.5,
               "truePeakDb": 2.0}),
    );
    both_ways(
        ExportCheck::Failed {
            reason: ExportFailure::Plugin {
                name: "Surge XT".to_owned(),
            },
        },
        json!({"state": "failed", "reason": {"kind": "plugin", "name": "Surge XT"}}),
    );
    both_ways(ExportFailure::CannotWrite, json!({"kind": "cannotWrite"}));
    // Mock F-Export: "Brick EQ ・ 3 ドラム".
    both_ways(
        ExportStoppedPlugin {
            name: "Brick EQ".to_owned(),
            slot: PluginSlotRef::Insert {
                insert: InsertRef {
                    strip: StripRef::Track { id: 3 },
                    slot: 0,
                },
            },
        },
        json!({"name": "Brick EQ", "slot": {"kind": "insert", "insert": {
            "strip": {"kind": "track", "id": 3}, "slot": 0}}}),
    );
}

#[test]
fn the_run_json_shape() {
    // Mock F7-Exporting.
    both_ways(
        ExportRunView::Running {
            percent: 46,
            file: "はじめての曲.wav".to_owned(),
            remaining_s: Some(40),
            realtime: false,
            realtime_forced: false,
            length_s: 182,
        },
        json!({"state": "running", "percent": 46, "file": "はじめての曲.wav",
               "remainingS": 40, "realtime": false, "realtimeForced": false,
               "lengthS": 182}),
    );
    both_ways(
        ExportRunView::Running {
            percent: 0,
            file: "はじめての曲.wav".to_owned(),
            remaining_s: None,
            realtime: true,
            realtime_forced: true,
            length_s: 182,
        },
        json!({"state": "running", "percent": 0, "file": "はじめての曲.wav",
               "remainingS": null, "realtime": true, "realtimeForced": true,
               "lengthS": 182}),
    );
    both_ways(
        ExportRunView::Done {
            file: "はじめての曲.wav".to_owned(),
            files: 1,
            place: "~/ミュージック/はじめての曲/はじめての曲.wav".to_owned(),
            length_s: 182,
            sample_rate: 48_000,
            bits: RecordFormat::Bits24,
            wave: vec![0, 128, 255],
            lufs: Some(-14.0),
            true_peak_db: Some(-1.0),
            mp3_kbps: None,
            midi_tracks: None,
        },
        json!({"state": "done", "file": "はじめての曲.wav", "files": 1,
               "place": "~/ミュージック/はじめての曲/はじめての曲.wav", "lengthS": 182,
               "sampleRate": 48_000, "bits": "bits24", "wave": [0, 128, 255],
               "lufs": -14.0, "truePeakDb": -1.0, "mp3Kbps": null, "midiTracks": null}),
    );
    both_ways(ExportRunView::Cancelled, json!({"state": "cancelled"}));
    both_ways(
        ExportRunView::Failed {
            reason: ExportFailure::CannotWrite,
        },
        json!({"state": "failed", "reason": {"kind": "cannotWrite"}}),
    );
}

#[test]
fn the_commands_json_shape() {
    both_ways(Command::OpenExport, json!({"type": "openExport"}));
    both_ways(Command::CloseExport, json!({"type": "closeExport"}));
    both_ways(
        Command::ChooseExportPlace,
        json!({"type": "chooseExportPlace"}),
    );
    both_ways(Command::StartExport, json!({"type": "startExport"}));
    both_ways(Command::CancelExport, json!({"type": "cancelExport"}));
    both_ways(Command::ShowExported, json!({"type": "showExported"}));
    let set = |change| Command::SetExport { change };
    both_ways(
        set(ExportChange::Range {
            range: ExportRange::Loop,
        }),
        json!({"type": "setExport", "change": {"kind": "range", "range": "loop"}}),
    );
    both_ways(
        set(ExportChange::Format {
            format: ExportFormat::Flac,
        }),
        json!({"type": "setExport", "change": {"kind": "format", "format": "flac"}}),
    );
    both_ways(
        set(ExportChange::AdvancedOpen { open: true }),
        json!({"type": "setExport", "change": {"kind": "advancedOpen", "open": true}}),
    );
    both_ways(
        set(ExportChange::SampleRate { hz: 44_100 }),
        json!({"type": "setExport", "change": {"kind": "sampleRate", "hz": 44_100}}),
    );
    both_ways(
        set(ExportChange::Bits {
            bits: RecordFormat::Bits16,
        }),
        json!({"type": "setExport", "change": {"kind": "bits", "bits": "bits16"}}),
    );
    both_ways(
        set(ExportChange::Tail {
            tail: ExportTail::Off,
        }),
        json!({"type": "setExport", "change": {"kind": "tail", "tail": "off"}}),
    );
    both_ways(
        set(ExportChange::Realtime { on: true }),
        json!({"type": "setExport", "change": {"kind": "realtime", "on": true}}),
    );
}

#[test]
fn the_section_and_the_status_line() {
    let view = ExportView {
        dialog: None,
        run: Some(ExportRunView::Cancelled),
    };
    both_ways(
        Change::Export(view.clone()),
        json!({"section": "export", "value": {"dialog": null, "run": {"state": "cancelled"}}}),
    );
    both_ways(
        StatusMessage::ExportDialogOpen,
        json!({"id": "exportDialogOpen"}),
    );
    // Only the export differs: only it is sent, and it lands in its place.
    let old = super::tests::sample_state();
    let mut new = old.clone();
    new.export = view.clone();
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(changes, [Change::Export(view)]);
    let mut applied = old;
    for change in changes {
        applied.apply(change);
    }
    assert_eq!(applied, new);
}

/// Writes `ui/src/bridge/generated/export.ts`: the lists of the dialog 「書き出し」.
#[test]
fn export_export_lists() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/bridge/generated");
    std::fs::create_dir_all(&dir).unwrap();
    let bits: Vec<Value> = ExportFormat::ALL
        .iter()
        .map(|f| json!({"format": f, "bits": f.bits_choices()}))
        .collect();
    std::fs::write(
        dir.join("export.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ ExportFormat }} from \"./ExportFormat\";\n\
             import type {{ RecordFormat }} from \"./RecordFormat\";\n\n\
             /** The tail is written at most this many seconds after the range ends. */\n\
             export const EXPORT_TAIL_MAX_S = {EXPORT_TAIL_MAX_S};\n\n\
             /** The formats meloet writes, in the order of the dialog, and the bits each holds. */\n\
             export const EXPORT_FORMATS: readonly {{ readonly format: ExportFormat; \
             readonly bits: readonly RecordFormat[] }}[] = {};\n",
            serde_json::to_string(&bits).unwrap(),
        ),
    )
    .unwrap();
}
