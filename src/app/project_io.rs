use crate::app::HdawApp;
use crate::audio::clap_effect::ClapEffectAdapter;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::{EffectInstance, EffectType};
use crate::audio::buffer::AudioBuffer;
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::clip_handle::ClipHandle;
use crate::project::midi_note::MidiNote;
use crate::project::track::{SerializedEffect, Track, TrackHandle};
use crate::project::Project;
use egui_file_dialog::FileDialog;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

pub(crate) fn resample(samples: &[f32], channels: u16, from_sr: u32, to_sr: u32) -> Vec<f32> {
    let from_frames = samples.len() / channels as usize;
    let to_frames = (from_frames as f64 * to_sr as f64 / from_sr as f64).round() as usize;
    let ratio = from_frames as f64 / to_frames as f64;
    let mut out = Vec::with_capacity(to_frames * channels as usize);
    for i in 0..to_frames {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;
        for ch in 0..channels as usize {
            let a = samples.get(src_idx * channels as usize + ch).copied().unwrap_or(0.0);
            let b = samples.get((src_idx + 1) * channels as usize + ch).copied().unwrap_or(0.0);
            out.push(a + (b - a) * frac as f32);
        }
    }
    out
}

impl HdawApp {
    pub fn import_audio(&mut self) {
        let mut dialog = FileDialog::new();
        if let Some(dir) = &self.preferences.last_import_dir {
            dialog = dialog.initial_directory(dir.clone());
        }
        dialog.pick_file();
        self.file_dialog = Some(dialog);
        self.import_requested = true;
    }

    pub(crate) fn handle_file_dialog(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.file_dialog {
            dialog.update(ctx);

            if let Some(path) = dialog.picked() {
                let path_buf = path.to_path_buf();
                self.preferences.last_import_dir = path_buf.parent().map(|p| p.to_path_buf());
                crate::app::prefs_io::save_preferences(&self.preferences);
                if let Some(path_str) = path_buf.to_str() {
                    if let Err(e) = self.load_audio_file(path_str) {
                        self.error_message = Some(e);
                    }
                }
                self.file_dialog = None;
                self.import_requested = false;
            } else if !self.import_requested {
                self.file_dialog = None;
            }
        }
    }

    pub fn load_audio_file(&mut self, path: &str) -> Result<(), String> {
        let (samples, channels, file_sr) = crate::utils::load_wav_file(path)?;

        let engine_sr = self.engine.transport.sample_rate();
        let samples = if file_sr != engine_sr {
            resample(&samples, channels, file_sr, engine_sr)
        } else {
            samples
        };
        let sample_rate = engine_sr;

        let file_name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let source_path = std::path::PathBuf::from(path);

        let mut handle = TrackHandle::new();

        let buffer = AudioBuffer::from_interleaved(samples, channels, sample_rate);
        let clip_kind = ClipKind::Audio(AudioClip::with_source_path(file_name.clone(), buffer, source_path));

        let buf = match &clip_kind {
            ClipKind::Audio(a) => match a.buffer.as_ref() {
                Some(b) => b,
                None => return Err("clip buffer missing after import".to_string()),
            },
            ClipKind::Midi(_) => return Err("unexpected midi clip on import".to_string()),
        };
        let clip_handle = ClipHandle::new(
            match &clip_kind { ClipKind::Audio(a) => a.id, ClipKind::Midi(_) => unreachable!() },
            (*buf.samples()).to_vec(),
            channels,
            sample_rate,
        );
        handle.add_clip(clip_handle);

        let track_ui = crate::app::TrackUiState {
            id: handle.id,
            name: file_name.clone(),
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

        self.track_ui.push(track_ui.clone());
        self.engine.add_track(handle);

        let mut project_track = Track::new(file_name);
        project_track.add_clip(clip_kind);
        self.project.add_track(project_track.clone());

        // Push undo snapshot
        let snapshot = crate::app::undo::ImportTrackSnapshot::new(project_track, track_ui);
        self.undo_state.push(crate::app::undo::UndoCommand::ImportAudio {
            tracks: vec![snapshot],
        });

        tracing::info!("loaded audio file: {path}");
        Ok(())
    }

    pub fn sync_engine_to_project(&mut self) {
        #[allow(clippy::type_complexity)]
        let snapshot: Vec<(f32, f32, bool, bool, Vec<Vec<crate::project::automation::AutomationPoint>>, Vec<SerializedEffect>, Vec<(uuid::Uuid, Vec<MidiNote>)>, Option<uuid::Uuid>, bool, bool, Vec<crate::project::track::SendSlotDef>)> =
            self.engine.tracks.lock().ok().map(|tracks| {
                tracks.iter().map(|handle| {
                    let vol = f32::from_bits(handle.volume.load(Ordering::Acquire));
                    let pan = f32::from_bits(handle.pan.load(Ordering::Acquire));
                    let mute = handle.mute.load(Ordering::Acquire);
                    let solo = handle.solo.load(Ordering::Acquire);
                    let auto_points: Vec<Vec<crate::project::automation::AutomationPoint>> = handle
                        .automation_lanes.iter().map(|l| l.points.clone()).collect();
                    let fx: Vec<SerializedEffect> = handle.fx_chain.iter().map(|inst| {
                        let pv: Vec<f32> = inst.parameter_info().iter()
                            .map(|p| inst.parameter_value(p.id))
                            .collect();
                        SerializedEffect {
                            name: inst.name.clone(),
                            effect_type: inst.effect_type.clone(),
                            bypass: inst.is_bypassed(),
                            param_values: pv,
                        }
                    }).collect();
                    let clip_notes: Vec<(uuid::Uuid, Vec<MidiNote>)> = handle.clips.iter()
                        .map(|c| (c.clip_id, c.midi_notes.clone()))
                        .collect();
                    let sends: Vec<crate::project::track::SendSlotDef> = handle.sends.iter().map(|s| {
                        crate::project::track::SendSlotDef {
                            target_id: s.target_id,
                            level: f32::from_bits(s.level.load(Ordering::Acquire)),
                            pre_fader: s.pre_fader,
                        }
                    }).collect();
                    (vol, pan, mute, solo, auto_points, fx, clip_notes, handle.parent_group, handle.is_group, handle.is_return, sends)
                }).collect()
            }).unwrap_or_default();

        for (ti, (vol, pan, mute, solo, auto_points, fx, clip_notes, parent_group, is_group, is_return, sends)) in snapshot.into_iter().enumerate() {
            if let Some(track) = self.project.tracks.get_mut(ti) {
                track.volume = vol;
                track.pan = pan;
                track.mute = mute;
                track.solo = solo;
                track.parent_group = parent_group;
                track.is_group = is_group;
                track.is_return = is_return;
                track.sends = sends;
                for (li, points) in auto_points.into_iter().enumerate() {
                    if let Some(lane) = track.automation_lanes.get_mut(li) {
                        if lane.points != points {
                            lane.points = points;
                        }
                    }
                }
                track.fx_chain = fx;
                for (clip_id, notes) in clip_notes {
                    if let Some(clip) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Midi(m) if m.id == clip_id)) {
                        if let ClipKind::Midi(m) = clip {
                            if m.notes != notes {
                                m.notes = notes;
                                m.thumb_dirty = true;
                            }
                        }
                    }
                }
            }
        }

        // Sync fade values from engine -> project
        let fade_snapshot: Vec<(uuid::Uuid, u64, u64)> = self.engine.tracks.lock().ok().map(|tracks| {
            tracks.iter().flat_map(|handle| {
                handle.clips.iter().map(|ch| {
                    (ch.clip_id, ch.fade_in_frames.load(Ordering::Acquire), ch.fade_out_frames.load(Ordering::Acquire))
                }).collect::<Vec<_>>()
            }).collect()
        }).unwrap_or_default();
        for (clip_id, fi, fo) in fade_snapshot {
            for track in self.project.tracks.iter_mut() {
                if let Some(ClipKind::Audio(a)) = track.clips.iter_mut().find(|c| matches!(c, ClipKind::Audio(ca) if ca.id == clip_id)) {
                    a.fade_in_frames = fi;
                    a.fade_out_frames = fo;
                    break;
                }
            }
        }
    }

    pub fn new_project(&mut self) {
        self.stop();
        self.engine.transport.seek_to_frame(0);
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            tracks.clear();
        }
        self.project = Project::new();
        self.project.bpm = self.preferences.default_bpm;
        self.project.time_signature_num = self.preferences.default_time_sig_num;
        self.project.time_signature_den = self.preferences.default_time_sig_den;
        self.timeline_state.pixels_per_second = self.preferences.default_zoom;
        self.timeline_state.snap_enabled = self.preferences.snap_default;
        self.track_ui.clear();
        self.selected_track = None;
        self.effect_editor_state.selected_track = None;
        self.effect_editor_state.selected_effect = None;
        self.current_path = None;
        self.undo_state.clear();
        self.midi_thumb_cache.clear();
        self.waveform_cache.clear();
    }

    pub fn save_current_project(&mut self, path: &str) -> Result<(), String> {
        self.sync_engine_to_project();
        let data = ron::ser::to_string_pretty(&self.project, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("serialize: {e}"))?;
        std::fs::write(path, &data).map_err(|e| format!("write: {e}"))?;
        self.current_path = Some(PathBuf::from(path));
        Ok(())
    }

    pub fn load_project_file(&mut self, path: &str) -> Result<(), String> {
        let data = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
        let mut project: Project = ron::de::from_str(&data).map_err(|e| format!("deserialize: {e}"))?;

        for track in &mut project.tracks {
            for clip in &mut track.clips {
                match clip {
                    ClipKind::Audio(audio_clip) => {
                        let _ = audio_clip.reload_buffer();
                    }
                    ClipKind::Midi(midi_clip) => {
                        midi_clip.thumb_dirty = true;
                    }
                }
            }
        }

        self.new_project();
        self.project = project;

        let mut engine_tracks = Vec::with_capacity(self.project.tracks.len());

        for track in &self.project.tracks {
            let mut handle = TrackHandle::new();
            handle.id = track.id;

            for clip in &track.clips {
                match clip {
                    ClipKind::Audio(audio_clip) => {
                        if let Some(buf) = &audio_clip.buffer {
                            let clip_handle = ClipHandle::new(
                                audio_clip.id,
                                (**buf.samples()).to_vec(),
                                buf.channels(),
                                buf.sample_rate(),
                            );
                            clip_handle.set_position(audio_clip.position_frames);
                            clip_handle.set_offset(audio_clip.offset_frames);
                            clip_handle.set_length(audio_clip.length_frames);
                            clip_handle.fade_in_frames.store(audio_clip.fade_in_frames, Ordering::Release);
                            clip_handle.fade_out_frames.store(audio_clip.fade_out_frames, Ordering::Release);
                            handle.add_clip(clip_handle);
                        }
                    }
                    ClipKind::Midi(midi_clip) => {
                        let clip_handle = ClipHandle::new_midi(
                            midi_clip.id,
                            midi_clip.notes.clone(),
                            midi_clip.length_frames,
                            self.engine.transport.sample_rate(),
                        );
                        clip_handle.set_position(midi_clip.position_frames);
                        handle.add_clip(clip_handle);
                    }
                }
            }

            for sfx in &track.fx_chain {
                let instance = match &sfx.effect_type {
                    EffectType::Clap { plugin_id, path } => {
                        let adapter = ClapEffectAdapter::new_instance(
                            plugin_id,
                            Path::new(path),
                            self.engine.transport.sample_rate(),
                        );
                        match adapter {
                            Ok(a) => EffectInstance::new_clap(sfx.name.clone(), sfx.effect_type.clone(), a),
                            Err(e) => {
                                tracing::error!("Failed to load CLAP effect {}: {}", plugin_id, e);
                                continue;
                            }
                        }
                    }
                    _ => {
                        let mut effect = create_effect(sfx.effect_type.clone());
                        for (i, val) in sfx.param_values.iter().enumerate() {
                            if let Some(info) = effect.parameter_info().get(i) {
                                effect.set_parameter(info.id, *val);
                            }
                        }
                        effect.reset(self.engine.transport.sample_rate());
                        let inst = EffectInstance::new_builtin(sfx.name.clone(), sfx.effect_type.clone(), effect);
                        inst
                    }
                };
                instance.set_bypass(sfx.bypass);
                handle.add_effect(instance);
            }

            handle.parent_group = track.parent_group;
            handle.is_group = track.is_group;
            handle.is_return = track.is_return;
            for sdef in &track.sends {
                handle.sends.push(crate::project::track::SendSlot::new(sdef.target_id, sdef.level, sdef.pre_fader));
            }

            let track_ui = crate::app::TrackUiState {
                id: track.id,
                name: track.name.clone(),
                color: track.color,
                volume: handle.volume.clone(),
                pan: handle.pan.clone(),
                mute: handle.mute.clone(),
                solo: handle.solo.clone(),
                armed: handle.armed.clone(),
                peak_left: handle.peak_left.clone(),
                peak_right: handle.peak_right.clone(),
                parent_group: track.parent_group,
                is_group: track.is_group,
                is_return: track.is_return,
                collapsed: false,
                send_levels: handle.sends.iter().map(|s| s.level.clone()).collect(),
            };

            handle.volume.store(f32::to_bits(track.volume), Ordering::Release);
            handle.pan.store(f32::to_bits(track.pan), Ordering::Release);
            handle.mute.store(track.mute, Ordering::Release);
            handle.solo.store(track.solo, Ordering::Release);

            for (li, lane) in track.automation_lanes.iter().enumerate() {
                if let Some(al) = handle.automation_lanes.get_mut(li) {
                    al.points.clone_from(&lane.points);
                }
            }

            self.track_ui.push(track_ui);
            engine_tracks.push(handle);
        }

        // Atomically swap the entire engine track list to avoid a silent gap
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            *tracks = engine_tracks;
        }

        self.current_path = Some(PathBuf::from(path));
        Ok(())
    }
}
