use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

pub struct DistortionEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
}

impl DistortionEffect {
    pub fn new() -> Self {
        Self {
            params: vec![
                ParameterValue::new(0.5),
                ParameterValue::new(0.5),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Drive".into(), label: "".into(), min_value: 0.0, max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Mix".into(),   label: "".into(), min_value: 0.0, max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
            ],
        }
    }
}

impl Default for DistortionEffect {
    fn default() -> Self { Self::new() }
}

impl Parameterizable for DistortionEffect {
    fn parameter_info(&self) -> &[ParameterInfo] { &self.info }
    fn parameter_value(&self, id: ParamId) -> f32 {
        self.params.get(id as usize).map(|p| p.get()).unwrap_or(0.0)
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

fn tanh_approx(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

impl DspEffect for DistortionEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let drive = self.params[0].get();
        let mix = self.params[1].get();
        let gain = 1.0 + drive * 10.0;

        for i in 0..input_l.len() {
            let dry_l = input_l[i];
            let dry_r = input_r[i];

            let wet_l = tanh_approx(dry_l * gain);
            let wet_r = tanh_approx(dry_r * gain);

            input_l[i] = dry_l * (1.0 - mix) + wet_l * mix;
            input_r[i] = dry_r * (1.0 - mix) + wet_r * mix;
        }
    }
    fn reset(&mut self, _sample_rate: u32) {}
}
