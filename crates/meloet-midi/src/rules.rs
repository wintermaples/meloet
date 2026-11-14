//! The parts that need no device: which port counts as a keyboard, how ALSA events map to
//! [`MidiEvent`], how ids are made.

use alsa::seq::{EvCtrl, EvNote, Event, EventType, PortCap, PortType};

use crate::{MidiEvent, MidiOptions, VKEYS_CLIENT_NAME};

pub(crate) struct PortFacts {
    pub port: i32,
    pub cap: PortCap,
    pub kind: PortType,
}

/// The one port of a client to listen to, if the client is a keyboard.
///
/// Only the lowest readable port: the second port of some keyboards (Keystation 49 MK3
/// "Transport") carries buttons that look like notes.
pub(crate) fn keyboard_port(
    client_name: &str,
    ports: &[PortFacts],
    options: MidiOptions,
) -> Option<i32> {
    let is_virtual = options.accept_virtual && client_name.starts_with(VKEYS_CLIENT_NAME);
    ports
        .iter()
        .filter(|p| p.cap.contains(PortCap::READ | PortCap::SUBS_READ))
        .filter(|p| is_virtual || p.kind.contains(PortType::HARDWARE))
        .map(|p| p.port)
        .min()
}

pub(crate) fn keyboard_id<'a>(
    client_name: &str,
    port_name: &str,
    mut taken: impl FnMut(&str) -> bool + 'a,
) -> String {
    let base = format!("{client_name}:{port_name}");
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}#{n}"))
        .find(|id| !taken(id))
        .expect("an endless range")
}

pub(crate) fn convert(event: &Event<'_>) -> Option<MidiEvent> {
    match event.get_type() {
        EventType::Noteon => {
            let n: EvNote = event.get_data()?;
            Some(note(true, n.note, n.velocity))
        }
        EventType::Noteoff => {
            let n: EvNote = event.get_data()?;
            Some(note(false, n.note, n.velocity))
        }
        EventType::Controller => {
            let c: EvCtrl = event.get_data()?;
            let controller = u8::try_from(c.param).ok().filter(|p| *p <= 127)?;
            Some(MidiEvent::Control {
                controller,
                value: c.value.clamp(0, 127) as u8,
            })
        }
        EventType::Pitchbend => {
            let c: EvCtrl = event.get_data()?;
            Some(MidiEvent::PitchBend {
                value: c.value.clamp(-8192, 8191) as i16,
            })
        }
        _ => None,
    }
}

fn note(on: bool, note: u8, velocity: u8) -> MidiEvent {
    let (note, velocity) = (note.min(127), velocity.min(127));
    // A note-on with velocity 0 is how many keyboards say "released".
    if on && velocity > 0 {
        MidiEvent::NoteOn { note, velocity }
    } else {
        MidiEvent::NoteOff { note, velocity }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const READ: PortCap = PortCap::READ.union(PortCap::SUBS_READ);
    const WRITE: PortCap = PortCap::WRITE.union(PortCap::SUBS_WRITE);
    const HW: PortType = PortType::MIDI_GENERIC
        .union(PortType::HARDWARE)
        .union(PortType::PORT);
    const APP: PortType = PortType::MIDI_GENERIC.union(PortType::APPLICATION);
    const NO: MidiOptions = MidiOptions {
        accept_virtual: false,
    };
    const YES: MidiOptions = MidiOptions {
        accept_virtual: true,
    };

    fn port(port: i32, cap: PortCap, kind: PortType) -> PortFacts {
        PortFacts { port, cap, kind }
    }

    #[test]
    fn hardware_keyboard_gives_only_its_lowest_readable_port() {
        let ports = [port(1, READ, HW), port(0, READ | WRITE, HW)];
        assert_eq!(keyboard_port("Keystation 49 MK3", &ports, NO), Some(0));
    }

    #[test]
    fn write_only_ports_are_skipped() {
        let ports = [port(0, WRITE, HW), port(1, READ, HW), port(2, READ, HW)];
        assert_eq!(keyboard_port("Synth", &ports, NO), Some(1));
        assert_eq!(keyboard_port("Synth", &[port(0, WRITE, HW)], NO), None);
    }

    #[test]
    fn readable_without_subscription_right_is_not_a_keyboard() {
        assert_eq!(
            keyboard_port("Odd", &[port(0, PortCap::READ, HW)], NO),
            None
        );
    }

    #[test]
    fn software_clients_are_not_keyboards() {
        assert_eq!(
            keyboard_port("PipeWire-System", &[port(0, READ, APP)], NO),
            None
        );
        assert_eq!(
            keyboard_port(
                "Midi Through",
                &[port(
                    0,
                    READ | WRITE,
                    PortType::MIDI_GENERIC | PortType::SOFTWARE | PortType::PORT
                )],
                YES
            ),
            None
        );
        assert_eq!(keyboard_port("No ports", &[], YES), None);
    }

    #[test]
    fn virtual_keyboard_counts_only_when_asked() {
        let ports = [port(0, READ, APP)];
        assert_eq!(keyboard_port("meloet-vkeys", &ports, NO), None);
        assert_eq!(keyboard_port("meloet-vkeys", &ports, YES), Some(0));
        assert_eq!(keyboard_port("meloet-vkeys-test-1", &ports, YES), Some(0));
        assert_eq!(keyboard_port("other-vkeys", &ports, YES), None);
    }

    #[test]
    fn id_comes_from_names_and_stays_unique() {
        assert_eq!(
            keyboard_id("Keystation 49 MK3", "MIDI 1", |_| false),
            "Keystation 49 MK3:MIDI 1"
        );
        let taken = ["K:P".to_string(), "K:P#2".to_string()];
        assert_eq!(
            keyboard_id("K", "P", |id| taken.iter().any(|t| t == id)),
            "K:P#3"
        );
    }

    fn note_event(kind: EventType, note: u8, velocity: u8) -> Event<'static> {
        Event::new(
            kind,
            &EvNote {
                channel: 3,
                note,
                velocity,
                off_velocity: 0,
                duration: 0,
            },
        )
    }

    fn ctrl_event(kind: EventType, param: u32, value: i32) -> Event<'static> {
        Event::new(
            kind,
            &EvCtrl {
                channel: 0,
                param,
                value,
            },
        )
    }

    #[test]
    fn notes_convert_and_velocity_zero_is_note_off() {
        assert_eq!(
            convert(&note_event(EventType::Noteon, 60, 90)),
            Some(MidiEvent::NoteOn {
                note: 60,
                velocity: 90
            })
        );
        assert_eq!(
            convert(&note_event(EventType::Noteon, 60, 0)),
            Some(MidiEvent::NoteOff {
                note: 60,
                velocity: 0
            })
        );
        assert_eq!(
            convert(&note_event(EventType::Noteoff, 61, 40)),
            Some(MidiEvent::NoteOff {
                note: 61,
                velocity: 40
            })
        );
    }

    #[test]
    fn controllers_and_bend_convert_and_stay_in_range() {
        assert_eq!(
            convert(&ctrl_event(EventType::Controller, 64, 127)),
            Some(MidiEvent::Control {
                controller: 64,
                value: 127
            })
        );
        assert_eq!(
            convert(&ctrl_event(EventType::Controller, 1, 300)),
            Some(MidiEvent::Control {
                controller: 1,
                value: 127
            })
        );
        assert_eq!(convert(&ctrl_event(EventType::Controller, 200, 1)), None);
        assert_eq!(
            convert(&ctrl_event(EventType::Pitchbend, 0, -8192)),
            Some(MidiEvent::PitchBend { value: -8192 })
        );
        assert_eq!(
            convert(&ctrl_event(EventType::Pitchbend, 0, 8191)),
            Some(MidiEvent::PitchBend { value: 8191 })
        );
        assert_eq!(
            convert(&ctrl_event(EventType::Pitchbend, 0, 9000)),
            Some(MidiEvent::PitchBend { value: 8191 })
        );
    }

    #[test]
    fn other_events_are_ignored() {
        assert_eq!(convert(&ctrl_event(EventType::Pgmchange, 0, 5)), None);
        assert_eq!(convert(&note_event(EventType::Keypress, 60, 10)), None);
    }
}
