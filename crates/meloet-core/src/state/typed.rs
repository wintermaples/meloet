//! R2 「キーで弾く」: while it is on, the PC keyboard is one more keyboard of the list, after the
//! ones plugged in (as if plugged in last). Its keys never come here: the service plays them
//! (`service/typed.rs`), and says `Input::KeyboardPlayed` when it becomes the one played last.

use meloet_midi::Keyboard;
use meloet_protocol::TYPED_KEYBOARD_ID;

use super::{KeyboardView, Model};

impl Model {
    /// Whether 「キーで弾く」 is on: the service plays `Command::TypedKey` only then.
    pub fn typed_keys(&self) -> bool {
        self.typed.is_some()
    }

    /// Whether the PC keyboard is the keyboard shown as played.
    pub fn typed_is_active(&self) -> bool {
        self.active_keyboard()
            .is_some_and(|k| k.id == TYPED_KEYBOARD_ID)
    }

    /// Not a step, and taken while recording too. Off forgets that it was played last.
    pub(super) fn set_typed_keys(&mut self, on: bool) {
        if on == self.typed.is_some() {
            return;
        }
        if on {
            self.typed = Some(Keyboard {
                id: TYPED_KEYBOARD_ID.into(),
                name: String::new(),
                port_name: String::new(),
                client: -1,
                port: -1,
            });
        } else {
            self.typed = None;
            if self.played.as_deref() == Some(TYPED_KEYBOARD_ID) {
                self.played = None;
            }
        }
    }

    /// The keyboards plugged in, oldest first, then the PC keyboard while it is on.
    pub(super) fn all_keyboards(&self) -> impl Iterator<Item = &Keyboard> {
        self.keyboards.iter().chain(&self.typed)
    }

    pub(super) fn keyboard_views(&self) -> Vec<KeyboardView> {
        let view = |k: &Keyboard| KeyboardView {
            id: k.id.clone(),
            name: k.name.clone(),
            port_name: k.port_name.clone(),
            address: k.address(),
        };
        // Its name is the screen's, and it has no ALSA address.
        let typed = self.typed.as_ref().map(|k| KeyboardView {
            address: String::new(),
            ..view(k)
        });
        self.keyboards.iter().map(view).chain(typed).collect()
    }
}
