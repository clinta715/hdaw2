use crate::audio::clap_instance::ClapPluginState;
use std::sync::Mutex;

pub struct ClapEffectAdapter {
    state: Mutex<ClapPluginState>,
}

impl ClapEffectAdapter {
    pub fn new(state: ClapPluginState) -> Self {
        Self {
            state: Mutex::new(state),
        }
    }

    pub fn name(&self) -> String {
        self.state.lock().unwrap().name().to_string()
    }

    pub fn parameter_info(&self) -> Vec<crate::audio::effects::parameter::ParameterInfo> {
        self.state.lock().unwrap().parameter_info().to_vec()
    }

    pub fn parameter_value(&self, id: crate::audio::effects::parameter::ParamId) -> f32 {
        self.state.lock().unwrap().parameter_value(id)
    }

    pub fn set_parameter(&self, id: crate::audio::effects::parameter::ParamId, value: f32) {
        self.state.lock().unwrap().set_parameter(id, value)
    }

    pub fn is_bypassed(&self) -> bool {
        self.state.lock().unwrap().is_bypassed()
    }

    pub fn set_bypass(&self, val: bool) {
        self.state.lock().unwrap().set_bypass(val)
    }

    pub fn process(&mut self, _input_l: &mut [f32], _input_r: &mut [f32], _sample_rate: u32) {
    }
}