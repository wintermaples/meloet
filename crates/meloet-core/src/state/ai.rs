//! The switch 「AI とつなぐ」 (spec 3 「AI link」, A-27) and what the screen shows of the socket.
//! The socket itself is not here yet: for now the port is `Waiting` while the switch is on.

use meloet_protocol::{AiLinkView, AiPort};

#[derive(Debug, Default)]
pub(super) struct AiSide {
    /// The switch, kept in the settings.
    pub(super) on: bool,
}

impl AiSide {
    pub(super) fn new(on: bool) -> AiSide {
        AiSide { on }
    }

    /// Returns whether it changed (the settings are then saved).
    pub(super) fn set(&mut self, on: bool) -> bool {
        let changed = self.on != on;
        self.on = on;
        changed
    }

    /// Where the socket's thread is to report what the port is doing ([`super::Input::AiPort`]).
    /// Nothing is kept yet: the view follows the switch alone.
    pub(super) fn port_said(&mut self, _port: AiPort) {}

    pub(super) fn view(&self) -> AiLinkView {
        AiLinkView {
            on: self.on,
            port: if self.on {
                AiPort::Waiting
            } else {
                AiPort::Closed
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use meloet_protocol::{AiLinkView, AiPort, Command};

    use crate::settings::Settings;
    use crate::state::{Effect, Input, Model};

    fn saved(effects: &[Effect]) -> Vec<bool> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::SaveSettings(s) => Some(s.ai_link),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_switch_is_a_setting_and_no_step() {
        let m = &mut Model::new(Settings::default());
        let off = AiLinkView {
            on: false,
            port: AiPort::Closed,
        };
        assert_eq!(m.view().ai_link, off);
        let steps = m.view().song.undo_count;

        let effects = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert_eq!(saved(&effects), [true]);
        assert_eq!(
            m.view().ai_link,
            AiLinkView {
                on: true,
                port: AiPort::Waiting
            }
        );
        assert_eq!(m.view().song.undo_count, steps);
        let again = m.handle(Input::Command(Command::SetAiLink { on: true }));
        assert!(saved(&again).is_empty(), "the same again saves nothing");

        // Nothing keeps what the port says yet.
        m.handle(Input::AiPort(AiPort::Connected));
        assert_eq!(m.view().ai_link.port, AiPort::Waiting);

        let effects = m.handle(Input::Command(Command::SetAiLink { on: false }));
        assert_eq!(saved(&effects), [false]);
        assert_eq!(m.view().ai_link, off);
    }

    #[test]
    fn the_switch_is_read_at_the_start() {
        let m = Model::new(Settings {
            ai_link: true,
            ..Settings::default()
        });
        assert_eq!(
            m.view().ai_link,
            AiLinkView {
                on: true,
                port: AiPort::Waiting
            }
        );
    }
}
