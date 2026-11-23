//! The shapes protocol 32 added (round 12, wave 2): keeping what was just played (R1), playing
//! with the PC keyboard (R2), the glow of the sounding notes (E1), drum patterns (P4),
//! repeating clips (A1), presets of the built-in effects (M1), three more effects (M6), 「音が
//! 大きすぎます」 (M2), mp3 and the song's information (F2), and the telemetry binary 5 (M2,
//! E7, M3).

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
    assert_eq!(PROTOCOL_VERSION, 32);
}

#[test]
fn the_commands_of_the_keys() {
    both_ways(Command::KeepPlayed, json!({"type": "keepPlayed"}));
    both_ways(
        Command::TypedKeys { on: true },
        json!({"type": "typedKeys", "on": true}),
    );
    both_ways(
        Command::TypedKey {
            note: 60,
            down: true,
            t_us: 123_456_789,
        },
        json!({"type": "typedKey", "note": 60, "down": true, "tUs": 123_456_789}),
    );
    both_ways(
        Command::SetNoteGlow { on: false },
        json!({"type": "setNoteGlow", "on": false}),
    );
    assert_eq!((TYPED_KEYBOARD_ID, TYPED_KEY_VELOCITY), ("typedKeys", 100));
    let state = to_json(&tests::sample_state());
    assert_eq!(state["transport"]["canKeepPlayed"], json!(true));
    assert_eq!(state["keyboards"]["typedKeys"], json!(false));
    assert_eq!(state["workspace"]["noteGlow"], json!(true));
}

#[test]
fn the_patterns_of_the_shelf() {
    both_ways(
        Command::ShelfSelectPattern {
            id: Some("eightBeat".into()),
        },
        json!({"type": "shelfSelectPattern", "id": "eightBeat"}),
    );
    both_ways(
        Command::ShelfSelectPattern { id: None },
        json!({"type": "shelfSelectPattern", "id": null}),
    );
    both_ways(
        Command::PlayPattern {
            id: "shuffle".into(),
        },
        json!({"type": "playPattern", "id": "shuffle"}),
    );
    both_ways(
        Command::PlacePattern {
            id: "tomFill".into(),
        },
        json!({"type": "placePattern", "id": "tomFill"}),
    );
    assert_eq!(to_json(&PatternGroup::ALL), json!(["basic", "fill"]));
    let shelf = to_json(&tests::sample_state().shelf);
    assert_eq!(
        shelf["patterns"],
        json!([{"id": "eightBeat", "group": "basic", "bars": 1}])
    );
    assert_eq!(shelf["selectedPattern"], json!("eightBeat"));
    assert_eq!(
        shelf["patternSample"],
        json!({"id": "eightBeat", "playing": null})
    );
}

#[test]
fn repeating_clips() {
    both_ways(
        Command::RepeatClips {
            clips: vec![4, 9],
            count: 3,
        },
        json!({"type": "repeatClips", "clips": [4, 9], "count": 3}),
    );
    assert_eq!((REPEAT_COUNT_DEFAULT, REPEAT_COUNT_MAX), (3, 99));
}

#[test]
fn the_presets() {
    let insert = InsertRef {
        strip: StripRef::Track { id: 7 },
        slot: 1,
    };
    both_ways(
        Command::ApplyPreset {
            insert,
            preset: PresetRef::Factory {
                id: "vocalEven".into(),
            },
        },
        json!({"type": "applyPreset", "insert": {"strip": {"kind": "track", "id": 7}, "slot": 1},
               "preset": {"kind": "factory", "id": "vocalEven"}}),
    );
    both_ways(
        Command::ApplyPreset {
            insert,
            preset: PresetRef::User {
                name: "いつものピアノ".into(),
            },
        },
        json!({"type": "applyPreset", "insert": {"strip": {"kind": "track", "id": 7}, "slot": 1},
               "preset": {"kind": "user", "name": "いつものピアノ"}}),
    );
    both_ways(
        Command::SavePreset {
            insert,
            name: "いつものピアノ".into(),
        },
        json!({"type": "savePreset", "insert": {"strip": {"kind": "track", "id": 7}, "slot": 1},
               "name": "いつものピアノ"}),
    );
    both_ways(
        Command::DeletePreset {
            effect: EffectKind::Compressor,
            name: "いつものピアノ".into(),
        },
        json!({"type": "deletePreset", "effect": "compressor", "name": "いつものピアノ"}),
    );
    assert_eq!((PRESET_NAME_MAX, PRESETS_PER_EFFECT), (40, 100));

    // 「自分で残したもの」 are a section of their own; the selected insert names its preset.
    let old = tests::sample_state();
    let v = to_json(&old);
    assert_eq!(
        v["presets"],
        json!({"user": [{"effect": "compressor", "names": ["いつものピアノ"]}]})
    );
    assert_eq!(
        v["params"]["selected"]["preset"],
        json!({"kind": "factory", "id": "smallRoom"})
    );
    let mut new = old.clone();
    new.presets.user.clear();
    assert_eq!(
        AppState::changes_from(&old, &new),
        vec![Change::Presets(PresetsView::default())]
    );
    let mut back = old.clone();
    back.apply(Change::Presets(PresetsView::default()));
    assert_eq!(back, new);
}

#[test]
fn three_more_effects() {
    assert_eq!(
        to_json(&EffectKind::ALL),
        json!([
            "eq4",
            "compressor",
            "reverb",
            "delay",
            "chorus",
            "limiter",
            "gate",
            "deEsser",
            "saturation"
        ])
    );
}

#[test]
fn too_loud() {
    both_ways(
        Command::LowerAllTracks { db: 6.0 },
        json!({"type": "lowerAllTracks", "db": 6.0}),
    );
    both_ways(Command::DismissTooLoud, json!({"type": "dismissTooLoud"}));
}

#[test]
fn mp3_and_the_songs_information() {
    assert_eq!(ExportFormat::Mp3.extension(), "mp3");
    assert_eq!(to_json(&ExportFormat::Mp3), json!("mp3"));
    assert_eq!(EXPORT_MP3_KBPS, [128, 192, 256, 320]);
    assert!(EXPORT_MP3_KBPS.contains(&EXPORT_MP3_DEFAULT_KBPS));
    assert_eq!(EXPORT_MP3_SAMPLE_RATES, [44_100, 48_000]);
    assert_eq!(EXPORT_PICTURE_MAX_BYTES, 5 * 1024 * 1024);
    let changes = [
        (
            ExportChange::Mp3Kbps { kbps: 320 },
            json!({"kind": "mp3Kbps", "kbps": 320}),
        ),
        (
            ExportChange::Title {
                title: "はじめての曲".into(),
            },
            json!({"kind": "title", "title": "はじめての曲"}),
        ),
        (
            ExportChange::Artist {
                artist: "わたし".into(),
            },
            json!({"kind": "artist", "artist": "わたし"}),
        ),
        (
            ExportChange::ChoosePicture,
            json!({"kind": "choosePicture"}),
        ),
        (
            ExportChange::RemovePicture,
            json!({"kind": "removePicture"}),
        ),
    ];
    for (change, want) in changes {
        both_ways(
            Command::SetExport { change },
            json!({"type": "setExport", "change": want}),
        );
    }
}

#[test]
fn the_new_step_names() {
    let names = [
        (StepName::KeepPlayed, "keepPlayed"),
        (StepName::PlacePattern, "placePattern"),
        (StepName::RepeatClips, "repeatClips"),
        (StepName::ApplyPreset, "applyPreset"),
        (StepName::LowerAll, "lowerAll"),
    ];
    for (name, kind) in names {
        assert!(StepName::every_kind().contains(&name), "{kind}");
        both_ways(name, json!({"kind": kind}));
    }
    assert_eq!(StepName::every_kind().len(), 89);
}

#[test]
fn the_new_status_messages() {
    let messages = [
        (StatusMessage::NothingToKeep, json!({"id": "nothingToKeep"})),
        (StatusMessage::TypedKeysOn, json!({"id": "typedKeysOn"})),
        (
            StatusMessage::PatternNeedsFourFour,
            json!({"id": "patternNeedsFourFour"}),
        ),
        (
            StatusMessage::PresetOverwritten {
                name: "いつものピアノ".into(),
            },
            json!({"id": "presetOverwritten", "name": "いつものピアノ"}),
        ),
        (
            StatusMessage::PresetLanesKept { count: 2 },
            json!({"id": "presetLanesKept", "count": 2}),
        ),
        (
            StatusMessage::TooManyPresets {
                effect: EffectKind::Reverb,
            },
            json!({"id": "tooManyPresets", "effect": "reverb"}),
        ),
        (
            StatusMessage::TooLoud {
                by_db: 2.5,
                lower_db: 3,
                limiter: false,
            },
            json!({"id": "tooLoud", "byDb": 2.5, "lowerDb": 3, "limiter": false}),
        ),
        (
            StatusMessage::ExportPictureTooLarge {
                file: "cover.png".into(),
            },
            json!({"id": "exportPictureTooLarge", "file": "cover.png"}),
        ),
        (
            StatusMessage::ExportPictureUnreadable {
                file: "cover.jpg".into(),
            },
            json!({"id": "exportPictureUnreadable", "file": "cover.jpg"}),
        ),
        (
            StatusMessage::ClipWorkFailed { count: 2 },
            json!({"id": "clipWorkFailed", "count": 2}),
        ),
        (
            StatusMessage::ValueClamped {
                field: ValueField::Percent,
                value: "300".into(),
                range: None,
            },
            json!({"id": "valueClamped", "field": "percent", "value": "300", "range": null}),
        ),
        (
            StatusMessage::ValueRejected {
                field: ValueField::Amount,
                kept: "8".into(),
                range: None,
            },
            json!({"id": "valueRejected", "field": "amount", "kept": "8", "range": null}),
        ),
    ];
    for (message, want) in messages {
        both_ways(message, want);
    }
}

#[test]
fn the_telemetry_numbers() {
    assert_eq!(
        (TELEMETRY_VERSION, TELEMETRY_HEADER_LEN, SPECTRUM_BANDS),
        (5, 124, 24)
    );
    assert_eq!(TELEMETRY_NO_VALUE, i16::MIN);
    let quiet = Telemetry::default();
    assert_eq!(
        (
            quiet.master_reduction,
            quiet.spectrum,
            quiet.lufs_short,
            quiet.lufs_integrated,
            quiet.true_peak
        ),
        (
            0,
            [0; SPECTRUM_BANDS],
            TELEMETRY_NO_VALUE,
            TELEMETRY_NO_VALUE,
            TELEMETRY_NO_VALUE
        )
    );
}

fn generated_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/bridge/generated")
}

/// Writes `ui/src/bridge/generated/round12wave2.ts`: the numbers and lists protocol 32 added.
#[test]
fn export_round12_wave2_numbers() {
    let dir = generated_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let list = |v: Value| serde_json::to_string(&v).unwrap();
    std::fs::write(
        dir.join("round12wave2.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ PatternGroup }} from \"./PatternGroup\";\n\n\
             /** The id of 「パソコンのキーボード」 in the list of keyboards while 「キーで弾く」 is on. */\n\
             export const TYPED_KEYBOARD_ID = {};\n\n\
             /** 「くり返す…」: at most this many copies; the box starts at the default. */\n\
             export const REPEAT_COUNT_MAX = {REPEAT_COUNT_MAX};\n\
             export const REPEAT_COUNT_DEFAULT = {REPEAT_COUNT_DEFAULT};\n\n\
             /** The headings of the shelf's 「パターン」, in their order. */\n\
             export const PATTERN_GROUPS: readonly PatternGroup[] = {};\n\n\
             /** 「いまの設定を残す…」: the longest name, and how many presets one kind of effect keeps. */\n\
             export const PRESET_NAME_MAX = {PRESET_NAME_MAX};\n\
             export const PRESETS_PER_EFFECT = {PRESETS_PER_EFFECT};\n\n\
             /** 「ビットレート」 of mp3 (kbit/s), in the order of the list, and the one chosen at first. */\n\
             export const EXPORT_MP3_KBPS: readonly number[] = {};\n\
             export const EXPORT_MP3_DEFAULT_KBPS = {EXPORT_MP3_DEFAULT_KBPS};\n\n\
             /** 「絵」 of 「曲の情報」: the largest file, in bytes. */\n\
             export const EXPORT_PICTURE_MAX_BYTES = {EXPORT_PICTURE_MAX_BYTES};\n",
            list(json!(TYPED_KEYBOARD_ID)),
            list(json!(PatternGroup::ALL)),
            list(json!(EXPORT_MP3_KBPS)),
        ),
    )
    .unwrap();
}

/// Writes `ui/src/bridge/generated/presets.ts`: 「はじめから入っているもの」 of
/// `meloet_model::presets`, each with every parameter's value.
#[test]
fn export_factory_presets() {
    // The shortest decimal of the f32, as `effects.ts` writes them.
    let num = |v: f32| v.to_string().parse::<f64>().unwrap();
    let mut table = serde_json::Map::new();
    for kind in EffectKind::ALL {
        let name = to_json(&kind).as_str().unwrap().to_owned();
        let model = meloet_model::effects::EffectKind::from_name(&name).unwrap();
        let presets: Vec<Value> = (meloet_model::presets::factory(model).iter())
            .map(|p| {
                let values: Vec<f64> = p.values_of(model).into_iter().map(num).collect();
                json!({"id": p.id, "values": values})
            })
            .collect();
        table.insert(name, presets.into());
    }
    std::fs::write(
        generated_dir().join("presets.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ EffectKind }} from \"./EffectKind\";\n\n\
             /** A preset that comes with meloet: every parameter's value, in the order of `EFFECT_PARAMS`. Its name is `preset.<effect>.<id>`. */\n\
             export interface FactoryPreset {{\n  \
             readonly id: string;\n  \
             readonly values: readonly number[];\n\
             }}\n\n\
             /** 「はじめから入っているもの」 of each effect that comes with meloet, in the order of the menu. */\n\
             export const FACTORY_PRESETS: {{ readonly [K in EffectKind]: readonly FactoryPreset[] }} = {};\n",
            serde_json::to_string_pretty(&table).unwrap()
        ),
    )
    .unwrap();
}
