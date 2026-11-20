//! What was read from PipeWire, as plain data, and the pure step that turns it into a picture.

use crate::{Availability, ClockSettings, DevicesPicture, Endpoint, Friendly, LinkKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

/// One `EnumRoute` or `Route` param of a device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteParam {
    pub index: u32,
    pub direction: Direction,
    pub name: String,
    pub description: String,
    pub priority: i32,
    pub available: Availability,
    /// `port.type` from the Route's info.
    pub port_type: Option<String>,
    /// The `card.profile.device` numbers this Route can serve.
    pub devices: Vec<u32>,
    /// Only on an active `Route`: the `card.profile.device` it is active on.
    pub device: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceFacts {
    pub id: u32,
    pub name: String,
    /// `device.description`
    pub description: Option<String>,
    pub bus: Option<String>,
    pub api: Option<String>,
    pub enum_routes: Vec<RouteParam>,
    pub active_routes: Vec<RouteParam>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeClass {
    Sink,
    Source,
}

impl NodeClass {
    pub fn from_media_class(media_class: &str) -> Option<Self> {
        match media_class {
            "Audio/Sink" => Some(Self::Sink),
            "Audio/Source" => Some(Self::Source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeFacts {
    pub id: u32,
    pub class: NodeClass,
    pub name: String,
    pub description: Option<String>,
    /// `device.id`
    pub device_id: Option<u32>,
    /// `card.profile.device`
    pub profile_device: Option<u32>,
    /// `audio.channels`
    pub channels: Option<u8>,
}

/// The friendly wording, only when the type is certain.
pub fn friendly_for(
    direction: Direction,
    port_type: Option<&str>,
    route_name: &str,
) -> Option<Friendly> {
    let name = route_name.to_ascii_lowercase();
    match direction {
        Direction::Output => {
            if port_type == Some("headphones") || name.contains("headphones") {
                Some(Friendly::Headphones)
            } else if port_type == Some("hdmi")
                || name.contains("hdmi")
                || name.contains("displayport")
            {
                Some(Friendly::Hdmi)
            } else if port_type == Some("line") || name.contains("lineout") {
                Some(Friendly::LineOut)
            } else {
                None
            }
        }
        Direction::Input => name.contains("rear-mic").then_some(Friendly::RearMic),
    }
}

/// Jack for an analog hole on a built-in card; Plug for everything else.
pub fn link_for(bus: Option<&str>, api: Option<&str>, port_type: Option<&str>) -> LinkKind {
    let plugged_bus = matches!(bus, Some("usb" | "bluetooth")) || api == Some("bluez5");
    let analog_hole = matches!(
        port_type,
        Some("headphones" | "line" | "mic" | "headset" | "aux")
    );
    if analog_hole && !plugged_bus {
        LinkKind::Jack
    } else {
        LinkKind::Plug
    }
}

pub fn build_picture(
    devices: &[DeviceFacts],
    nodes: &[NodeFacts],
    clock: ClockSettings,
) -> DevicesPicture {
    DevicesPicture {
        outputs: endpoints(devices, nodes, Direction::Output),
        inputs: endpoints(devices, nodes, Direction::Input),
        clock,
    }
}

fn endpoints(devices: &[DeviceFacts], nodes: &[NodeFacts], direction: Direction) -> Vec<Endpoint> {
    let class = match direction {
        Direction::Output => NodeClass::Sink,
        Direction::Input => NodeClass::Source,
    };
    let nodes: Vec<&NodeFacts> = nodes
        .iter()
        .filter(|n| n.class == class && !n.name.ends_with(".monitor"))
        .collect();
    let mut used = vec![false; nodes.len()];
    let mut out = Vec::new();

    for dev in devices {
        let mut routes: Vec<&RouteParam> = dev
            .enum_routes
            .iter()
            .filter(|r| r.direction == direction)
            .collect();
        routes.sort_by_key(|r| std::cmp::Reverse(r.priority));

        for route in routes {
            let endpoint = |node: Option<&NodeFacts>| Endpoint {
                id: format!("{}:{}", dev.name, route.name),
                name: route.description.clone(),
                friendly: friendly_for(direction, route.port_type.as_deref(), &route.name),
                availability: route.available,
                link: link_for(
                    dev.bus.as_deref(),
                    dev.api.as_deref(),
                    route.port_type.as_deref(),
                ),
                active: dev.active_routes.iter().any(|a| {
                    a.direction == direction
                        && a.index == route.index
                        && match (node.and_then(|n| n.profile_device), a.device) {
                            (Some(wanted), Some(on)) => wanted == on,
                            _ => true,
                        }
                }),
                node_name: node.map(|n| n.name.clone()),
                device_name: Some(dev.name.clone()),
                device_description: dev.description.clone(),
                channels: node.and_then(|n| n.channels),
            };

            let mut matched = false;
            for (i, node) in nodes.iter().enumerate() {
                let serves = node.device_id == Some(dev.id)
                    && node
                        .profile_device
                        .is_some_and(|d| route.devices.contains(&d));
                if serves {
                    used[i] = true;
                    matched = true;
                    out.push(endpoint(Some(node)));
                }
            }
            if !matched {
                out.push(endpoint(None));
            }
        }
    }

    for (node, _) in nodes.iter().zip(&used).filter(|(_, used)| !**used) {
        let dev = devices.iter().find(|d| Some(d.id) == node.device_id);
        out.push(Endpoint {
            id: node.name.clone(),
            name: node
                .description
                .clone()
                .unwrap_or_else(|| node.name.clone()),
            friendly: None,
            availability: Availability::Unknown,
            link: LinkKind::Plug,
            // No Route to be on or off: the node itself is the endpoint.
            active: true,
            node_name: Some(node.name.clone()),
            device_name: dev.map(|d| d.name.clone()),
            device_description: dev.and_then(|d| d.description.clone()),
            channels: node.channels,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_only_when_the_type_is_known() {
        use Direction::*;
        let f = friendly_for;
        assert_eq!(
            f(Output, Some("headphones"), "analog-output-headphones"),
            Some(Friendly::Headphones)
        );
        assert_eq!(
            f(Output, None, "analog-output-headphones"),
            Some(Friendly::Headphones)
        );
        assert_eq!(
            f(Output, Some("hdmi"), "hdmi-output-1"),
            Some(Friendly::Hdmi)
        );
        assert_eq!(f(Output, None, "displayport-output"), Some(Friendly::Hdmi));
        assert_eq!(
            f(Output, Some("line"), "analog-output-lineout"),
            Some(Friendly::LineOut)
        );
        assert_eq!(
            f(Input, Some("mic"), "analog-input-rear-mic"),
            Some(Friendly::RearMic)
        );

        assert_eq!(f(Output, Some("speaker"), "analog-output-speaker"), None);
        assert_eq!(f(Output, None, "iec958-stereo-output"), None);
        assert_eq!(f(Input, Some("mic"), "analog-input-front-mic"), None);
        assert_eq!(f(Input, Some("line"), "analog-input-linein"), None);
        assert_eq!(f(Input, Some("headphones"), "odd"), None);
    }

    #[test]
    fn link_is_jack_only_for_analog_holes_on_a_built_in_card() {
        assert_eq!(
            link_for(Some("pci"), Some("alsa"), Some("headphones")),
            LinkKind::Jack
        );
        assert_eq!(
            link_for(Some("pci"), Some("alsa"), Some("mic")),
            LinkKind::Jack
        );
        assert_eq!(
            link_for(Some("pci"), Some("alsa"), Some("hdmi")),
            LinkKind::Plug
        );
        assert_eq!(
            link_for(Some("usb"), Some("alsa"), Some("headphones")),
            LinkKind::Plug
        );
        assert_eq!(
            link_for(None, Some("bluez5"), Some("headset")),
            LinkKind::Plug
        );
        assert_eq!(
            link_for(Some("pci"), Some("alsa"), Some("speaker")),
            LinkKind::Plug
        );
        assert_eq!(link_for(None, None, None), LinkKind::Plug);
    }

    fn node(id: u32, class: NodeClass, name: &str, device_id: Option<u32>) -> NodeFacts {
        NodeFacts {
            id,
            class,
            name: name.into(),
            description: Some(format!("{name} (desc)")),
            device_id,
            profile_device: None,
            channels: Some(2),
        }
    }

    #[test]
    fn sink_without_a_route_becomes_one_endpoint_named_by_its_description() {
        let devices = [DeviceFacts {
            id: 70,
            name: "bluez_card.00_11".into(),
            description: Some("Some Headset".into()),
            api: Some("bluez5".into()),
            ..Default::default()
        }];
        let nodes = [
            node(71, NodeClass::Sink, "bluez_output.00_11.1", Some(70)),
            node(72, NodeClass::Sink, "null-sink", None),
            node(73, NodeClass::Source, "null-sink.monitor", None),
        ];
        let p = build_picture(&devices, &nodes, ClockSettings::default());
        assert_eq!(p.inputs, []);
        assert_eq!(p.outputs.len(), 2);
        assert_eq!(
            p.outputs[0],
            Endpoint {
                id: "bluez_output.00_11.1".into(),
                name: "bluez_output.00_11.1 (desc)".into(),
                friendly: None,
                availability: Availability::Unknown,
                link: LinkKind::Plug,
                active: true,
                node_name: Some("bluez_output.00_11.1".into()),
                device_name: Some("bluez_card.00_11".into()),
                device_description: Some("Some Headset".into()),
                channels: Some(2),
            }
        );
        assert_eq!(p.outputs[1].id, "null-sink");
        assert_eq!(p.outputs[1].device_name, None);
        assert_eq!(p.outputs[1].device_description, None);
    }
}
