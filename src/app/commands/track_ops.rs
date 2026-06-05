use crate::app::undo::UndoCommand;
use crate::app::{HdawApp, TrackUiState};
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::clip::ClipKind;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl HdawApp {
    pub fn toggle_track_mute(&mut self, track_index: usize) {
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                let old = track.mute.load(Ordering::Acquire);
                let new = !old;
                track.mute.store(new, Ordering::Release);
                if let Some(pt) = self.project.tracks.get_mut(track_index) {
                    pt.mute = new;
                }
                self.undo_service.push(UndoCommand::ToggleMute {
                    track_index,
                    old_value: old,
                });
            }
        }
    }

    pub fn toggle_track_arm(&mut self, track_index: usize) {
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                let old = track.armed.load(Ordering::Acquire);
                track.armed.store(!old, Ordering::Release);
            }
        }
        if let Some(tui) = self.track_ui.get(track_index) {
            let old = tui.armed.load(Ordering::Acquire);
            tui.armed.store(!old, Ordering::Release);
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
                self.undo_service.push(UndoCommand::ToggleSolo {
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

    pub fn add_instrument_track(&mut self, desc: &crate::audio::clap_scanner::PluginDescriptor) {
        let name = desc.name.to_string();
        let sr = self.engine.transport.sample_rate();
        let adapter = match crate::audio::clap_effect::ClapEffectAdapter::new_instance(&desc.id, &desc.path, sr) {
            Ok(a) => a,
            Err(e) => {
                self.error_message = Some(format!("Failed to load instrument {}: {}", desc.name, e));
                return;
            }
        };
        let etype = EffectType::Clap {
            plugin_id: desc.id.clone(),
            path: desc.path.to_string_lossy().into_owned(),
        };
        let instance = EffectInstance::new_clap(desc.name.clone(), etype, adapter);

        let mut handle = crate::project::track::TrackHandle::new();
        handle.add_effect(instance);

        let track_ui = TrackUiState {
            id: handle.id,
            name: name.clone(),
            color: [0x2a, 0x1a, 0x2a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            armed: handle.armed.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
            parent_group: None,
            is_group: false,
            is_return: false,
            collapsed: false,
            send_levels: Vec::new(),
        };

        let mut track = crate::project::track::Track::new(name);
        track.color = track_ui.color;

        self.track_ui.push(track_ui.clone());
        self.engine.add_track(handle);
        self.project.add_track(track.clone());

        let new_index = self.track_ui.len() - 1;
        self.selected_track = Some(new_index);
        self.effect_editor_state.selected_track = Some(new_index);
        self.effect_editor_state.show_editor = true;

        self.undo_service.push(UndoCommand::AddTrack {
            track_index: new_index,
            track,
            track_ui,
        });
    }

    pub fn add_blank_track(&mut self) {
        let track_count = self.project.tracks.len();
        let name = format!("Track {}", track_count + 1);

        let mut handle = crate::project::track::TrackHandle::new();
        let mut track_ui = TrackUiState {
            id: handle.id,
            name: name.clone(),
            color: [0x1a, 0x2a, 0x1a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            armed: handle.armed.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
            parent_group: None,
            is_group: false,
            is_return: false,
            collapsed: false,
            send_levels: Vec::new(),
        };

        let mut track = crate::project::track::Track::new(name);
        track.color = track_ui.color;

        for rt in self.project.tracks.iter().filter(|t| t.is_return) {
            let sid = rt.id;
            handle.sends.push(crate::project::track::SendSlot::new(sid, 0.0, false));
            track_ui.send_levels.push(Arc::new(std::sync::atomic::AtomicU32::new(f32::to_bits(0.0))));
            track.sends.push(crate::project::track::SendSlotDef {
                target_id: sid,
                level: 0.0,
                pre_fader: false,
            });
        }

        self.track_ui.push(track_ui.clone());
        self.engine.add_track(handle);
        self.project.add_track(track.clone());

        let new_index = self.track_ui.len() - 1;
        self.selected_track = Some(new_index);
        self.effect_editor_state.selected_track = Some(new_index);

        self.undo_service.push(UndoCommand::AddTrack {
            track_index: new_index,
            track,
            track_ui,
        });
    }

    pub fn add_group_track(&mut self) {
        let track_count = self.project.tracks.len();
        let name = format!("Group {}", track_count + 1);

        let handle = crate::project::track::TrackHandle::new_group();
        let track_ui = TrackUiState {
            id: handle.id,
            name: name.clone(),
            color: [0x3a, 0x3a, 0x2a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            armed: handle.armed.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
            parent_group: None,
            is_group: true,
            is_return: false,
            collapsed: false,
            send_levels: Vec::new(),
        };

        let track = crate::project::track::Track::new_group(name);

        self.track_ui.push(track_ui.clone());
        self.engine.add_track(handle);
        self.project.add_track(track.clone());

        let new_index = self.track_ui.len() - 1;
        self.selected_track = Some(new_index);

        self.undo_service.push(UndoCommand::AddTrack {
            track_index: new_index,
            track,
            track_ui,
        });
    }

    pub fn add_return_track(&mut self) {
        let track_count = self.project.tracks.len();
        let name = format!("Return {}", track_count + 1);

        let handle = crate::project::track::TrackHandle::new_return();
        let track_ui = TrackUiState {
            id: handle.id,
            name: name.clone(),
            color: [0x3a, 0x2a, 0x3a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            armed: handle.armed.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
            parent_group: None,
            is_group: false,
            is_return: true,
            collapsed: false,
            send_levels: Vec::new(),
        };

        let track = crate::project::track::Track::new_return(name);

        let return_id = handle.id;
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            for t in tracks.iter_mut() {
                t.sends.push(crate::project::track::SendSlot::new(return_id, 0.0, false));
            }
        }
        for tui in self.track_ui.iter_mut() {
            tui.send_levels.push(Arc::new(std::sync::atomic::AtomicU32::new(f32::to_bits(0.0))));
        }
        for t in self.project.tracks.iter_mut() {
            t.sends.push(crate::project::track::SendSlotDef {
                target_id: return_id,
                level: 0.0,
                pre_fader: false,
            });
        }

        self.track_ui.push(track_ui.clone());
        self.engine.add_track(handle);
        self.project.add_track(track.clone());

        let new_index = self.track_ui.len() - 1;
        self.selected_track = Some(new_index);

        self.undo_service.push(UndoCommand::AddTrack {
            track_index: new_index,
            track,
            track_ui,
        });
    }

    pub fn delete_track(&mut self, track_index: usize) {
        if track_index >= self.project.tracks.len() {
            return;
        }

        let track = self.project.tracks.get(track_index).cloned();
        let track_ui = self.track_ui.get(track_index).cloned();

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if track_index < tracks.len() {
                tracks.remove(track_index);
            }
        }

        if let Some(track) = &track {
            for clip in &track.clips {
                let id = match clip {
                    ClipKind::Audio(a) => a.id,
                    ClipKind::Midi(m) => m.id,
                };
                self.waveform_cache.remove(&id);
                self.midi_thumb_cache.remove(&id);
                let pool_clip = crate::project::pool::PoolClip::from_clip(clip.clone());
                self.project.audio_pool.push(pool_clip);
            }
        }

        self.project.remove_track(track_index);

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
            for track in self.project.tracks.iter() {
                if track.clips.iter().any(|c| match c {
                    ClipKind::Audio(a) => a.id == selected,
                    ClipKind::Midi(m) => m.id == selected,
                }) {
                    found = true;
                    break;
                }
            }
            if !found {
                self.timeline_state.selected_clip_id = None;
            }
        }

        if let (Some(track), Some(track_ui)) = (track, track_ui) {
            self.undo_service.push(UndoCommand::DeleteTrack {
                track_index,
                track,
                track_ui,
            });
        }
    }
}
