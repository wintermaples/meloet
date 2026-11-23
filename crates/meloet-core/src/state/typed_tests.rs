//! R2 「キーで弾く」 in the rules: the PC keyboard is a keyboard of the list while it is on.

use meloet_midi::Keyboard;
use meloet_protocol::{ChooserView, Command, StatusMessage, TYPED_KEYBOARD_ID};

use super::tests::playing;
use super::*;

fn keyboard(n: i32) -> Keyboard {
    Keyboard {
        id: format!("kb{n}"),
        name: format!("Keystation {n}"),
        port_name: "MIDI 1".into(),
        client: 20 + n,
        port: 0,
    }
}

fn typed(m: &mut Model, on: bool) -> Vec<Effect> {
    m.handle(Input::Command(Command::TypedKeys { on }))
}

fn typed_id() -> Option<String> {
    Some(TYPED_KEYBOARD_ID.into())
}

#[test]
fn the_pc_keyboard_is_in_the_list_after_the_keyboards_only_while_on() {
    let mut m = playing();
    m.handle(Input::KeyboardPlayed { id: "kb1".into() });
    let before = m.view();
    assert!(!before.keyboards.typed_keys);
    assert_eq!(before.keyboards.active.as_deref(), Some("kb1"));

    assert_eq!(typed(&mut m, true), [], "not a step, no sound");
    let view = m.view();
    assert!(view.keyboards.typed_keys);
    let ids: Vec<_> = view.keyboards.list.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(ids, ["kb1", TYPED_KEYBOARD_ID]);
    let pc = &view.keyboards.list[1];
    assert_eq!(
        (pc.name.as_str(), pc.port_name.as_str(), pc.address.as_str()),
        ("", "", ""),
        "named by the screen"
    );
    assert_eq!(
        view.keyboards.active.as_deref(),
        Some("kb1"),
        "the one played last"
    );
    assert_eq!(view.song.undo_count, before.song.undo_count);
    assert_eq!(view.status.message, Some(StatusMessage::TypedKeysOn));

    // The service says when it was played; the MIDI input is still the keyboard's.
    m.handle(Input::KeyboardPlayed {
        id: TYPED_KEYBOARD_ID.into(),
    });
    assert!(m.typed_is_active());
    let view = m.view();
    assert_eq!(view.keyboards.active, typed_id());
    assert_eq!(view.audio.midi_input, None);
    m.handle(Input::KeyboardPlayed { id: "kb1".into() });
    assert!(!m.typed_is_active());
    m.handle(Input::KeyboardPlayed {
        id: TYPED_KEYBOARD_ID.into(),
    });

    // A key changes nothing in the rules, and leaves the notice.
    let notice = Some(StatusMessage::NothingToKeep);
    m.notice = notice.clone();
    let typed_key = Command::TypedKey {
        note: 60,
        down: true,
        t_us: 1,
    };
    assert_eq!(m.handle(Input::Command(typed_key)), []);
    assert_eq!(m.view().status.message, notice);
    m.notice = None;

    // Off: gone from the list, and the keyboard is the one shown again.
    assert_eq!(typed(&mut m, false), []);
    let view = m.view();
    assert!(!view.keyboards.typed_keys);
    assert_eq!(view.keyboards.list.len(), 1);
    assert_eq!(view.keyboards.active.as_deref(), Some("kb1"));
    assert_eq!(view.keyboards.lost, None);
    assert_ne!(view.status.message, Some(StatusMessage::TypedKeysOn));
    assert!(!m.typed_keys());
    // Played last while it was on: not any more.
    m.handle(Input::KeyboardPlayed {
        id: TYPED_KEYBOARD_ID.into(),
    });
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb1"));
}

#[test]
fn the_pc_keyboard_alone_is_a_keyboard_for_the_chooser_and_nothing_is_lost_when_it_goes() {
    let mut m = Model::new(Settings::default());
    assert_eq!(m.view().workspace.chooser, None);
    assert_eq!(
        m.view().status.message,
        Some(StatusMessage::SearchingKeyboard)
    );
    typed(&mut m, true);
    let view = m.view();
    assert!(view.workspace.chooser.is_some(), "a keyboard is there");
    assert_eq!(view.keyboards.active, typed_id(), "plugged in last");
    assert_eq!(
        view.status.message,
        Some(StatusMessage::KeyboardFound {
            keyboard: String::new()
        }),
        "named by the screen"
    );
    typed(&mut m, false);
    let view = m.view();
    assert_eq!(view.workspace.chooser, None);
    assert_eq!((view.keyboards.active, view.keyboards.lost), (None, None));
    assert_eq!(view.status.message, Some(StatusMessage::SearchingKeyboard));

    // Turned on again and again: one of it.
    typed(&mut m, true);
    typed(&mut m, true);
    m.handle(Input::KeyboardAdded(keyboard(2)));
    let ids: Vec<_> = (m.view().keyboards.list.iter())
        .map(|k| k.id.clone())
        .collect();
    assert_eq!(ids, ["kb2", TYPED_KEYBOARD_ID]);
    assert_eq!(
        m.view().workspace.chooser,
        Some(ChooserView { highlighted: None })
    );
}

#[test]
fn the_switch_is_taken_while_recording_and_the_recording_line_stays() {
    let mut m = playing();
    let take = m.handle(Input::Command(Command::Record));
    assert!(!take.is_empty());
    let before = m.view().status.message;
    assert!(
        matches!(
            before,
            Some(StatusMessage::CountingIn { .. } | StatusMessage::Recording { .. })
        ),
        "{before:?}"
    );
    typed(&mut m, true);
    assert!(m.view().keyboards.typed_keys);
    assert_eq!(m.view().status.message, before);
}
