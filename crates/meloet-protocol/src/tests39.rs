//! The shapes protocol 39 added (bug sweep, 2026-11-25): the kind of the clips kept for
//! 「貼りつけ」, so the screen can turn the row off over a track of the other kind.

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
fn the_clipboard_kind() {
    both_ways(
        ArrangeView {
            clipboard_clips: 2,
            clipboard_kind: Some(ClipKind::Vocal),
            ..ArrangeView::default()
        },
        json!({"selectedClips": [], "selectedClip": null, "pianoRollClip": null,
               "clipboardClips": 2, "clipboardKind": "vocal", "tallTrack": null}),
    );
    both_ways(ClipKind::Keys, json!("keys"));
}
