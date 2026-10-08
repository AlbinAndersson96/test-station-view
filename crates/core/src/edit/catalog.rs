use crate::edit::{ObjectId, Plan, Rejection};
use crate::ids::{DeviceId, ModelId};
use crate::limits::Limits;
use crate::model::{CatalogEntry, Device, DeviceKind, Document, ModelPort, display_name};
use crate::name::{ModelText, Name};

/// Makes a catalogue entry from the device's height, colour and ports, and links the device
/// to it.
pub fn plan_save_model(
    doc: &Document,
    device_id: DeviceId,
    manufacturer: ModelText,
    model: ModelText,
) -> Result<Plan, Rejection> {
    let (ri, di) = doc.device_location(device_id).ok_or(Rejection::NotFound)?;
    check_identity(doc, None, &manufacturer, &model)?;
    let device = &doc.racks[ri].devices[di];
    let entry = CatalogEntry {
        id: ModelId::new(),
        manufacturer,
        model,
        height_u: device.height_u,
        color: device.color,
        ports: model_ports(device),
    };
    let mut document = doc.clone();
    document.racks[ri].devices[di].kind = DeviceKind::Model(entry.id);
    document.catalog.push(entry);
    Ok(Plan {
        document,
        subject: Some(ObjectId::Device(device_id)),
    })
}

/// Replaces the linked entry's height, colour and ports with the device's. Other devices
/// placed from the entry are not changed.
pub fn plan_update_model(doc: &Document, device_id: DeviceId) -> Result<Plan, Rejection> {
    let (_, device) = doc.device(device_id).ok_or(Rejection::NotFound)?;
    let DeviceKind::Model(model_id) = device.kind else {
        return Err(Rejection::NotLinked);
    };
    let index = model_index(doc, model_id)?;
    let mut document = doc.clone();
    let entry = &mut document.catalog[index];
    entry.height_u = device.height_u;
    entry.color = device.color;
    entry.ports = model_ports(device);
    Ok(Plan {
        document,
        subject: Some(ObjectId::Device(device_id)),
    })
}

pub fn plan_rename_model(
    doc: &Document,
    model_id: ModelId,
    manufacturer: ModelText,
    model: ModelText,
) -> Result<Plan, Rejection> {
    let index = model_index(doc, model_id)?;
    check_identity(doc, Some(model_id), &manufacturer, &model)?;
    let mut document = doc.clone();
    let entry = &mut document.catalog[index];
    entry.manufacturer = manufacturer;
    entry.model = model;
    Ok(Plan {
        document,
        subject: None,
    })
}

/// Removes the entry; devices placed from it become ad hoc.
pub fn plan_remove_model(doc: &Document, model_id: ModelId) -> Result<Plan, Rejection> {
    let index = model_index(doc, model_id)?;
    let mut document = doc.clone();
    document.catalog.remove(index);
    for device in document.racks.iter_mut().flat_map(|r| &mut r.devices) {
        if device.kind == DeviceKind::Model(model_id) {
            device.kind = DeviceKind::AdHoc;
        }
    }
    Ok(Plan {
        document,
        subject: None,
    })
}

/// A new device with the entry's contents (ports get new IDs), linked to it, at U1 (the drop
/// places it). Its name is the model text without whitespace, cut to fit.
pub(crate) fn device_from_model(entry: &CatalogEntry, limits: &Limits) -> Device {
    let compact: String = entry
        .model
        .as_str()
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(limits.name_max_len)
        .collect();
    let name = Name::parse(&compact, limits)
        .or_else(|_| Name::parse("Device", limits))
        .expect("'Device' is a valid name");
    Device {
        id: DeviceId::new(),
        name,
        bottom_u: 1,
        height_u: entry.height_u,
        color: entry.color,
        kind: DeviceKind::Model(entry.id),
        ports: entry
            .ports
            .iter()
            .map(|p| crate::model::Port {
                id: crate::ids::PortId::new(),
                name: p.name.clone(),
                row: p.row,
                col: p.col,
                kind: p.kind,
                gender: p.gender,
            })
            .collect(),
    }
}

fn model_ports(device: &Device) -> Vec<ModelPort> {
    device
        .ports
        .iter()
        .map(|p| ModelPort {
            name: p.name.clone(),
            row: p.row,
            col: p.col,
            kind: p.kind,
            gender: p.gender,
        })
        .collect()
}

fn model_index(doc: &Document, model_id: ModelId) -> Result<usize, Rejection> {
    doc.catalog
        .iter()
        .position(|e| e.id == model_id)
        .ok_or(Rejection::NotFound)
}

/// The model must not be empty, and manufacturer + model must be unused (except by `except`).
fn check_identity(
    doc: &Document,
    except: Option<ModelId>,
    manufacturer: &ModelText,
    model: &ModelText,
) -> Result<(), Rejection> {
    if model.is_empty() {
        return Err(Rejection::ModelRequired);
    }
    let name = display_name(manufacturer, model);
    let key = name.to_lowercase();
    if doc
        .catalog
        .iter()
        .any(|e| Some(e.id) != except && e.key() == key)
    {
        return Err(Rejection::ModelTaken(name));
    }
    Ok(())
}
