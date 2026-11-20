//! The table the screen's copy of the fade shape is checked against
//! (`ui/src/components/arrange/fade.ts`, `ui/tests/arrange.fade.test.ts`).

use std::fmt::Write as _;
use std::path::PathBuf;

use meloet_model::{clip_fade_gain, fade_gain};

fn table_json() -> String {
    let mut out = String::from("{\n  \"shape\": [\n");
    let xs: Vec<f64> = [-0.5, 1.5]
        .into_iter()
        .chain((0..=64).map(|i| f64::from(i) / 64.0))
        .chain([1e-9, 0.001, 0.333, 0.999, 1.0 - 1e-9])
        .collect();
    for (i, x) in xs.iter().enumerate() {
        let comma = if i + 1 < xs.len() { "," } else { "" };
        let _ = writeln!(out, "    [{x:?}, {:?}]{comma}", fade_gain(*x));
    }
    out.push_str("  ],\n  \"clips\": [\n");
    // length, fade in, fade out
    let clips = [
        (1000.0, 0.0, 0.0),
        (1000.0, 100.0, 200.0),
        (7680.0, 1920.0, 0.0),
        (7680.0, 0.0, 960.0),
        (100.0, 100.0, 100.0),
        (48_000.0, 5760.0, 12_000.5),
    ];
    let mut rows = Vec::new();
    for (length, fade_in, fade_out) in clips {
        for at in [
            -1.0,
            0.0,
            0.5,
            fade_in / 3.0,
            fade_in,
            length / 2.0,
            length - fade_out,
            length - fade_out / 4.0,
            length - 0.5,
            length,
        ] {
            rows.push(format!(
                "    {{\"length\": {length:?}, \"fadeIn\": {fade_in:?}, \"fadeOut\": {fade_out:?}, \
                 \"at\": {at:?}, \"gain\": {:?}}}",
                clip_fade_gain(at, length, fade_in, fade_out)
            ));
        }
    }
    out.push_str(&rows.join(",\n"));
    out.push_str("\n  ]\n}\n");
    out
}

/// Writes `ui/src/components/arrange/fade.table.json`, as `tests/tempo_map.rs` writes its
/// table: one that is not the committed one shows up in `git status`.
#[test]
fn write_the_table_for_the_screen() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/src/components/arrange/fade.table.json");
    let table = table_json();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(table.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &table).unwrap();
    }
    assert!(!table.contains("NaN") && !table.contains("inf"));
}
