use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::automation::AutomationLane;
use crate::project::clip::ClipKind;
use crate::project::clip_handle::ClipHandle;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SendSlot {
    pub target_id: Uuid,
    pub level: Arc<AtomicU32>,
    pub pre_fader: bool,
}

impl SendSlot {
    pub fn new(target_id: Uuid, level: f32, pre_fader: bool) -> Self {
        Self {
            target_id,
            level: Arc::new(AtomicU32::new(level.to_bits())),
            pre_fader,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendSlotDef {
    pub target_id: Uuid,
    pub level: f32,
    pub pre_fader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEffect {
    pub name: String,
    pub effect_type: EffectType,
    pub bypass: bool,
    pub param_values: Vec<f32>,
}

pub struct TrackHandle {
    pub id: Uuid,
    pub volume: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
    pub mute: Arc<AtomicBool>,
    pub solo: Arc<AtomicBool>,
    pub peak_left: Arc<AtomicU32>,
    pub peak_right: Arc<AtomicU32>,
    pub clips: Vec<ClipHandle>,
    pub fx_chain: Vec<EffectInstance>,
    pub automation_lanes: Vec<AutomationLane>,
    pub armed: Arc<AtomicBool>,
    pub parent_group: Option<Uuid>,
    pub is_group: bool,
    pub is_return: bool,
    pub sends: Vec<SendSlot>,
}

impl TrackHandle {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            volume: Arc::new(AtomicU32::new(f32::to_bits(1.0))),
            pan: Arc::new(AtomicU32::new(f32::to_bits(0.0))),
            mute: Arc::new(AtomicBool::new(false)),
            solo: Arc::new(AtomicBool::new(false)),
            peak_left: Arc::new(AtomicU32::new(0)),
            peak_right: Arc::new(AtomicU32::new(0)),
            clips: Vec::new(),
            fx_chain: Vec::new(),
            automation_lanes: vec![AutomationLane::volume_lane(), AutomationLane::pan_lane()],
            armed: Arc::new(AtomicBool::new(false)),
            parent_group: None,
            is_group: false,
            is_return: false,
            sends: Vec::new(),
        }
    }

    pub fn add_clip(&mut self, clip: ClipHandle) {
        self.clips.push(clip);
    }

    pub fn find_clip_by_id(&self, clip_id: Uuid) -> Option<usize> {
        self.clips.iter().position(|c| c.clip_id == clip_id)
    }

    pub fn add_effect(&mut self, instance: EffectInstance) {
        self.fx_chain.push(instance);
    }

    pub fn remove_effect(&mut self, index: usize) {
        if index < self.fx_chain.len() {
            self.fx_chain.remove(index);
        }
    }

    pub fn set_effect_bypass(&mut self, index: usize, bypass: bool) {
        if let Some(instance) = self.fx_chain.get(index) {
            instance.set_bypass(bypass);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: Uuid,
    pub name: String,
    pub color: [u8; 3],
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub clips: Vec<ClipKind>,
    pub automation_lanes: Vec<AutomationLane>,
    pub fx_chain: Vec<SerializedEffect>,
    #[serde(default)]
    pub parent_group: Option<Uuid>,
    #[serde(default)]
    pub is_group: bool,
    #[serde(default)]
    pub is_return: bool,
    #[serde(default)]
    pub sends: Vec<SendSlotDef>,
}

impl Track {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            color: [0x1a, 0x2a, 0x1a],
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            clips: Vec::new(),
            automation_lanes: vec![AutomationLane::volume_lane(), AutomationLane::pan_lane()],
            fx_chain: Vec::new(),
            parent_group: None,
            is_group: false,
            is_return: false,
            sends: Vec::new(),
        }
    }

    pub fn new_group(name: String) -> Self {
        let mut t = Self::new(name);
        t.is_group = true;
        t
    }

    pub fn new_return(name: String) -> Self {
        let mut t = Self::new(name);
        t.is_return = true;
        t
    }

    pub fn add_clip(&mut self, clip: ClipKind) {
        self.clips.push(clip);
    }
}

impl TrackHandle {
    pub fn new_group() -> Self {
        let mut t = Self::new();
        t.is_group = true;
        t
    }

    pub fn new_return() -> Self {
        let mut t = Self::new();
        t.is_return = true;
        t
    }
}

impl Default for TrackHandle {
    fn default() -> Self {
        Self::new()
    }
}
