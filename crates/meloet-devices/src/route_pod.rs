//! Reads one `SPA_TYPE_OBJECT_ParamRoute` pod (an `EnumRoute` or `Route` param).

use pipewire::spa::{
    pod::{Value, ValueArray, deserialize::PodDeserializer},
    sys,
};

use crate::{
    Availability,
    facts::{Direction, RouteParam},
};

/// `None` when the bytes are not a Route object or a needed field is missing.
pub fn route_from_pod_bytes(bytes: &[u8]) -> Option<RouteParam> {
    let (_, value) = PodDeserializer::deserialize_any_from(bytes).ok()?;
    route_from_value(&value)
}

pub fn route_from_value(value: &Value) -> Option<RouteParam> {
    let Value::Object(object) = value else {
        return None;
    };
    if object.type_ != sys::SPA_TYPE_OBJECT_ParamRoute {
        return None;
    }

    let mut index = None;
    let mut direction = None;
    let mut name = None;
    let mut description = None;
    let mut priority = 0;
    let mut available = Availability::Unknown;
    let mut port_type = None;
    let mut devices = Vec::new();
    let mut device = None;

    for property in &object.properties {
        match (property.key, &property.value) {
            (sys::SPA_PARAM_ROUTE_index, Value::Int(v)) => index = u32::try_from(*v).ok(),
            (sys::SPA_PARAM_ROUTE_direction, Value::Id(id)) => {
                direction = match id.0 {
                    sys::SPA_DIRECTION_INPUT => Some(Direction::Input),
                    sys::SPA_DIRECTION_OUTPUT => Some(Direction::Output),
                    _ => None,
                }
            }
            (sys::SPA_PARAM_ROUTE_name, Value::String(v)) => name = Some(v.clone()),
            (sys::SPA_PARAM_ROUTE_description, Value::String(v)) => description = Some(v.clone()),
            (sys::SPA_PARAM_ROUTE_priority, Value::Int(v)) => priority = *v,
            (sys::SPA_PARAM_ROUTE_available, Value::Id(id)) => {
                available = match id.0 {
                    sys::SPA_PARAM_AVAILABILITY_yes => Availability::Yes,
                    sys::SPA_PARAM_AVAILABILITY_no => Availability::No,
                    _ => Availability::Unknown,
                }
            }
            (sys::SPA_PARAM_ROUTE_info, Value::Struct(fields)) => {
                port_type = info_value(fields, "port.type")
            }
            (sys::SPA_PARAM_ROUTE_devices, Value::ValueArray(ValueArray::Int(v))) => {
                devices = v.iter().filter_map(|d| u32::try_from(*d).ok()).collect()
            }
            (sys::SPA_PARAM_ROUTE_device, Value::Int(v)) => device = u32::try_from(*v).ok(),
            _ => {}
        }
    }

    let name = name?;
    Some(RouteParam {
        index: index?,
        direction: direction?,
        description: description.unwrap_or_else(|| name.clone()),
        name,
        priority,
        available,
        port_type,
        devices,
        device,
    })
}

/// The info is a struct: the pair count, then key, value, key, value, ...
fn info_value(fields: &[Value], wanted: &str) -> Option<String> {
    let pairs = fields.get(1..)?;
    pairs.as_chunks::<2>().0.iter().find_map(|pair| match pair {
        [Value::String(key), Value::String(value)] if key == wanted => Some(value.clone()),
        _ => None,
    })
}
