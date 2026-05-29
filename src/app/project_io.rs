use crate::app::HdawApp;
use crate::audio::effects::create_effect;
use crate::audio::effects::dsp_effect::EffectInstance;
use crate::audio::buffer::AudioBuffer;
use crate::project::clip::AudioClip;
use crate::project::clip_handle::ClipHandle;
use crate::project::track::{SerializedEffect, Track, TrackHandle};
use crate::project::Project;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

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
        let (samples, channels, sample_rate) = crate::utils::load_wav_file(path)?;

        let file_name = std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        let source_path = std::path::PathBuf::from(path);

        let mut handle = TrackHandle::new();

        let buffer = AudioBuffer::from_interleaved(samples, channels, sample_rate);
        let clip = AudioClip::with_source_path(file_name.clone(), buffer, source_path);

        let buf = match clip.buffer.as_ref() {
            Some(b) => b,
            None => return Err("clip buffer missing after import".to_string()),
        };
        let clip_handle = ClipHandle::new(clip.id, (*buf.samples()).to_vec(), channels, sample_rate);
        handle.add_clip(clip_handle);

        let track_ui = crate::app::TrackUiState {
            name: file_name.clone(),
            color: [0x1a, 0x2a, 0x1a],
            volume: handle.volume.clone(),
            pan: handle.pan.clone(),
            mute: handle.mute.clone(),
            solo: handle.solo.clone(),
            peak_left: handle.peak_left.clone(),
            peak_right: handle.peak_right.clone(),
        };

        self.track_ui.push(track_ui);
        self.engine.add_track(handle);

        let mut track = Track::new(file_name);
        track.add_clip(clip);
        self.project.add_track(track);

        tracing::info!("loaded audio file: {path}");
        Ok(())
    }

    pub fn sync_engine_to_project(&mut self) {
        let snapshot: Vec<(f32, f32, bool, bool, Vec<Vec<crate::project::automation::AutomationPoint>>, Vec<SerializedEffect>)> =
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
                    (vol, pan, mute, solo, auto_points, fx)
                }).collect()
            }).unwrap_or_default();

        for (ti, (vol, pan, mute, solo, auto_points, fx)) in snapshot.into_iter().enumerate() {
            if let Some(track) = self.project.tracks.get_mut(ti) {
                track.volume = vol;
                track.pan = pan;
                track.mute = mute;
                track.solo = solo;
                for (li, points) in auto_points.into_iter().enumerate() {
                    if let Some(lane) = track.automation_lanes.get_mut(li) {
                        if lane.points != points {
                            lane.points = points;
                        }
                    }
                }
                track.fx_chain = fx;
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
                let _ = clip.reload_buffer();
            }
        }

        self.new_project();
        self.project = project;

        for track in &self.project.tracks {
            let mut handle = TrackHandle::new();

            for clip in &track.clips {
                if let Some(buf) = &clip.buffer {
                    let clip_handle = ClipHandle::new(
                        clip.id,
                        (**buf.samples()).to_vec(),
                        buf.channels(),
                        buf.sample_rate(),
                    );
                    clip_handle.set_position(clip.position_frames);
                    clip_handle.set_offset(clip.offset_frames);
                    clip_handle.set_length(clip.length_frames);
                    handle.add_clip(clip_handle);
                }
            }

            for sfx in &track.fx_chain {
                let mut effect = create_effect(sfx.effect_type.clone());
                for (i, val) in sfx.param_values.iter().enumerate() {
                    if let Some(info) = effect.parameter_info().get(i) {
                        effect.set_parameter(info.id, *val);
                    }
                }
                effect.reset(self.engine.transport.sample_rate());
                let instance = EffectInstance::new_builtin(sfx.name.clone(), sfx.effect_type.clone(), effect);
                instance.set_bypass(sfx.bypass);
                handle.add_effect(instance);
            }

            let track_ui = crate::app::TrackUiState {
                name: track.name.clone(),
                color: track.color,
                volume: handle.volume.clone(),
                pan: handle.pan.clone(),
                mute: handle.mute.clone(),
                solo: handle.solo.clone(),
                peak_left: handle.peak_left.clone(),
                peak_right: handle.peak_right.clone(),
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
            self.engine.add_track(handle);
        }

        self.current_path = Some(PathBuf::from(path));
        Ok(())
    }
}
