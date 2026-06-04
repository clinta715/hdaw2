use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

struct Allpass {
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl Allpass {
    fn new() -> Self { Self { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 } }
    fn reset(&mut self) { self.x1 = 0.0; self.x2 = 0.0; self.y1 = 0.0; self.y2 = 0.0; }
    fn process(&mut self, x: f32, coeff: f32) -> f32 {
        let y = coeff * (x - self.y1) + self.x1;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct PhaserEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    stages_l: [Allpass; 6],
    stages_r: [Allpass; 6],
    phase: f64,
    sample_rate: u32,
}

impl PhaserEffect {
    pub fn new() -> Self {
        Self {
            params: vec![
                ParameterValue::new(0.5),
                ParameterValue::new(0.5),
                ParameterValue::new(0.3),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Rate".into(),      label: "Hz".into(), min_value: 0.05, max_value: 5.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Depth".into(),     label: "".into(),    min_value: 0.0,  max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 2, name: "Feedback".into(),  label: "".into(),    min_value: 0.0,  max_value: 0.95, default_value: 0.3, flags: ParameterFlags::default() },
            ],
            stages_l: [Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new()],
            stages_r: [Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new(), Allpass::new()],
            phase: 0.0,
            sample_rate: 0,
        }
    }
}

impl Parameterizable for PhaserEffect {
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

/// Compute allpass coefficient from center frequency and sample rate.
/// coeff = (tan(pi*f/sr) - 1) / (tan(pi*f/sr) + 1)
fn ap_coeff(freq: f32, sr: f32) -> f32 {
    let t = (std::f32::consts::PI * freq / sr).tan();
    (t - 1.0) / (t + 1.0)
}

impl DspEffect for PhaserEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let rate = self.params[0].get();
        let depth = self.params[1].get();
        let feedback = self.params[2].get();
        if self.sample_rate == 0 { return; }
        let sr = self.sample_rate as f32;

        let mut fb_l = 0.0f32;
        let mut fb_r = 0.0f32;

        for i in 0..input_l.len() {
            let lfo = (self.phase * std::f64::consts::TAU).sin() as f32;
            let freq = 200.0 + (lfo * 0.5 + 0.5) * depth * 1800.0 + 20.0;
            let coeff = ap_coeff(freq, sr);

            let mut x_l = input_l[i] + fb_l * feedback;
            let mut x_r = input_r[i] + fb_r * feedback;

            for s in 0..6 {
                x_l = self.stages_l[s].process(x_l, coeff);
                x_r = self.stages_r[s].process(x_r, coeff);
            }

            fb_l = x_l;
            fb_r = x_r;

            input_l[i] = input_l[i] + x_l;
            input_r[i] = input_r[i] + x_r;

            self.phase += rate as f64 / self.sample_rate as f64;
            if self.phase >= 1.0 { self.phase -= 1.0; }
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.phase = 0.0;
        for s in 0..6 {
            self.stages_l[s].reset();
            self.stages_r[s].reset();
        }
    }
}
