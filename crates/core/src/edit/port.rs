use crate::edit::cable::drop_dangling_cables;
use crate::edit::{ObjectId, Plan, Rejection};
use crate::ids::{DeviceId, PortId};
use crate::limits::Limits;
use crate::model::{Document, Gender, Port, PortKind};
use crate::name::Name;
use crate::port_grid::{self, Cell, GridSize, PushDir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSource {
    New {
        name: Name,
        kind: PortKind,
        gender: Gender,
    },
    Existing(PortId),
}

/// Places a new or existing port of `device_id` on `target`, pushing an occupant in `dir`.
/// New ports never auto-rename: a clashing name is rejected.
pub fn plan_port_drop(
    doc: &Document,
    limits: &Limits,
    device_id: DeviceId,
    source: &PortSource,
    target: Cell,
    dir: PushDir,
) -> Result<Plan, Rejection> {
    let (ri, di) = doc.device_location(device_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    let device = &mut document.racks[ri].devices[di];
    let grid = GridSize {
        rows: device.height_u * limits.port_rows_per_u,
        cols: limits.port_cols,
    };

    let mut port = match source {
        PortSource::New { name, kind, gender } => {
            if device.ports.iter().any(|p| p.name.same_as(name)) {
                return Err(Rejection::NameTaken(name.to_string()));
            }
            Port {
                id: PortId::new(),
                name: name.clone(),
                row: 0,
                col: 0,
                kind: *kind,
                gender: *gender,
            }
        }
        PortSource::Existing(port_id) => {
            let index = device
                .ports
                .iter()
                .position(|p| p.id == *port_id)
                .ok_or_else(|| {
                    if doc.port(*port_id).is_some() {
                        Rejection::WrongDevice
                    } else {
                        Rejection::NotFound
                    }
                })?;
            device.ports.remove(index)
        }
    };

    let occupants: Vec<(PortId, Cell)> = device
        .ports
        .iter()
        .map(|p| {
            (
                p.id,
                Cell {
                    row: p.row,
                    col: p.col,
                },
            )
        })
        .collect();
    let moved = port_grid::push_for_drop(&occupants, target, dir, grid)?;
    for p in &mut device.ports {
        if let Some((_, cell)) = moved.iter().find(|(id, _)| *id == p.id) {
            p.row = cell.row;
            p.col = cell.col;
        }
    }

    port.row = target.row;
    port.col = target.col;
    let subject = Some(ObjectId::Port(port.id));
    device.ports.push(port);
    Ok(Plan { document, subject })
}

pub fn plan_remove_port(doc: &Document, port_id: PortId) -> Result<Plan, Rejection> {
    let (ri, di, pi) = doc.port_location(port_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    document.racks[ri].devices[di].ports.remove(pi);
    drop_dangling_cables(&mut document);
    Ok(Plan {
        document,
        subject: None,
    })
}

pub fn plan_rename_port(doc: &Document, port_id: PortId, name: Name) -> Result<Plan, Rejection> {
    let (ri, di, pi) = doc.port_location(port_id).ok_or(Rejection::NotFound)?;
    let siblings = &doc.racks[ri].devices[di].ports;
    if siblings
        .iter()
        .any(|p| p.id != port_id && p.name.same_as(&name))
    {
        return Err(Rejection::NameTaken(name.to_string()));
    }
    let mut document = doc.clone();
    document.racks[ri].devices[di].ports[pi].name = name;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Port(port_id)),
    })
}

pub fn plan_set_port_kind(
    doc: &Document,
    port_id: PortId,
    kind: PortKind,
) -> Result<Plan, Rejection> {
    let (ri, di, pi) = doc.port_location(port_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    document.racks[ri].devices[di].ports[pi].kind = kind;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Port(port_id)),
    })
}

pub fn plan_set_port_gender(
    doc: &Document,
    port_id: PortId,
    gender: Gender,
) -> Result<Plan, Rejection> {
    let (ri, di, pi) = doc.port_location(port_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    document.racks[ri].devices[di].ports[pi].gender = gender;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Port(port_id)),
    })
}
