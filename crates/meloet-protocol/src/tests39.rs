//! The shapes protocol 39 added (bug sweep, 2026-11-25): the export dialog asks before it
//! writes over files that are there already.

use serde_json::{Value, json};

use super::*;

fn both_ways<T>(v: T, json: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(serde_json::to_value(&v).unwrap(), json);
    assert_eq!(serde_json::from_value::<T>(json).unwrap(), v);
}

#[test]
fn the_version() {
    assert_eq!(PROTOCOL_VERSION, 39);
}

#[test]
fn the_question_before_writing_over() {
    both_ways(
        ExportReplace {
            count: 1,
            file: "はじめての曲.wav".into(),
        },
        json!({"count": 1, "file": "はじめての曲.wav"}),
    );
    both_ways(
        ExportReplace {
            count: 3,
            file: "ピアノ.wav".into(),
        },
        json!({"count": 3, "file": "ピアノ.wav"}),
    );
}
