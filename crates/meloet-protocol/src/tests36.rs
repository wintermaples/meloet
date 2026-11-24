//! The shapes protocol 36 added (the owner, 2026-11-24): MIDI export from the dialog
//! 「書き出し」 (spec 3 "MIDI export", spec 7 A-31).

use serde_json::{Value, json};

use super::*;

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn both_ways<T>(v: T, json: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(to_json(&v), json);
    assert_eq!(serde_json::from_value::<T>(json).unwrap(), v);
}

#[test]
fn the_version() {
    assert_eq!(PROTOCOL_VERSION, 36);
}

#[test]
fn the_format() {
    both_ways(ExportFormat::Midi, json!("midi"));
    assert_eq!(ExportFormat::Midi.extension(), "mid");
    assert_eq!(ExportFormat::Midi.bits_choices(), []);
    assert_eq!(ExportFormat::ALL.last(), Some(&ExportFormat::Midi));
}

#[test]
fn the_choices_of_the_tracks() {
    assert_eq!(ExportMidiTracks::default(), ExportMidiTracks::All);
    both_ways(
        [
            ExportMidiTracks::All,
            ExportMidiTracks::Selected,
            ExportMidiTracks::Numbers,
        ],
        json!(["all", "selected", "numbers"]),
    );
    both_ways(
        ExportMidiTrack {
            number: 2,
            name: "ベース".to_owned(),
        },
        json!({"number": 2, "name": "ベース"}),
    );
}

#[test]
fn the_problems() {
    both_ways(
        ExportMidiProblem::NoSuchTrack { number: 8 },
        json!({"kind": "noSuchTrack", "number": 8}),
    );
    both_ways(ExportMidiProblem::Unreadable, json!({"kind": "unreadable"}));
    both_ways(ExportMidiProblem::NoTrack, json!({"kind": "noTrack"}));
}

#[test]
fn the_changes() {
    let set = |change: ExportChange| Command::SetExport { change };
    both_ways(
        set(ExportChange::MidiTracks {
            tracks: ExportMidiTracks::Numbers,
        }),
        json!({"type": "setExport", "change": {"kind": "midiTracks", "tracks": "numbers"}}),
    );
    both_ways(
        set(ExportChange::MidiNumbers {
            text: "1-3, 5".to_owned(),
        }),
        json!({"type": "setExport", "change": {"kind": "midiNumbers", "text": "1-3, 5"}}),
    );
}

/// 「番号で」 「1-2, 4」 where 4 is a vocal track: two tracks taken, a gray note for the third.
#[test]
fn the_dialog_under_midi() {
    let track = |number: u32, name: &str| ExportMidiTrack {
        number,
        name: name.to_owned(),
    };
    let view = ExportDialogView {
        range: ExportRange::Song,
        loop_range: None,
        tracks: 2,
        span: TickSpan {
            start: 0,
            end: 122_880,
        },
        format: ExportFormat::Midi,
        formats: ExportFormat::ALL.to_vec(),
        place: "~/音楽/はじめての曲.mid".to_owned(),
        check: ExportCheck::Waiting,
        solo: None,
        stopped_plugins: Vec::new(),
        advanced_open: false,
        sample_rate: 48_000,
        sample_rate_choices: vec![48_000],
        bits: RecordFormat::Bits24,
        bits_choices: Vec::new(),
        tail: ExportTail::UntilSilent,
        realtime: false,
        realtime_forced: false,
        can_export: true,
        loudness: ExportLoudness::AsIs,
        can_match: false,
        loudness_result: None,
        mp3_kbps: 192,
        mp3_kbps_choices: EXPORT_MP3_KBPS.to_vec(),
        title: "はじめての曲".to_owned(),
        artist: String::new(),
        picture: None,
        picture_usable: false,
        midi_tracks: ExportMidiTracks::Numbers,
        midi_numbers: "1-2, 4".to_owned(),
        midi_selected_usable: false,
        midi_taken: vec![track(1, "ピアノ"), track(2, "ベース")],
        midi_vocal: vec![track(4, "歌")],
        midi_problem: None,
    };
    let json = to_json(&view);
    assert_eq!(json["format"], json!("midi"));
    assert_eq!(json["formats"], json!(["wav", "flac", "mp3", "midi"]));
    assert_eq!(json["place"], json!("~/音楽/はじめての曲.mid"));
    assert_eq!(json["midiTracks"], json!("numbers"));
    assert_eq!(json["midiNumbers"], json!("1-2, 4"));
    assert_eq!(json["midiSelectedUsable"], json!(false));
    assert_eq!(
        json["midiTaken"],
        json!([{"number": 1, "name": "ピアノ"}, {"number": 2, "name": "ベース"}])
    );
    assert_eq!(json["midiVocal"], json!([{"number": 4, "name": "歌"}]));
    assert_eq!(json["midiProblem"], json!(null));
    assert_eq!(
        serde_json::from_value::<ExportDialogView>(json).unwrap(),
        view
    );

    let wrong = ExportDialogView {
        can_export: false,
        midi_numbers: "1-3, 9".to_owned(),
        midi_problem: Some(ExportMidiProblem::NoSuchTrack { number: 9 }),
        ..view
    };
    assert_eq!(
        to_json(&wrong)["midiProblem"],
        json!({"kind": "noSuchTrack", "number": 9})
    );
}

#[test]
fn done_with_midi() {
    let done = |file: &str, files: u32, place: &str, midi_tracks: u32| ExportRunView::Done {
        file: file.to_owned(),
        files,
        place: place.to_owned(),
        length_s: 182,
        sample_rate: 48_000,
        bits: RecordFormat::Bits24,
        wave: Vec::new(),
        lufs: None,
        true_peak_db: None,
        mp3_kbps: None,
        midi_tracks: Some(midi_tracks),
    };
    both_ways(
        done(
            "はじめての曲.mid",
            1,
            "~/ミュージック/はじめての曲/はじめての曲.mid",
            3,
        ),
        json!({"state": "done", "file": "はじめての曲.mid", "files": 1,
               "place": "~/ミュージック/はじめての曲/はじめての曲.mid", "lengthS": 182,
               "sampleRate": 48_000, "bits": "bits24", "wave": [], "lufs": null,
               "truePeakDb": null, "mp3Kbps": null, "midiTracks": 3}),
    );
    // 「トラックごと」: one track in each file.
    assert_eq!(
        to_json(&done("ピアノ.mid", 3, "~/ミュージック/はじめての曲/", 3))["midiTracks"],
        json!(3)
    );
}
