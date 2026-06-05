use crate::app::TrackUiState;
use crate::app::undo::UndoCommand;
use crate::app::undo_service::UndoService;
use crate::audio::engine::AudioEngine;
use crate::audio::clap_scanner::PluginDescriptor;
use crate::project::Project;
use crate::ui::effect_editor::EffectEditorState;
use crate::ui::preferences::PreferencesState;

/// Orchestrates domain services (audio engine, project model, undo) and
/// mediates between the UI layer and the engine. HdawApp delegates
/// cross-cutting operations here.
pub struct AppCoordinator {
    pub engine: AudioEngine,
    pub project: Project,
    pub undo_service: UndoService,
    pub preferences: PreferencesState,
    pub plugin_registry: Vec<PluginDescriptor>,
    pub current_path: Option<std::path::PathBuf>,
}

impl AppCoordinator {
    pub fn new() -> Self {
        let mut engine = AudioEngine::new();
        engine.init();
        Self {
            engine,
            project: Project::new(),
            undo_service: UndoService::new(),
            preferences: crate::app::prefs_io::load_preferences().unwrap_or_default(),
            plugin_registry: Vec::new(),
            current_path: None,
        }
    }

    // ── transport pass-through ─────────────────────────────────

    pub fn play(&self) { self.engine.play(); }
    pub fn pause(&self) { self.engine.pause(); }
    pub fn stop(&self) { self.engine.stop(); }
    pub fn is_playing(&self) -> bool { self.engine.transport.is_playing() }
    pub fn master_volume(&self) -> f32 { self.engine.master_bus.get_volume() }
    pub fn set_master_volume(&self, vol: f32) { self.engine.master_bus.set_volume(vol); }

    // ── undo / redo ────────────────────────────────────────────

    pub fn undo(
        &mut self,
        track_ui: &mut Vec<TrackUiState>,
        selected_track: &mut Option<usize>,
        effect_editor_state: &mut EffectEditorState,
    ) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_service.stack.undo() {
                match cmd {
                    UndoCommand::AddTrack { track_index, .. } => {
                        if *track_index < tracks.len() {
                            for fx in &mut tracks[*track_index].fx_chain {
                                if let crate::audio::effects::dsp_effect::EffectKind::Clap(adapter) = &fx.kind {
                                    if let Ok(mut a) = adapter.lock() {
                                        a.deactivate();
                                    }
                                }
                            }
                            tracks.remove(*track_index);
                        }
                        if *track_index < self.project.tracks.len() {
                            self.project.tracks.remove(*track_index);
                        }
                        if *track_index < track_ui.len() {
                            track_ui.remove(*track_index);
                        }
                        *selected_track = None;
                        effect_editor_state.selected_track = None;
                    }
                    UndoCommand::RecordAudio { track_indices, clip_ids, .. } => {
                        for (ti, cid) in track_indices.iter().zip(clip_ids.iter()) {
                            if let Some(track) = tracks.get_mut(*ti) {
                                track.clips.retain(|c| c.clip_id != *cid);
                            }
                            if let Some(pt) = self.project.tracks.get_mut(*ti) {
                                pt.clips.pop();
                            }
                            self.project.audio_pool.pop();
                        }
                    }
                    UndoCommand::DeleteTrack { track_index, track, track_ui: track_ui_snap, .. } => {
                        let mut handle = crate::project::track::TrackHandle::new();
                        handle.id = track.id;
                        handle.parent_group = track.parent_group;
                        handle.is_group = track.is_group;
                        handle.is_return = track.is_return;
                        handle.volume.store(f32::to_bits(track.volume), std::sync::atomic::Ordering::Release);
                        handle.pan.store(f32::to_bits(track.pan), std::sync::atomic::Ordering::Release);
                        handle.mute.store(track.mute, std::sync::atomic::Ordering::Release);
                        handle.solo.store(track.solo, std::sync::atomic::Ordering::Release);
                        for clip_kind in &track.clips {
                            match clip_kind {
                                crate::project::clip::ClipKind::Audio(audio_clip) => {
                                    if let Some(buf) = &audio_clip.buffer {
                                        let ch = crate::project::clip_handle::ClipHandle::new(
                                            audio_clip.id, (**buf.samples()).to_vec(),
                                            buf.channels(), buf.sample_rate(),
                                        );
                                        ch.set_position(audio_clip.position_frames);
                                        ch.set_offset(audio_clip.offset_frames);
                                        ch.set_length(audio_clip.length_frames);
                                        handle.add_clip(ch);
                                    }
                                }
                                crate::project::clip::ClipKind::Midi(midi_clip) => {
                                    let ch = crate::project::clip_handle::ClipHandle::new_midi(
                                        midi_clip.id, midi_clip.notes.clone(),
                                        midi_clip.length_frames, sr,
                                    );
                                    ch.set_position(midi_clip.position_frames);
                                    handle.add_clip(ch);
                                }
                            }
                        }
                        handle.automation_lanes = track.automation_lanes.clone();
                        for serialized in &track.fx_chain {
                            let instance = match &serialized.effect_type {
                                crate::audio::effects::dsp_effect::EffectType::Clap { plugin_id, path } => {
                                    match crate::audio::clap_effect::ClapEffectAdapter::new_instance(plugin_id, std::path::Path::new(path), sr) {
                                        Ok(adapter) => {
                                            let inst = crate::audio::effects::dsp_effect::EffectInstance::new_clap(
                                                serialized.name.clone(), serialized.effect_type.clone(), adapter,
                                            );
                                            inst.set_bypass(serialized.bypass);
                                            inst
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to recreate CLAP effect {}: {}", plugin_id, e);
                                            let effect = crate::audio::effects::create_effect(crate::audio::effects::dsp_effect::EffectType::Gain);
                                            let inst = crate::audio::effects::dsp_effect::EffectInstance::new_builtin(
                                                serialized.name.clone(), serialized.effect_type.clone(), effect,
                                            );
                                            inst.set_bypass(serialized.bypass);
                                            inst
                                        }
                                    }
                                }
                                _ => {
                                    let effect = crate::audio::effects::create_effect(serialized.effect_type.clone());
                                    for (i, val) in serialized.param_values.iter().enumerate() {
                                        if let Some(info) = effect.parameter_info().get(i) {
                                            effect.set_parameter(info.id, *val);
                                        }
                                    }
                                    let inst = crate::audio::effects::dsp_effect::EffectInstance::new_builtin(
                                        serialized.name.clone(), serialized.effect_type.clone(), effect,
                                    );
                                    inst.set_bypass(serialized.bypass);
                                    inst
                                }
                            };
                            handle.fx_chain.push(instance);
                        }
                        for send_def in &track.sends {
                            handle.sends.push(crate::project::track::SendSlot::new(send_def.target_id, send_def.level, send_def.pre_fader));
                        }
                        let idx = (*track_index).min(tracks.len());
                        tracks.insert(idx, handle);
                        self.project.tracks.insert(idx, track.clone());
                        track_ui.insert(idx, track_ui_snap.clone());
                    }
                    UndoCommand::ImportAudio { tracks: snapshots }
                    | UndoCommand::ImportMidi { tracks: snapshots } => {
                        let count = snapshots.len();
                        for _ in 0..count {
                            let ti = self.project.tracks.len().saturating_sub(1);
                            if ti < tracks.len() {
                                for fx in &mut tracks[ti].fx_chain {
                                    if let crate::audio::effects::dsp_effect::EffectKind::Clap(adapter) = &fx.kind {
                                        if let Ok(mut a) = adapter.lock() {
                                            a.deactivate();
                                        }
                                    }
                                }
                                tracks.remove(ti);
                            }
                            if ti < self.project.tracks.len() {
                                self.project.tracks.remove(ti);
                            }
                            if ti < track_ui.len() {
                                track_ui.remove(ti);
                            }
                        }
                        *selected_track = None;
                        effect_editor_state.selected_track = None;
                    }
                    _ => crate::app::undo::apply_undo(&mut self.project, &mut tracks, cmd, sr),
                }
            }
        }
    }

    pub fn redo(
        &mut self,
        track_ui: &mut Vec<TrackUiState>,
        selected_track: &mut Option<usize>,
        effect_editor_state: &mut EffectEditorState,
    ) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_service.stack.redo() {
                match cmd {
                    UndoCommand::AddTrack { track_index, track, track_ui: track_ui_snap, .. } => {
                        let mut handle = crate::project::track::TrackHandle::new();
                        handle.id = track.id;
                        handle.parent_group = track.parent_group;
                        handle.is_group = track.is_group;
                        handle.is_return = track.is_return;
                        handle.volume.store(f32::to_bits(track.volume), std::sync::atomic::Ordering::Release);
                        handle.pan.store(f32::to_bits(track.pan), std::sync::atomic::Ordering::Release);
                        handle.mute.store(track.mute, std::sync::atomic::Ordering::Release);
                        handle.solo.store(track.solo, std::sync::atomic::Ordering::Release);
                        handle.automation_lanes = track.automation_lanes.clone();
                        for send_def in &track.sends {
                            handle.sends.push(crate::project::track::SendSlot::new(send_def.target_id, send_def.level, send_def.pre_fader));
                        }
                        let idx = (*track_index).min(tracks.len());
                        tracks.insert(idx, handle);
                        self.project.tracks.insert(idx, track.clone());
                        track_ui.insert(idx, track_ui_snap.clone());
                    }
                    UndoCommand::DeleteTrack { track_index, .. } => {
                        if *track_index < tracks.len() {
                            for fx in &mut tracks[*track_index].fx_chain {
                                if let crate::audio::effects::dsp_effect::EffectKind::Clap(adapter) = &fx.kind {
                                    if let Ok(mut a) = adapter.lock() {
                                        a.deactivate();
                                    }
                                }
                            }
                            tracks.remove(*track_index);
                        }
                        if *track_index < self.project.tracks.len() {
                            let track = self.project.tracks.remove(*track_index);
                            for clip in &track.clips {
                                let pool_clip = crate::project::pool::PoolClip::from_clip(clip.clone());
                                self.project.audio_pool.push(pool_clip);
                            }
                        }
                        if *track_index < track_ui.len() {
                            track_ui.remove(*track_index);
                        }
                        *selected_track = None;
                        effect_editor_state.selected_track = None;
                    }
                    UndoCommand::ImportAudio { tracks: snapshots }
                    | UndoCommand::ImportMidi { tracks: snapshots } => {
                        for snap in snapshots {
                            let track = &snap.track;
                            let track_ui_snap = &snap.track_ui;
                            let mut handle = crate::project::track::TrackHandle::new();
                            handle.id = track.id;
                            handle.parent_group = track.parent_group;
                            handle.is_group = track.is_group;
                            handle.is_return = track.is_return;
                            handle.volume.store(f32::to_bits(track.volume), std::sync::atomic::Ordering::Release);
                            handle.pan.store(f32::to_bits(track.pan), std::sync::atomic::Ordering::Release);
                            handle.mute.store(track.mute, std::sync::atomic::Ordering::Release);
                            handle.solo.store(track.solo, std::sync::atomic::Ordering::Release);
                            handle.automation_lanes = track.automation_lanes.clone();
                            for send_def in &track.sends {
                                handle.sends.push(crate::project::track::SendSlot::new(send_def.target_id, send_def.level, send_def.pre_fader));
                            }
                            for clip_kind in &track.clips {
                                match clip_kind {
                                    crate::project::clip::ClipKind::Audio(audio_clip) => {
                                        if let Some(buf) = &audio_clip.buffer {
                                            let ch = crate::project::clip_handle::ClipHandle::new(
                                                audio_clip.id, (**buf.samples()).to_vec(),
                                                buf.channels(), buf.sample_rate(),
                                            );
                                            ch.set_position(audio_clip.position_frames);
                                            ch.set_offset(audio_clip.offset_frames);
                                            ch.set_length(audio_clip.length_frames);
                                            handle.add_clip(ch);
                                        }
                                    }
                                    crate::project::clip::ClipKind::Midi(midi_clip) => {
                                        let ch = crate::project::clip_handle::ClipHandle::new_midi(
                                            midi_clip.id, midi_clip.notes.clone(),
                                            midi_clip.length_frames, sr,
                                        );
                                        ch.set_position(midi_clip.position_frames);
                                        handle.add_clip(ch);
                                    }
                                }
                            }
                            let idx = tracks.len();
                            tracks.insert(idx, handle);
                            self.project.tracks.insert(idx, track.clone());
                            track_ui.insert(idx, track_ui_snap.clone());
                        }
                    }
                    UndoCommand::RecordAudio { track_indices, clip_ids, clip_kind } => {
                        let r_samples = match clip_kind {
                            crate::project::clip::ClipKind::Audio(ref a) => {
                                a.buffer.as_ref().map(|buf| ((**buf.samples()).clone(), buf.channels(), buf.sample_rate()))
                            }
                            _ => None,
                        };
                        let Some((ref r_samples, channels, sr)) = r_samples else { return };
                        for (ti, cid) in track_indices.iter().zip(clip_ids.iter()) {
                            if let Some(track) = tracks.get_mut(*ti) {
                                let ch = crate::project::clip_handle::ClipHandle::new(*cid, r_samples.clone(), channels, sr);
                                ch.set_position(0);
                                ch.set_length(r_samples.len() as u64 / channels as u64);
                                track.add_clip(ch);
                            }
                            if let Some(pt) = self.project.tracks.get_mut(*ti) {
                                pt.add_clip(clip_kind.clone());
                            }
                        }
                        self.project.audio_pool.push(crate::project::pool::PoolClip::from_clip(clip_kind.clone()));
                    }
                    _ => crate::app::undo::apply_redo(&mut self.project, &mut tracks, cmd, sr),
                }
            }
        }
    }
}

impl Default for AppCoordinator {
    fn default() -> Self { Self::new() }
}
