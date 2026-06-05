use crate::app::HdawApp;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use uuid::Uuid;

impl HdawApp {
    pub fn set_track_parent(&mut self, track_idx: usize, parent_id: Option<Uuid>) -> bool {
        if let Some(pid) = parent_id {
            let track_id = if let Some(tui) = self.track_ui.get(track_idx) {
                tui.id
            } else {
                return false;
            };
            let mut visited = HashSet::new();
            let mut cursor = pid;
            loop {
                if cursor == track_id {
                    self.error_message = Some("Cannot route: would create a cycle".to_string());
                    return false;
                }
                if !visited.insert(cursor) {
                    break;
                }
                let found = self.track_ui.iter().find(|t| t.id == cursor)
                    .and_then(|t| t.parent_group);
                match found {
                    Some(next) => cursor = next,
                    None => break,
                }
            }
        }

        if let Some(tui) = self.track_ui.get_mut(track_idx) {
            tui.parent_group = parent_id;
        }
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            track.parent_group = parent_id;
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                handle.parent_group = parent_id;
            }
        }
        true
    }

    pub fn set_send_level(&mut self, track_idx: usize, send_idx: usize, level: f32) {
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                if let Some(slot) = handle.sends.get(send_idx) {
                    slot.level.store(level.to_bits(), Ordering::Release);
                }
            }
        }
        if let Some(tui) = self.track_ui.get_mut(track_idx) {
            if let Some(al) = tui.send_levels.get(send_idx) {
                al.store(level.to_bits(), Ordering::Release);
            }
        }
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            if let Some(slot) = track.sends.get_mut(send_idx) {
                slot.level = level;
            }
        }
    }

    pub fn set_send_pre_fader(&mut self, track_idx: usize, send_idx: usize, pre_fader: bool) {
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_idx) {
                if let Some(slot) = handle.sends.get_mut(send_idx) {
                    slot.pre_fader = pre_fader;
                }
            }
        }
        if let Some(track) = self.project.tracks.get_mut(track_idx) {
            if let Some(slot) = track.sends.get_mut(send_idx) {
                slot.pre_fader = pre_fader;
            }
        }
    }
}
