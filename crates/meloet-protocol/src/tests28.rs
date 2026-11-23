//! The shapes protocol 28 added: clip names, cutting at a place, joining clips, making an
//! empty clip, a marker at a place, overdub and the chord stamp.

use serde_json::{Value, json};

use super::*;

const BAR: u64 = 4 * 1920;

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
fn the_version() {
    assert_eq!(PROTOCOL_VERSION, 28);
}

#[test]
fn the_clip_commands() {
    let cases = vec![
        (
            Command::RenameClip {
                clip: 4,
                name: "サビ".into(),
            },
            json!({"type": "renameClip", "clip": 4, "name": "サビ"}),
        ),
        (
            Command::SplitClipAt {
                clip: 4,
                tick: 6 * BAR + 960,
            },
            json!({"type": "splitClipAt", "clip": 4, "tick": 6 * BAR + 960}),
        ),
        (
            Command::JoinClips {
                clips: vec![4, 9, 12],
            },
            json!({"type": "joinClips", "clips": [4, 9, 12]}),
        ),
        (
            Command::CreateClip {
                track: 7,
                tick: 9 * BAR + 480,
            },
            json!({"type": "createClip", "track": 7, "tick": 9 * BAR + 480}),
        ),
    ];
    for (command, json) in cases {
        both_ways(command, json);
    }
}

#[test]
fn a_marker_at_a_place_or_at_the_play_line() {
    both_ways(
        Command::AddMarker {
            name: "マーカー 2".into(),
            tick: Some(3 * BAR + 100),
        },
        json!({"type": "addMarker", "name": "マーカー 2", "tick": 3 * BAR + 100}),
    );
    both_ways(
        Command::AddMarker {
            name: "マーカー 2".into(),
            tick: None,
        },
        json!({"type": "addMarker", "name": "マーカー 2", "tick": null}),
    );
    // tools/design/shoot_app.py still sends the shape of protocol 27.
    assert_eq!(
        serde_json::from_value::<Command>(json!({"type": "addMarker", "name": "サビ"})).unwrap(),
        Command::AddMarker {
            name: "サビ".into(),
            tick: None,
        }
    );
}

#[test]
fn overdub_is_a_command_and_a_field_of_the_transport() {
    both_ways(
        Command::SetOverdub { on: true },
        json!({"type": "setOverdub", "on": true}),
    );
    let transport = TransportView {
        state: TransportState::Stopped,
        position: 0,
        metronome: false,
        count_in_bars: 1,
        count_in_beats: 4,
        can_play: true,
        can_record: true,
        recording: None,
        recording_vocals: Vec::new(),
        recording_faults: None,
        overdub: true,
    };
    assert_eq!(to_json(&transport)["overdub"], true);
}

#[test]
fn the_chord_stamp() {
    both_ways(
        Command::AddNotes {
            take: Some(31),
            start: 2 * BAR,
            length: 1920,
            keys: vec![60, 64, 67],
            velocity: 96,
        },
        json!({"type": "addNotes", "take": 31, "start": 2 * BAR, "length": 1920,
               "keys": [60, 64, 67], "velocity": 96}),
    );
    both_ways(
        Command::AddNotes {
            take: None,
            start: 0,
            length: 480,
            keys: vec![57, 60, 64],
            velocity: 100,
        },
        json!({"type": "addNotes", "take": null, "start": 0, "length": 480,
               "keys": [57, 60, 64], "velocity": 100}),
    );
}

#[test]
fn the_new_step_names() {
    for (name, kind) in [
        (StepName::RenameClip, "renameClip"),
        (StepName::JoinClips, "joinClips"),
        (StepName::WriteClip, "writeClip"),
    ] {
        both_ways(name, json!({ "kind": kind }));
    }
    both_ways(
        StepWhat::Renamed {
            from: "ピアノ".into(),
            to: "サビ".into(),
        },
        json!({"kind": "renamed", "from": "ピアノ", "to": "サビ"}),
    );
}
