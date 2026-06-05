use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

pub struct FlangerEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    phase: f64,
    sample_rate: u32,
}

impl FlangerEffect {
    pub fn new() -> Self {
        let max_delay = (0.01 * 96000.0) as usize;
        Self {
            params: vec![
                ParameterValue::new(0.3),
                ParameterValue::new(0.5),
                ParameterValue::new(0.3),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Rate".into(),      label: "Hz".into(), min_value: 0.05, max_value: 5.0, default_value: 0.3, flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Depth".into(),     label: "".into(),    min_value: 0.0,  max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 2, name: "Feedback".into(),  label: "".into(),    min_value: 0.0,  max_value: 0.95, default_value: 0.3, flags: ParameterFlags::default() },
            ],
            buffer_l: vec![0.0; max_delay],
            buffer_r: vec![0.0; max_delay],
            write_pos: 0,
            phase: 0.0,
            sample_rate: 0,
        }
    }
}

impl Default for FlangerEffect {
    fn default() -> Self { Self::new() }
}

impl Parameterizable for FlangerEffect {
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

impl DspEffect for FlangerEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let rate = self.params[0].get();
        let depth = self.params[1].get();
        let feedback = self.params[2].get();
        let max_delay = self.buffer_l.len() as f32;
        if self.sample_rate == 0 { return; }
        let sr = self.sample_rate as f64;

        for i in 0..input_l.len() {
            let lfo = (self.phase * std::f64::consts::TAU).sin() as f32;
            let mod_delay = (lfo * 0.5 + 0.5) * depth * (max_delay - 1.0);
            let delay_samples = mod_delay as usize + 1;

            let read_pos = if self.write_pos >= delay_samples {
                self.write_pos - delay_samples
            } else {
                self.buffer_l.len() - (delay_samples - self.write_pos)
            };

            let delayed_l = self.buffer_l[read_pos % self.buffer_l.len()];
            let delayed_r = self.buffer_r[read_pos % self.buffer_l.len()];

            self.buffer_l[self.write_pos] = input_l[i] + delayed_l * feedback;
            self.buffer_r[self.write_pos] = input_r[i] + delayed_r * feedback;
            self.write_pos = (self.write_pos + 1) % self.buffer_l.len();

            input_l[i] += delayed_l;
            input_r[i] += delayed_r;

            self.phase += rate as f64 / sr;
            if self.phase >= 1.0 { self.phase -= 1.0; }
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.write_pos = 0;
        self.phase = 0.0;
        let max_delay = (0.01 * sample_rate as f32) as usize;
        self.buffer_l = vec![0.0; max_delay];
        self.buffer_r = vec![0.0; max_delay];
    }
}
