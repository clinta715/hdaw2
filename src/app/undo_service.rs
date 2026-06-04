use crate::app::undo::{UndoCommand, UndoStack};

/// Service wrapping the undo/redo stack.
pub struct UndoService {
    pub stack: UndoStack,
}

impl UndoService {
    pub fn new() -> Self {
        Self {
            stack: UndoStack::new(),
        }
    }

    pub fn push(&mut self, cmd: UndoCommand) {
        self.stack.push(cmd);
    }

    pub fn can_undo(&self) -> bool {
        self.stack.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.stack.can_redo()
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn undo_index(&self) -> usize {
        self.stack.saved_index()
    }
}

impl Default for UndoService {
    fn default() -> Self {
        Self::new()
    }
}
