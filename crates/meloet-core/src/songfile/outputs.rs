//! `"outputs"` of a plugin instrument (still version 7; docs/architecture.md 20-15):
//! `{"rule": 1, "sets": [{"id": "out3", "gainDb": -3.0, "pan": -0.5, "on": false}]}`, `pan`
//! from -1 (all left) to 1 (all right). Left out at the default, and inside it `rule` when 1,
//! `sets` when empty, `gainDb` when 0, `pan` and `on` when the rule decides them. An insert's
//! plugin never has it.
//!
//! What is not understood reads as the default; a rule this meloet does not know is kept as
//! read and written back.

use meloet_model::{Gain, OutputSetting, Pan, PluginOutputs};
use serde::{Deserialize, Serialize};

pub(super) const FIELD: &str = "outputs";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputsFile {
    #[serde(default = "default_rule", skip_serializing_if = "is_default_rule")]
    rule: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sets: Vec<SetFile>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetFile {
    id: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    gain_db: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pan: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    on: Option<bool>,
}

fn default_rule() -> u32 {
    PluginOutputs::DEFAULT_RULE
}

fn is_default_rule(rule: &u32) -> bool {
    *rule == PluginOutputs::DEFAULT_RULE
}

fn is_zero(db: &f64) -> bool {
    *db == 0.0
}

pub(super) fn outputs_file(outputs: &PluginOutputs) -> Option<serde_json::Value> {
    if outputs.is_default() {
        return None;
    }
    let file = OutputsFile {
        rule: outputs.rule,
        sets: (outputs.sets.iter())
            .map(|set| SetFile {
                id: set.id.to_string(),
                gain_db: set.gain.db().unwrap_or(Gain::MIN_DB),
                pan: set.pan.map(|pan| f64::from(pan.get()) / 100.0),
                on: set.on,
            })
            .collect(),
    };
    Some(serde_json::to_value(file).expect("the outputs serialize"))
}

pub(super) fn outputs_of(value: Option<serde_json::Value>) -> PluginOutputs {
    let Some(file) = value.and_then(|v| serde_json::from_value::<OutputsFile>(v).ok()) else {
        return PluginOutputs::default();
    };
    PluginOutputs::new(
        file.rule,
        file.sets.into_iter().map(|set| OutputSetting {
            id: set.id.into(),
            gain: Gain::from_db(set.gain_db),
            pan: set.pan.map(|pan| Pan::new((pan * 100.0).round() as i32)),
            on: set.on,
        }),
    )
}
