use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    pub id: Uuid,
    pub position_frames: u64,
    pub name: String,
    pub color: [u8; 3],
}

impl Marker {
    pub fn new(position_frames: u64, name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            position_frames,
            name,
            color: [0xff, 0xcc, 0x44],
        }
    }
}
