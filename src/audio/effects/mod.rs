pub mod chorus;
pub mod compressor;
pub mod delay;
pub mod distortion;
pub mod dsp_effect;
pub mod eq;
pub mod flanger;
pub mod gain;
pub mod parameter;
pub mod phaser;
pub mod reverb;

use dsp_effect::{DspEffect, EffectType};

pub fn create_effect(etype: EffectType) -> Box<dyn DspEffect> {
    match etype {
        EffectType::Gain => Box::new(gain::GainEffect::new()),
        EffectType::Equalizer => Box::new(eq::EqEffect::new()),
        EffectType::Compressor => Box::new(compressor::CompressorEffect::new()),
        EffectType::Reverb => Box::new(reverb::ReverbEffect::new()),
        EffectType::Delay => Box::new(delay::DelayEffect::new()),
        EffectType::Chorus => Box::new(chorus::ChorusEffect::new()),
        EffectType::Flanger => Box::new(flanger::FlangerEffect::new()),
        EffectType::Phaser => Box::new(phaser::PhaserEffect::new()),
        EffectType::Distortion => Box::new(distortion::DistortionEffect::new()),
        EffectType::Clap { .. } => panic!("CLAP effects must be created via ClapEffectAdapter, not create_effect"),
    }
}
