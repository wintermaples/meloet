//! The shapes protocol 38 added (bug sweep, 2026-11-25): what the status line says when the
//! files of a recording, or a file brought in, cannot be written.

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
    assert_eq!(PROTOCOL_VERSION, 38);
}

#[test]
fn the_messages() {
    both_ways(
        StatusMessage::VocalsNotRecorded { keys: true },
        json!({"id": "vocalsNotRecorded", "keys": true}),
    );
    both_ways(
        StatusMessage::RecordingStoppedDisk {
            reason: DiskStop::CannotWrite,
        },
        json!({"id": "recordingStoppedDisk", "reason": "cannotWrite"}),
    );
    both_ways(
        StatusMessage::RecordingStoppedDisk {
            reason: DiskStop::FileFull,
        },
        json!({"id": "recordingStoppedDisk", "reason": "fileFull"}),
    );
    both_ways(
        StatusMessage::AudioNotCopied {
            file: "声.wav".into(),
        },
        json!({"id": "audioNotCopied", "file": "声.wav"}),
    );
}
