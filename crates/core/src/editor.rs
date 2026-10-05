use std::mem;

use crate::edit::Plan;
use crate::model::Document;

/// The current document plus whole-document undo/redo snapshots (session only).
#[derive(Debug, Clone)]
pub struct Editor {
    document: Document,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl Editor {
    pub fn new(document: Document) -> Editor {
        Editor { document, undo: Vec::new(), redo: Vec::new() }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn commit(&mut self, plan: Plan) {
        let previous = mem::replace(&mut self.document, plan.document);
        self.undo.push(previous);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(previous) => {
                let current = mem::replace(&mut self.document, previous);
                self.redo.push(current);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                let current = mem::replace(&mut self.document, next);
                self.undo.push(current);
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn replace_clearing_history(&mut self, document: Document) {
        self.document = document;
        self.undo.clear();
        self.redo.clear();
    }
}
