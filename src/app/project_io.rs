use crate::app::{HdawApp, MainView};
use crate::app::project_service;
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::clip_handle::ClipHandle;
use crate::project::track::{Track, TrackHandle};
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
        let engine_sr = self.engine.transport.sample_rate();
        let (buffer, file_name) = project_service::load_audio_file_resampled(path, engine_sr)?;
        let sample_rate = engine_sr;
        let source_path = PathBuf::from(path);

        let mut handle = TrackHandle::new();
        let clip_kind = ClipKind::Audio(AudioClip::with_source_path(
            file_name.clone(),
            buffer,
            source_path,
        ));

        let buf = match &clip_kind {
            ClipKind::Audio(a) => match a.buffer.as_ref() {
                Some(b) => b,
                None => return Err("clip buffer missing after import".to_string()),
            },
            ClipKind::Midi(_) => return Err("unexpected midi clip on import".to_string()),
        };
        let clip_handle = ClipHandle::new(
            match &clip_kind {
                ClipKind::Audio(a) => a.id,
                ClipKind::Midi(_) => unreachable!(),
            },
            (*buf.samples()).to_vec(),
            buf.channels(),
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

        let snapshot = crate::app::undo::ImportTrackSnapshot::new(project_track, track_ui);
        self.undo_service.push(crate::app::undo::UndoCommand::ImportAudio {
            tracks: vec![snapshot],
        });

        tracing::info!("loaded audio file: {path}");
        Ok(())
    }

    pub fn sync_engine_to_project(&mut self) {
        if let Ok(tracks) = self.engine.tracks.lock() {
            project_service::sync_engine_to_project(&mut self.project, &tracks);
        }
        let (loop_in, loop_out) = self.engine.transport.load_loop_region();
        self.project.loop_in_frames = loop_in;
        self.project.loop_out_frames = loop_out;
        self.project.loop_enabled = self.engine.transport.loop_enabled.load(Ordering::Acquire);
    }

    pub fn new_project(&mut self) {
        self.stop();
        self.engine.transport.seek_to_frame(0);
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            tracks.clear();
        }
        self.project = crate::project::Project::new();
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
        self.undo_service.clear();
        self.mark_saved();
        self.midi_thumb_cache.clear();
        self.waveform_cache.clear();
        self.main_view = MainView::Arrange;
        self.editing_midi_clip_id = None;
    }

    pub fn save_current_project(&mut self, path: &str) -> Result<(), String> {
        self.sync_engine_to_project();
        project_service::save_to_file(&self.project, path)?;
        self.current_path = Some(PathBuf::from(path));
        self.undo_service.clear();
        self.mark_saved();
        Ok(())
    }

    pub fn load_project_file(&mut self, path: &str) -> Result<(), String> {
        let project = project_service::load_from_file(path)?;

        self.new_project();
        self.project = project;

        let (engine_tracks, track_ui) = project_service::rebuild_engine_handles(
            &self.project,
            self.engine.transport.sample_rate(),
        );

        self.track_ui = track_ui;
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            *tracks = engine_tracks;
        }

        self.engine.transport.set_loop_region(self.project.loop_in_frames, self.project.loop_out_frames);
        self.engine.transport.loop_enabled.store(self.project.loop_enabled, Ordering::Release);

        self.current_path = Some(PathBuf::from(path));
        Ok(())
    }
}
