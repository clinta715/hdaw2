use crate::app::undo::UndoCommand;
use crate::app::{HdawApp, TrackUiState};
use crate::project::clip::AudioClip;
use crate::project::clip_handle::ClipHandle;
use std::sync::atomic::Ordering;

impl HdawApp {
    pub fn update_clip_position(&mut self, track_index: usize, clip_id: uuid::Uuid, new_position: u64) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                clip.position_frames = new_position;
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    ch.set_position(new_position);
                }
            }
        }
    }

    pub fn update_clip_trim(&mut self, track_index: usize, clip_id: uuid::Uuid, position: Option<u64>, offset: Option<u64>, length: Option<u64>) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(clip) = track.clips.iter_mut().find(|c| c.id == clip_id) {
                if let Some(p) = position { clip.position_frames = p; }
                if let Some(o) = offset { clip.offset_frames = o; }
                if let Some(l) = length { clip.length_frames = l; }
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    if let Some(p) = position { ch.set_position(p); }
                    if let Some(o) = offset { ch.set_offset(o); }
                    if let Some(l) = length { ch.set_length(l); }
                }
            }
        }
    }

    pub fn remove_selected_clip(&mut self) {
        let clip_id = match self.timeline_state.selected_clip_id {
            Some(id) => id,
            None => return,
        };
        for ti in 0..self.project.tracks.len() {
            if let Some(track) = self.project.tracks.get(ti) {
                if let Some(ci) = track.clips.iter().position(|c| c.id == clip_id) {
                    let clip = track.clips[ci].clone();
                    if let Some(track) = self.project.tracks.get_mut(ti) {
                        track.clips.remove(ci);
                    }
                    if let Ok(mut tracks) = self.engine.tracks.lock() {
                        if let Some(handle) = tracks.get_mut(ti) {
                            if let Some(eng_idx) = handle.find_clip_by_id(clip_id) {
                                handle.clips.remove(eng_idx);
                            }
                        }
                    }
                    self.undo_state.push(UndoCommand::DeleteClip {
                        track_index: ti,
                        clip_index: ci,
                        clip,
                    });
                    break;
                }
            }
        }
        self.timeline_state.selected_clip_id = None;
    }

    pub fn toggle_track_mute(&mut self, track_index: usize) {
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                let old = track.mute.load(Ordering::Acquire);
                let new = !old;
                track.mute.store(new, Ordering::Release);
                if let Some(pt) = self.project.tracks.get_mut(track_index) {
                    pt.mute = new;
                }
                self.undo_state.push(UndoCommand::ToggleMute {
                    track_index,
                    old_value: old,
                });
            }
        }
    }

    pub fn toggle_track_solo(&mut self, track_index: usize) {
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                let old = track.solo.load(Ordering::Acquire);
                let new = !old;
                track.solo.store(new, Ordering::Release);
                if let Some(pt) = self.project.tracks.get_mut(track_index) {
                    pt.solo = new;
                }
                self.undo_state.push(UndoCommand::ToggleSolo {
                    track_index,
                    old_value: old,
                });
            }
        }
    }

    pub fn select_track(&mut self, track_index: usize) {
        self.selected_track = Some(track_index);
        self.effect_editor_state.selected_track = Some(track_index);
    }

    pub fn add_blank_track(&mut self) {
        let track_count = self.project.tracks.len();
        let name = format!("Track {}", track_count + 1);

        let handle = crate::project::track::TrackHandle::new();
        let track_ui = TrackUiState {
            name: name.clone(),
            color: [0x1a, 0x2a, 0x1a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
        };

        let mut track = crate::project::track::Track::new(name);
        track.color = track_ui.color;

        self.track_ui.push(track_ui);
        self.engine.add_track(handle);
        self.project.add_track(track);

        let new_index = self.track_ui.len() - 1;
        self.selected_track = Some(new_index);
        self.effect_editor_state.selected_track = Some(new_index);
    }

    pub fn delete_track(&mut self, track_index: usize) {
        if track_index >= self.project.tracks.len() {
            return;
        }

        let track = self.project.tracks.get(track_index).cloned();

        if let Some(track) = track {
            for clip in &track.clips {
                let pool_clip = crate::project::pool::PoolClip::from_clip(clip.clone());
                self.project.audio_pool.push(pool_clip);
            }
        }

        self.project.remove_track(track_index);

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if track_index < tracks.len() {
                tracks.remove(track_index);
            }
        }

        if track_index < self.track_ui.len() {
            self.track_ui.remove(track_index);
        }

        if let Some(selected) = self.selected_track {
            if selected == track_index {
                self.selected_track = None;
                self.effect_editor_state.selected_track = None;
            } else if selected > track_index {
                self.selected_track = Some(selected - 1);
                self.effect_editor_state.selected_track = Some(selected - 1);
            }
        }

        if let Some(selected) = self.timeline_state.selected_clip_id {
            let mut found = false;
            for (_ti, track) in self.project.tracks.iter().enumerate() {
                if track.clips.iter().any(|c| c.id == selected) {
                    found = true;
                    break;
                }
            }
            if !found {
                self.timeline_state.selected_clip_id = None;
            }
        }
    }

    pub fn restore_pool_clip_to_track(&mut self, pool_clip_id: uuid::Uuid, track_index: usize) {
        self.restore_pool_clip_to_track_at(pool_clip_id, track_index, 0);
    }

    pub fn restore_pool_clip_to_track_at(&mut self, pool_clip_id: uuid::Uuid, track_index: usize, position_frames: u64) {
        let pool_idx = self.project.audio_pool.iter()
            .position(|p| p.id == pool_clip_id);
        let pool_clip = match pool_idx {
            Some(idx) => self.project.audio_pool.remove(idx),
            None => return,
        };

        let buffer = match pool_clip.clip.buffer.clone() {
            Some(buf) => buf,
            None => return,
        };
        let clip_id = pool_clip.clip.id;

        let clip_handle = ClipHandle::new(
            clip_id,
            (*buffer.samples()).to_vec(),
            buffer.channels(),
            buffer.sample_rate(),
        );
        clip_handle.set_position(position_frames);

        let mut audio_clip = AudioClip::new(pool_clip.name.clone(), buffer);
        audio_clip.position_frames = position_frames;

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(audio_clip);
        }

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.add_clip(clip_handle);
            }
        }
    }
}
