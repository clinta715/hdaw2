use crate::audio::effects::parameter::Parameterizable;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EffectType {
    Gain,
    Equalizer,
    Compressor,
    Reverb,
    Delay,
}

pub struct EffectInstance {
    pub id: Uuid,
    pub name: String,
    pub effect_type: EffectType,
    pub bypass: AtomicBool,
    pub effect: Box<dyn DspEffect>,
}

impl EffectInstance {
    pub fn new(name: String, effect_type: EffectType, effect: Box<dyn DspEffect>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            effect_type,
            bypass: AtomicBool::new(false),
            effect,
        }
    }

    pub fn is_bypassed(&self) -> bool {
        self.bypass.load(Ordering::Acquire)
    }

    pub fn set_bypass(&self, val: bool) {
        self.bypass.store(val, Ordering::Release);
    }
}

pub trait DspEffect: Parameterizable {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], sample_rate: u32);
    fn reset(&mut self, sample_rate: u32);
}
