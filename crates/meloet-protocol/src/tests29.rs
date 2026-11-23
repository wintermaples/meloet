//! The shapes protocol 29 added: 「ピアノロールでひらく」 of a track's header, and the AI link.

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
fn the_new_commands() {
    both_ways(
        Command::OpenPianoRoll { track: 3 },
        json!({"type": "openPianoRoll", "track": 3}),
    );
    both_ways(
        Command::SetAiLink { on: true },
        json!({"type": "setAiLink", "on": true}),
    );
    both_ways(
        Command::SetAiLink { on: false },
        json!({"type": "setAiLink", "on": false}),
    );
}

#[test]
fn the_ai_link_section() {
    for (port, name) in [
        (AiPort::Closed, "closed"),
        (AiPort::Waiting, "waiting"),
        (AiPort::Connected, "connected"),
    ] {
        both_ways(
            AiLinkView { on: true, port },
            json!({"on": true, "port": name}),
        );
    }
    assert_eq!(
        AiLinkView::default(),
        AiLinkView {
            on: false,
            port: AiPort::Closed
        }
    );
    let state = tests::sample_state();
    assert_eq!(
        to_json(&state)["aiLink"],
        json!({"on": false, "port": "closed"})
    );
}

#[test]
fn the_ai_link_is_a_section_of_its_own() {
    let old = tests::sample_state();
    let mut new = old.clone();
    new.ai_link = AiLinkView {
        on: true,
        port: AiPort::Connected,
    };
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(changes, vec![Change::AiLink(new.ai_link)]);
    assert_eq!(
        to_json(&changes[0]),
        json!({"section": "aiLink", "value": {"on": true, "port": "connected"}})
    );
    let mut applied = old;
    applied.apply(changes[0].clone());
    assert_eq!(applied, new);
}
