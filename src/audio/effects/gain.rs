use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

pub struct GainEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
}

impl GainEffect {
    pub fn new() -> Self {
        Self {
            params: vec![
                ParameterValue::new(1.0),
                ParameterValue::new(1.0),
            ],
            info: vec![
                ParameterInfo {
                    id: 0,
                    name: "Input Gain".into(),
                    label: "".into(),
                    min_value: 0.0,
                    max_value: 2.0,
                    default_value: 1.0,
                    flags: ParameterFlags::default(),
                },
                ParameterInfo {
                    id: 1,
                    name: "Output Gain".into(),
                    label: "".into(),
                    min_value: 0.0,
                    max_value: 2.0,
                    default_value: 1.0,
                    flags: ParameterFlags::default(),
                },
            ],
        }
    }
}

impl Default for GainEffect {
    fn default() -> Self { Self::new() }
}

impl Parameterizable for GainEffect {
    fn parameter_info(&self) -> &[ParameterInfo] {
        &self.info
    }
    fn parameter_value(&self, id: ParamId) -> f32 {
        self.params.get(id as usize).map(|p| p.get()).unwrap_or(1.0)
    }
    fn set_parameter(&self, id: ParamId, value: f32) {
        if let Some(p) = self.params.get(id as usize) {
            if let Some(info) = self.info.get(id as usize) {
                p.set_clamped(value, info.min_value, info.max_value);
            }
        }
    }
    fn parameter_ptr(&self, id: ParamId) -> Option<&ParameterValue> {
        self.params.get(id as usize)
    }
}

impl DspEffect for GainEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let in_gain = self.params[0].get();
        let out_gain = self.params[1].get();
        let total = in_gain * out_gain;
        for i in 0..input_l.len() {
            input_l[i] *= total;
            input_r[i] *= total;
        }
    }
    fn reset(&mut self, _sample_rate: u32) {}
}
