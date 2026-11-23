use super::*;
use crate::settings::temp_dir;
use meloet_model::effects::{compressor, delay};

fn all_of(kind: EffectKind) -> Vec<(u32, f32)> {
    (kind.params().iter().enumerate())
        .map(|(i, p)| (i as u32, p.default))
        .collect()
}

#[test]
fn names_are_trimmed_and_at_most_forty_characters() {
    assert_eq!(name_of("  いつもの  ").as_deref(), Some("いつもの"));
    assert_eq!(name_of("   "), None);
    assert_eq!(name_of(""), None);
    let long = "あ".repeat(PRESET_NAME_MAX + 5);
    assert_eq!(
        name_of(&long).map(|n| n.chars().count()),
        Some(PRESET_NAME_MAX)
    );
    let spaced = format!("{} b", "a".repeat(PRESET_NAME_MAX - 1));
    assert_eq!(
        name_of(&spaced).as_deref(),
        Some(&*"a".repeat(PRESET_NAME_MAX - 1))
    );
}

#[test]
fn the_same_name_is_written_over_in_its_place_and_a_hundred_at_most() {
    let mut presets = UserPresets::default();
    let kind = EffectKind::Compressor;
    assert_eq!(presets.keep(kind, "a".into(), all_of(kind)), Kept::New);
    assert_eq!(presets.keep(kind, "b".into(), all_of(kind)), Kept::New);
    let mut other = all_of(kind);
    other[compressor::RATIO as usize].1 = 8.0;
    assert_eq!(
        presets.keep(kind, "a".into(), other.clone()),
        Kept::Overwritten
    );
    let names: Vec<&str> = presets.of(kind).iter().map(|p| &*p.name).collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(presets.find(kind, "a").map(|p| &p.values), Some(&other));
    assert!(presets.of(EffectKind::Reverb).is_empty());

    for n in 2..PRESETS_PER_EFFECT {
        assert_eq!(presets.keep(kind, n.to_string(), all_of(kind)), Kept::New);
    }
    assert_eq!(
        presets.keep(kind, "one more".into(), all_of(kind)),
        Kept::TooMany
    );
    assert_eq!(
        presets.keep(kind, "b".into(), other),
        Kept::Overwritten,
        "written over at a hundred"
    );
    assert_eq!(
        presets.keep(EffectKind::Limiter, "a".into(), all_of(EffectKind::Limiter)),
        Kept::New,
        "a hundred for each kind"
    );

    assert!(presets.remove(kind, "a"));
    assert!(!presets.remove(kind, "a"));
    assert_eq!(presets.of(kind).len(), PRESETS_PER_EFFECT - 1);
}

#[test]
fn the_view_lists_the_kinds_that_have_any_in_their_order() {
    let mut presets = UserPresets::default();
    presets.keep(EffectKind::Limiter, "x".into(), Vec::new());
    presets.keep(EffectKind::Eq4, "y".into(), Vec::new());
    presets.keep(EffectKind::Eq4, "z".into(), Vec::new());
    let view = presets.view();
    let kinds: Vec<_> = view.user.iter().map(|u| u.effect).collect();
    assert_eq!(
        kinds,
        [
            meloet_protocol::EffectKind::Eq4,
            meloet_protocol::EffectKind::Limiter
        ]
    );
    assert_eq!(view.user[0].names, ["y", "z"]);
}

#[test]
fn a_file_goes_round_with_steps_and_switches_by_name() {
    let dir = temp_dir("presets");
    let mut presets = UserPresets::default();
    let mut values = all_of(EffectKind::Delay);
    values[delay::INTERVAL_STEP as usize].1 = 1.0;
    values[delay::PING_PONG as usize].1 = 1.0;
    values[delay::FEEDBACK as usize].1 = 42.5;
    presets.keep(EffectKind::Delay, "はねる".into(), values);
    presets.keep(
        EffectKind::Compressor,
        "いつもの".into(),
        all_of(EffectKind::Compressor),
    );
    presets.keep(EffectKind::Gate, "静かに".into(), all_of(EffectKind::Gate));
    save(&dir, &presets).unwrap();
    assert!(!dir.join("presets.json.tmp").exists());
    assert_eq!(load(&dir), presets);

    let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["version"], 1);
    let delay = &json["effects"]["delay"][0];
    assert_eq!(delay["name"], "はねる");
    assert_eq!(delay["params"]["intervalStep"], "eighthDotted");
    assert_eq!(delay["params"]["pingPong"], true);
    assert_eq!(delay["params"]["feedback"], 42.5);
}

#[test]
fn unknown_parameters_are_skipped_missing_ones_left_out_and_values_brought_into_range() {
    let dir = temp_dir("presets-odd");
    std::fs::write(
        dir.join(FILE_NAME),
        r#"{"version": 1, "effects": {
            "compressor": [
                {"name": "  a  ", "params": {"ratio": 99, "wobble": 3, "attack": "fast"}},
                {"name": "a", "params": {"ratio": 2}},
                {"name": "", "params": {}},
                {"params": {}},
                7
            ],
            "flanger": [{"name": "later", "params": {"rate": 1}}]
        }}"#,
    )
    .unwrap();
    let presets = load(&dir);
    let kind = EffectKind::Compressor;
    assert_eq!(
        presets.of(kind),
        [UserPreset {
            name: "a".into(),
            values: vec![(compressor::RATIO, 20.0)],
        }]
    );
    save(&dir, &presets).unwrap();
    let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        json["effects"]["flanger"][0]["name"], "later",
        "an effect this meloet does not know is written back"
    );
    assert_eq!(load(&dir), presets);
}

#[test]
fn missing_or_broken_files_have_no_presets() {
    let dir = temp_dir("presets-broken");
    assert_eq!(load(&dir), UserPresets::default());
    for broken in [
        "",
        "{",
        r#"{"version":2,"effects":{"limiter":[{"name":"a","params":{}}]}}"#,
        r#"{"effects":{}}"#,
        "[]",
    ] {
        std::fs::write(dir.join(FILE_NAME), broken).unwrap();
        assert_eq!(load(&dir), UserPresets::default(), "{broken:?}");
    }
}
