//! The shapes protocol 27 added: several vocal clips, the check before an export, the
//! messages of the twelfth edition and the sound pack.

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
fn several_clips() {
    // Mock F8-MultiClip: three clips on two tracks; the volumes differ.
    both_ways(
        ParamsView {
            selected: None,
            clips: Some(SelectedClips {
                clips: vec![31, 32, 41],
                tracks: vec![6, 7],
                gain_db: None,
                follow: Some(true),
                semitones: None,
                cents: Some(0),
                keep_formant: Some(true),
            }),
        },
        json!({"selected": null, "clips": {"clips": [31, 32, 41], "tracks": [6, 7],
               "gainDb": null, "follow": true, "semitones": null, "cents": 0,
               "keepFormant": true}}),
    );
    both_ways(
        Command::ShiftClipGains {
            clips: vec![31, 32],
            by_db: -3.0,
        },
        json!({"type": "shiftClipGains", "clips": [31, 32], "byDb": -3.0}),
    );
    both_ways(
        Command::SetClipGains {
            clips: vec![31, 32],
            db: 0.0,
        },
        json!({"type": "setClipGains", "clips": [31, 32], "db": 0.0}),
    );
    both_ways(
        Command::SetClipsStretch {
            clips: vec![31, 32],
            change: StretchChange::KeepFormant { on: false },
        },
        json!({"type": "setClipsStretch", "clips": [31, 32],
               "change": {"kind": "keepFormant", "on": false}}),
    );
}

#[test]
fn the_check_counts_places_over_0_db_and_names_the_solo() {
    // Mock F8-ExportClip.
    both_ways(
        ExportCheck::Done {
            peak_db: Some(1.25),
            clips: true,
            over: 3,
            lufs: None,
            true_peak_db: None,
        },
        json!({"state": "done", "peakDb": 1.25, "clips": true, "over": 3, "lufs": null,
               "truePeakDb": null}),
    );
    // 32 bits: over 0 dB, and nothing clips.
    both_ways(
        ExportCheck::Counting {
            percent: 38,
            peak_db: Some(0.5),
            clips: false,
            over: 1,
            lufs: None,
            true_peak_db: None,
        },
        json!({"state": "counting", "percent": 38, "peakDb": 0.5, "clips": false, "over": 1,
               "lufs": null, "truePeakDb": null}),
    );
    // Mock F8-ExportCheck: "いま 2 ベース をソロにしています。…"
    both_ways(
        ExportSolo {
            first: TrackRef {
                number: 2,
                name: "ベース".to_owned(),
            },
            others: 1,
        },
        json!({"first": {"number": 2, "name": "ベース"}, "others": 1}),
    );
}

#[test]
fn the_messages_of_the_twelfth_edition() {
    let messages = [
        (
            StatusMessage::InputLost {
                input: VocalInput::Mono1,
                device: "USB マイク".to_owned(),
            },
            json!({"id": "inputLost", "input": "mono1", "device": "USB マイク"}),
        ),
        (
            StatusMessage::RecordingGaps { count: 3 },
            json!({"id": "recordingGaps", "count": 3}),
        ),
        (
            StatusMessage::AudioFound {
                file: "歌 テイク 1.wav".to_owned(),
                clips: 2,
            },
            json!({"id": "audioFound", "file": "歌 テイク 1.wav", "clips": 2}),
        ),
        (
            StatusMessage::AudioFoundMismatch,
            json!({"id": "audioFoundMismatch"}),
        ),
        (
            StatusMessage::AudioBroken {
                file: "声.wav".to_owned(),
            },
            json!({"id": "audioBroken", "file": "声.wav"}),
        ),
        (
            StatusMessage::AudioTooManyChannels {
                file: "8ch.wav".to_owned(),
            },
            json!({"id": "audioTooManyChannels", "file": "8ch.wav"}),
        ),
        (
            StatusMessage::SavingAudio {
                done: 3,
                total: 12,
                done_bytes: 80_000_000,
                total_bytes: 240_000_000,
            },
            json!({"id": "savingAudio", "done": 3, "total": 12, "doneBytes": 80_000_000,
                   "totalBytes": 240_000_000}),
        ),
        (StatusMessage::DiskTooSlow, json!({"id": "diskTooSlow"})),
        (
            StatusMessage::SourceTempoFound {
                name: "ドラム".to_owned(),
                bpm: 120.0,
                bars: 2,
                same: true,
            },
            json!({"id": "sourceTempoFound", "name": "ドラム", "bpm": 120.0, "bars": 2,
                   "same": true}),
        ),
        (StatusMessage::PackAsking, json!({"id": "packAsking"})),
        (
            StatusMessage::SoundReady {
                instrument: InstrumentKind::GrandPiano,
            },
            json!({"id": "soundReady", "instrument": "grandPiano"}),
        ),
        (
            StatusMessage::PackFetched { count: 31 },
            json!({"id": "packFetched", "count": 31}),
        ),
        (
            StatusMessage::PackFailed {
                failure: PackFailure::DiskFull {
                    needed_bytes: 160_000_000,
                    free_bytes: 10_000_000,
                },
                done_bytes: 92_000_000,
                total_bytes: 240_000_000,
            },
            json!({"id": "packFailed",
                   "failure": {"kind": "diskFull", "neededBytes": 160_000_000,
                               "freeBytes": 10_000_000},
                   "doneBytes": 92_000_000, "totalBytes": 240_000_000}),
        ),
        (
            StatusMessage::NewPack { bytes: 12_000_000 },
            json!({"id": "newPack", "bytes": 12_000_000}),
        ),
    ];
    for (message, want) in messages {
        both_ways(message, want);
    }
}

#[test]
fn the_sound_pack() {
    // Mock F8-PackGet: the grand piano came, the electric piano is coming.
    let view = SoundsView {
        ask: false,
        fetch: PackFetchView::Fetching {
            instrument: Some(InstrumentKind::ElectricPiano),
            done_bytes: 92_000_000,
            total_bytes: 240_000_000,
            remaining_s: Some(120),
        },
        have: 6,
        total: 31,
        pack_bytes: 240_000_000,
        new_pack_bytes: None,
        place: "~/.local/share/meloet/sounds".to_owned(),
    };
    both_ways(
        Change::Sounds(view.clone()),
        json!({"section": "sounds", "value": {
            "ask": false,
            "fetch": {"state": "fetching", "instrument": "electricPiano",
                      "doneBytes": 92_000_000, "totalBytes": 240_000_000, "remainingS": 120},
            "have": 6, "total": 31, "packBytes": 240_000_000, "newPackBytes": null,
            "place": "~/.local/share/meloet/sounds"}}),
    );
    both_ways(PackFetchView::Idle, json!({"state": "idle"}));
    both_ways(
        PackFetchView::Failed {
            failure: PackFailure::Offline,
            done_bytes: 92_000_000,
            total_bytes: 240_000_000,
        },
        json!({"state": "failed", "failure": {"kind": "offline"}, "doneBytes": 92_000_000,
               "totalBytes": 240_000_000}),
    );
    both_ways(
        [PackFailure::CutOff, PackFailure::Corrupt],
        json!([{"kind": "cutOff"}, {"kind": "corrupt"}]),
    );
    both_ways(Command::FetchPack, json!({"type": "fetchPack"}));
    both_ways(Command::PackLater, json!({"type": "packLater"}));
    both_ways(Command::StopPackFetch, json!({"type": "stopPackFetch"}));
    // Only the pack differs: only it is sent, and it lands in its place.
    let old = super::tests::sample_state();
    let mut new = old.clone();
    new.sounds = view.clone();
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(changes, [Change::Sounds(view)]);
    let mut applied = old;
    for change in changes {
        applied.apply(change);
    }
    assert_eq!(applied, new);
}
