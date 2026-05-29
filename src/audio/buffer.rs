use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    samples: Arc<Vec<f32>>,
    channels: u16,
    sample_rate: u32,
}

impl AudioBuffer {
    pub fn from_interleaved(samples: Vec<f32>, channels: u16, sample_rate: u32) -> Self {
        Self {
            samples: Arc::new(samples),
            channels,
            sample_rate,
        }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn samples(&self) -> &Arc<Vec<f32>> {
        &self.samples
    }

}
