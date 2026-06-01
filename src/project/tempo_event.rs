use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoEvent {
    pub position_frames: u64,
    pub tempo: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSigEvent {
    pub position_frames: u64,
    pub numerator: u8,
    pub denominator: u8,
}

pub fn tempo_at(events: &[TempoEvent], position_frames: u64) -> f64 {
    events
        .iter()
        .rev()
        .find(|e| e.position_frames <= position_frames)
        .map(|e| e.tempo)
        .unwrap_or(120.0)
}

pub fn time_sig_at(events: &[TimeSigEvent], position_frames: u64) -> (u8, u8) {
    events
        .iter()
        .rev()
        .find(|e| e.position_frames <= position_frames)
        .map(|e| (e.numerator, e.denominator))
        .unwrap_or((4, 4))
}

pub fn frames_to_beats(target_frames: u64, events: &[TempoEvent], sample_rate: u32) -> f64 {
    let sr = sample_rate as f64;
    let mut total_beats = 0.0;
    let mut prev_frame = 0u64;
    let mut tempo = 120.0;

    for event in events {
        if event.position_frames >= target_frames {
            let delta = (target_frames - prev_frame) as f64;
            total_beats += delta * tempo / (60.0 * sr);
            return total_beats;
        }
        let delta = (event.position_frames - prev_frame) as f64;
        total_beats += delta * tempo / (60.0 * sr);
        tempo = event.tempo;
        prev_frame = event.position_frames;
    }

    let delta = (target_frames - prev_frame) as f64;
    total_beats += delta * tempo / (60.0 * sr);
    total_beats
}

pub fn beats_to_frames(target_beats: f64, events: &[TempoEvent], sample_rate: u32) -> u64 {
    let sr = sample_rate as f64;
    let mut accumulated_beats = 0.0;
    let mut prev_frame = 0u64;
    let mut tempo = 120.0;

    for event in events {
        let segment_secs = (event.position_frames - prev_frame) as f64 / sr;
        let segment_beats = segment_secs * tempo / 60.0;
        if accumulated_beats + segment_beats >= target_beats {
            let remaining = target_beats - accumulated_beats;
            let remaining_secs = remaining * 60.0 / tempo;
            let frame_delta = (remaining_secs * sr).round() as u64;
            return prev_frame + frame_delta;
        }
        accumulated_beats += segment_beats;
        tempo = event.tempo;
        prev_frame = event.position_frames;
    }

    let remaining = target_beats - accumulated_beats;
    let remaining_secs = remaining * 60.0 / tempo;
    prev_frame + (remaining_secs * sr).round() as u64
}

pub fn segment_tempo(events: &[TempoEvent], frame: u64) -> f64 {
    tempo_at(events, frame)
}
