//! Where values are on the knobs a person sees, 0..=1: what automation points hold
//! (docs/architecture.md 21-5). The screen has the same formulas (`dbToFrac`, `fracToDb`,
//! `fractionOf`, `valueAt`); the table written here is what it is checked against.

use std::fmt::Write;
use std::path::PathBuf;

use meloet_model::effects::{EffectKind, ParamShape};
use meloet_model::{Gain, Pan};

#[test]
fn the_fader_scale_has_its_marks() {
    for (db, position) in [
        (6.0, 1.0),
        (0.0, 0.8),
        (-3.0, 0.7),
        (-6.0, 0.6),
        (-12.0, 0.4),
        (-24.0, 0.2),
        (-60.0, 0.0),
    ] {
        let gain = Gain::from_db(db);
        assert!((gain.position() - position).abs() < 1e-6, "{db}");
        if position > 0.0 {
            assert_eq!(Gain::from_position(position), gain, "{position}");
        }
    }
    assert_eq!(Gain::SILENT.position(), 0.0);
    assert_eq!(Gain::from_position(0.0), Gain::SILENT);
    assert_eq!(Gain::from_position(f32::NAN), Gain::SILENT);
    assert_eq!(Gain::from_position(1.5), Gain::from_db(6.0));
    // Halfway between 0 dB and +6.
    assert_eq!(Gain::from_position(0.9), Gain::from_db(3.0));
    // Back and forth, within the 0.01 dB a gain is kept in.
    for i in 1..=1000 {
        let p = i as f32 / 1000.0;
        let back = Gain::from_position(p).position();
        assert!((back - p).abs() < 2e-4, "{p} {back}");
    }
}

#[test]
fn the_pan_is_a_straight_line() {
    assert_eq!(Pan::new(-100).position(), 0.0);
    assert_eq!(Pan::CENTER.position(), 0.5);
    assert_eq!(Pan::new(100).position(), 1.0);
    assert_eq!(Pan::new(-20).position(), 0.4);
    assert!((Pan::unit_at(0.4) + 0.2).abs() < 1e-6);
    assert_eq!(Pan::unit_at(2.0), 1.0);
    assert_eq!(Pan::unit_at(f32::NAN), 0.0);
}

#[test]
fn parameters_go_by_their_bar() {
    for kind in EffectKind::ALL {
        for spec in kind.params() {
            assert_eq!(spec.position(spec.min), 0.0, "{}", spec.name);
            assert_eq!(spec.position(spec.max), 1.0, "{}", spec.name);
            assert_eq!(spec.value_at(0.0), spec.min, "{}", spec.name);
            assert!((spec.value_at(1.0) - spec.max).abs() <= spec.max.abs() * 1e-6);
            for i in 0..=100 {
                let p = i as f32 / 100.0;
                let v = spec.value_at(p);
                assert!(v >= spec.min && v <= spec.max, "{} {p} {v}", spec.name);
                if matches!(spec.shape, ParamShape::Continuous) {
                    assert!((spec.position(v) - p).abs() < 1e-4, "{} {p}", spec.name);
                }
            }
        }
    }
    // A frequency goes by its logarithm: halfway from 20 Hz to 1000 Hz is about 141 Hz.
    let low = EffectKind::Eq4.param(0).unwrap();
    assert!((low.value_at(0.5) - (20.0f32 * 1000.0).sqrt()).abs() < 0.01);
    // The delay's interval: five steps.
    let step = EffectKind::Delay.param(0).unwrap();
    assert_eq!(step.position(2.0), 0.5);
    assert_eq!(step.value_at(0.6), 2.0);
    assert_eq!(step.value_at(0.63), 3.0);
    let switch = EffectKind::Delay.param(2).unwrap();
    assert_eq!((switch.value_at(0.49), switch.value_at(0.5)), (0.0, 1.0));
}

fn number(x: f64) -> String {
    let s = format!("{x}");
    if s == "-0" { "0".into() } else { s }
}

fn table_json() -> String {
    let mut out = String::from("{\n  \"fader\": [\n");
    let dbs: Vec<Option<f64>> = std::iter::once(None)
        .chain((-610..=60).step_by(7).map(|i| Some(f64::from(i) / 10.0)))
        .chain([Some(-3.0), Some(0.0), Some(6.0)])
        .collect();
    let rows: Vec<String> = dbs
        .iter()
        .map(|db| {
            let gain = db.map_or(Gain::SILENT, Gain::from_db);
            let db = db.map_or("null".into(), number);
            format!(
                "    {{ \"db\": {db}, \"position\": {} }}",
                number(f64::from(gain.position()))
            )
        })
        .chain((0..=100).map(|i| {
            let p = i as f32 / 100.0;
            let db = Gain::from_position(p).db().map_or("null".into(), number);
            format!(
                "    {{ \"position\": {}, \"db\": {db} }}",
                number(f64::from(p))
            )
        }))
        .collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n  ],\n  \"pan\": [\n");
    let rows: Vec<String> = (-100..=100)
        .step_by(10)
        .map(|pan| {
            format!(
                "    {{ \"pan\": {pan}, \"position\": {} }}",
                number(f64::from(Pan::new(pan).position()))
            )
        })
        .collect();
    out.push_str(&rows.join(",\n"));
    out.push_str("\n  ],\n  \"params\": [\n");
    let mut rows = Vec::new();
    for kind in EffectKind::ALL {
        for (index, spec) in kind.params().iter().enumerate() {
            let mut row = format!(
                "    {{ \"effect\": \"{}\", \"param\": {index}, \"values\": [",
                kind.name()
            );
            let values: Vec<f32> = (0..=10)
                .map(|i| spec.min + (spec.max - spec.min) * i as f32 / 10.0)
                .collect();
            let pairs: Vec<String> = (values.iter())
                .map(|&v| {
                    format!(
                        "[{}, {}]",
                        number(f64::from(v)),
                        number(f64::from(spec.position(v)))
                    )
                })
                .collect();
            row.push_str(&pairs.join(", "));
            row.push_str("], \"positions\": [");
            let pairs: Vec<String> = (0..=20)
                .map(|i| {
                    let p = i as f32 / 20.0;
                    format!(
                        "[{}, {}]",
                        number(f64::from(p)),
                        number(f64::from(spec.value_at(p)))
                    )
                })
                .collect();
            row.push_str(&pairs.join(", "));
            row.push_str("] }");
            rows.push(row);
        }
    }
    out.push_str(&rows.join(",\n"));
    let _ = writeln!(out, "\n  ]\n}}");
    out
}

/// Writes `ui/tests/fixtures/automation-positions.json`, as the tempo map's table is written:
/// a table that is not the committed one shows up in `git status`. `params.positions` holds
/// the value of the line, not rounded to what the panel shows.
#[test]
fn write_the_table_for_the_screen() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/tests/fixtures/automation-positions.json");
    let table = table_json();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(table.as_str()) {
        std::fs::write(&path, &table).unwrap();
    }
    assert!(!table.contains("NaN") && !table.contains("inf"));
}
