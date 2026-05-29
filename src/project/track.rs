use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::automation::AutomationLane;
use crate::project::clip::AudioClip;
use crate::project::clip_handle::ClipHandle;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedEffect {
    pub name: String,
    pub effect_type: EffectType,
    pub bypass: bool,
    pub param_values: Vec<f32>,
}

pub struct TrackHandle {
    pub volume: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
    pub mute: Arc<AtomicBool>,
    pub solo: Arc<AtomicBool>,
    pub peak_left: Arc<AtomicU32>,
    pub peak_right: Arc<AtomicU32>,
    pub clips: Vec<ClipHandle>,
    pub fx_chain: Vec<EffectInstance>,
    pub automation_lanes: Vec<AutomationLane>,
}

impl TrackHandle {
    pub fn new() -> Self {
        Self {
            volume: Arc::new(AtomicU32::new(f32::to_bits(1.0))),
            pan: Arc::new(AtomicU32::new(f32::to_bits(0.0))),
            mute: Arc::new(AtomicBool::new(false)),
            solo: Arc::new(AtomicBool::new(false)),
            peak_left: Arc::new(AtomicU32::new(0)),
            peak_right: Arc::new(AtomicU32::new(0)),
            clips: Vec::new(),
            fx_chain: Vec::new(),
            automation_lanes: vec![AutomationLane::volume_lane(), AutomationLane::pan_lane()],
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
    pub clips: Vec<AudioClip>,
    pub automation_lanes: Vec<AutomationLane>,
    pub fx_chain: Vec<SerializedEffect>,
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
        }
    }

    pub fn add_clip(&mut self, clip: AudioClip) {
        self.clips.push(clip);
    }
}

impl Default for TrackHandle {
    fn default() -> Self {
        Self::new()
    }
}
