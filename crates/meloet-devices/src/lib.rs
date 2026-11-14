//! Human names for the audio outputs and inputs (PipeWire device Routes), and the
//! PC-wide clock settings. This crate only reads; it never changes a Route or a setting.

pub mod facts;
pub mod route_pod;
mod watcher;

pub use watcher::{DeviceWatcher, DevicesError};

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

/// Order for the output chooser: plugged in first (headphones before the rest),
/// then unknown, then unplugged. Keeps the given order inside each group.
pub fn sort_for_choice(endpoints: &mut [Endpoint]) {
    endpoints.sort_by_key(|e| match e.availability {
        Availability::Yes => (0, e.friendly != Some(Friendly::Headphones)),
        Availability::Unknown => (1, false),
        Availability::No => (2, false),
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
            active: false,
            node_name: None,
            device_name: None,
        }
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
