use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

pub struct CompressorEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    envelope_l: f32,
    envelope_r: f32,
    sample_rate: u32,
}

impl CompressorEffect {
    pub fn new() -> Self {
        Self {
            params: vec![
                ParameterValue::new(0.0),
                ParameterValue::new(2.0),
                ParameterValue::new(0.005),
                ParameterValue::new(0.100),
                ParameterValue::new(0.0),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Threshold".into(), label: "dB".into(), min_value: -60.0, max_value: 0.0,   default_value: 0.0,   flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Ratio".into(),     label: ":1".into(),  min_value: 1.0,   max_value: 20.0,  default_value: 2.0,   flags: ParameterFlags::default() },
                ParameterInfo { id: 2, name: "Attack".into(),    label: "s".into(),   min_value: 0.001, max_value: 0.100, default_value: 0.005, flags: ParameterFlags::default() },
                ParameterInfo { id: 3, name: "Release".into(),   label: "s".into(),   min_value: 0.010, max_value: 2.0,   default_value: 0.100, flags: ParameterFlags::default() },
                ParameterInfo { id: 4, name: "Makeup".into(),    label: "dB".into(),  min_value: 0.0,   max_value: 30.0,  default_value: 0.0,   flags: ParameterFlags::default() },
            ],
            envelope_l: 0.0,
            envelope_r: 0.0,
            sample_rate: 44100,
        }
    }

    pub fn gain_reduction(&self) -> f32 {
        self.envelope_l.max(self.envelope_r)
    }
}

impl Default for CompressorEffect {
    fn default() -> Self { Self::new() }
}

impl Parameterizable for CompressorEffect {
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

impl DspEffect for CompressorEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let threshold_db = self.params[0].get();
        let ratio = self.params[1].get();
        let attack_s = self.params[2].get();
        let release_s = self.params[3].get();
        let makeup_db = self.params[4].get();

        let sr = self.sample_rate as f32;
        let attack_coeff = (-1.0 / (sr * attack_s.max(0.001))).exp();
        let release_coeff = (-1.0 / (sr * release_s.max(0.001))).exp();
        let threshold_lin = 10.0f32.powf(threshold_db / 20.0);
        let makeup_lin = 10.0f32.powf(makeup_db / 20.0);
        let slope = 1.0 - 1.0 / ratio.max(1.0);

        for i in 0..input_l.len() {
            let level_l = input_l[i].abs();
            let level_r = input_r[i].abs();

            let env_target_l = if level_l > self.envelope_l { level_l } else { self.envelope_l * 0.999 + level_l * 0.001 };
            let env_target_r = if level_r > self.envelope_r { level_r } else { self.envelope_r * 0.999 + level_r * 0.001 };
            let above = env_target_l.max(env_target_r) > threshold_lin;

            let coeff = if above { attack_coeff } else { release_coeff };
            self.envelope_l = self.envelope_l + (1.0 - coeff) * (env_target_l - self.envelope_l);
            self.envelope_r = self.envelope_r + (1.0 - coeff) * (env_target_r - self.envelope_r);

            let env = self.envelope_l.max(self.envelope_r);
            let gain_db = if env > threshold_lin {
                (20.0 * env.log10() - threshold_db) * -slope
            } else {
                0.0
            };
            let gain_lin = 10.0f32.powf(gain_db / 20.0) * makeup_lin;
            input_l[i] *= gain_lin;
            input_r[i] *= gain_lin;
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.envelope_l = 0.0;
        self.envelope_r = 0.0;
    }
}
