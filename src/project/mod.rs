pub mod automation;
pub mod clip;
pub mod clip_handle;
pub mod marker;
pub mod midi_clip;
pub mod midi_note;
pub mod pool;
pub mod track;

use marker::Marker;
use serde::{Deserialize, Serialize};
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
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}
