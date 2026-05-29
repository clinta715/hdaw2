use crate::project::clip::AudioClip;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolClip {
    pub id: Uuid,
    pub name: String,
    pub clip: AudioClip,
}

impl PoolClip {
    pub fn from_clip(clip: AudioClip) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: clip.name.clone(),
            clip,
        }
    }
}