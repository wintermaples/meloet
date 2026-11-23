//! The controllers of a keyboard that are not played into the song: the fader (CC 7), All
//! Notes Off (CC 123) and Reset All Controllers (CC 121). None of them reaches an instrument or
//! a take. What each keyboard holds is kept here, on the MIDI thread, so that CC 123 and 121 let
//! go of that keyboard's keys only.

use meloet_midi::MidiEvent;

pub(super) const FADER: u8 = 7;
pub(super) const RESET_ALL_CONTROLLERS: u8 = 121;
pub(super) const ALL_NOTES_OFF: u8 = 123;

const SUSTAIN: u8 = 64;
const MODULATION: u8 = 1;

#[derive(Default)]
struct Holding {
    notes: u128,
    sustain: bool,
    modulation: bool,
    bend: bool,
}

/// What each keyboard holds down, by [`meloet_midi::Keyboard::id`].
#[derive(Default)]
pub(super) struct Hands {
    by_keyboard: Vec<(String, Holding)>,
}

impl Hands {
    pub(super) fn saw(&mut self, keyboard: &str, event: MidiEvent) {
        let h = match self.by_keyboard.iter().position(|(k, _)| k == keyboard) {
            Some(i) => &mut self.by_keyboard[i].1,
            None => {
                self.by_keyboard
                    .push((keyboard.to_owned(), Holding::default()));
                &mut self.by_keyboard.last_mut().expect("just pushed").1
            }
        };
        match event {
            MidiEvent::NoteOn { note, .. } => h.notes |= 1 << note.min(127),
            MidiEvent::NoteOff { note, .. } => h.notes &= !(1 << note.min(127)),
            MidiEvent::Control {
                controller: SUSTAIN,
                value,
            } => h.sustain = value >= 64,
            MidiEvent::Control {
                controller: MODULATION,
                value,
            } => h.modulation = value > 0,
            MidiEvent::Control { .. } => {}
            MidiEvent::PitchBend { value } => h.bend = value != 0,
        }
    }

    /// The events that let go of what `keyboard` holds, lowest key first: its keys, and with
    /// `reset` (CC 121) also the bend to the middle, the modulation to 0 and the sustain
    /// pedal up. What is at rest already gets nothing.
    pub(super) fn let_go(&mut self, keyboard: &str, reset: bool) -> Vec<MidiEvent> {
        let Some((_, h)) = self.by_keyboard.iter_mut().find(|(k, _)| k == keyboard) else {
            return Vec::new();
        };
        let mut out: Vec<MidiEvent> = (0..128u8)
            .filter(|n| h.notes & (1 << n) != 0)
            .map(|note| MidiEvent::NoteOff { note, velocity: 0 })
            .collect();
        h.notes = 0;
        if reset {
            if std::mem::take(&mut h.bend) {
                out.push(MidiEvent::PitchBend { value: 0 });
            }
            if std::mem::take(&mut h.modulation) {
                out.push(MidiEvent::Control {
                    controller: MODULATION,
                    value: 0,
                });
            }
            if std::mem::take(&mut h.sustain) {
                out.push(MidiEvent::Control {
                    controller: SUSTAIN,
                    value: 0,
                });
            }
        }
        out
    }

    pub(super) fn forget(&mut self, keyboard: &str) {
        self.by_keyboard.retain(|(k, _)| k != keyboard);
    }

    pub(super) fn clear(&mut self) {
        self.by_keyboard.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(note: u8) -> MidiEvent {
        MidiEvent::NoteOn { note, velocity: 80 }
    }

    fn off(note: u8) -> MidiEvent {
        MidiEvent::NoteOff { note, velocity: 0 }
    }

    #[test]
    fn all_notes_off_lets_go_of_this_keyboards_keys_only() {
        let mut hands = Hands::default();
        hands.saw("a", on(60));
        hands.saw("a", on(127));
        hands.saw("a", on(0));
        hands.saw("a", off(0));
        hands.saw("b", on(62));
        hands.saw(
            "a",
            MidiEvent::Control {
                controller: SUSTAIN,
                value: 127,
            },
        );
        assert_eq!(hands.let_go("a", false), [off(60), off(127)]);
        assert_eq!(hands.let_go("a", false), [], "nothing held any more");
        assert_eq!(hands.let_go("b", false), [off(62)]);
        assert_eq!(hands.let_go("nobody", true), []);
    }

    #[test]
    fn reset_all_controllers_also_puts_bend_modulation_and_sustain_at_rest() {
        let mut hands = Hands::default();
        hands.saw("a", on(64));
        hands.saw("a", MidiEvent::PitchBend { value: 3000 });
        hands.saw(
            "a",
            MidiEvent::Control {
                controller: MODULATION,
                value: 40,
            },
        );
        hands.saw(
            "a",
            MidiEvent::Control {
                controller: SUSTAIN,
                value: 127,
            },
        );
        assert_eq!(
            hands.let_go("a", true),
            [
                off(64),
                MidiEvent::PitchBend { value: 0 },
                MidiEvent::Control {
                    controller: MODULATION,
                    value: 0
                },
                MidiEvent::Control {
                    controller: SUSTAIN,
                    value: 0
                },
            ]
        );
        assert_eq!(hands.let_go("a", true), []);
        hands.saw("a", MidiEvent::PitchBend { value: 0 });
        assert_eq!(
            hands.let_go("a", true),
            [],
            "a bend in the middle is at rest"
        );
    }

    #[test]
    fn a_keyboard_pulled_out_is_forgotten() {
        let mut hands = Hands::default();
        hands.saw("a", on(60));
        hands.forget("a");
        assert_eq!(hands.let_go("a", false), []);
        hands.saw("b", on(61));
        hands.clear();
        assert_eq!(hands.let_go("b", false), []);
    }
}
