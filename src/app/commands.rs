use crate::app::undo::UndoCommand;
use crate::app::{HdawApp, TrackUiState};
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::clip_handle::ClipHandle;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use uuid::Uuid;

impl HdawApp {
    pub fn update_clip_position(&mut self, track_index: usize, clip_id: uuid::Uuid, new_position: u64) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => a.position_frames = new_position,
                    ClipKind::Midi(m) if m.id == clip_id => m.position_frames = new_position,
                    _ => continue,
                }
                break;
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

    pub fn move_clip_to_track(&mut self, clip_id: uuid::Uuid, from: usize, to: usize, position: u64) {
        // Remove from source and capture the clip
        let removed = if let Some(track) = self.project.tracks.get_mut(from) {
            let idx = track.clips.iter().position(|c| match c {
                ClipKind::Audio(a) => a.id == clip_id,
                ClipKind::Midi(m) => m.id == clip_id,
            });
            idx.map(|i| track.clips.remove(i))
        } else { None };
        // Add to destination
        if let Some(mut c) = removed {
            match &mut c {
                ClipKind::Audio(a) => a.position_frames = position,
                ClipKind::Midi(m) => m.position_frames = position,
            }
            if let Some(track) = self.project.tracks.get_mut(to) {
                track.add_clip(c);
            }
        }
        // Engine model: find clip by ID, remove from source, add to target
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            let clip_handle = if from < tracks.len() {
                let pos = tracks[from].clips.iter().position(|c| c.clip_id == clip_id);
                pos.map(|p| tracks[from].clips.remove(p))
            } else { None };
            if let Some(mut ch) = clip_handle {
                ch.set_position(position);
                if to < tracks.len() {
                    tracks[to].clips.push(ch);
                }
            }
        }
    }

    pub fn update_clip_trim(&mut self, track_index: usize, clip_id: uuid::Uuid, position: Option<u64>, offset: Option<u64>, length: Option<u64>) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => {
                        if let Some(p) = position { a.position_frames = p; }
                        if let Some(o) = offset { a.offset_frames = o; }
                        if let Some(l) = length { a.length_frames = l; }
                    }
                    ClipKind::Midi(m) if m.id == clip_id => {
                        if let Some(p) = position { m.position_frames = p; }
                        if let Some(l) = length { m.length_frames = l; }
                    }
                    _ => continue,
                }
                break;
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
                if let Some(ci) = track.clips.iter().position(|c| match c {
                    ClipKind::Audio(a) => a.id == clip_id,
                    ClipKind::Midi(m) => m.id == clip_id,
                }) {
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
                    self.waveform_cache.remove(&clip_id);
                    self.midi_thumb_cache.remove(&clip_id);
                    // Strip audio buffer from undo — the buffer stays in the pool entry
                    let undo_clip = match &clip {
                        ClipKind::Audio(a) => {
                            let mut stripped = a.clone();
                            stripped.buffer = None;
                            ClipKind::Audio(stripped)
                        }
                        other => other.clone(),
                    };
                    self.undo_service.push(UndoCommand::DeleteClip {
                        track_index: ti,
                        clip_index: ci,
                        clip: undo_clip,
                    });
                    break;
                }
            }
        }
        self.timeline_state.selected_clip_id = None;
    }

    pub fn split_selected_clip(&mut self) {
        let clip_id = match self.timeline_state.selected_clip_id {
            Some(id) => id,
            None => return,
        };
        let pos = self.engine.transport.position_frames();
        for ti in 0..self.project.tracks.len() {
            if self.project.tracks[ti].clips.iter().any(|c| match c {
                crate::project::clip::ClipKind::Audio(a) => a.id == clip_id,
                crate::project::clip::ClipKind::Midi(m) => m.id == clip_id,
            }) {
                self.split_clip_at(ti, clip_id, pos);
                return;
            }
        }
    }

    pub fn update_clip_fade(&mut self, track_index: usize, clip_id: uuid::Uuid, fade_in: Option<u64>, fade_out: Option<u64>) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            for clip in track.clips.iter_mut() {
                match clip {
                    ClipKind::Audio(a) if a.id == clip_id => {
                        if let Some(fi) = fade_in { a.fade_in_frames = fi; }
                        if let Some(fo) = fade_out { a.fade_out_frames = fo; }
                    }
                    _ => continue,
                }
                break;
            }
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            if let Some(track) = tracks.get(track_index) {
                if let Some(ch) = track.clips.iter().find(|c| c.clip_id == clip_id) {
                    if let Some(fi) = fade_in { ch.fade_in_frames.store(fi, Ordering::Release); }
                    if let Some(fo) = fade_out { ch.fade_out_frames.store(fo, Ordering::Release); }
                }
            }
        }
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
        let name = format!("{}", desc.name);
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

    pub fn assign_instrument(&mut self, track_index: usize, desc: &crate::audio::clap_scanner::PluginDescriptor) {
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
        let instance = EffectInstance::new_clap(desc.name.clone(), etype.clone(), adapter);

        let effect_index;
        let serialized;
        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                effect_index = t.fx_chain.len();
                t.add_effect(instance);
                // Convert to serialized effect for project state
                let inst = t.fx_chain.last().unwrap();
                let pv: Vec<f32> = inst.parameter_info().iter()
                    .map(|p| inst.parameter_value(p.id)).collect();
                serialized = crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type.clone(),
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                };
            } else { return; }
        } else { return; }

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            let idx = effect_index.min(track.fx_chain.len());
            track.fx_chain.insert(idx, serialized.clone());
        }

        self.undo_service.push(UndoCommand::AddEffect {
            track_index,
            effect_index,
            serialized,
        });
    }

    pub fn replace_instrument(&mut self, track_index: usize, desc: &crate::audio::clap_scanner::PluginDescriptor) {
        let old_inst_idx: Option<usize>;
        let old_serialized: Option<crate::project::track::SerializedEffect>;
        let old_undo: Option<UndoCommand>;

        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                old_inst_idx = t.fx_chain.iter().position(|e| e.has_note_input);
                if let Some(idx) = old_inst_idx {
                    let inst = &t.fx_chain[idx];
                    let pv: Vec<f32> = inst.parameter_info().iter()
                        .map(|p| inst.parameter_value(p.id)).collect();
                    old_serialized = Some(crate::project::track::SerializedEffect {
                        name: inst.name.clone(),
                        effect_type: inst.effect_type.clone(),
                        bypass: inst.is_bypassed(),
                        param_values: pv,
                    });
                    old_undo = Some(UndoCommand::RemoveEffect {
                        track_index,
                        effect_index: idx,
                        serialized: old_serialized.clone().unwrap(),
                        removed_lanes: Vec::new(),
                    });
                    t.fx_chain.remove(idx);
                } else {
                    old_serialized = None;
                    old_undo = None;
                }
            } else {
                return;
            }
        } else {
            return;
        }

        if let (Some(idx), Some(_)) = (old_inst_idx, &old_serialized) {
            if let Some(track) = self.project.tracks.get_mut(track_index) {
                if idx < track.fx_chain.len() {
                    track.fx_chain.remove(idx);
                }
            }
            self.undo_service.push(old_undo.unwrap());
        }

        // Assign new instrument (uses assign_instrument logic but without pushing a separate undo;
        // we push a single compound undo below)
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
        let instance = EffectInstance::new_clap(desc.name.clone(), etype.clone(), adapter);

        let effect_index;
        let serialized;
        if let Ok(mut ts) = self.engine.tracks.lock() {
            if let Some(t) = ts.get_mut(track_index) {
                effect_index = t.fx_chain.len();
                t.add_effect(instance);
                let inst = t.fx_chain.last().unwrap();
                let pv: Vec<f32> = inst.parameter_info().iter()
                    .map(|p| inst.parameter_value(p.id)).collect();
                serialized = crate::project::track::SerializedEffect {
                    name: inst.name.clone(),
                    effect_type: inst.effect_type.clone(),
                    bypass: inst.is_bypassed(),
                    param_values: pv,
                };
            } else {
                return;
            }
        } else {
            return;
        }

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            let idx = effect_index.min(track.fx_chain.len());
            track.fx_chain.insert(idx, serialized.clone());
        }

        self.undo_service.push(UndoCommand::AddEffect {
            track_index,
            effect_index,
            serialized,
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

        // Add send slots for existing return tracks
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

        // Add send slots on all existing tracks pointing to this return track
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

    pub fn set_track_parent(&mut self, track_idx: usize, parent_id: Option<Uuid>) -> bool {
        // Cycle detection: DFS from parent to see if we'd reach the track
        if let Some(pid) = parent_id {
            let track_id = if let Some(tui) = self.track_ui.get(track_idx) {
                tui.id
            } else {
                return false;
            };
            // Follow parent_group links from pid
            let mut visited = HashSet::new();
            let mut cursor = pid;
            loop {
                if cursor == track_id {
                    self.error_message = Some("Cannot route: would create a cycle".to_string());
                    return false;
                }
                if !visited.insert(cursor) {
                    break; // break cycles in existing data
                }
                // Find the track with this id and get its parent_group
                let found = self.track_ui.iter().find(|t| t.id == cursor)
                    .and_then(|t| t.parent_group);
                match found {
                    Some(next) => cursor = next,
                    None => break,
                }
            }
        }

        // Update all three models
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

    pub fn add_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note: crate::project::midi_note::MidiNote) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                clip.notes.push(note.clone());
                clip.notes.sort_by_key(|n| n.start_frame);
                clip.thumb_dirty = true;
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_notes.push(note.clone());
                    ch.midi_notes.sort_by_key(|n| n.start_frame);
                }
            }
        }
        self.undo_service.push(UndoCommand::AddMidiNote {
            track_index,
            clip_id,
            note,
        });
    }

    pub fn remove_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note_idx: usize) {
        let note = self.project.tracks.get(track_index).and_then(|track| {
            track.clips.iter().find_map(|c| match c {
                ClipKind::Midi(m) if m.id == clip_id => m.notes.get(note_idx).cloned(),
                _ => None,
            })
        });
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                if note_idx < clip.notes.len() {
                    clip.notes.remove(note_idx);
                    clip.thumb_dirty = true;
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if note_idx < ch.midi_notes.len() {
                        ch.midi_notes.remove(note_idx);
                    }
                }
            }
        }
        if let Some(note) = note {
            self.undo_service.push(UndoCommand::RemoveMidiNote {
                track_index,
                clip_id,
                note,
                note_index: note_idx,
            });
        }
    }

    pub fn update_midi_note(&mut self, track_index: usize, clip_id: uuid::Uuid, note_idx: usize, new_note: crate::project::midi_note::MidiNote) {
        let old_note = self.project.tracks.get(track_index).and_then(|track| {
            track.clips.iter().find_map(|c| match c {
                ClipKind::Midi(m) if m.id == clip_id => m.notes.get(note_idx).cloned(),
                _ => None,
            })
        });

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                if note_idx < clip.notes.len() {
                    clip.notes[note_idx] = new_note.clone();
                    clip.thumb_dirty = true;
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if note_idx < ch.midi_notes.len() {
                        ch.midi_notes[note_idx] = new_note.clone();
                    }
                }
            }
        }
        
        if let Some(old) = old_note {
             self.undo_service.push(UndoCommand::UpdateMidiNote {
                track_index,
                clip_id,
                note_index: note_idx,
                old_note: old,
                new_note,
            });
        }
    }

    pub fn add_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, event: crate::project::cc_event::CCEvent) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(crate::project::clip::ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, crate::project::clip::ClipKind::Midi(m) if m.id == clip_id)) {
                clip.cc_events.push(event.clone());
                clip.cc_events.sort_by_key(|e| e.time_frames);
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_cc_events.push(event.clone());
                    ch.midi_cc_events.sort_by_key(|e| e.time_frames);
                }
            }
        }
        self.undo_service.push(UndoCommand::AddCcEvent { track_index, clip_id, event });
    }

    pub fn remove_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, event: &crate::project::cc_event::CCEvent, event_index: usize) {
        let event_clone = event.clone();
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(crate::project::clip::ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, crate::project::clip::ClipKind::Midi(m) if m.id == clip_id)) {
                clip.cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.midi_cc_events.retain(|e| e.cc_number != event.cc_number || e.time_frames != event.time_frames);
                }
            }
        }
        self.undo_service.push(UndoCommand::RemoveCcEvent { track_index, clip_id, event: event_clone, event_index });
    }

    pub fn update_midi_cc_event(&mut self, track_index: usize, clip_id: uuid::Uuid, old_event: &crate::project::cc_event::CCEvent, new_event: crate::project::cc_event::CCEvent) {
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(crate::project::clip::ClipKind::Midi(clip)) = track.clips.iter_mut().find(|c| matches!(c, crate::project::clip::ClipKind::Midi(m) if m.id == clip_id)) {
                if let Some(ctx) = clip.cc_events.iter_mut().find(|e| e.cc_number == old_event.cc_number && e.time_frames == old_event.time_frames) {
                    *ctx = new_event.clone();
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if let Some(ctx) = ch.midi_cc_events.iter_mut().find(|e| e.cc_number == old_event.cc_number && e.time_frames == old_event.time_frames) {
                        *ctx = new_event.clone();
                    }
                }
            }
        }
    }

    pub fn duplicate_clip(&mut self, track_index: usize, clip_id: uuid::Uuid) {
        let Some(track) = self.project.tracks.get(track_index) else { return };
        let Some((clip, new_id)) = track.clips.iter().find_map(|c| match c {
            ClipKind::Audio(a) if a.id == clip_id => {
                let mut new = a.clone();
                let new_id = uuid::Uuid::new_v4();
                new.id = new_id;
                new.position_frames = a.position_frames + a.length_frames;
                Some((ClipKind::Audio(new), new_id))
            }
            ClipKind::Midi(m) if m.id == clip_id => {
                let mut new = m.clone();
                let new_id = uuid::Uuid::new_v4();
                new.id = new_id;
                new.position_frames = m.position_frames + m.length_frames;
                Some((ClipKind::Midi(new), new_id))
            }
            _ => None,
        }) else { return };
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(clip.clone());
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            let sr = self.engine.transport.sample_rate();
            let ch = match &clip {
                ClipKind::Audio(a) => {
                    let buffer = a.buffer.as_ref().map(|b| (**b.samples()).clone()).unwrap_or_default();
                    let ch = crate::project::clip_handle::ClipHandle::new(new_id, buffer, a.buffer.as_ref().map(|b| b.channels()).unwrap_or(0), a.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(sr));
                    ch.set_position(a.position_frames);
                    ch
                }
                ClipKind::Midi(m) => {
                    let ch = crate::project::clip_handle::ClipHandle::new_midi(new_id, m.notes.clone(), m.length_frames, sr);
                    ch.set_position(m.position_frames);
                    ch
                }
            };
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.add_clip(ch);
            }
        }
        self.undo_service.push(UndoCommand::DuplicateClip { track_index, clip_id, new_clip_id: new_id });
    }

    pub fn split_clip_at(&mut self, track_index: usize, clip_id: uuid::Uuid, split_frame: u64) {
        // Collect data before any mutable borrows
        let (pos, off, len, audio_buf, midi_notes) = {
            let track = match self.project.tracks.get(track_index) { Some(t) => t, None => return };
            let found = track.clips.iter().find_map(|c| match c {
                ClipKind::Audio(a) if a.id == clip_id => {
                    Some((a.position_frames, a.offset_frames, a.length_frames, a.buffer.clone(), None))
                }
                ClipKind::Midi(m) if m.id == clip_id => {
                    let notes = m.notes.clone();
                    Some((m.position_frames, 0, m.length_frames, None, Some(notes)))
                }
                _ => None,
            });
            match found { Some(v) => v, None => return }
        };
        let split_local = split_frame.saturating_sub(pos);
        if split_local <= off || split_local >= off + len { return; }
        let left_len = split_local - off;
        let right_len = off + len - split_local;
        let right_id = uuid::Uuid::new_v4();
        // Trim original clip on left
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            if let Some(ClipKind::Audio(a)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(a) if a.id == clip_id)) {
                a.length_frames = left_len;
            }
            if let Some(ClipKind::Midi(m)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                m.length_frames = left_len;
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    ch.set_length(left_len);
                }
            }
        }
        // Create right clip
        let audio_for_engine = audio_buf.clone();
        let right_clip = match (audio_buf, midi_notes) {
            (Some(buf), _) => {
                let mut a = crate::project::clip::AudioClip::new("split".into(), buf);
                a.id = right_id;
                a.position_frames = split_frame;
                a.offset_frames = split_local;
                a.length_frames = right_len;
                ClipKind::Audio(a)
            }
            (_, Some(orig_notes)) => {
                let local_split = split_frame - pos;
                let right_notes: Vec<_> = orig_notes.iter()
                    .filter(|n| n.start_frame >= local_split)
                    .map(|n| {
                        let mut nn = n.clone();
                        nn.start_frame = nn.start_frame.saturating_sub(local_split);
                        nn
                    })
                    .collect();
                let m = crate::project::midi_clip::MidiClip {
                    id: right_id,
                    name: "split".into(),
                    position_frames: split_frame,
                    length_frames: right_len,
                    notes: right_notes,
                    color: [0x8a, 0x2b, 0xe2],
                    cc_events: Vec::new(),
                    thumb_dirty: true,
                };
                ClipKind::Midi(m)
            }
            _ => return,
        };
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(right_clip);
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                let sr = self.engine.transport.sample_rate();
                let right_ch = match audio_for_engine {
                    Some(ref buf) => {
                        let ch = crate::project::clip_handle::ClipHandle::new(right_id, (**buf.samples()).clone(), buf.channels(), buf.sample_rate());
                        ch.set_offset(split_local);
                        ch.set_position(split_frame);
                        ch.set_length(right_len);
                        ch
                    }
                    None => {
                        let ch = crate::project::clip_handle::ClipHandle::new_midi(right_id, vec![], right_len, sr);
                        ch.set_position(split_frame);
                        ch
                    }
                };
                handle.add_clip(right_ch);
            }
        }
        self.undo_service.push(UndoCommand::SplitClip {
            track_index,
            clip_id,
            new_clip_id: right_id,
            old_length: len,
            left_length: left_len,
            right_length: right_len,
        });
    }

    pub fn glue_clips(&mut self, track_index: usize, clip_a_id: uuid::Uuid, clip_b_id: uuid::Uuid) {
        // Collect clip data before mutating
        let clip_data = if let Some(track) = self.project.tracks.get(track_index) {
            let a = track.clips.iter().find(|c| match c {
                ClipKind::Audio(a) => a.id == clip_a_id,
                ClipKind::Midi(m) => m.id == clip_a_id,
            }).cloned();
            let b = track.clips.iter().find(|c| match c {
                ClipKind::Audio(a) => a.id == clip_b_id,
                ClipKind::Midi(m) => m.id == clip_b_id,
            }).cloned();
            (a, b)
        } else { (None, None) };
        let (clip_a, clip_b) = match clip_data {
            (Some(a), Some(b)) => (a, b),
            _ => return,
        };

        let merged_id = uuid::Uuid::new_v4();
        let merged = match (&clip_a, &clip_b) {
            (ClipKind::Audio(a), ClipKind::Audio(b)) => {
                let sr = a.buffer.as_ref().map(|buf| buf.sample_rate()).unwrap_or(44100);
                let channels = a.buffer.as_ref().map(|buf| buf.channels()).unwrap_or(2);
                // Calculate merged position and length
                let pos = a.position_frames.min(b.position_frames);
                let end_a = a.position_frames + a.length_frames;
                let end_b = b.position_frames + b.length_frames;
                let len = end_b - pos;
                // Build merged buffer by sampling from each clip's region
                let total_samples = len as usize * channels as usize;
                let mut merged_samples = vec![0.0f32; total_samples];

                let mut fill_clip = |clip: &crate::project::clip::AudioClip, start_offset: u64| {
                    if let Some(ref buf) = clip.buffer {
                        let samples = buf.samples();
                        let ch = buf.channels() as usize;
                        let clip_start_sample = clip.offset_frames as usize * ch;
                        let clip_len_samples = clip.length_frames as usize * ch;
                        let copy_len = clip_len_samples.min(samples.len().saturating_sub(clip_start_sample));
                        let merged_start_sample = (clip.position_frames - pos) as usize * ch;
                        for j in 0..copy_len {
                            let dst = merged_start_sample + j;
                            if dst < merged_samples.len() {
                                merged_samples[dst] = samples[clip_start_sample + j];
                            }
                        }
                    }
                };
                fill_clip(a, a.position_frames);
                fill_clip(b, b.position_frames);

                let buffer = crate::audio::buffer::AudioBuffer::from_interleaved(merged_samples, channels, sr);
                let mut merged = crate::project::clip::AudioClip::new("glued".into(), buffer);
                merged.id = merged_id;
                merged.position_frames = pos;
                merged.offset_frames = 0;
                merged.length_frames = len;
                ClipKind::Audio(merged)
            }
            (ClipKind::Midi(a), ClipKind::Midi(b)) => {
                let pos = a.position_frames.min(b.position_frames);
                let end_b = b.position_frames + b.length_frames;
                let len = end_b - pos;
                let mut merged_notes: Vec<crate::project::midi_note::MidiNote> = a.notes.iter().map(|n| {
                    let mut nn = n.clone();
                    nn.start_frame = nn.start_frame - (a.position_frames - pos);
                    nn
                }).collect();
                let offset_b = b.position_frames - pos;
                merged_notes.extend(b.notes.iter().map(|n| {
                    let mut nn = n.clone();
                    nn.start_frame = nn.start_frame + offset_b;
                    nn
                }));
                let m = crate::project::midi_clip::MidiClip {
                    id: merged_id,
                    name: "glued".into(),
                    position_frames: pos,
                    length_frames: len,
                    notes: merged_notes,
                    color: [0x6a, 0x4a, 0x9a],
                    cc_events: Vec::new(),
                    thumb_dirty: true,
                };
                ClipKind::Midi(m)
            }
            _ => return,
        };

        // Remove old clips from project model
        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.clips.retain(|c| match c {
                ClipKind::Audio(a) => a.id != clip_a_id && a.id != clip_b_id,
                ClipKind::Midi(m) => m.id != clip_a_id && m.id != clip_b_id,
            });
            track.add_clip(merged.clone());
        }
        // Remove old clips from engine model, add merged
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.clips.retain(|c| c.clip_id != clip_a_id && c.clip_id != clip_b_id);
                let sr = self.engine.transport.sample_rate();
                let ch = match &merged {
                    ClipKind::Audio(a) => {
                        let buf_data = a.buffer.as_ref().map(|b| (**b.samples()).clone()).unwrap_or_default();
                        let ch = crate::project::clip_handle::ClipHandle::new(
                            merged_id, buf_data,
                            a.buffer.as_ref().map(|b| b.channels()).unwrap_or(2),
                            a.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(sr),
                        );
                        ch.set_position(a.position_frames);
                        ch
                    }
                    ClipKind::Midi(m) => {
                        let ch = crate::project::clip_handle::ClipHandle::new_midi(merged_id, m.notes.clone(), m.length_frames, sr);
                        ch.set_position(m.position_frames);
                        ch
                    }
                };
                handle.add_clip(ch);
            }
        }
        self.undo_service.push(UndoCommand::GlueClips {
            track_index,
            clip_a: clip_a.clone(),
            clip_b: clip_b.clone(),
            merged_clip: merged,
        });
    }

    pub fn add_midi_clip(&mut self, track_index: usize, position_frames: u64, length_frames: u64) {
        let clip_id = uuid::Uuid::new_v4();
        let midi_clip = crate::project::midi_clip::MidiClip {
            id: clip_id,
            name: "MIDI Clip".into(),
            position_frames,
            length_frames,
            notes: Vec::new(),
            color: [0x8a, 0x2b, 0xe2],
            cc_events: Vec::new(),
            thumb_dirty: true,
        };
        let clip = ClipKind::Midi(midi_clip);

        if let Some(track) = self.project.tracks.get_mut(track_index) {
            track.add_clip(clip.clone());
        }

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                let sr = self.engine.transport.sample_rate();
                let clip_handle = ClipHandle::new_midi(clip_id, Vec::new(), length_frames, sr);
                clip_handle.set_position(position_frames);
                handle.add_clip(clip_handle);
            }
        }

        if let Some(tui) = self.track_ui.get_mut(track_index) {
            tui.color = [0x2a, 0x1a, 0x3a];
        }

        self.undo_service.push(UndoCommand::AddMidiClip {
            track_index,
            clip,
        });
    }

    pub fn delete_track(&mut self, track_index: usize) {
        if track_index >= self.project.tracks.len() {
            return;
        }

        let track = self.project.tracks.get(track_index).cloned();
        let track_ui = self.track_ui.get(track_index).cloned();

        // Remove from engine first to keep dual-model sync atomic
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if track_index < tracks.len() {
                tracks.remove(track_index);
            }
        }

        if let Some(track) = &track {
            for clip in &track.clips {
                let id = match clip {
                    crate::project::clip::ClipKind::Audio(a) => a.id,
                    crate::project::clip::ClipKind::Midi(m) => m.id,
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
            for (_ti, track) in self.project.tracks.iter().enumerate() {
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

        let (buffer, clip_id) = match &pool_clip.clip {
            ClipKind::Audio(a) => match a.buffer.clone() {
                Some(buf) => (buf, a.id),
                None => return,
            },
            ClipKind::Midi(_) => return,
        };

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
            track.add_clip(ClipKind::Audio(audio_clip));
        }

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                handle.add_clip(clip_handle);
            }
        }
    }
}
