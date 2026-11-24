//! The shape protocol 37 added (the owner, 2026-11-25): the keys a plugin instrument names
//! (LV2 midnam), for the drum piano roll of its track.

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
    assert_eq!(PROTOCOL_VERSION, 37);
}

#[test]
fn a_named_key() {
    both_ways(
        vec![
            PluginKeyName {
                key: 36,
                name: "Kick Drum".into(),
            },
            PluginKeyName {
                key: 38,
                name: "Snare Center".into(),
            },
        ],
        json!([{"key": 36, "name": "Kick Drum"}, {"key": 38, "name": "Snare Center"}]),
    );
}
