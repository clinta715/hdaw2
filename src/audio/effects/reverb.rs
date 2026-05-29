use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};

struct CombFilter {
    buffer: Vec<f32>,
    index: usize,
    size: usize,
    feedback: f32,
    damping: f32,
    damp_state: f32,
}

impl CombFilter {
    fn new(delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            index: 0,
            size: delay_samples,
            feedback: 0.5,
            damping: 0.5,
            damp_state: 0.0,
        }
    }
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.index];
        self.damp_state = out * (1.0 - self.damping) + self.damp_state * self.damping;
        let feedback = input + self.damp_state * self.feedback;
        self.buffer[self.index] = feedback;
        self.index = (self.index + 1) % self.size;
        out
    }
    fn reset(&mut self) {
        for s in &mut self.buffer { *s = 0.0; }
        self.index = 0;
        self.damp_state = 0.0;
    }
}

struct AllPassFilter {
    buffer: Vec<f32>,
    index: usize,
    size: usize,
    gain: f32,
}

impl AllPassFilter {
    fn new(delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; delay_samples],
            index: 0,
            size: delay_samples,
            gain: 0.5,
        }
    }
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.index];
        let out = buf_out - input;
        self.buffer[self.index] = input + buf_out * self.gain;
        self.index = (self.index + 1) % self.size;
        out
    }
    fn reset(&mut self) {
        for s in &mut self.buffer { *s = 0.0; }
        self.index = 0;
    }
}

pub struct ReverbEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    comb_l: [CombFilter; 4],
    comb_r: [CombFilter; 4],
    allpass_l: [AllPassFilter; 2],
    allpass_r: [AllPassFilter; 2],
    sample_rate: u32,
}

impl ReverbEffect {
    pub fn new() -> Self {
        let comb_delays_l = [479, 1603, 2111, 3011];
        let comb_delays_r = [503, 1637, 2137, 3043];
        let allpass_delays_l = [347, 601];
        let allpass_delays_r = [367, 631];
        let comb_l = [
            CombFilter::new(comb_delays_l[0]),
            CombFilter::new(comb_delays_l[1]),
            CombFilter::new(comb_delays_l[2]),
            CombFilter::new(comb_delays_l[3]),
        ];
        let comb_r = [
            CombFilter::new(comb_delays_r[0]),
            CombFilter::new(comb_delays_r[1]),
            CombFilter::new(comb_delays_r[2]),
            CombFilter::new(comb_delays_r[3]),
        ];
        let allpass_l = [
            AllPassFilter::new(allpass_delays_l[0]),
            AllPassFilter::new(allpass_delays_l[1]),
        ];
        let allpass_r = [
            AllPassFilter::new(allpass_delays_r[0]),
            AllPassFilter::new(allpass_delays_r[1]),
        ];
        Self {
            params: vec![
                ParameterValue::new(0.5),
                ParameterValue::new(0.5),
                ParameterValue::new(0.3),
            ],
            info: vec![
                ParameterInfo { id: 0, name: "Room Size".into(), label: "".into(), min_value: 0.0, max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 1, name: "Damping".into(),   label: "".into(), min_value: 0.0, max_value: 1.0, default_value: 0.5, flags: ParameterFlags::default() },
                ParameterInfo { id: 2, name: "Mix".into(),       label: "".into(), min_value: 0.0, max_value: 1.0, default_value: 0.3, flags: ParameterFlags::default() },
            ],
            comb_l,
            comb_r,
            allpass_l,
            allpass_r,
            sample_rate: 0,
        }
    }
}

impl Parameterizable for ReverbEffect {
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

impl DspEffect for ReverbEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        let room = self.params[0].get();
        let damping = self.params[1].get();
        let mix = self.params[2].get();
        let fb = 0.7 + room * 0.28;

        for comb in &mut self.comb_l {
            comb.feedback = fb;
            comb.damping = damping;
        }
        for comb in &mut self.comb_r {
            comb.feedback = fb;
            comb.damping = damping;
        }

        for i in 0..input_l.len() {
            let dry_l = input_l[i];
            let dry_r = input_r[i];
            let mut wet_l = 0.0f32;
            let mut wet_r = 0.0f32;

            for comb in self.comb_l.iter_mut() {
                wet_l += comb.process(dry_l) * 0.25;
            }
            for ap in self.allpass_l.iter_mut() {
                wet_l = ap.process(wet_l);
            }

            for comb in self.comb_r.iter_mut() {
                wet_r += comb.process(dry_r) * 0.25;
            }
            for ap in self.allpass_r.iter_mut() {
                wet_r = ap.process(wet_r);
            }

            input_l[i] = dry_l * (1.0 - mix) + wet_l * mix;
            input_r[i] = dry_r * (1.0 - mix) + wet_r * mix;
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        for c in &mut self.comb_l { c.reset(); }
        for c in &mut self.comb_r { c.reset(); }
        for a in &mut self.allpass_l { a.reset(); }
        for a in &mut self.allpass_r { a.reset(); }
    }
}
