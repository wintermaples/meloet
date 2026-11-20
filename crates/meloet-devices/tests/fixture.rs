//! Builds the picture from a trimmed `pw-dump` of the development PC, without touching PipeWire.
//! Each Route goes JSON -> pod bytes -> `route_from_pod_bytes`, the same reader the watcher uses.

use std::io::Cursor;

use meloet_devices::{
    Availability, ClockSettings, DevicesPicture, Endpoint, Friendly, LinkKind,
    facts::{DeviceFacts, NodeClass, NodeFacts, RouteParam, build_picture},
    route_pod::route_from_pod_bytes,
    sort_for_choice,
};
use pipewire::spa::{
    pod::{Object, Property, Value, ValueArray, serialize::PodSerializer},
    sys,
    utils::Id,
};
use serde_json::Value as Json;

fn int(json: &Json) -> i32 {
    json.as_i64().unwrap() as i32
}

fn int_array(json: &Json) -> Value {
    Value::ValueArray(ValueArray::Int(
        json.as_array().unwrap().iter().map(int).collect(),
    ))
}

fn route_pod(route: &Json) -> Vec<u8> {
    let mut properties = Vec::new();
    for (key, value) in route.as_object().unwrap() {
        let (key, value) = match key.as_str() {
            "index" => (sys::SPA_PARAM_ROUTE_index, Value::Int(int(value))),
            "direction" => (
                sys::SPA_PARAM_ROUTE_direction,
                Value::Id(Id(match value.as_str().unwrap() {
                    "Input" => sys::SPA_DIRECTION_INPUT,
                    "Output" => sys::SPA_DIRECTION_OUTPUT,
                    other => panic!("direction {other}"),
                })),
            ),
            "name" => (
                sys::SPA_PARAM_ROUTE_name,
                Value::String(value.as_str().unwrap().into()),
            ),
            "description" => (
                sys::SPA_PARAM_ROUTE_description,
                Value::String(value.as_str().unwrap().into()),
            ),
            "priority" => (sys::SPA_PARAM_ROUTE_priority, Value::Int(int(value))),
            "available" => (
                sys::SPA_PARAM_ROUTE_available,
                Value::Id(Id(match value.as_str().unwrap() {
                    "unknown" => sys::SPA_PARAM_AVAILABILITY_unknown,
                    "no" => sys::SPA_PARAM_AVAILABILITY_no,
                    "yes" => sys::SPA_PARAM_AVAILABILITY_yes,
                    other => panic!("available {other}"),
                })),
            ),
            "info" => (
                sys::SPA_PARAM_ROUTE_info,
                Value::Struct(
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| match v {
                            Json::Number(_) => Value::Int(int(v)),
                            other => Value::String(other.as_str().unwrap().into()),
                        })
                        .collect(),
                ),
            ),
            "profiles" => (sys::SPA_PARAM_ROUTE_profiles, int_array(value)),
            "devices" => (sys::SPA_PARAM_ROUTE_devices, int_array(value)),
            "device" => (sys::SPA_PARAM_ROUTE_device, Value::Int(int(value))),
            "profile" => (sys::SPA_PARAM_ROUTE_profile, Value::Int(int(value))),
            "save" => (
                sys::SPA_PARAM_ROUTE_save,
                Value::Bool(value.as_bool().unwrap()),
            ),
            other => panic!("route key {other}"),
        };
        properties.push(Property::new(key, value));
    }
    let object = Value::Object(Object {
        type_: sys::SPA_TYPE_OBJECT_ParamRoute,
        id: sys::SPA_PARAM_EnumRoute,
        properties,
    });
    PodSerializer::serialize(Cursor::new(Vec::new()), &object)
        .unwrap()
        .0
        .into_inner()
}

fn routes(params: &Json, key: &str) -> Vec<RouteParam> {
    params[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| route_from_pod_bytes(&route_pod(r)).expect("a Route the reader understands"))
        .collect()
}

fn picture_from_fixture() -> DevicesPicture {
    let dump: Json = serde_json::from_str(include_str!("fixtures/this-pc.json")).unwrap();
    let mut devices = Vec::new();
    let mut nodes = Vec::new();
    let mut clock = ClockSettings::default();

    for object in dump.as_array().unwrap() {
        let id = object["id"].as_u64().unwrap() as u32;
        let props = &object["info"]["props"];
        let text = |key: &str| props[key].as_str().map(str::to_owned);
        let number = |key: &str| props[key].as_u64().map(|n| n as u32);
        match object["type"].as_str().unwrap() {
            "PipeWire:Interface:Device" => devices.push(DeviceFacts {
                id,
                name: text("device.name").unwrap(),
                description: text("device.description"),
                bus: text("device.bus"),
                api: text("device.api"),
                enum_routes: routes(&object["info"]["params"], "EnumRoute"),
                active_routes: routes(&object["info"]["params"], "Route"),
            }),
            "PipeWire:Interface:Node" => nodes.push(NodeFacts {
                id,
                class: NodeClass::from_media_class(&text("media.class").unwrap()).unwrap(),
                name: text("node.name").unwrap(),
                description: text("node.description"),
                device_id: number("device.id"),
                profile_device: number("card.profile.device"),
                channels: number("audio.channels").map(|n| n as u8),
            }),
            "PipeWire:Interface:Metadata" => {
                for entry in object["metadata"].as_array().unwrap() {
                    let value = match &entry["value"] {
                        Json::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    clock.apply(entry["key"].as_str(), Some(&value));
                }
            }
            other => panic!("fixture type {other}"),
        }
    }
    build_picture(&devices, &nodes, clock)
}

const ANALOG: &str = "alsa_card.pci-0000_0c_00.6";
const RYZEN: &str = "Ryzen HD Audio Controller";

fn by_id<'a>(list: &'a [Endpoint], id: &str) -> &'a Endpoint {
    list.iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("no endpoint {id}"))
}

#[test]
fn analog_card_gives_two_outputs_on_one_sink() {
    let picture = picture_from_fixture();
    let sink = Some("alsa_output.pci-0000_0c_00.6.analog-stereo".to_owned());

    assert_eq!(
        by_id(
            &picture.outputs,
            &format!("{ANALOG}:analog-output-headphones")
        ),
        &Endpoint {
            id: format!("{ANALOG}:analog-output-headphones"),
            name: "アナログヘッドフォン".into(),
            friendly: Some(Friendly::Headphones),
            availability: Availability::Yes,
            link: LinkKind::Jack,
            active: true,
            node_name: sink.clone(),
            device_name: Some(ANALOG.into()),
            device_description: Some(RYZEN.into()),
            channels: Some(2),
        }
    );
    assert_eq!(
        by_id(&picture.outputs, &format!("{ANALOG}:analog-output-lineout")),
        &Endpoint {
            id: format!("{ANALOG}:analog-output-lineout"),
            name: "ライン出力".into(),
            friendly: Some(Friendly::LineOut),
            availability: Availability::Yes,
            link: LinkKind::Jack,
            active: false,
            node_name: sink,
            device_name: Some(ANALOG.into()),
            device_description: Some(RYZEN.into()),
            channels: Some(2),
        }
    );
}

#[test]
fn hdmi_routes_without_a_sink_are_listed_as_unplugged() {
    let picture = picture_from_fixture();
    let radeon = "alsa_card.pci-0000_0c_00.1";
    assert_eq!(
        by_id(&picture.outputs, &format!("{radeon}:hdmi-output-1")),
        &Endpoint {
            id: format!("{radeon}:hdmi-output-1"),
            name: "HDMI / DisplayPort 2".into(),
            friendly: Some(Friendly::Hdmi),
            availability: Availability::No,
            link: LinkKind::Plug,
            active: false,
            node_name: None,
            device_name: Some(radeon.into()),
            device_description: Some("Radeon High Definition Audio Controller".into()),
            channels: None,
        }
    );

    let names: Vec<&str> = picture.outputs.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "HDMI / DisplayPort",
            "HDMI / DisplayPort 2",
            "HDMI / DisplayPort 3",
            "HDMI / DisplayPort 4",
            "HDMI / DisplayPort",
            "HDMI / DisplayPort 2",
            "HDMI / DisplayPort 3",
            "アナログヘッドフォン",
            "ライン出力",
        ]
    );
    let mut ids: Vec<&str> = picture.outputs.iter().map(|e| e.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), picture.outputs.len(), "ids are unique");
}

#[test]
fn inputs_come_from_the_source_and_its_input_routes() {
    let picture = picture_from_fixture();
    let source = Some("alsa_input.pci-0000_0c_00.6.analog-stereo".to_owned());
    let seen: Vec<_> = picture
        .inputs
        .iter()
        .map(|e| {
            (
                e.name.as_str(),
                e.friendly,
                e.availability,
                e.link,
                e.active,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                "フロントマイクロフォン",
                None,
                Availability::Yes,
                LinkKind::Jack,
                false
            ),
            (
                "リアマイクロフォン",
                Some(Friendly::RearMic),
                Availability::Yes,
                LinkKind::Jack,
                true
            ),
            ("ラインイン", None, Availability::No, LinkKind::Jack, false),
        ]
    );
    assert!(picture.inputs.iter().all(|e| e.node_name == source));
    assert_eq!(
        picture.inputs[1].id,
        format!("{ANALOG}:analog-input-rear-mic")
    );
}

#[test]
fn clock_comes_from_the_settings_metadata() {
    assert_eq!(
        picture_from_fixture().clock,
        ClockSettings {
            rate: Some(48000),
            quantum: Some(256),
            force_quantum: Some(256),
            force_rate: Some(48000),
        }
    );
}

#[test]
fn chooser_order_puts_headphones_on_top() {
    let mut outputs = picture_from_fixture().outputs;
    sort_for_choice(&mut outputs);
    let names: Vec<&str> = outputs.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        &names[..3],
        [
            "アナログヘッドフォン",
            "HDMI / DisplayPort",
            "HDMI / DisplayPort 2"
        ]
    );
    assert_eq!(names[8], "ライン出力");

    use meloet_devices::Unusable::*;
    let seen: Vec<_> = outputs
        .iter()
        .map(|e| (e.unusable(), e.device_description.as_deref().unwrap()))
        .collect();
    let gb206 = "GB206 High Definition Audio Controller";
    let radeon = "Radeon High Definition Audio Controller";
    assert_eq!(
        seen,
        [
            (None, RYZEN),
            (Some(StoppedByPc), gb206),
            (Some(StoppedByPc), gb206),
            (Some(StoppedByPc), gb206),
            (Some(StoppedByPc), gb206),
            (Some(StoppedByPc), radeon),
            (Some(StoppedByPc), radeon),
            (Some(StoppedByPc), radeon),
            (Some(SwitchedByPc), RYZEN),
        ]
    );
}

#[test]
fn bytes_that_are_not_a_route_are_refused() {
    assert_eq!(route_from_pod_bytes(&[]), None);
    let not_a_route = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Int(3))
        .unwrap()
        .0
        .into_inner();
    assert_eq!(route_from_pod_bytes(&not_a_route), None);
}
