//! Full invariant check for documents that did not come from the editor (imports, storage).
//! All arithmetic is in u64 so absurd values cannot overflow.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::limits::Limits;
use crate::model::{Device, Document, Rack};
use crate::name::Name;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error("The ID {0} is used more than once")]
    DuplicateId(String),
    #[error("The rack name '{0}' is used more than once")]
    DuplicateRackName(String),
    #[error("Rack '{rack}': the height must be at least 1U")]
    RackZeroHeight { rack: String },
    #[error("Rack '{rack}': the device name '{name}' is used more than once")]
    DuplicateDeviceName { rack: String, name: String },
    #[error("Rack '{rack}': device '{device}' must be at least 1U high")]
    DeviceZeroHeight { rack: String, device: String },
    #[error("Rack '{rack}': device '{device}' is outside the rack")]
    DeviceOutsideRack { rack: String, device: String },
    #[error("Rack '{rack}': devices '{a}' and '{b}' overlap at U{u}")]
    DevicesOverlap { rack: String, a: String, b: String, u: u32 },
    #[error("Device '{device}': the port name '{name}' is used more than once")]
    DuplicatePortName { device: String, name: String },
    #[error("Device '{device}': port '{port}' is outside the device face")]
    PortOutsideGrid { device: String, port: String },
    #[error("Device '{device}': ports '{a}' and '{b}' share a cell")]
    PortsOverlap { device: String, a: String, b: String },
}

pub fn validate(doc: &Document, limits: &Limits) -> Result<(), ValidationError> {
    check_unique_ids(doc)?;
    if let Some(name) = first_duplicate(doc.racks.iter().map(|r| &r.name)) {
        return Err(ValidationError::DuplicateRackName(name.to_string()));
    }
    for rack in &doc.racks {
        check_rack(rack, limits)?;
    }
    Ok(())
}

fn check_unique_ids(doc: &Document) -> Result<(), ValidationError> {
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut insert = |id: Uuid| {
        if seen.insert(id) { Ok(()) } else { Err(ValidationError::DuplicateId(id.to_string())) }
    };
    for rack in &doc.racks {
        insert(rack.id.0)?;
        for device in &rack.devices {
            insert(device.id.0)?;
            for port in &device.ports {
                insert(port.id.0)?;
            }
        }
    }
    Ok(())
}

/// The first name whose case-insensitive form was already seen.
fn first_duplicate<'a>(mut names: impl Iterator<Item = &'a Name>) -> Option<&'a Name> {
    let mut seen = HashSet::new();
    names.find(|n| !seen.insert(n.key()))
}

fn top(device: &Device) -> u64 {
    u64::from(device.bottom_u) + u64::from(device.height_u) - 1
}

fn check_rack(rack: &Rack, limits: &Limits) -> Result<(), ValidationError> {
    let rack_name = rack.name.to_string();
    if rack.height_u == 0 {
        return Err(ValidationError::RackZeroHeight { rack: rack_name });
    }
    if let Some(name) = first_duplicate(rack.devices.iter().map(|d| &d.name)) {
        return Err(ValidationError::DuplicateDeviceName { rack: rack_name, name: name.to_string() });
    }
    for device in &rack.devices {
        if device.height_u == 0 {
            return Err(ValidationError::DeviceZeroHeight {
                rack: rack_name,
                device: device.name.to_string(),
            });
        }
        if device.bottom_u < 1 || top(device) > u64::from(rack.height_u) {
            return Err(ValidationError::DeviceOutsideRack {
                rack: rack_name,
                device: device.name.to_string(),
            });
        }
        check_ports(device, limits)?;
    }
    let mut sorted: Vec<&Device> = rack.devices.iter().collect();
    sorted.sort_by_key(|d| d.bottom_u);
    for pair in sorted.windows(2) {
        let (lower, upper) = (pair[0], pair[1]);
        if u64::from(upper.bottom_u) <= top(lower) {
            return Err(ValidationError::DevicesOverlap {
                rack: rack_name,
                a: lower.name.to_string(),
                b: upper.name.to_string(),
                u: upper.bottom_u,
            });
        }
    }
    Ok(())
}

fn check_ports(device: &Device, limits: &Limits) -> Result<(), ValidationError> {
    let device_name = device.name.to_string();
    if let Some(name) = first_duplicate(device.ports.iter().map(|p| &p.name)) {
        return Err(ValidationError::DuplicatePortName { device: device_name, name: name.to_string() });
    }
    let rows = u64::from(device.height_u) * u64::from(limits.port_rows_per_u);
    for port in &device.ports {
        if u64::from(port.row) >= rows || port.col >= limits.port_cols {
            return Err(ValidationError::PortOutsideGrid {
                device: device_name,
                port: port.name.to_string(),
            });
        }
    }
    let mut cells = HashMap::new();
    for port in &device.ports {
        if let Some(other) = cells.insert((port.row, port.col), &port.name) {
            return Err(ValidationError::PortsOverlap {
                device: device_name,
                a: other.to_string(),
                b: port.name.to_string(),
            });
        }
    }
    Ok(())
}
