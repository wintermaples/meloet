//! Human names for the audio outputs and inputs (PipeWire device Routes), and the
//! PC-wide clock settings. This crate only reads; it never changes a Route or a setting.

pub mod facts;
pub mod route_pod;
mod watcher;

pub use watcher::{DeviceWatcher, DevicesError, DevicesEvent};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DevicesPicture {
    pub outputs: Vec<Endpoint>,
    pub inputs: Vec<Endpoint>,
    pub clock: ClockSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// `<device.name>:<route name>`, or the node name when there is no Route. Stable across replugging.
    pub id: String,
    pub name: String,
    pub friendly: Option<Friendly>,
    pub availability: Availability,
    pub link: LinkKind,
    pub active: bool,
    pub node_name: Option<String>,
    pub device_name: Option<String>,
    /// `device.description`, the name the PC shows for the device.
    pub device_description: Option<String>,
    /// How many channels the node has (`audio.channels`). `None`: no node, or it does not say.
    pub channels: Option<u8>,
}

/// Why meloet cannot play through an endpoint without changing a PC-wide setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    /// It has a node, but the PC has that device on another Route now.
    SwitchedByPc,
    /// It has no node (its profile is off).
    StoppedByPc,
}

impl Endpoint {
    /// An endpoint without a Route counts as on.
    pub fn unusable(&self) -> Option<Unusable> {
        match (&self.node_name, self.active) {
            (None, _) => Some(Unusable::StoppedByPc),
            (Some(_), false) => Some(Unusable::SwitchedByPc),
            (Some(_), true) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Friendly {
    Headphones,
    Hdmi,
    LineOut,
    RearMic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    /// An analog hole.
    Jack,
    /// HDMI, USB, Bluetooth and everything else.
    Plug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClockSettings {
    pub rate: Option<u32>,
    pub quantum: Option<u32>,
    /// `clock.force-quantum`; 0 (not forced) is `None`.
    pub force_quantum: Option<u32>,
    /// `clock.force-rate`; 0 (not forced) is `None`.
    pub force_rate: Option<u32>,
}

impl ClockSettings {
    /// Applies one property event of the `settings` metadata.
    /// `key == None` means everything was removed, `value == None` that this key was removed.
    pub fn apply(&mut self, key: Option<&str>, value: Option<&str>) {
        let Some(key) = key else {
            *self = Self::default();
            return;
        };
        let number = value.and_then(|v| v.trim().parse::<u32>().ok());
        let forced = number.filter(|n| *n != 0);
        match key {
            "clock.rate" => self.rate = number,
            "clock.quantum" => self.quantum = number,
            "clock.force-quantum" => self.force_quantum = forced,
            "clock.force-rate" => self.force_rate = forced,
            _ => {}
        }
    }
}

/// Order for the output chooser: the usable ones first, and among them plugged in first
/// (headphones before the rest), then unknown, then unplugged. Keeps the given order inside
/// each group, and among the unusable ones.
pub fn sort_for_choice(endpoints: &mut [Endpoint]) {
    endpoints.sort_by_key(|e| match (e.unusable(), e.availability) {
        (Some(_), _) => (3, false),
        (None, Availability::Yes) => (0, e.friendly != Some(Friendly::Headphones)),
        (None, Availability::Unknown) => (1, false),
        (None, Availability::No) => (2, false),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: &str, friendly: Option<Friendly>, availability: Availability) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: id.into(),
            friendly,
            availability,
            link: LinkKind::Plug,
            active: true,
            node_name: Some("node".into()),
            device_name: None,
            device_description: None,
            channels: None,
        }
    }

    #[test]
    fn unusable_tells_switched_from_stopped() {
        let usable = ep("a", None, Availability::Yes);
        assert_eq!(usable.unusable(), None);
        let switched = Endpoint {
            active: false,
            ..usable.clone()
        };
        assert_eq!(switched.unusable(), Some(Unusable::SwitchedByPc));
        for active in [false, true] {
            let stopped = Endpoint {
                active,
                node_name: None,
                ..usable.clone()
            };
            assert_eq!(stopped.unusable(), Some(Unusable::StoppedByPc));
        }
    }

    #[test]
    fn sort_puts_unusable_after_every_usable_one() {
        use Availability::*;
        let switched = |id: &str, av| Endpoint {
            active: false,
            ..ep(id, Some(Friendly::Headphones), av)
        };
        let stopped = |id: &str, av| Endpoint {
            node_name: None,
            ..ep(id, None, av)
        };
        let mut v = vec![
            stopped("hdmi-yes", Yes),
            ep("usb-no", None, No),
            switched("hp-switched", Yes),
            ep("usb-unknown", None, Unknown),
            stopped("hdmi-no", No),
            ep("line-yes", None, Yes),
        ];
        sort_for_choice(&mut v);
        let ids: Vec<&str> = v.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "line-yes",
                "usb-unknown",
                "usb-no",
                "hdmi-yes",
                "hp-switched",
                "hdmi-no"
            ]
        );
    }

    #[test]
    fn sort_puts_plugged_first_and_headphones_on_top() {
        use Availability::*;
        let mut v = vec![
            ep("z-no", None, No),
            ep("hp-no", Some(Friendly::Headphones), No),
            ep("b-unknown", None, Unknown),
            ep("z-yes", Some(Friendly::LineOut), Yes),
            ep("a-yes", None, Yes),
            ep("hp-yes", Some(Friendly::Headphones), Yes),
            ep("a-unknown", None, Unknown),
            ep("a-no", None, No),
        ];
        sort_for_choice(&mut v);
        let ids: Vec<&str> = v.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "hp-yes",
                "z-yes",
                "a-yes",
                "b-unknown",
                "a-unknown",
                "z-no",
                "hp-no",
                "a-no"
            ]
        );
    }

    #[test]
    fn clock_reads_numbers_and_treats_zero_force_as_none() {
        let mut c = ClockSettings::default();
        c.apply(Some("clock.rate"), Some("48000"));
        c.apply(Some("clock.quantum"), Some("256"));
        c.apply(Some("clock.force-quantum"), Some("1024"));
        c.apply(Some("clock.force-rate"), Some("0"));
        c.apply(Some("clock.allowed-rates"), Some("[ 48000 ]"));
        c.apply(Some("log.level"), Some("2"));
        assert_eq!(
            c,
            ClockSettings {
                rate: Some(48000),
                quantum: Some(256),
                force_quantum: Some(1024),
                force_rate: None
            }
        );
        c.apply(Some("clock.force-quantum"), Some("0"));
        assert_eq!(c.force_quantum, None);
        c.apply(Some("clock.quantum"), None);
        assert_eq!(c.quantum, None);
        c.apply(None, None);
        assert_eq!(c, ClockSettings::default());
    }
}
