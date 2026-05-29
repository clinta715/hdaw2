pub mod compressor;
pub mod delay;
pub mod dsp_effect;
pub mod eq;
pub mod gain;
pub mod parameter;
pub mod reverb;

use dsp_effect::{DspEffect, EffectType};

pub fn create_effect(etype: EffectType) -> Box<dyn DspEffect> {
    match etype {
        EffectType::Gain => Box::new(gain::GainEffect::new()),
        EffectType::Equalizer => Box::new(eq::EqEffect::new()),
        EffectType::Compressor => Box::new(compressor::CompressorEffect::new()),
        EffectType::Reverb => Box::new(reverb::ReverbEffect::new()),
        EffectType::Delay => Box::new(delay::DelayEffect::new()),
    }
}
