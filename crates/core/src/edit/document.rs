use crate::edit::Plan;
use crate::limits::Limits;
use crate::model::Document;
use crate::name::DocumentName;

pub fn plan_rename_document(doc: &Document, name: DocumentName) -> Plan {
    let mut document = doc.clone();
    document.name = name;
    Plan { document, subject: None }
}

pub fn plan_new_document(limits: &Limits) -> Plan {
    Plan { document: Document::new_default(limits), subject: None }
}
