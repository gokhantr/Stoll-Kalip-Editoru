use std::collections::VecDeque;

use crate::model::document::FormDocument;

/// Snapshot-based Undo/Redo stack for `FormDocument`.
/// Captures lightweight clones of `(header, elements)` with a configurable depth limit.
#[derive(Debug, Clone)]
pub struct UndoStack {
    undo: VecDeque<(String, FormDocument)>,
    redo: Vec<(String, FormDocument)>,
    max_depth: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new(100)
    }
}

impl UndoStack {
    pub fn new(max_depth: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            max_depth: max_depth.max(1),
        }
    }

    /// Push a pre-edit snapshot onto the undo stack and clear redo history.
    pub fn push(&mut self, label: impl Into<String>, snapshot: &FormDocument) {
        if self.undo.len() >= self.max_depth {
            self.undo.pop_front();
        }
        self.undo.push_back((label.into(), snapshot.clone()));
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|(l, _)| l.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|(l, _)| l.as_str())
    }

    /// Undo the last change, restoring `current` and moving its prior state to redo.
    pub fn undo(&mut self, current: &mut FormDocument) -> bool {
        if let Some((label, mut prev)) = self.undo.pop_back() {
            let cur_clone = current.clone();
            self.redo.push((label, cur_clone));
            prev.dirty = true;
            *current = prev;
            true
        } else {
            false
        }
    }

    /// Redo the last undone change.
    pub fn redo(&mut self, current: &mut FormDocument) -> bool {
        if let Some((label, mut next)) = self.redo.pop() {
            let cur_clone = current.clone();
            self.undo.push_back((label, cur_clone));
            next.dirty = true;
            *current = next;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
