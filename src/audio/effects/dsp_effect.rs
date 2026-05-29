use crate::audio::clap_effect::ClapEffectAdapter;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, Parameterizable};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EffectType {
    Gain,
    Equalizer,
    Compressor,
    Reverb,
    Delay,
    Clap { plugin_id: String, path: String },
}

pub enum EffectKind {
    BuiltIn(Box<dyn DspEffect>),
    Clap(Mutex<ClapEffectAdapter>),
}

pub struct EffectInstance {
    pub id: Uuid,
    pub name: String,
    pub effect_type: EffectType,
    pub bypass: AtomicBool,
    pub kind: EffectKind,
}

impl EffectInstance {
    pub fn new_builtin(name: String, effect_type: EffectType, effect: Box<dyn DspEffect>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            effect_type,
            bypass: AtomicBool::new(false),
            kind: EffectKind::BuiltIn(effect),
        }
    }

    pub fn new_clap(name: String, effect_type: EffectType, adapter: ClapEffectAdapter) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            effect_type,
            bypass: AtomicBool::new(false),
            kind: EffectKind::Clap(Mutex::new(adapter)),
        }
    }

    pub fn is_bypassed(&self) -> bool {
        if self.bypass.load(Ordering::Acquire) {
            return true;
        }
        match &self.kind {
            EffectKind::Clap(adapter) => adapter.lock().unwrap().is_bypassed(),
            EffectKind::BuiltIn(_) => false,
        }
    }

    pub fn set_bypass(&self, val: bool) {
        self.bypass.store(val, Ordering::Release);
        if let EffectKind::Clap(adapter) = &self.kind {
            adapter.lock().unwrap().set_bypass(val);
        }
    }

    pub fn parameter_info(&self) -> Vec<ParameterInfo> {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.parameter_info().to_vec(),
            EffectKind::Clap(adapter) => adapter.lock().unwrap().parameter_info(),
        }
    }

    pub fn parameter_value(&self, id: ParamId) -> f32 {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.parameter_value(id),
            EffectKind::Clap(adapter) => adapter.lock().unwrap().parameter_value(id),
        }
    }

    pub fn set_parameter(&self, id: ParamId, value: f32) {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.set_parameter(id, value),
            EffectKind::Clap(adapter) => adapter.lock().unwrap().set_parameter(id, value),
        }
    }
}

pub trait DspEffect: Parameterizable {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], sample_rate: u32);
    fn reset(&mut self, sample_rate: u32);
}