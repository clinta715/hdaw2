use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

pub type ParamId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    pub id: ParamId,
    pub name: String,
    pub label: String,
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub flags: ParameterFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterFlags {
    pub is_automateable: bool,
    pub is_bypass: bool,
    pub is_bool: bool,
    pub is_hidden: bool,
}

impl Default for ParameterFlags {
    fn default() -> Self {
        Self {
            is_automateable: true,
            is_bypass: false,
            is_bool: false,
            is_hidden: false,
        }
    }
}

pub struct ParameterValue {
    value: AtomicU32,
}

impl ParameterValue {
    pub fn new(value: f32) -> Self {
        Self {
            value: AtomicU32::new(value.to_bits()),
        }
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Acquire))
    }

    pub fn set_clamped(&self, value: f32, min: f32, max: f32) {
        let clamped = value.clamp(min, max);
        self.value.store(clamped.to_bits(), Ordering::Release);
    }
}

pub trait Parameterizable: Send {
    fn parameter_info(&self) -> &[ParameterInfo];
    fn parameter_value(&self, id: ParamId) -> f32;
    fn set_parameter(&self, id: ParamId, value: f32);
    fn parameter_ptr(&self, id: ParamId) -> Option<&ParameterValue>;
}
