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
    Chorus,
    Flanger,
    Phaser,
    Distortion,
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
    pub has_note_input: bool,
}

impl EffectInstance {
    pub fn new_builtin(name: String, effect_type: EffectType, effect: Box<dyn DspEffect>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            effect_type,
            bypass: AtomicBool::new(false),
            kind: EffectKind::BuiltIn(effect),
            has_note_input: false,
        }
    }

    pub fn new_clap(name: String, effect_type: EffectType, adapter: ClapEffectAdapter) -> Self {
        let has_note_input = adapter.has_note_input();
        Self {
            id: Uuid::new_v4(),
            name,
            effect_type,
            bypass: AtomicBool::new(false),
            kind: EffectKind::Clap(Mutex::new(adapter)),
            has_note_input,
        }
    }

    fn lock_clap(&self) -> std::sync::MutexGuard<'_, ClapEffectAdapter> {
        match &self.kind {
            EffectKind::Clap(adapter) => adapter.lock().unwrap_or_else(|e| e.into_inner()),
            EffectKind::BuiltIn(_) => panic!("not a CLAP effect"),
        }
    }

    #[allow(clippy::mut_mutex_lock)]
    fn lock_clap_mut(&mut self) -> std::sync::MutexGuard<'_, ClapEffectAdapter> {
        match &mut self.kind {
            EffectKind::Clap(adapter) => adapter.lock().unwrap_or_else(|e| e.into_inner()),
            EffectKind::BuiltIn(_) => panic!("not a CLAP effect"),
        }
    }

    pub fn is_bypassed(&self) -> bool {
        if self.bypass.load(Ordering::Acquire) {
            return true;
        }
        match &self.kind {
            EffectKind::Clap(_) => self.lock_clap().is_bypassed(),
            EffectKind::BuiltIn(_) => false,
        }
    }

    pub fn set_bypass(&self, val: bool) {
        self.bypass.store(val, Ordering::Release);
        if let EffectKind::Clap(_) = &self.kind {
            self.lock_clap().set_bypass(val);
        }
    }

    pub fn parameter_info(&self) -> Vec<ParameterInfo> {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.parameter_info().to_vec(),
            EffectKind::Clap(_) => self.lock_clap().parameter_info(),
        }
    }

    pub fn parameter_value(&self, id: ParamId) -> f32 {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.parameter_value(id),
            EffectKind::Clap(_) => self.lock_clap().parameter_value(id),
        }
    }

    pub fn set_parameter(&mut self, id: ParamId, value: f32) {
        match &mut self.kind {
            EffectKind::BuiltIn(effect) => effect.set_parameter(id, value),
            EffectKind::Clap(_) => self.lock_clap_mut().set_parameter(id, value),
        }
    }

    /// Non-blocking variant for audio-thread use. For CLAP effects, uses
    /// `try_lock` on the adapter — never stalls. For built-in effects,
    /// delegates to the same set_parameter (no locking issue).
    pub fn try_set_parameter(&self, id: ParamId, value: f32) {
        match &self.kind {
            EffectKind::BuiltIn(effect) => effect.set_parameter(id, value),
            EffectKind::Clap(adapter) => {
                if let Ok(a) = adapter.try_lock() {
                    a.apply_parameter(id, value);
                }
            }
        }
    }
}

pub trait DspEffect: Parameterizable {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], sample_rate: u32);
    fn reset(&mut self, sample_rate: u32);
}