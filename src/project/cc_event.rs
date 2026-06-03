use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CCEvent {
    pub cc_number: u8,
    pub time_frames: u64,
    pub value: f32,
}
