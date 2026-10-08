//! Versioned JSON file format (spec §4.1, §5). The `File*` types are the on-disk shape and
//! are kept separate from the in-memory model so the model can change without breaking files.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::ids::{CableId, DeviceId, PortId, RackId};
use crate::limits::Limits;
use crate::model::{Cable, Device, DeviceKind, Document, Port, PortKind, Rack, Rgb};
use crate::name::{DocumentName, Name};
use crate::validate::{ValidationError, validate};

pub const FORMAT_VERSION: u64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    #[error("The file is not valid JSON: {0}")]
    NotJson(String),
    #[error("The file has no format version.")]
    MissingVersion,
    #[error("This file was made by a newer version of the app.")]
    TooNew { found: u64 },
    #[error("The file is malformed: {0}")]
    Malformed(String),
    #[error("Invalid name '{name}': {reason}")]
    InvalidName { name: String, reason: String },
    #[error("Invalid colour '{0}'")]
    InvalidColor(String),
    #[error(transparent)]
    Invalid(#[from] ValidationError),
}

#[derive(Serialize, Deserialize)]
struct FileDocument {
    format_version: u64,
    name: String,
    racks: Vec<FileRack>,
    cables: Vec<FileCable>,
}

#[derive(Serialize, Deserialize)]
struct FileRack {
    id: Uuid,
    name: String,
    height_u: u32,
    devices: Vec<FileDevice>,
}

#[derive(Serialize, Deserialize)]
struct FileDevice {
    id: Uuid,
    name: String,
    bottom_u: u32,
    height_u: u32,
    color: String,
    #[serde(default)]
    kind: FileDeviceKind,
    ports: Vec<FilePort>,
}

#[derive(Serialize, Deserialize)]
struct FilePort {
    id: Uuid,
    name: String,
    row: u32,
    col: u32,
    #[serde(default)]
    kind: FilePortKind,
}

#[derive(Serialize, Deserialize)]
struct FileCable {
    id: Uuid,
    name: String,
    color: String,
    a: Uuid,
    b: Uuid,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum FileDeviceKind {
    #[default]
    AdHoc,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum FilePortKind {
    #[default]
    Unspecified,
}

pub fn to_json(doc: &Document) -> String {
    let file = FileDocument {
        format_version: FORMAT_VERSION,
        name: doc.name.as_str().to_owned(),
        racks: doc.racks.iter().map(rack_to_file).collect(),
        cables: doc.cables.iter().map(cable_to_file).collect(),
    };
    serde_json::to_string_pretty(&file).expect("serializing plain data cannot fail")
}

pub fn from_json(text: &str, limits: &Limits) -> Result<Document, LoadError> {
    let value: Value = serde_json::from_str(text).map_err(|e| LoadError::NotJson(e.to_string()))?;
    let version = value
        .get("format_version")
        .and_then(Value::as_u64)
        .ok_or(LoadError::MissingVersion)?;
    if version > FORMAT_VERSION {
        return Err(LoadError::TooNew { found: version });
    }
    let value = migrate(value, version)?;
    let file: FileDocument =
        serde_json::from_value(value).map_err(|e| LoadError::Malformed(e.to_string()))?;
    let doc = document_from_file(file, limits)?;
    validate(&doc, limits)?;
    Ok(doc)
}

/// Upgrades older format versions step by step to `FORMAT_VERSION`.
fn migrate(mut value: Value, version: u64) -> Result<Value, LoadError> {
    match version {
        // Version 2 added cables.
        1 => {
            if let Some(object) = value.as_object_mut() {
                object.insert("cables".into(), Value::Array(Vec::new()));
            }
            migrate(value, 2)
        }
        2 => Ok(value),
        other => Err(LoadError::Malformed(format!(
            "unsupported format version {other}"
        ))),
    }
}

fn rack_to_file(rack: &Rack) -> FileRack {
    FileRack {
        id: rack.id.0,
        name: rack.name.as_str().to_owned(),
        height_u: rack.height_u,
        devices: rack.devices.iter().map(device_to_file).collect(),
    }
}

fn device_to_file(device: &Device) -> FileDevice {
    FileDevice {
        id: device.id.0,
        name: device.name.as_str().to_owned(),
        bottom_u: device.bottom_u,
        height_u: device.height_u,
        color: device.color.to_hex(),
        kind: match device.kind {
            DeviceKind::AdHoc => FileDeviceKind::AdHoc,
        },
        ports: device.ports.iter().map(port_to_file).collect(),
    }
}

fn port_to_file(port: &Port) -> FilePort {
    FilePort {
        id: port.id.0,
        name: port.name.as_str().to_owned(),
        row: port.row,
        col: port.col,
        kind: match port.kind {
            PortKind::Unspecified => FilePortKind::Unspecified,
        },
    }
}

fn cable_to_file(cable: &Cable) -> FileCable {
    FileCable {
        id: cable.id.0,
        name: cable.name.as_str().to_owned(),
        color: cable.color.to_hex(),
        a: cable.a.0,
        b: cable.b.0,
    }
}

fn document_from_file(file: FileDocument, limits: &Limits) -> Result<Document, LoadError> {
    let name = DocumentName::parse(&file.name).map_err(|e| LoadError::InvalidName {
        name: file.name.clone(),
        reason: e.to_string(),
    })?;
    if name.as_str() != file.name {
        return Err(LoadError::InvalidName {
            name: file.name,
            reason: "has leading or trailing whitespace".into(),
        });
    }
    let racks = file
        .racks
        .into_iter()
        .map(|r| rack_from_file(r, limits))
        .collect::<Result<Vec<_>, _>>()?;
    let cables = file
        .cables
        .into_iter()
        .map(|c| cable_from_file(c, limits))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Document {
        name,
        racks,
        cables,
    })
}

fn cable_from_file(cable: FileCable, limits: &Limits) -> Result<Cable, LoadError> {
    Ok(Cable {
        id: CableId(cable.id),
        name: strict_name(&cable.name, limits)?,
        color: Rgb::from_hex(&cable.color).ok_or(LoadError::InvalidColor(cable.color))?,
        a: PortId(cable.a),
        b: PortId(cable.b),
    })
}

fn rack_from_file(rack: FileRack, limits: &Limits) -> Result<Rack, LoadError> {
    Ok(Rack {
        id: RackId(rack.id),
        name: strict_name(&rack.name, limits)?,
        height_u: rack.height_u,
        devices: rack
            .devices
            .into_iter()
            .map(|d| device_from_file(d, limits))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn device_from_file(device: FileDevice, limits: &Limits) -> Result<Device, LoadError> {
    Ok(Device {
        id: DeviceId(device.id),
        name: strict_name(&device.name, limits)?,
        bottom_u: device.bottom_u,
        height_u: device.height_u,
        color: Rgb::from_hex(&device.color).ok_or(LoadError::InvalidColor(device.color))?,
        kind: match device.kind {
            FileDeviceKind::AdHoc => DeviceKind::AdHoc,
        },
        ports: device
            .ports
            .into_iter()
            .map(|p| port_from_file(p, limits))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn port_from_file(port: FilePort, limits: &Limits) -> Result<Port, LoadError> {
    Ok(Port {
        id: PortId(port.id),
        name: strict_name(&port.name, limits)?,
        row: port.row,
        col: port.col,
        kind: match port.kind {
            FilePortKind::Unspecified => PortKind::Unspecified,
        },
    })
}

/// Like `Name::parse`, but rejects names that would need normalising: files are never
/// silently corrected.
fn strict_name(raw: &str, limits: &Limits) -> Result<Name, LoadError> {
    let name = Name::parse(raw, limits).map_err(|e| LoadError::InvalidName {
        name: raw.to_owned(),
        reason: e.to_string(),
    })?;
    if name.as_str() != raw {
        return Err(LoadError::InvalidName {
            name: raw.to_owned(),
            reason: "contains whitespace".into(),
        });
    }
    Ok(name)
}
