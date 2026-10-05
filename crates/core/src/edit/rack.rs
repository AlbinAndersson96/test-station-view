use crate::edit::{ObjectId, Plan, Rejection};
use crate::ids::RackId;
use crate::limits::Limits;
use crate::model::{DEFAULT_RACK_HEIGHT_U, Document, Rack};
use crate::name::Name;

/// Appends a rack named after the first free `RackN`, as tall as the last rack (or 42U).
pub fn plan_add_rack(doc: &Document, limits: &Limits) -> Plan {
    let height_u = doc
        .racks
        .last()
        .map_or(DEFAULT_RACK_HEIGHT_U, |r| r.height_u);
    let name = (1u32..)
        .map(|n| Name::parse(&format!("Rack{n}"), limits).expect("generated rack names are valid"))
        .find(|candidate| !doc.racks.iter().any(|r| r.name.same_as(candidate)))
        .expect("an unused rack name always exists");
    let rack = Rack {
        id: RackId::new(),
        name,
        height_u,
        devices: Vec::new(),
    };
    let subject = Some(ObjectId::Rack(rack.id));
    let mut document = doc.clone();
    document.racks.push(rack);
    Plan { document, subject }
}

pub fn plan_remove_rack(doc: &Document, rack_id: RackId) -> Result<Plan, Rejection> {
    let index = doc.rack_index(rack_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    document.racks.remove(index);
    Ok(Plan {
        document,
        subject: None,
    })
}

/// Moves the rack to `new_index` in the row; indexes past the end mean "last".
pub fn plan_move_rack(
    doc: &Document,
    rack_id: RackId,
    new_index: usize,
) -> Result<Plan, Rejection> {
    let index = doc.rack_index(rack_id).ok_or(Rejection::NotFound)?;
    let mut document = doc.clone();
    let rack = document.racks.remove(index);
    let new_index = new_index.min(document.racks.len());
    document.racks.insert(new_index, rack);
    Ok(Plan {
        document,
        subject: Some(ObjectId::Rack(rack_id)),
    })
}

pub fn plan_rename_rack(doc: &Document, rack_id: RackId, name: Name) -> Result<Plan, Rejection> {
    let index = doc.rack_index(rack_id).ok_or(Rejection::NotFound)?;
    if doc
        .racks
        .iter()
        .any(|r| r.id != rack_id && r.name.same_as(&name))
    {
        return Err(Rejection::NameTaken(name.to_string()));
    }
    let mut document = doc.clone();
    document.racks[index].name = name;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Rack(rack_id)),
    })
}

pub fn plan_set_rack_height(
    doc: &Document,
    limits: &Limits,
    rack_id: RackId,
    height_u: u32,
) -> Result<Plan, Rejection> {
    let index = doc.rack_index(rack_id).ok_or(Rejection::NotFound)?;
    if height_u == 0 {
        return Err(Rejection::ZeroHeight);
    }
    if height_u > limits.max_rack_height_u {
        return Err(Rejection::HeightAboveLimit {
            max: limits.max_rack_height_u,
        });
    }
    if doc.racks[index]
        .devices
        .iter()
        .any(|d| d.top_u() > height_u)
    {
        return Err(Rejection::EquipmentOutside);
    }
    let mut document = doc.clone();
    document.racks[index].height_u = height_u;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Rack(rack_id)),
    })
}
