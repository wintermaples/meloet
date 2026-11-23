//! The shapes protocol 25 added: following the song's tempo, the tempo of the audio, the
//! pitch of a vocal clip.

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

#[test]
fn the_numbers_are_the_models() {
    assert_eq!(CLIP_SEMITONES_MAX, meloet_model::AudioShape::SEMITONES_MAX);
    assert_eq!(CLIP_CENTS_MAX, meloet_model::AudioShape::CENTS_MAX);
    let centi = |bpm: f64| (bpm * 100.0) as u32;
    assert_eq!(centi(SOURCE_BPM_MIN), meloet_model::Tempo::MIN_CENTI);
    assert_eq!(centi(SOURCE_BPM_MAX), meloet_model::Tempo::MAX_CENTI);
}

#[test]
fn a_clip_that_follows_json_shape() {
    // Sung through a ramp, and now played a little faster and two semitones up.
    let stretch = StretchView {
        follow: true,
        source_bpm: Some(96.0),
        ratio: Some(1.25),
        semitones: 2,
        cents: 0,
        keep_formant: true,
        tune: None,
    };
    both_ways(
        stretch,
        json!({"follow": true, "sourceBpm": 96.0, "ratio": 1.25, "semitones": 2, "cents": 0,
               "keepFormant": true, "tune": null}),
    );
    both_ways(
        SourceTempoView {
            tempo_bpm: 96.0,
            head_ramp: true,
            tempo_points: vec![
                TempoPointView {
                    tick: 7680,
                    tempo_bpm: 104.0,
                    ramp: false,
                },
                TempoPointView {
                    tick: 23_040,
                    tempo_bpm: 120.5,
                    ramp: true,
                },
            ],
        },
        json!({"tempoBpm": 96.0, "headRamp": true,
               "tempoPoints": [{"tick": 7680, "tempoBpm": 104.0, "ramp": false},
                               {"tick": 23_040, "tempoBpm": 120.5, "ramp": true}]}),
    );
    // A file brought in: no tempo, no ratio, nothing to place the waveform by.
    both_ways(
        StretchView {
            follow: false,
            source_bpm: None,
            ratio: None,
            semitones: -1,
            cents: 30,
            keep_formant: false,
            tune: None,
        },
        json!({"follow": false, "sourceBpm": null, "ratio": null, "semitones": -1,
               "cents": 30, "keepFormant": false, "tune": null}),
    );
}

#[test]
fn command_25_json_shape() {
    let stretch = |change| Command::SetClipStretch { clip: 9, change };
    for (command, json) in [
        (
            stretch(StretchChange::Follow { on: true }),
            json!({"type": "setClipStretch", "clip": 9,
                   "change": {"kind": "follow", "on": true}}),
        ),
        (
            stretch(StretchChange::SourceBpm { bpm: 96.5 }),
            json!({"type": "setClipStretch", "clip": 9,
                   "change": {"kind": "sourceBpm", "bpm": 96.5}}),
        ),
        (
            stretch(StretchChange::Semitones { semitones: 2.0 }),
            json!({"type": "setClipStretch", "clip": 9,
                   "change": {"kind": "semitones", "semitones": 2.0}}),
        ),
        (
            stretch(StretchChange::Cents { cents: -30.0 }),
            json!({"type": "setClipStretch", "clip": 9,
                   "change": {"kind": "cents", "cents": -30.0}}),
        ),
        (
            stretch(StretchChange::KeepFormant { on: false }),
            json!({"type": "setClipStretch", "clip": 9,
                   "change": {"kind": "keepFormant", "on": false}}),
        ),
        (
            Command::FindSourceTempo { clip: 9 },
            json!({"type": "findSourceTempo", "clip": 9}),
        ),
    ] {
        both_ways(command, json);
    }
    // The rows of the group 「クリップ」 are as they were.
    assert_eq!(
        json!([
            ClipField::Start,
            ClipField::Length,
            ClipField::GainDb,
            ClipField::FadeIn,
            ClipField::FadeOut
        ]),
        json!(["start", "length", "gainDb", "fadeIn", "fadeOut"])
    );
    // A switch is a bool, never a number; a whole number reads as the number it is.
    let reads = |change: Value| {
        serde_json::from_value::<Command>(
            json!({"type": "setClipStretch", "clip": 9, "change": change}),
        )
    };
    assert!(reads(json!({"kind": "follow", "on": 1})).is_err());
    assert_eq!(
        reads(json!({"kind": "semitones", "semitones": -3})).unwrap(),
        stretch(StretchChange::Semitones { semitones: -3.0 })
    );
}

#[test]
fn status_and_steps_25_json_shape() {
    both_ways(
        StatusMessage::SourceTempoFound {
            name: "ドラム".to_owned(),
            bpm: 137.14,
            bars: 4,
            same: false,
        },
        json!({"id": "sourceTempoFound", "name": "ドラム", "bpm": 137.14, "bars": 4, "same": false}),
    );
    both_ways(
        StatusMessage::SourceTempoNotFound {
            name: "ドラム".to_owned(),
        },
        json!({"id": "sourceTempoNotFound", "name": "ドラム"}),
    );
    assert_eq!(
        json!([
            StepName::SetClipFollow,
            StepName::SetSourceTempo,
            StepName::SetClipPitch
        ]),
        json!([{"kind": "setClipFollow"}, {"kind": "setSourceTempo"}, {"kind": "setClipPitch"}])
    );
    // 「（何を）」 of the three is the clip's name, as of a fade.
    both_ways(
        StepWhat::Name {
            name: "歌 テイク 1".to_owned(),
        },
        json!({"kind": "name", "name": "歌 テイク 1"}),
    );
}

#[test]
fn the_audio_being_made_json_shape() {
    both_ways(
        AudioWorkView {
            jobs: vec![AudioJob {
                kind: AudioJobKind::Rendering,
                audio: 60,
                clip: Some(70),
                percent: 34,
            }],
            bounce: None,
        },
        json!({"jobs": [{"kind": "rendering", "audio": 60, "clip": 70, "percent": 34}],
               "bounce": null}),
    );
}
