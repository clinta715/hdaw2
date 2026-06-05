/// Shared biquad filter math. Used by both the EQ effect (real-time)
/// and the effect editor UI (frequency response graph).

#[derive(Clone)]
pub enum BiquadType {
    LowShelf,
    Peaking,
    HighShelf,
}

#[derive(Clone)]
pub struct BiquadCoeffs {
    pub b0: f32, pub b1: f32, pub b2: f32,
    pub a1: f32, pub a2: f32,
}

#[derive(Clone)]
pub struct BiquadState {
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl BiquadState {
    pub fn new() -> Self { Self { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 } }
    pub fn reset(&mut self) { self.x1 = 0.0; self.x2 = 0.0; self.y1 = 0.0; self.y2 = 0.0; }
}

impl Default for BiquadState {
    fn default() -> Self { Self::new() }
}

pub fn compute_coeffs(btype: &BiquadType, freq: f32, gain_db: f32, q: f32, sr: f32) -> BiquadCoeffs {
    let freq_clamped = freq.min(sr * 0.499);
    let w0 = 2.0 * std::f32::consts::PI * freq_clamped / sr;
    let cos_w = w0.cos();
    let sin_w = w0.sin();
    let alpha = sin_w / (2.0 * q);
    let a = 10.0f32.powf(gain_db / 40.0);
    let sqrt_a = a.sqrt();

    match btype {
        BiquadType::LowShelf => {
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha);
            let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w);
            let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha);
            let a0 = (a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha;
            let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w);
            let a2 = (a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha;
            BiquadCoeffs { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
        }
        BiquadType::Peaking => {
            let b0 = 1.0 + alpha * a;
            let b1 = -2.0 * cos_w;
            let b2 = 1.0 - alpha * a;
            let a0 = 1.0 + alpha / a;
            let a1 = -2.0 * cos_w;
            let a2 = 1.0 - alpha / a;
            BiquadCoeffs { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
        }
        BiquadType::HighShelf => {
            let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
            let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w + two_sqrt_a_alpha);
            let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w);
            let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w - two_sqrt_a_alpha);
            let a0 = (a + 1.0) - (a - 1.0) * cos_w + two_sqrt_a_alpha;
            let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w);
            let a2 = (a + 1.0) - (a - 1.0) * cos_w - two_sqrt_a_alpha;
            BiquadCoeffs { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
        }
    }
}

pub fn process_biquad(ch: &mut [f32], state: &mut BiquadState, c: &BiquadCoeffs) {
    for sample in ch.iter_mut() {
        let x = *sample;
        let y = c.b0 * x + c.b1 * state.x1 + c.b2 * state.x2 - c.a1 * state.y1 - c.a2 * state.y2;
        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;
        *sample = y;
    }
}

/// Compute the magnitude response in dB for a biquad filter at a given frequency.
pub fn frequency_response(c: &BiquadCoeffs, freq_hz: f32, sr: f32) -> f32 {
    if sr <= 0.0 { return 0.0; }
    let w = 2.0 * std::f32::consts::PI * freq_hz / sr;
    let cos_w = w.cos();
    let sin_w = w.sin();
    let re_num = c.b0 + c.b1 * cos_w + c.b2 * (2.0 * cos_w * cos_w - 1.0);
    let im_num = c.b1 * sin_w + c.b2 * 2.0 * sin_w * cos_w;
    let re_den = 1.0 + c.a1 * cos_w + c.a2 * (2.0 * cos_w * cos_w - 1.0);
    let im_den = c.a1 * sin_w + c.a2 * 2.0 * sin_w * cos_w;
    let mag_num = ((re_num * re_num + im_num * im_num) as f64).sqrt() as f32;
    let mag_den = (re_den * re_den + im_den * im_den) as f64;
    let mag_den = mag_den.sqrt() as f32;
    let mag = if mag_den > 0.0 { mag_num / mag_den } else { 1.0 };
    20.0 * mag.log10()
}
