mod commands;

use crate::project::automation::AutomationPoint;
use crate::project::clip::AudioClip;
use crate::project::track::SerializedEffect;

pub use commands::{apply_undo, apply_redo};

const MAX_UNDO: usize = 128;

pub enum UndoCommand {
    MoveClip {
        track_index: usize,
        clip_id: uuid::Uuid,
        old_position: u64,
        new_position: u64,
    },
    TrimClip {
        track_index: usize,
        clip_id: uuid::Uuid,
        old_offset: u64,
        old_length: u64,
        new_offset: u64,
        new_length: u64,
    },
    DeleteClip {
        track_index: usize,
        clip_index: usize,
        clip: AudioClip,
    },
    AddEffect {
        track_index: usize,
        effect_index: usize,
        serialized: SerializedEffect,
    },
    RemoveEffect {
        track_index: usize,
        effect_index: usize,
        serialized: SerializedEffect,
    },
    ToggleMute {
        track_index: usize,
        old_value: bool,
    },
    ToggleSolo {
        track_index: usize,
        old_value: bool,
    },
    AutomationAddPoint {
        track_index: usize,
        lane_index: usize,
        point: AutomationPoint,
    },
    AutomationRemovePoint {
        track_index: usize,
        lane_index: usize,
        point_index: usize,
        point: AutomationPoint,
    },
    AutomationMovePoint {
        track_index: usize,
        lane_index: usize,
        point_index: usize,
        old_value: f32,
        new_value: f32,
    },
}

pub struct UndoStack {
    stack: Vec<UndoCommand>,
    index: usize,
}

impl UndoStack {
    pub fn new() -> Self { Self { stack: Vec::new(), index: 0 } }

    pub fn push(&mut self, cmd: UndoCommand) {
        self.stack.truncate(self.index);
        self.stack.push(cmd);
        if self.stack.len() > MAX_UNDO {
            self.stack.remove(0);
        }
        self.index = self.stack.len();
    }

    pub fn undo(&mut self) -> Option<&UndoCommand> {
        if self.index == 0 { return None; }
        self.index -= 1;
        Some(&self.stack[self.index])
    }

    pub fn redo(&mut self) -> Option<&UndoCommand> {
        if self.index >= self.stack.len() { return None; }
        let cmd = &self.stack[self.index];
        self.index += 1;
        Some(cmd)
    }

    pub fn clear(&mut self) {
        self.stack.clear();
        self.index = 0;
    }

    pub fn can_undo(&self) -> bool { self.index > 0 }
    pub fn can_redo(&self) -> bool { self.index < self.stack.len() }
}
