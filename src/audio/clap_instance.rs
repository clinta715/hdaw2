use crate::audio::effects::parameter::{ParamId, ParameterInfo};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub struct ClapPluginState {
    name: String,
    plugin_id: String,
    path: std::path::PathBuf,
    param_infos: Vec<ParameterInfo>,
    param_values: Vec<Arc<AtomicU32>>,
    bypass: AtomicBool,
}

impl ClapPluginState {
    pub fn new(
        name: String,
        plugin_id: String,
        path: std::path::PathBuf,
        param_infos: Vec<ParameterInfo>,
    ) -> Self {
        let param_values = param_infos
            .iter()
            .map(|info| Arc::new(AtomicU32::new(info.default_value.to_bits())))
            .collect();
        Self {
            name,
            plugin_id,
            path,
            param_infos,
            param_values,
            bypass: AtomicBool::new(false),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn parameter_info(&self) -> &[ParameterInfo] {
        &self.param_infos
    }

    pub fn parameter_value(&self, id: ParamId) -> f32 {
        self.param_values
            .iter()
            .zip(&self.param_infos)
            .find(|(_, info)| info.id == id)
            .map(|(v, _)| f32::from_bits(v.load(Ordering::Acquire)))
            .unwrap_or(0.0)
    }

    pub fn set_parameter(&self, id: ParamId, value: f32) {
        if let Some((v, info)) = self
            .param_values
            .iter()
            .zip(&self.param_infos)
            .find(|(_, info)| info.id == id)
        {
            let clamped = value.clamp(info.min_value, info.max_value);
            v.store(clamped.to_bits(), Ordering::Release);
        }
    }

    pub fn is_bypassed(&self) -> bool {
        self.bypass.load(Ordering::Acquire)
    }

    pub fn set_bypass(&self, val: bool) {
        self.bypass.store(val, Ordering::Release);
    }
}