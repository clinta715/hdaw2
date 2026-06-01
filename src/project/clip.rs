use crate::audio::buffer::AudioBuffer;
use crate::project::midi_clip::MidiClip;
use crate::utils::waveform::WaveformPeaks;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClip {
    pub id: Uuid,
    pub name: String,
    pub position_frames: u64,
    pub offset_frames: u64,
    pub length_frames: u64,
    pub gain: f32,
    #[serde(default)]
    pub fade_in_frames: u64,
    #[serde(default)]
    pub fade_out_frames: u64,
    pub source_path: Option<PathBuf>,
    #[serde(skip)]
    pub buffer: Option<AudioBuffer>,
    #[serde(skip)]
    pub waveform_peaks: Option<Arc<WaveformPeaks>>,
}

impl AudioClip {
    pub fn new(name: String, buffer: AudioBuffer) -> Self {
        let frames = buffer.frames() as u64;
        let peaks = Self::generate_peaks(&buffer);
        Self {
            id: Uuid::new_v4(),
            name,
            position_frames: 0,
            offset_frames: 0,
            length_frames: frames,
            gain: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            source_path: None,
            buffer: Some(buffer),
            waveform_peaks: Some(Arc::new(peaks)),
        }
    }

    pub fn with_source_path(name: String, buffer: AudioBuffer, source_path: PathBuf) -> Self {
        let frames = buffer.frames() as u64;
        let peaks = Self::generate_peaks(&buffer);
        Self {
            id: Uuid::new_v4(),
            name,
            position_frames: 0,
            offset_frames: 0,
            length_frames: frames,
            gain: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            source_path: Some(source_path),
            buffer: Some(buffer),
            waveform_peaks: Some(Arc::new(peaks)),
        }
    }

    fn generate_peaks(buffer: &AudioBuffer) -> WaveformPeaks {
        let samples = buffer.samples();
        WaveformPeaks::from_samples(
            samples,
            buffer.channels(),
            buffer.sample_rate(),
            buffer.frames(),
        )
    }

    pub fn reload_buffer(&mut self) -> Result<(), String> {
        if self.buffer.is_some() {
            return Ok(());
        }
        let path = match &self.source_path {
            Some(p) => p,
            None => return Err("no source path for clip".into()),
        };
        let path_str = path.to_str().ok_or("invalid source path")?;
        let (samples, channels, sample_rate) = crate::utils::load_wav_file(path_str)?;
        let buffer = AudioBuffer::from_interleaved(samples, channels, sample_rate);
        self.waveform_peaks = Some(Arc::new(Self::generate_peaks(&buffer)));
        self.buffer = Some(buffer);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipKind {
    Audio(AudioClip),
    Midi(MidiClip),
}
