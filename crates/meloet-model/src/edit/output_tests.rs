use std::sync::Arc;

use super::*;
use crate::{History, PluginOutputs, PluginRef};

const T1: TrackId = TrackId(1);

/// The first track of a new song with Surge XT for its instrument.
fn surge_doc() -> Doc {
    let mut doc = Doc::new_default();
    let surge = PluginRef::new("lv2:urn:surge", "Surge XT", "Surge Synth Team", "");
    Arc::make_mut(&mut doc.tracks[0]).instrument = InstrumentRef::Plugin(Arc::new(surge));
    doc
}

fn outputs(doc: &Doc) -> PluginOutputs {
    match &doc.track(T1).unwrap().instrument {
        InstrumentRef::Plugin(p) => p.outputs.clone(),
        other => panic!("{other:?}"),
    }
}

fn setting(id: &str, db: f64, pan: Option<i32>, on: Option<bool>) -> OutputSetting {
    OutputSetting {
        id: id.into(),
        gain: Gain::from_db(db),
        pan: pan.map(Pan::new),
        on,
    }
}

fn set(doc: &Doc, id: &str, change: OutputChange) -> Result<Doc, EditError> {
    set_plugin_output(doc, T1, id, change).map(|e| e.doc)
}

#[test]
fn a_set_is_kept_changed_and_reset() {
    let doc = surge_doc();
    assert_eq!(outputs(&doc), PluginOutputs::default());
    assert!(outputs(&doc).is_default());

    let e = set_plugin_output(
        &doc,
        T1,
        "out3",
        OutputChange {
            gain: Some(Gain::from_db(-3.0)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(e.step, StepKind::SetPluginOutput { track: T1 });
    let doc = e.doc;
    assert_eq!(
        *outputs(&doc).sets,
        [setting("out3", -3.0, None, None)],
        "the other fields stay at the defaults"
    );

    // Another field of the same set, and another set, which goes before it by its id.
    let doc = set(
        &doc,
        "out3",
        OutputChange {
            pan: Some(Some(Pan::new(-50))),
            ..Default::default()
        },
    )
    .unwrap();
    let doc = set(
        &doc,
        "out1",
        OutputChange {
            on: Some(Some(false)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        *outputs(&doc).sets,
        [
            setting("out1", 0.0, None, Some(false)),
            setting("out3", -3.0, Some(-50), None)
        ]
    );
    assert_eq!(outputs(&doc).rule, PluginOutputs::DEFAULT_RULE);

    // A field back to what the rule says.
    let doc = set(
        &doc,
        "out3",
        OutputChange {
            pan: Some(None),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        outputs(&doc).get("out3"),
        Some(&setting("out3", -3.0, None, None))
    );

    // Reset, then the other fields: only those are left.
    let doc = set(
        &doc,
        "out3",
        OutputChange {
            on: Some(Some(true)),
            reset: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        outputs(&doc).get("out3"),
        Some(&setting("out3", 0.0, None, Some(true)))
    );

    // Reset alone: the entry goes.
    let doc = set(
        &doc,
        "out3",
        OutputChange {
            reset: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        *outputs(&doc).sets,
        [setting("out1", 0.0, None, Some(false))]
    );

    // Every field back to its default by hand: the entry goes too.
    let doc = set(
        &doc,
        "out1",
        OutputChange {
            on: Some(None),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(outputs(&doc), PluginOutputs::default());
}

#[test]
fn values_out_of_range_are_brought_into_range() {
    let doc = set(
        &surge_doc(),
        "out2",
        OutputChange {
            gain: Some(Gain::SILENT),
            pan: Some(Some(Pan::new(300))),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        outputs(&doc).get("out2"),
        Some(&setting("out2", Gain::MIN_DB, Some(100), None)),
        "−∞ is −60 dB"
    );
    let doc = set(
        &doc,
        "out2",
        OutputChange {
            gain: Some(Gain::from_db(20.0)),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        outputs(&doc).get("out2").unwrap().gain.db(),
        Some(Gain::MAX_DB)
    );
}

#[test]
fn the_same_again_is_no_change() {
    let doc = surge_doc();
    for change in [
        OutputChange::default(),
        OutputChange {
            reset: true,
            ..Default::default()
        },
        OutputChange {
            gain: Some(Gain::UNITY),
            pan: Some(None),
            on: Some(None),
            reset: false,
        },
    ] {
        assert_eq!(
            set(&doc, "out3", change),
            Err(EditError::Unchanged),
            "{change:?}"
        );
    }
    let lower = OutputChange {
        gain: Some(Gain::from_db(-6.0)),
        ..Default::default()
    };
    let doc = set(&doc, "out3", lower).unwrap();
    assert_eq!(set(&doc, "out3", lower), Err(EditError::Unchanged));
    let silent = OutputChange {
        gain: Some(Gain::SILENT),
        ..Default::default()
    };
    let doc = set(&doc, "out3", silent).unwrap();
    assert_eq!(
        set(&doc, "out3", silent),
        Err(EditError::Unchanged),
        "−60 dB already"
    );
}

#[test]
fn only_a_plugin_instrument_has_output_sets() {
    let change = OutputChange {
        on: Some(Some(false)),
        ..Default::default()
    };
    let piano = Doc::new_default();
    assert_eq!(
        set_plugin_output(&piano, T1, "out1", change),
        Err(EditError::NoSuchPlugin)
    );
    assert_eq!(
        set_plugin_output(&surge_doc(), TrackId(99), "out1", change),
        Err(EditError::NoSuchTrack(TrackId(99)))
    );
}

#[test]
fn undo_and_redo_bring_the_table_back() {
    let start = surge_doc();
    let mut now = start.clone();
    let mut h = History::new();
    let first = setting("out3", -3.0, Some(25), Some(false));
    let e = set_plugin_output(
        &now,
        T1,
        "out3",
        OutputChange {
            gain: Some(first.gain),
            pan: Some(first.pan),
            on: Some(first.on),
            reset: false,
        },
    )
    .unwrap();
    h.apply(&mut now, e);
    let after_first = now.clone();
    let e = set_plugin_output(
        &now,
        T1,
        "out3",
        OutputChange {
            reset: true,
            ..Default::default()
        },
    )
    .unwrap();
    h.apply(&mut now, e);
    assert_eq!(outputs(&now), PluginOutputs::default());

    assert_eq!(
        h.undo(&mut now),
        Some(StepKind::SetPluginOutput { track: T1 })
    );
    assert_eq!(now, after_first);
    assert_eq!(*outputs(&now).sets, [first]);
    assert!(h.undo(&mut now).is_some());
    assert_eq!(now, start);
    assert!(h.redo(&mut now).is_some());
    assert_eq!(now, after_first);
    assert!(h.redo(&mut now).is_some());
    assert_eq!(outputs(&now), PluginOutputs::default());
}

#[test]
fn a_table_is_kept_in_one_form() {
    let made = PluginOutputs::new(
        1,
        [
            setting("b", -1.0, None, None),
            setting("a", 0.0, None, None),
            setting("c", 0.0, Some(10), None),
            setting("b", -2.0, None, None),
            setting("d", -90.0, None, None),
        ],
    );
    assert_eq!(
        *made.sets,
        [
            setting("b", -1.0, None, None),
            setting("c", 0.0, Some(10), None),
            setting("d", Gain::MIN_DB, None, None)
        ],
        "in the order of the ids, the first of each id, none at the defaults"
    );
    let same = PluginOutputs::new(
        1,
        [
            setting("d", -60.0, None, None),
            setting("c", 0.0, Some(10), None),
            setting("b", -1.0, None, None),
        ],
    );
    assert_eq!(made, same);
    assert!(PluginOutputs::new(1, []).is_default());
    assert!(!PluginOutputs::new(2, []).is_default());

    // Another state keeps the table, and another table is another instrument.
    let plugin = PluginRef::new("lv2:urn:surge", "Surge XT", "", "").with_outputs(made.clone());
    assert_eq!(plugin.with_state(Arc::from([1u8])).outputs, made);
    assert_ne!(plugin, plugin.with_outputs(PluginOutputs::default()));
}
