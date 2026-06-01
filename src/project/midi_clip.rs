use crate::project::midi_note::MidiNote;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiClip {
    pub id: Uuid,
    pub name: String,
    pub position_frames: u64,
    pub length_frames: u64,
    pub notes: Vec<MidiNote>,
    pub color: [u8; 3],
    #[serde(skip)]
    pub thumb_dirty: bool,
}
