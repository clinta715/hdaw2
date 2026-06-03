pub mod waveform;
pub mod native_window;

pub fn load_wav_file(path: &str) -> Result<(Vec<f32>, u16, u32), String> {
    let reader = hound::WavReader::open(path).map_err(|e| format!("failed to open: {e}"))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect()
        }
        hound::SampleFormat::Int => {
            let max = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max)
                .collect()
        }
    };

    tracing::info!(
        "loaded {}: {} frames, {} channels, {} Hz",
        path,
        samples.len() / channels as usize,
        channels,
        sample_rate
    );

    Ok((samples, channels, sample_rate))
}
