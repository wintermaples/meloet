//! The shapes protocol 35 added (the owner, 2026-11-24): 「このテイク以外を消す」 in the
//! right-click menu of a clip, and its step name.

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
fn the_command() {
    both_ways(
        Command::DeleteOtherTakes { clip: 4 },
        json!({"type": "deleteOtherTakes", "clip": 4}),
    );
}

#[test]
fn the_step_name_and_its_row() {
    assert!(StepName::every_kind().contains(&StepName::DeleteOtherTakes));
    assert_eq!(StepName::every_kind().len(), 105);
    both_ways(
        StepName::DeleteOtherTakes,
        json!({"kind": "deleteOtherTakes"}),
    );
    both_ways(
        HistoryRow {
            index: 3,
            step: 7,
            name: StepName::DeleteOtherTakes,
            what: StepWhat::Take { number: 2 },
            place: None,
        },
        json!({"index": 3, "step": 7, "name": {"kind": "deleteOtherTakes"},
               "what": {"kind": "take", "number": 2}, "place": null}),
    );
}
