//! The shapes protocol 39 added (bug sweep, 2026-11-25): the song dialog that asks, right
//! after "戻さない", whether the recovery file goes too; the export dialog asks before it
//! writes over files that are there already; the kind of the clips the clipboard holds.

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
fn the_recovery_file_question() {
    both_ways(
        SongDialogView {
            id: 4,
            kind: SongDialogKind::DeleteRecovery,
            title: "はじめての曲".into(),
            previous: false,
        },
        json!({"id": 4, "kind": "deleteRecovery", "title": "はじめての曲", "previous": false}),
    );
    both_ways(
        Command::AnswerSongDialog {
            dialog: 4,
            answer: SongAnswer::Delete,
        },
        json!({"type": "answerSongDialog", "dialog": 4, "answer": "delete"}),
    );
    both_ways(
        Command::AnswerSongDialog {
            dialog: 4,
            answer: SongAnswer::Keep,
        },
        json!({"type": "answerSongDialog", "dialog": 4, "answer": "keep"}),
    );
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
