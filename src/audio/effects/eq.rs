use crate::audio::effects::dsp_effect::DspEffect;
use crate::audio::effects::parameter::{ParamId, ParameterInfo, ParameterFlags, ParameterValue, Parameterizable};
use crate::dsp::biquad::{self, BiquadCoeffs, BiquadState, BiquadType};
use std::sync::atomic::{AtomicBool, Ordering};

pub struct EqEffect {
    params: Vec<ParameterValue>,
    info: Vec<ParameterInfo>,
    btypes: Vec<BiquadType>,
    coeffs: Vec<BiquadCoeffs>,
    state_l: Vec<BiquadState>,
    state_r: Vec<BiquadState>,
    sample_rate: u32,
    dirty: AtomicBool,
}

impl EqEffect {
    pub fn new() -> Self {
        let info = vec![
            ParameterInfo { id: 0,  name: "Band 1 Freq".into(),    label: "Hz".into(),  min_value: 20.0,  max_value: 500.0,   default_value: 80.0,   flags: ParameterFlags::default() },
            ParameterInfo { id: 1,  name: "Band 1 Gain".into(),    label: "dB".into(),  min_value: -15.0, max_value: 15.0,    default_value: 0.0,    flags: ParameterFlags::default() },
            ParameterInfo { id: 2,  name: "Band 1 Q".into(),       label: "".into(),     min_value: 0.3,   max_value: 10.0,    default_value: 0.7,    flags: ParameterFlags::default() },
            ParameterInfo { id: 3,  name: "Band 2 Freq".into(),    label: "Hz".into(),  min_value: 50.0,  max_value: 2000.0,  default_value: 300.0,  flags: ParameterFlags::default() },
            ParameterInfo { id: 4,  name: "Band 2 Gain".into(),    label: "dB".into(),  min_value: -15.0, max_value: 15.0,    default_value: 0.0,    flags: ParameterFlags::default() },
            ParameterInfo { id: 5,  name: "Band 2 Q".into(),       label: "".into(),     min_value: 0.3,   max_value: 10.0,    default_value: 0.7,    flags: ParameterFlags::default() },
            ParameterInfo { id: 6,  name: "Band 3 Freq".into(),    label: "Hz".into(),  min_value: 200.0, max_value: 8000.0,  default_value: 1000.0, flags: ParameterFlags::default() },
            ParameterInfo { id: 7,  name: "Band 3 Gain".into(),    label: "dB".into(),  min_value: -15.0, max_value: 15.0,    default_value: 0.0,    flags: ParameterFlags::default() },
            ParameterInfo { id: 8,  name: "Band 3 Q".into(),       label: "".into(),     min_value: 0.3,   max_value: 10.0,    default_value: 0.7,    flags: ParameterFlags::default() },
            ParameterInfo { id: 9,  name: "Band 4 Freq".into(),    label: "Hz".into(),  min_value: 1000.0, max_value: 20000.0, default_value: 5000.0, flags: ParameterFlags::default() },
            ParameterInfo { id: 10, name: "Band 4 Gain".into(),    label: "dB".into(),  min_value: -15.0, max_value: 15.0,    default_value: 0.0,    flags: ParameterFlags::default() },
            ParameterInfo { id: 11, name: "Band 4 Q".into(),       label: "".into(),     min_value: 0.3,   max_value: 10.0,    default_value: 0.7,    flags: ParameterFlags::default() },
        ];
        let params = info.iter().map(|p| ParameterValue::new(p.default_value)).collect();
        let btypes = vec![
            BiquadType::LowShelf,
            BiquadType::Peaking,
            BiquadType::Peaking,
            BiquadType::HighShelf,
        ];
        let dummy = BiquadCoeffs { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };
        Self {
            params,
            info,
            btypes,
            coeffs: vec![dummy; 4],
            state_l: vec![BiquadState::new(); 4],
            state_r: vec![BiquadState::new(); 4],
            sample_rate: 44100,
            dirty: AtomicBool::new(true),
        }
    }

    fn rebuild_coeffs(&mut self) {
        let sr = self.sample_rate as f32;
        for band in 0..4 {
            let freq = self.params[band * 3].get();
            let gain = self.params[band * 3 + 1].get();
            let q = self.params[band * 3 + 2].get();
            self.coeffs[band] = biquad::compute_coeffs(&self.btypes[band], freq, gain, q.max(0.1), sr);
        }
        self.dirty.store(false, Ordering::Release);
    }

    pub fn frequency_response(&self, freq_hz: f32) -> f32 {
        let sr = self.sample_rate as f32;
        let mut total_db = 0.0;
        for band in 0..4 {
            let c = if self.dirty.load(Ordering::Acquire) {
                let freq = self.params[band * 3].get();
                let gain = self.params[band * 3 + 1].get();
                let q = self.params[band * 3 + 2].get();
                biquad::compute_coeffs(&self.btypes[band], freq, gain, q.max(0.1), sr)
            } else {
                self.coeffs[band].clone()
            };
            total_db += biquad::frequency_response(&c, freq_hz, sr);
        }
        total_db
    }
}

impl Parameterizable for EqEffect {
    fn parameter_info(&self) -> &[ParameterInfo] { &self.info }
    fn parameter_value(&self, id: ParamId) -> f32 {
        self.params.get(id as usize).map(|p| p.get()).unwrap_or(0.0)
    }
    fn set_parameter(&self, id: ParamId, value: f32) {
        if let Some(p) = self.params.get(id as usize) {
            if let Some(info) = self.info.get(id as usize) {
                p.set_clamped(value, info.min_value, info.max_value);
                self.dirty.store(true, Ordering::Release);
            }
        }
    }
    fn parameter_ptr(&self, id: ParamId) -> Option<&ParameterValue> {
        self.params.get(id as usize)
    }
}

impl DspEffect for EqEffect {
    fn process(&mut self, input_l: &mut [f32], input_r: &mut [f32], _sample_rate: u32) {
        if self.dirty.load(Ordering::Acquire) {
            self.rebuild_coeffs();
        }
        for band in 0..4 {
            biquad::process_biquad(input_l, &mut self.state_l[band], &self.coeffs[band]);
            biquad::process_biquad(input_r, &mut self.state_r[band], &self.coeffs[band]);
        }
    }
    fn reset(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
        self.dirty.store(true, Ordering::Release);
        for s in &mut self.state_l { s.reset(); }
        for s in &mut self.state_r { s.reset(); }
    }
}
