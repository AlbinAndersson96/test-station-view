use crate::edit::{ObjectId, Plan, Rejection};
use crate::ids::{CableId, PortId};
use crate::limits::Limits;
use crate::model::{Cable, Document, Rgb};
use crate::name::Name;

/// Connects ports `a` and `b` with a new cable named after the first free `CableN`.
pub fn plan_connect(
    doc: &Document,
    limits: &Limits,
    a: PortId,
    b: PortId,
) -> Result<Plan, Rejection> {
    let (_, _, port_a) = doc.port(a).ok_or(Rejection::NotFound)?;
    let (_, _, port_b) = doc.port(b).ok_or(Rejection::NotFound)?;
    if a == b {
        return Err(Rejection::SamePort);
    }
    for port in [port_a, port_b] {
        if doc.cable_at_port(port.id).is_some() {
            return Err(Rejection::PortInUse(port.name.to_string()));
        }
    }
    let name = (1u32..)
        .map(|n| {
            Name::parse(&format!("Cable{n}"), limits).expect("generated cable names are valid")
        })
        .find(|candidate| !doc.cables.iter().any(|c| c.name.same_as(candidate)))
        .expect("an unused cable name always exists");
    let cable = Cable {
        id: CableId::new(),
        name,
        color: Rgb::CABLE_BLUE,
        a,
        b,
    };
    let subject = Some(ObjectId::Cable(cable.id));
    let mut document = doc.clone();
    document.cables.push(cable);
    Ok(Plan { document, subject })
}

pub fn plan_remove_cable(doc: &Document, cable_id: CableId) -> Result<Plan, Rejection> {
    let index = cable_index(doc, cable_id)?;
    let mut document = doc.clone();
    document.cables.remove(index);
    Ok(Plan {
        document,
        subject: None,
    })
}

pub fn plan_rename_cable(doc: &Document, cable_id: CableId, name: Name) -> Result<Plan, Rejection> {
    let index = cable_index(doc, cable_id)?;
    if doc
        .cables
        .iter()
        .any(|c| c.id != cable_id && c.name.same_as(&name))
    {
        return Err(Rejection::NameTaken(name.to_string()));
    }
    let mut document = doc.clone();
    document.cables[index].name = name;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Cable(cable_id)),
    })
}

pub fn plan_set_cable_color(
    doc: &Document,
    cable_id: CableId,
    color: Rgb,
) -> Result<Plan, Rejection> {
    let index = cable_index(doc, cable_id)?;
    let mut document = doc.clone();
    document.cables[index].color = color;
    Ok(Plan {
        document,
        subject: Some(ObjectId::Cable(cable_id)),
    })
}

fn cable_index(doc: &Document, cable_id: CableId) -> Result<usize, Rejection> {
    doc.cables
        .iter()
        .position(|c| c.id == cable_id)
        .ok_or(Rejection::NotFound)
}

/// Drops every cable with an end on a port that no longer exists (after a removal).
pub(crate) fn drop_dangling_cables(document: &mut Document) {
    let mut cables = std::mem::take(&mut document.cables);
    cables.retain(|c| c.ends().iter().all(|&p| document.port(p).is_some()));
    document.cables = cables;
}
