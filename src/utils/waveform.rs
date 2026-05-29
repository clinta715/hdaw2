use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Peak {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone)]
pub struct WaveformPeaks {
    pub peaks: Arc<Vec<Peak>>,
    pub total_frames: usize,
    pub channels: u16,
    pub sample_rate: u32,
}

impl WaveformPeaks {
    pub fn from_samples(samples: &[f32], channels: u16, sample_rate: u32, target_pixels: usize) -> Self {
        let total_frames = samples.len() / channels as usize;
        let peaks = compute_peaks(samples, channels, target_pixels);
        Self {
            peaks: Arc::new(peaks),
            total_frames,
            channels,
            sample_rate,
        }
    }

    pub fn len(&self) -> usize {
        self.peaks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peaks.is_empty()
    }
}

fn compute_peaks(samples: &[f32], channels: u16, target_pixels: usize) -> Vec<Peak> {
    let frames = samples.len() / channels as usize;
    if frames == 0 || target_pixels == 0 {
        return Vec::new();
    }

    let mut peaks = Vec::with_capacity(target_pixels);
    let frames_per_pixel = (frames as f64 / target_pixels as f64).max(1.0);

    let mut pixel_start = 0usize;
    for _ in 0..target_pixels {
        let pixel_end = (pixel_start as f64 + frames_per_pixel).round() as usize;
        let pixel_end = pixel_end.min(frames);

        let mut min = 1.0f32;
        let mut max = -1.0f32;
        for f in pixel_start..pixel_end {
            let mut frame_sum = 0.0f32;
            for ch in 0..channels {
                let idx = f * channels as usize + ch as usize;
                if idx < samples.len() {
                    frame_sum += samples[idx];
                }
            }
            let sample = frame_sum / channels as f32;
            min = min.min(sample);
            max = max.max(sample);
        }

        peaks.push(Peak { min, max });
        pixel_start = pixel_end;
    }

    peaks
}
