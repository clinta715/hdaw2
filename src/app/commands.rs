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
                    self.undo_service.push(UndoCommand::DeleteClip {
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
                    clip.notes.sort_by_key(|n| n.start_frame);
                    clip.thumb_dirty = true;
                }
            }
        }
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(handle) = tracks.get_mut(track_index) {
                if let Some(ch) = handle.clips.iter_mut().find(|c| c.clip_id == clip_id) {
                    if note_idx < ch.midi_notes.len() {
                        ch.midi_notes[note_idx] = new_note.clone();
                        ch.midi_notes.sort_by_key(|n| n.start_frame);
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
