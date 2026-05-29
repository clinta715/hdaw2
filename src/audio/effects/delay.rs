use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

pub struct DelayEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    sample_rate: u32,
}

impl DelayEffect {
    pub fn new() -> Self {
        let max_samples = (2.0 * 96000.0) as usize;
        Self {
            params: vec![
                ParameterValue::new(0.5),
                ParameterValue::new(0.3),
                ParameterValue::new(0.3),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Time".into(),     label: "s".into(), min_value: 0.01, max_value: 2.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Feedback".into(), label: "".into(),   min_value: 0.0,  max_value: 0.99, default_value: 0.3, flags: ParameterFlags::default() },
                ParameterInfo { id: 2, name: "Mix".into(),      label: "".into(),   min_value: 0.0,  max_value: 1.0,  default_value: 0.3, flags: ParameterFlags::default() },
            ],
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate: 0,
        }
    }
}

impl Parameterizable for DelayEffect {
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

impl DspEffect for DelayEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let time_s = self.params[0].get();
        let feedback = self.params[1].get();
        let mix = self.params[2].get();

        let delay_samples = (time_s * self.sample_rate as f32) as usize;
        let capacity = self.buffer_l.len();

        for i in 0..input_l.len() {
            let read_pos = if self.write_pos >= delay_samples {
                self.write_pos - delay_samples
            } else {
                capacity - (delay_samples - self.write_pos)
            };
            let delayed_l = self.buffer_l[read_pos % capacity];
            let delayed_r = self.buffer_r[read_pos % capacity];

            self.buffer_l[self.write_pos] = input_l[i] + delayed_l * feedback;
            self.buffer_r[self.write_pos] = input_r[i] + delayed_r * feedback;
            self.write_pos = (self.write_pos + 1) % capacity;

            input_l[i] = input_l[i] * (1.0 - mix) + delayed_l * mix;
            input_r[i] = input_r[i] * (1.0 - mix) + delayed_r * mix;
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.write_pos = 0;
        let max_samples = (2.0 * sample_rate as f32) as usize;
        self.buffer_l = vec![0.0; max_samples];
        self.buffer_r = vec![0.0; max_samples];
    }
}
