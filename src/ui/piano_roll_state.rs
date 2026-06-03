use crate::project::cc_event::CCEvent;
use crate::ui::preferences::GridDivision;

#[derive(Clone, Copy, PartialEq)]
pub enum ControllerLane {
    None,
    Velocity,
    ReleaseVelocity,
    Cc(u8),
}

#[derive(Clone)]
pub struct CcDragState {
    pub event_idx: usize,
    pub original_event: CCEvent,
}

pub struct PianoRollState {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub zoom_x: f64,
    pub zoom_y: f64,
    pub drag_target: Option<PianoRollDragTarget>,
    pub grid_division: GridDivision,
    pub note_length: f64,
    pub controller_lane: ControllerLane,
    pub controller_drag_note: Option<usize>,
    pub cc_number: u8,
    pub cc_drag: Option<CcDragState>,
}

#[derive(Clone, PartialEq)]
pub enum PianoRollDragTarget {
    NoteMove { note_idx: usize, original_note: crate::project::midi_note::MidiNote },
    NoteResize { note_idx: usize, original_duration: u64 },
    NoteCreate { pitch: u8, start_frame: u64, current_end_frame: u64 },
}

impl Default for PianoRollState {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom_x: 60.0,
            zoom_y: 20.0,
            drag_target: None,
            grid_division: GridDivision::Quarter,
            note_length: 1.0,
            controller_lane: ControllerLane::Velocity,
            controller_drag_note: None,
            cc_number: 1,
            cc_drag: None,
        }
    }
}
