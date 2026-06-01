pub mod commands;
pub mod input;
pub mod prefs_io;
pub mod project_io;
pub mod undo;

use crate::audio::buffer::AudioBuffer;
use crate::audio::clap_scanner::PluginDescriptor;
use crate::audio::engine::AudioEngine;
use crate::project::clip::{AudioClip, ClipKind};
use crate::project::clip_handle::ClipHandle;
use crate::project::pool::PoolClip;
use crate::project::Project;
use crate::ui::audio_pool::AudioPoolPanelState;
use crate::ui::effect_editor::EffectEditorState;
use crate::ui::mixer_panel::MixerPanelState;
use crate::ui::preferences::PreferencesState;
use crate::ui::timeline::TimelineState;
use crate::ui::toolbar::ToolbarState;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct TrackUiState {
    pub id: Uuid,
    pub name: String,
    pub color: [u8; 3],
    pub volume: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
    pub mute: Arc<AtomicBool>,
    pub solo: Arc<AtomicBool>,
    pub armed: Arc<AtomicBool>,
    pub peak_left: Arc<AtomicU32>,
    pub peak_right: Arc<AtomicU32>,
    pub parent_group: Option<Uuid>,
    pub is_group: bool,
    pub is_return: bool,
    pub collapsed: bool,
    pub send_levels: Vec<Arc<AtomicU32>>,
}

pub struct HdawApp {
    pub project: Project,
    pub engine: AudioEngine,
    pub track_ui: Vec<TrackUiState>,
    pub selected_track: Option<usize>,
    pub toolbar_state: ToolbarState,
    pub timeline_state: TimelineState,
    pub mixer_state: MixerPanelState,
    pub effect_editor_state: EffectEditorState,
    pub audio_pool_state: AudioPoolPanelState,
    pub play_requested: bool,
    pub pause_requested: bool,
    pub stop_requested: bool,
    pub seek_requested: bool,
    pub record_requested: bool,
    pub recording: bool,
    pub seek_frame: u64,
    pub file_dialog: Option<FileDialog>,
    pub import_requested: bool,
    pub midi_file_dialog: Option<FileDialog>,
    pub midi_import_requested: bool,
    pub midi_import_tracks: Vec<crate::project::midi_import::ImportedMidiTrack>,
    pub midi_track_selection: Vec<bool>,
    pub midi_import_file_name: String,
    pub show_midi_track_selector: bool,
    pub save_dialog: Option<FileDialog>,
    pub open_dialog: Option<FileDialog>,
    pub new_project_requested: bool,
    pub save_requested: bool,
    pub save_as_requested: bool,
    pub open_requested: bool,
    pub export_requested: bool,
    pub export_bit_depth: u16,
    pub export_use_loop_range: bool,
    pub exporting: bool,
    pub export_progress: f64,
    pub export_cancel: Arc<std::sync::atomic::AtomicBool>,
    pub export_dialog: Option<FileDialog>,
    pub export_save_path: Option<PathBuf>,
    pub export_done_message: Option<String>,
    pub current_path: Option<PathBuf>,
    pub error_message: Option<String>,
    pub show_instrument_dialog: bool,
    pub show_piano_roll: bool,
    pub editing_midi_clip_id: Option<Uuid>,
    pub undo_state: undo::UndoStack,
    pub preferences: PreferencesState,
    pub plugin_registry: Vec<PluginDescriptor>,
    pub waveform_cache: std::collections::HashMap<Uuid, egui::TextureHandle>,
    pub midi_thumb_cache: std::collections::HashMap<Uuid, egui::TextureHandle>,
}

impl HdawApp {
    pub fn new() -> Self {
        let mut engine = AudioEngine::new();
        engine.init();

        let mut app = Self {
            project: Project::new(),
            engine,
            track_ui: Vec::new(),
            selected_track: None,
            toolbar_state: ToolbarState::default(),
            timeline_state: TimelineState::default(),
            mixer_state: MixerPanelState::default(),
            effect_editor_state: EffectEditorState::default(),
            audio_pool_state: AudioPoolPanelState::default(),
            play_requested: false,
            pause_requested: false,
            stop_requested: false,
            seek_requested: false,
            seek_frame: 0,
            record_requested: false,
            recording: false,
            file_dialog: None,
            import_requested: false,
            midi_file_dialog: None,
            midi_import_requested: false,
            midi_import_tracks: Vec::new(),
            midi_track_selection: Vec::new(),
            midi_import_file_name: String::new(),
            show_midi_track_selector: false,
            save_dialog: None,
            open_dialog: None,
            new_project_requested: false,
            save_requested: false,
            save_as_requested: false,
            open_requested: false,
            export_requested: false,
            export_bit_depth: 16,
            export_use_loop_range: false,
            exporting: false,
            export_progress: 0.0,
            export_cancel: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            export_dialog: None,
            export_save_path: None,
            export_done_message: None,
            current_path: None,
            error_message: None,
            show_instrument_dialog: false,
            show_piano_roll: false,
            editing_midi_clip_id: None,
            undo_state: undo::UndoStack::new(),
            preferences: prefs_io::load_preferences().unwrap_or_default(),
            plugin_registry: Vec::new(),
            waveform_cache: std::collections::HashMap::new(),
            midi_thumb_cache: std::collections::HashMap::new(),
        };
        app.project.bpm = app.preferences.default_bpm;
        app.project.time_signature_num = app.preferences.default_time_sig_num;
        app.project.time_signature_den = app.preferences.default_time_sig_den;
        app.timeline_state.pixels_per_second = app.preferences.default_zoom;
        app.timeline_state.snap_enabled = app.preferences.snap_default;
        app.timeline_state.header_width = app.preferences.header_width;
        app.timeline_state.track_height = app.preferences.track_height;
        app.mixer_state.visible = app.preferences.show_mixer_on_start;
        app.audio_pool_state.visible = app.preferences.show_pool_on_start;
        app.scan_plugins();
        app.add_blank_track();
        app
    }

    pub fn master_volume(&self) -> f32 {
        self.engine.master_bus.get_volume()
    }

    pub fn set_master_volume(&self, vol: f32) {
        self.engine.master_bus.set_volume(vol);
    }

    pub fn play(&self) {
        self.engine.play();
    }

    pub fn pause(&self) {
        self.engine.pause();
    }

    pub fn stop(&self) {
        self.engine.stop();
    }

    pub fn is_playing(&self) -> bool {
        self.engine.transport.is_playing()
    }

    pub fn project_length_frames(&self) -> u64 {
        let mut max_end = 0u64;
        if let Ok(tracks) = self.engine.tracks.lock() {
            for handle in tracks.iter() {
                for clip in &handle.clips {
                    let pos = clip.position_frames.load(Ordering::Acquire);
                    let len = clip.length_frames.load(Ordering::Acquire);
                    max_end = max_end.max(pos + len);
                }
            }
        }
        max_end
    }

    pub fn position_seconds(&self) -> f64 {
        self.engine.transport.position_seconds()
    }

    pub fn undo(&mut self) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_state.undo() {
                match cmd {
                    undo::UndoCommand::AddTrack { track_index, .. } => {
                        if *track_index < tracks.len() {
                            // Deactivate CLAP effects before removing
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
                        if *track_index < self.track_ui.len() {
                            self.track_ui.remove(*track_index);
                        }
                        self.selected_track = None;
                        self.effect_editor_state.selected_track = None;
                    }
                    undo::UndoCommand::RecordAudio { track_indices, clip_ids } => {
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
                    undo::UndoCommand::ImportAudio { tracks: snapshots }
                    | undo::UndoCommand::ImportMidi { tracks: snapshots } => {
                        // Undo: remove tracks from both models, going backwards
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
                            if ti < self.track_ui.len() {
                                self.track_ui.remove(ti);
                            }
                        }
                        self.selected_track = None;
                        self.effect_editor_state.selected_track = None;
                    }
                    _ => undo::apply_undo(&mut self.project, &mut tracks, cmd, sr),
                }
            }
        }
    }

    pub fn redo(&mut self) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_state.redo() {
                match cmd {
                    undo::UndoCommand::AddTrack { track_index, track, track_ui } => {
                        let handle = crate::project::track::TrackHandle::new();
                        handle.volume.store(f32::to_bits(track.volume), std::sync::atomic::Ordering::Release);
                        handle.pan.store(f32::to_bits(track.pan), std::sync::atomic::Ordering::Release);
                        handle.mute.store(track.mute, std::sync::atomic::Ordering::Release);
                        handle.solo.store(track.solo, std::sync::atomic::Ordering::Release);
                        let idx = (*track_index).min(tracks.len());
                        tracks.insert(idx, handle);
                        self.project.tracks.insert(idx, track.clone());
                        self.track_ui.insert(idx, track_ui.clone());
                    }
                    undo::UndoCommand::DeleteTrack { track_index, .. } => {
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
                        if *track_index < self.track_ui.len() {
                            self.track_ui.remove(*track_index);
                        }
                        self.selected_track = None;
                        self.effect_editor_state.selected_track = None;
                    }
                    undo::UndoCommand::ImportAudio { tracks: snapshots }
                    | undo::UndoCommand::ImportMidi { tracks: snapshots } => {
                        for snap in snapshots {
                            let track = &snap.track;
                            let track_ui = &snap.track_ui;
                            let mut handle = crate::project::track::TrackHandle::new();
                            handle.volume.store(f32::to_bits(track.volume), std::sync::atomic::Ordering::Release);
                            handle.pan.store(f32::to_bits(track.pan), std::sync::atomic::Ordering::Release);
                            handle.mute.store(track.mute, std::sync::atomic::Ordering::Release);
                            handle.solo.store(track.solo, std::sync::atomic::Ordering::Release);
                            // Restore clips to engine handle
                            for clip_kind in &track.clips {
                                match clip_kind {
                                    crate::project::clip::ClipKind::Audio(audio_clip) => {
                                        if let Some(buf) = &audio_clip.buffer {
                                            let ch = crate::project::clip_handle::ClipHandle::new(
                                                audio_clip.id,
                                                (**buf.samples()).to_vec(),
                                                buf.channels(),
                                                buf.sample_rate(),
                                            );
                                            ch.set_position(audio_clip.position_frames);
                                            ch.set_offset(audio_clip.offset_frames);
                                            ch.set_length(audio_clip.length_frames);
                                            handle.add_clip(ch);
                                        }
                                    }
                                    crate::project::clip::ClipKind::Midi(midi_clip) => {
                                        let ch = crate::project::clip_handle::ClipHandle::new_midi(
                                            midi_clip.id,
                                            midi_clip.notes.clone(),
                                            midi_clip.length_frames,
                                            sr,
                                        );
                                        ch.set_position(midi_clip.position_frames);
                                        handle.add_clip(ch);
                                    }
                                }
                            }
                            let idx = tracks.len();
                            tracks.insert(idx, handle);
                            self.project.tracks.insert(idx, track.clone());
                            self.track_ui.insert(idx, track_ui.clone());
                        }
                    }
                    _ => undo::apply_redo(&mut self.project, &mut tracks, cmd, sr),
                }
            }
        }
    }

    pub fn apply_preferences(&mut self, prefs: &PreferencesState) {
        self.engine.rebuild_stream_with_config(
            &prefs.audio_device,
            prefs.sample_rate,
            match prefs.buffer_size {
                crate::ui::preferences::BufferSizePref::Small => cpal::BufferSize::Fixed(64),
                crate::ui::preferences::BufferSizePref::Default => cpal::BufferSize::Default,
                crate::ui::preferences::BufferSizePref::Large => cpal::BufferSize::Fixed(2048),
            },
        );
        self.timeline_state.header_width = prefs.header_width;
        self.timeline_state.track_height = prefs.track_height;
        self.mixer_state.visible = prefs.show_mixer_on_start;
        self.audio_pool_state.visible = prefs.show_pool_on_start;
        self.effect_editor_state.show_editor = prefs.show_effect_editor_on_start;
        // Invalidate cached thumbnails when track height changes
        self.waveform_cache.clear();
        self.midi_thumb_cache.clear();
    }

    pub fn scan_plugins(&mut self) {
        self.plugin_registry = crate::audio::clap_scanner::scan_plugins();
    }
}

impl Default for HdawApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for HdawApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.engine.check_rebuild();

        // Sync BPM/time-sig from project -> transport (for audio thread metronome)
        let pos = self.engine.transport.position_frames();
        self.engine.transport.set_bpm(self.project.tempo_at(pos));
        let (ts_num, ts_den) = self.project.time_sig_at(pos);
        self.engine.transport.set_time_sig_num(ts_num as u32);
        self.engine.transport.set_time_sig_den(ts_den as u32);

        input::handle_keyboard_input(self, ctx);
        input::handle_pending_requests(self, ctx);
        self.handle_file_dialog(ctx);
        self.handle_midi_file_dialog(ctx);
        self.handle_midi_track_selector(ctx);
        
        // Sync UI visibility back to preferences for persistence
        self.preferences.show_mixer_on_start = self.mixer_state.visible;
        self.preferences.show_pool_on_start = self.audio_pool_state.visible;
        self.preferences.show_effect_editor_on_start = self.effect_editor_state.show_editor;

        crate::ui::app_ui::render(self, ctx);
        if self.is_playing() {
            ctx.request_repaint();
        }
    }
}

impl HdawApp {
    pub fn toggle_loop(&self) {
        self.engine.transport.toggle_loop();
    }

    pub fn start_recording(&mut self) {
        // Determine recording directory
        let rec_dir = if let Some(ref proj_path) = self.current_path {
            let mut dir = proj_path.parent().unwrap_or(&std::path::PathBuf::from(".")).to_path_buf();
            dir.push("Audio Recordings");
            dir
        } else {
            std::path::PathBuf::from("Audio Recordings")
        };
        let _ = std::fs::create_dir_all(&rec_dir);

        // Build a timestamp-based filename
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Find first armed track name for the filename
        let track_name = self.track_ui.iter()
            .find(|t| t.armed.load(Ordering::Acquire))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Recorded".to_string());

        let sanitized: String = track_name.chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
            .collect();

        let file_name = format!("{}_{}.wav", sanitized.trim(), now);
        let file_path = rec_dir.join(&file_name);

        let sr = self.engine.transport.sample_rate();
        let channels: u16 = 2;
        let start_frame = self.engine.transport.position_frames();
        let device_name = if self.preferences.audio_device.is_empty() {
            None
        } else {
            Some(self.preferences.audio_device.as_str())
        };

        match self.engine.start_recording(file_path.clone(), sr, channels, start_frame, device_name) {
            Ok(()) => {
                self.play();
                self.recording = true;
                tracing::info!("recording started: {}", file_path.display());
            }
            Err(e) => {
                self.error_message = Some(format!("record: {e}"));
            }
        }
    }

    pub fn finish_recording(&mut self) {
        let info = self.engine.stop_recording();
        self.recording = false;

        let Some((result, file_path, start_frame)) = info else {
            self.error_message = Some("recording failed to finalize".to_string());
            return;
        };

        // Load the recorded WAV
        let (samples, channels, file_sr) = match crate::utils::load_wav_file(
            file_path.to_str().unwrap_or(""),
        ) {
            Ok(v) => v,
            Err(e) => {
                self.error_message = Some(format!("load recording: {e}"));
                return;
            }
        };

        let engine_sr = self.engine.transport.sample_rate();
        let samples = if file_sr != engine_sr {
            crate::app::project_io::resample(&samples, channels, file_sr, engine_sr)
        } else {
            samples
        };

        let source_path = file_path.clone();

        // Create clips on all armed tracks
        let track_indices: Vec<usize> = self.track_ui.iter()
            .enumerate()
            .filter(|(_, t)| t.armed.load(Ordering::Acquire))
            .map(|(i, _)| i)
            .collect();

        if track_indices.is_empty() {
            return;
        }

        let _clip_id = uuid::Uuid::new_v4();
        let buffer = AudioBuffer::from_interleaved(samples, channels, engine_sr);
        let file_name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Recording")
            .to_string();

        let clip_kind = ClipKind::Audio(AudioClip::with_source_path(
            file_name,
            buffer,
            source_path,
        ));

        let buf = match &clip_kind {
            ClipKind::Audio(a) => match a.buffer.as_ref() {
                Some(b) => b,
                None => return,
            },
            _ => return,
        };

        let pool_entry = PoolClip::from_clip(clip_kind.clone());

        // Track clip IDs for undo
        let mut engine_clip_ids: Vec<uuid::Uuid> = Vec::new();

        if let Ok(mut tracks) = self.engine.tracks.lock() {
            for &ti in &track_indices {
                if let Some(track) = tracks.get_mut(ti) {
                    let new_id = uuid::Uuid::new_v4();
                    let ch = ClipHandle::new(
                        new_id,
                        (**buf.samples()).to_vec(),
                        channels,
                        engine_sr,
                    );
                    ch.set_position(start_frame);
                    ch.set_length(result.num_frames);
                    track.add_clip(ch);
                    engine_clip_ids.push(new_id);
                }
            }
        }

        for &ti in &track_indices {
            if let Some(pt) = self.project.tracks.get_mut(ti) {
                pt.add_clip(clip_kind.clone());
            }
        }

        self.project.audio_pool.push(pool_entry);

        self.undo_state.push(crate::app::undo::UndoCommand::RecordAudio {
            track_indices: track_indices.clone(),
            clip_ids: engine_clip_ids,
        });

        // Disarm all tracks
        for tui in &self.track_ui {
            tui.armed.store(false, Ordering::Release);
        }
        if let Ok(tracks) = self.engine.tracks.lock() {
            for handle in tracks.iter() {
                handle.armed.store(false, Ordering::Release);
            }
        }

        tracing::info!(
            "recording finished: {} ({} frames, {} Hz)",
            file_path.display(),
            result.num_frames,
            engine_sr,
        );
    }

    pub fn add_marker_at_playhead(&mut self, name: String) {
        let frame = self.engine.transport.position_frames();
        let marker = crate::project::marker::Marker::new(frame, name);
        self.project.markers.push(marker);
    }

    pub fn handle_pool_drop(&mut self, response: &egui::Response, ui: &egui::Ui, rect: &egui::Rect) {
        let drag_id = match self.audio_pool_state.dragging_clip_id {
            Some(id) => id,
            None => return,
        };
        if !response.drag_stopped() {
            return;
        }
        let pos = match ui.input(|i| i.pointer.interact_pos()) {
            Some(p) => p,
            None => {
                self.audio_pool_state.dragging_clip_id = None;
                return;
            }
        };
        let header_width = self.timeline_state.header_width;
        let track_height = self.timeline_state.track_height;
        if pos.x < rect.left() + header_width || pos.x > rect.right() {
            self.audio_pool_state.dragging_clip_id = None;
            return;
        }
        if pos.y < rect.top() + 20.0 || pos.y > rect.bottom() {
            self.audio_pool_state.dragging_clip_id = None;
            return;
        }
        let sr = self.engine.transport.sample_rate();
        let track_index = ((pos.y - rect.top() - 20.0) as f64 - self.timeline_state.scroll_y) as usize / (track_height as usize);
        if track_index >= self.project.tracks.len() {
            self.audio_pool_state.dragging_clip_id = None;
            return;
        }
        let timeline_x = (pos.x - rect.left() - header_width) as f64 + self.timeline_state.scroll_x;
        let time = timeline_x / self.timeline_state.pixels_per_second;
        let frame = (time * sr as f64).round().max(0.0) as u64;
        self.restore_pool_clip_to_track_at(drag_id, track_index, frame);
        self.audio_pool_state.dragging_clip_id = None;
    }

    pub fn import_midi(&mut self) {
        let dir = self.preferences.last_import_dir.clone();
        let mut dialog = egui_file_dialog::FileDialog::new();
        if let Some(d) = &dir {
            dialog = dialog.initial_directory(d.clone());
        }
        dialog = dialog.add_file_filter(
            "MIDI Files",
            Arc::new(|path: &std::path::Path| -> bool {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("mid") || e.eq_ignore_ascii_case("midi"))
                    .unwrap_or(false)
            }),
        );
        dialog.pick_file();
        self.midi_file_dialog = Some(dialog);
        self.midi_import_requested = true;
    }

    pub(crate) fn handle_midi_file_dialog(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.midi_file_dialog {
            dialog.update(ctx);

            if let Some(path) = dialog.picked() {
                let path_buf = path.to_path_buf();
                self.preferences.last_import_dir = path_buf.parent().map(|p| p.to_path_buf());
                crate::app::prefs_io::save_preferences(&self.preferences);
                if let Some(path_str) = path_buf.to_str() {
                    self.midi_import_file_name = path_buf
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string();
                    let bpm = self.project.bpm;
                    let sr = self.engine.transport.sample_rate();
                    match crate::project::midi_import::parse_midi_file(path_str, bpm, sr) {
                        Ok(tracks) => {
                            if tracks.len() <= 1 {
                                // Single track: import directly
                                self.commit_midi_import(tracks);
                            } else {
                                // Multi-track: show selection dialog
                                self.midi_import_tracks = tracks;
                                self.midi_track_selection = vec![true; self.midi_import_tracks.len()];
                                self.show_midi_track_selector = true;
                            }
                        }
                        Err(e) => {
                            self.error_message = Some(format!("MIDI import error: {e}"));
                        }
                    }
                }
                self.midi_file_dialog = None;
                self.midi_import_requested = false;
            } else if !self.midi_import_requested {
                self.midi_file_dialog = None;
            }
        }
    }

    pub(crate) fn handle_midi_track_selector(&mut self, ctx: &egui::Context) {
        if !self.show_midi_track_selector {
            return;
        }

        let tracks = std::mem::take(&mut self.midi_import_tracks);
        let selection = std::mem::take(&mut self.midi_track_selection);
        let file_name = self.midi_import_file_name.clone();
        self.show_midi_track_selector = false;

        egui::Window::new("Select MIDI Tracks to Import")
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(format!("File: {}", file_name));
                ui.separator();
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let mut sel = selection.clone();
                    for (i, track) in tracks.iter().enumerate() {
                        ui.checkbox(&mut sel[i], format!("{} ({} notes)", track.name, track.notes.len()));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Import Selected").clicked() {
                            let selected: Vec<_> = tracks.into_iter().enumerate()
                                .filter(|(i, _)| sel[*i])
                                .map(|(_, t)| t)
                                .collect();
                            if !selected.is_empty() {
                                self.commit_midi_import(selected);
                            }
                            ui.close_menu();
                        }
                        if ui.button("Cancel").clicked() {
                            ui.close_menu();
                        }
                    });
                });
            });
    }

    fn commit_midi_import(&mut self, imported_tracks: Vec<crate::project::midi_import::ImportedMidiTrack>) {
        let sr = self.engine.transport.sample_rate();
        let mut snapshots: Vec<crate::app::undo::ImportTrackSnapshot> = Vec::new();

        for imported in &imported_tracks {
            let clip_id = uuid::Uuid::new_v4();
            let name = imported.name.clone();

            let notes = imported.notes.clone();
            let duration = imported.duration_frames;

            let midi_clip = crate::project::midi_clip::MidiClip {
                id: clip_id,
                name: name.clone(),
                position_frames: 0,
                length_frames: duration,
                notes: notes.clone(),
                color: [0x1a, 0x2a, 0x3a],
                thumb_dirty: true,
            };
            let clip_kind = crate::project::clip::ClipKind::Midi(midi_clip);

            let clip_handle = crate::project::clip_handle::ClipHandle::new_midi(clip_id, notes, duration, sr);
            clip_handle.set_position(0);

            let mut engine_handle = crate::project::track::TrackHandle::new();
            engine_handle.add_clip(clip_handle);

            let track_ui = crate::app::TrackUiState {
                id: engine_handle.id,
                name: name.clone(),
                color: [0x1a, 0x2a, 0x3a],
                volume: engine_handle.volume.clone(),
                pan: engine_handle.pan.clone(),
                mute: engine_handle.mute.clone(),
                solo: engine_handle.solo.clone(),
                armed: engine_handle.armed.clone(),
                peak_left: engine_handle.peak_left.clone(),
                peak_right: engine_handle.peak_right.clone(),
                parent_group: None,
                is_group: false,
                is_return: false,
                collapsed: false,
                send_levels: Vec::new(),
            };

            let mut project_track = crate::project::track::Track::new(name);
            project_track.add_clip(clip_kind);

            self.track_ui.push(track_ui.clone());
            self.engine.add_track(engine_handle);
            self.project.add_track(project_track.clone());

            snapshots.push(crate::app::undo::ImportTrackSnapshot::new(project_track, track_ui));
        }

        self.undo_state.push(crate::app::undo::UndoCommand::ImportMidi { tracks: snapshots });
    }
}
