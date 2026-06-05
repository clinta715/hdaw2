pub mod automation;
pub mod cc_event;
pub mod clip;
pub mod clip_handle;
pub mod marker;
pub mod midi_clip;
pub mod midi_import;
pub mod midi_note;
pub mod pool;
pub mod tempo_event;
pub mod track;

use marker::Marker;
use serde::{Deserialize, Serialize};
use tempo_event::{TempoEvent, TimeSigEvent};
use track::Track;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub sample_rate: u32,
    pub bpm: f64,
    pub time_signature_num: u8,
    pub time_signature_den: u8,
    pub tracks: Vec<Track>,
    pub markers: Vec<Marker>,
    pub audio_pool: Vec<pool::PoolClip>,
    #[serde(default)]
    pub tempo_events: Vec<TempoEvent>,
    #[serde(default)]
    pub time_sig_events: Vec<TimeSigEvent>,
    #[serde(default)]
    pub loop_in_frames: u64,
    #[serde(default)]
    pub loop_out_frames: u64,
    #[serde(default)]
    pub loop_enabled: bool,
}

impl Project {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::from("Untitled"),
            sample_rate: 44100,
            bpm: 120.0,
            time_signature_num: 4,
            time_signature_den: 4,
            tracks: Vec::new(),
            markers: Vec::new(),
            audio_pool: Vec::new(),
            tempo_events: Vec::new(),
            time_sig_events: Vec::new(),
            loop_in_frames: 0,
            loop_out_frames: 0,
            loop_enabled: false,
        }
    }

    pub fn add_track(&mut self, track: Track) {
        self.tracks.push(track);
    }

    pub fn remove_track(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.tracks.remove(index);
        }
    }

    pub fn tempo_at(&self, position_frames: u64) -> f64 {
        crate::project::tempo_event::tempo_at(&self.tempo_events, position_frames)
    }

    pub fn time_sig_at(&self, position_frames: u64) -> (u8, u8) {
        crate::project::tempo_event::time_sig_at(&self.time_sig_events, position_frames)
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}
