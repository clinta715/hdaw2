mod commands;

use crate::app::TrackUiState;
use crate::project::automation::{AutomationLane, AutomationPoint};
use crate::project::clip::ClipKind;
use crate::project::midi_note::MidiNote;
use crate::project::track::{SerializedEffect, Track};

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
        clip: ClipKind,
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
        removed_lanes: Vec<AutomationLane>,
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
    AddMidiNote {
        track_index: usize,
        clip_id: uuid::Uuid,
        note: MidiNote,
    },
    RemoveMidiNote {
        track_index: usize,
        clip_id: uuid::Uuid,
        note: MidiNote,
        note_index: usize,
    },
    FadeClip {
        track_index: usize,
        clip_id: uuid::Uuid,
        old_fade_in: u64,
        old_fade_out: u64,
        new_fade_in: u64,
        new_fade_out: u64,
    },
    AddMidiClip {
        track_index: usize,
        clip: ClipKind,
    },
    ImportAudio {
        tracks: Vec<ImportTrackSnapshot>,
    },
    ImportMidi {
        tracks: Vec<ImportTrackSnapshot>,
    },
    RecordAudio {
        track_indices: Vec<usize>,
        clip_ids: Vec<uuid::Uuid>,
    },
    AddTrack {
        track_index: usize,
        track: Track,
        track_ui: TrackUiState,
    },
    DeleteTrack {
        track_index: usize,
        track: Track,
        track_ui: TrackUiState,
    },
}

#[derive(Clone)]
pub struct ImportTrackSnapshot {
    pub track: Track,
    pub track_ui: TrackUiState,
}

impl ImportTrackSnapshot {
    pub fn new(track: Track, track_ui: TrackUiState) -> Self {
        Self { track, track_ui }
    }
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
