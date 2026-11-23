//! The shapes protocol 31 added (round 12, wave 1): the metronome's settings (R3), nine kinds
//! of scale and 「色をつける」 (P1), the velocity and length tools (P3), notes of several
//! starts (P6), the dialog 「書き出しました」 (E9), 「トラックを複製」 (A3), the songs opened
//! lately (A5), 「ノーマライズ」 and 「逆にする」 (A9), the loudness of an export (F1), the user
//! guide (G1) and the check for a new version (D1).

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
fn the_click_settings() {
    assert_eq!(
        to_json(&ClickSound::ALL),
        json!(["beep", "tick", "cowbell"])
    );
    assert_eq!(to_json(&ClickWhen::ALL), json!(["always", "recordingOnly"]));
    for sound in ClickSound::ALL {
        assert_eq!(to_json(&sound), json!(sound.name()));
        assert_eq!(ClickSound::from_name(sound.name()), Some(sound));
    }
    for when in ClickWhen::ALL {
        assert_eq!(to_json(&when), json!(when.name()));
        assert_eq!(ClickWhen::from_name(when.name()), Some(when));
    }
    assert_eq!(ClickSound::from_name("bell"), None);
    assert_eq!(
        (ClickSound::default(), ClickWhen::default()),
        (ClickSound::Beep, ClickWhen::Always)
    );
    assert_eq!((CLICK_VOLUME_MIN_DB, CLICK_VOLUME_MAX_DB), (-24.0, 6.0));
    both_ways(
        Command::SetClick {
            volume_db: Some(-12.0),
            sound: None,
            when: Some(ClickWhen::RecordingOnly),
            commit: false,
        },
        json!({"type": "setClick", "volumeDb": -12.0, "sound": null,
               "when": "recordingOnly", "commit": false}),
    );
    let transport = to_json(&tests::sample_state().transport);
    assert_eq!(transport["clickVolumeDb"], json!(-12.5));
    assert_eq!(transport["clickSound"], json!("cowbell"));
    assert_eq!(transport["clickWhen"], json!("recordingOnly"));
}

#[test]
fn the_nine_kinds_of_scale() {
    assert_eq!(
        to_json(&ScaleMode::ALL),
        json!([
            "major",
            "minor",
            "harmonicMinor",
            "melodicMinor",
            "majorPentatonic",
            "minorPentatonic",
            "blues",
            "dorian",
            "mixolydian"
        ])
    );
    both_ways(
        MusicalScale {
            root: 7,
            mode: ScaleMode::Blues,
            lock: false,
            highlight: true,
        },
        json!({"root": 7, "mode": "blues", "lock": false, "highlight": true}),
    );
    both_ways(
        Command::SetMusicalScale {
            root: 2,
            mode: ScaleMode::Dorian,
            lock: true,
            highlight: false,
        },
        json!({"type": "setMusicalScale", "root": 2, "mode": "dorian", "lock": true,
               "highlight": false}),
    );
}

#[test]
fn the_note_commands() {
    both_ways(
        Command::ScaleVelocity {
            take: 6,
            notes: vec![1, 2],
            percent: 120,
        },
        json!({"type": "scaleVelocity", "take": 6, "notes": [1, 2], "percent": 120}),
    );
    both_ways(
        Command::RampVelocity {
            take: 6,
            notes: vec![1, 2, 3],
            from: 40,
            to: 110,
        },
        json!({"type": "rampVelocity", "take": 6, "notes": [1, 2, 3], "from": 40, "to": 110}),
    );
    both_ways(
        Command::RandomVelocity {
            take: 6,
            notes: vec![0],
            amount: 8,
            seed: 0xDEAD_BEEF,
        },
        json!({"type": "randomVelocity", "take": 6, "notes": [0], "amount": 8,
               "seed": 0xDEAD_BEEF_u32}),
    );
    both_ways(
        Command::Legato {
            take: 6,
            notes: vec![4, 5],
        },
        json!({"type": "legato", "take": 6, "notes": [4, 5]}),
    );
    both_ways(
        Command::AddNotesAt {
            take: None,
            notes: vec![
                NoteAt {
                    start: 0,
                    length: 7680,
                    key: 53,
                    velocity: 100,
                },
                NoteAt {
                    start: 7680,
                    length: 7680,
                    key: 55,
                    velocity: 100,
                },
            ],
        },
        json!({"type": "addNotesAt", "take": null, "notes": [
            {"start": 0, "length": 7680, "key": 53, "velocity": 100},
            {"start": 7680, "length": 7680, "key": 55, "velocity": 100}]}),
    );
}

#[test]
fn the_other_commands() {
    let plain = [
        (Command::PlayExported, "playExported"),
        (Command::ClearRecent, "clearRecent"),
        (Command::OpenUserGuide, "openUserGuide"),
        (Command::OpenReleasePage, "openReleasePage"),
        (Command::DismissNewVersion, "dismissNewVersion"),
    ];
    for (command, name) in plain {
        both_ways(command, json!({"type": name}));
    }
    both_ways(
        Command::DuplicateTrack { track: 7 },
        json!({"type": "duplicateTrack", "track": 7}),
    );
    both_ways(
        Command::OpenRecent { id: 3 },
        json!({"type": "openRecent", "id": 3}),
    );
    both_ways(
        Command::NormalizeClips { clips: vec![4, 9] },
        json!({"type": "normalizeClips", "clips": [4, 9]}),
    );
    both_ways(
        Command::ReverseClips { clips: vec![4] },
        json!({"type": "reverseClips", "clips": [4]}),
    );
    both_ways(
        Command::SetCheckUpdates { on: false },
        json!({"type": "setCheckUpdates", "on": false}),
    );
    both_ways(
        Command::SetExport {
            change: ExportChange::Loudness {
                loudness: ExportLoudness::Match,
            },
        },
        json!({"type": "setExport", "change": {"kind": "loudness", "loudness": "match"}}),
    );
    both_ways(
        [ExportLoudness::AsIs, ExportLoudness::Match],
        json!(["asIs", "match"]),
    );
}

#[test]
fn the_new_step_names() {
    let names = [
        (StepName::ScaleVelocity, "scaleVelocity"),
        (StepName::RampVelocity, "rampVelocity"),
        (StepName::RandomVelocity, "randomVelocity"),
        (StepName::Legato, "legato"),
        (StepName::DuplicateTrack, "duplicateTrack"),
        (StepName::NormalizeClips, "normalizeClips"),
        (StepName::ReverseClips, "reverseClips"),
    ];
    for (name, kind) in names {
        assert!(StepName::every_kind().contains(&name), "{kind}");
        both_ways(name, json!({"kind": kind}));
    }
}

#[test]
fn the_new_status_messages() {
    let messages = [
        (
            StatusMessage::VelocityScaled {
                count: 3,
                percent: 120,
            },
            json!({"id": "velocityScaled", "count": 3, "percent": 120}),
        ),
        (
            StatusMessage::VelocityRamped {
                count: 3,
                from: 40,
                to: 110,
            },
            json!({"id": "velocityRamped", "count": 3, "from": 40, "to": 110}),
        ),
        (
            StatusMessage::VelocityRandomized {
                count: 3,
                amount: 8,
            },
            json!({"id": "velocityRandomized", "count": 3, "amount": 8}),
        ),
        (
            StatusMessage::NotesLegato { count: 4 },
            json!({"id": "notesLegato", "count": 4}),
        ),
        (StatusMessage::TooManyTracks, json!({"id": "tooManyTracks"})),
        (
            StatusMessage::NormalizeLimited { count: 1 },
            json!({"id": "normalizeLimited", "count": 1}),
        ),
        (
            StatusMessage::NormalizeSilent { count: 2 },
            json!({"id": "normalizeSilent", "count": 2}),
        ),
        (
            StatusMessage::ReverseFollowOff { count: 1 },
            json!({"id": "reverseFollowOff", "count": 1}),
        ),
        (
            StatusMessage::NewVersion {
                version: "0.4.0".into(),
            },
            json!({"id": "newVersion", "version": "0.4.0"}),
        ),
    ];
    for (message, want) in messages {
        both_ways(message, want);
    }
}

#[test]
fn the_recent_songs_and_the_updates_are_sections_of_their_own() {
    let old = tests::sample_state();
    let v = to_json(&old);
    assert_eq!(
        v["recent"],
        json!([{"id": 1, "name": "はじめての曲", "place": "~/ミュージック/はじめての曲/"}])
    );
    assert_eq!(v["updates"], json!({"check": true, "newVersion": null}));
    let mut new = old.clone();
    new.recent.clear();
    new.updates.new_version = Some("0.4.0".into());
    assert_eq!(
        AppState::changes_from(&old, &new),
        vec![
            Change::Recent(Vec::new()),
            Change::Updates(UpdatesView {
                check: true,
                new_version: Some("0.4.0".into()),
            }),
        ]
    );
    assert_eq!(RECENT_SHOWN, 10);
}

/// Writes `ui/src/bridge/generated/round12.ts`: the numbers and lists protocol 31 added.
#[test]
fn export_round12_numbers() {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/bridge/generated");
    std::fs::create_dir_all(&dir).unwrap();
    let list = |v: Value| serde_json::to_string(&v).unwrap();
    std::fs::write(
        dir.join("round12.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ ClickSound }} from \"./ClickSound\";\n\
             import type {{ ClickWhen }} from \"./ClickWhen\";\n\
             import type {{ ScaleMode }} from \"./ScaleMode\";\n\n\
             /** The range of the metronome's 「音量」, dB. */\n\
             export const CLICK_VOLUME_MIN_DB = {CLICK_VOLUME_MIN_DB};\n\
             export const CLICK_VOLUME_MAX_DB = {CLICK_VOLUME_MAX_DB};\n\n\
             /** 「音」 and 「鳴らすとき」, in the order of the window. */\n\
             export const CLICK_SOUNDS: readonly ClickSound[] = {};\n\
             export const CLICK_WHENS: readonly ClickWhen[] = {};\n\n\
             /** The kinds of scale, in the order of the list. */\n\
             export const SCALE_MODES: readonly ScaleMode[] = {};\n\n\
             /** How many bars the waveform of 「書き出しました」 has. */\n\
             export const EXPORT_WAVE_BARS = {EXPORT_WAVE_BARS};\n\n\
             /** 「公開先に合わせる」: LUFS, the true-peak limit in dBTP, and the quietest song it takes. */\n\
             export const EXPORT_MATCH_LUFS = {EXPORT_MATCH_LUFS};\n\
             export const EXPORT_MATCH_TRUE_PEAK_DB = {EXPORT_MATCH_TRUE_PEAK_DB};\n\
             export const EXPORT_MATCH_QUIETEST_LUFS = {EXPORT_MATCH_QUIETEST_LUFS};\n\n\
             /** How many rows 「最近ひらいた曲 ▸」 shows at most. */\n\
             export const RECENT_SHOWN = {RECENT_SHOWN};\n",
            list(json!(ClickSound::ALL)),
            list(json!(ClickWhen::ALL)),
            list(json!(ScaleMode::ALL)),
        ),
    )
    .unwrap();
}

#[test]
fn the_export_constants() {
    assert_eq!(EXPORT_WAVE_BARS, 200);
    assert_eq!(
        (
            EXPORT_MATCH_LUFS,
            EXPORT_MATCH_TRUE_PEAK_DB,
            EXPORT_MATCH_QUIETEST_LUFS
        ),
        (-14.0, -1.0, -70.0)
    );
}
