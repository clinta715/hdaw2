use crate::project::midi_note::MidiNote;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

pub struct ClipHandle {
    pub clip_id: Uuid,
    pub position_frames: AtomicU64,
    pub offset_frames: AtomicU64,
    pub length_frames: AtomicU64,
    pub gain: Arc<AtomicU32>,
    pub audio_data: Arc<Vec<f32>>,
    pub channels: u16,
    pub sample_rate: u32,
    pub midi_notes: Vec<MidiNote>,
}

impl ClipHandle {
    pub fn new(clip_id: Uuid, audio_data: Vec<f32>, channels: u16, sample_rate: u32) -> Self {
        let frames = audio_data.len() / channels as usize;
        Self {
            clip_id,
            position_frames: AtomicU64::new(0),
            offset_frames: AtomicU64::new(0),
            length_frames: AtomicU64::new(frames as u64),
            gain: Arc::new(AtomicU32::new(f32::to_bits(1.0))),
            audio_data: Arc::new(audio_data),
            channels,
            sample_rate,
            midi_notes: Vec::new(),
        }
    }

    pub fn new_midi(clip_id: Uuid, notes: Vec<MidiNote>, length: u64, sample_rate: u32) -> Self {
        Self {
            clip_id,
            position_frames: AtomicU64::new(0),
            offset_frames: AtomicU64::new(0),
            length_frames: AtomicU64::new(length),
            gain: Arc::new(AtomicU32::new(f32::to_bits(1.0))),
            audio_data: Arc::new(Vec::new()),
            channels: 0,
            sample_rate,
            midi_notes: notes,
        }
    }

    pub fn frames(&self) -> usize {
        if !self.midi_notes.is_empty() {
            return self.length_frames.load(Ordering::Acquire) as usize;
        }
        self.audio_data.len() / self.channels as usize
    }

    pub fn set_position(&self, frames: u64) {
        self.position_frames.store(frames, Ordering::Release);
    }

    pub fn set_offset(&self, frames: u64) {
        self.offset_frames.store(frames, Ordering::Release);
    }

    pub fn set_length(&self, frames: u64) {
        self.length_frames.store(frames, Ordering::Release);
    }
}
