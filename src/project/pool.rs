use crate::project::clip::ClipKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolClip {
    pub id: Uuid,
    pub name: String,
    pub clip: ClipKind,
}

impl PoolClip {
    pub fn from_clip(clip: ClipKind) -> Self {
        let name = match &clip {
            ClipKind::Audio(a) => a.name.clone(),
            ClipKind::Midi(m) => m.name.clone(),
        };
        Self {
            id: Uuid::new_v4(),
            name,
            clip,
        }
    }
}