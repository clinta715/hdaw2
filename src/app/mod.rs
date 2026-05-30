pub mod commands;
pub mod input;
pub mod prefs_io;
pub mod project_io;
pub mod undo;

use crate::audio::clap_scanner::PluginDescriptor;
use crate::audio::engine::AudioEngine;
use crate::project::Project;
use crate::ui::audio_pool::AudioPoolPanelState;
use crate::ui::effect_editor::EffectEditorState;
use crate::ui::mixer_panel::MixerPanelState;
use crate::ui::preferences::PreferencesState;
use crate::ui::timeline::TimelineState;
use crate::ui::toolbar::ToolbarState;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use uuid::Uuid;

pub struct TrackUiState {
    pub name: String,
    pub color: [u8; 3],
    pub volume: Arc<AtomicU32>,
    pub pan: Arc<AtomicU32>,
    pub mute: Arc<AtomicBool>,
    pub solo: Arc<AtomicBool>,
    pub peak_left: Arc<AtomicU32>,
    pub peak_right: Arc<AtomicU32>,
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
    pub seek_frame: u64,
    pub file_dialog: Option<FileDialog>,
    pub import_requested: bool,
    pub save_dialog: Option<FileDialog>,
    pub open_dialog: Option<FileDialog>,
    pub new_project_requested: bool,
    pub save_requested: bool,
    pub save_as_requested: bool,
    pub open_requested: bool,
    pub current_path: Option<PathBuf>,
    pub error_message: Option<String>,
    pub show_instrument_dialog: bool,
    pub show_piano_roll: bool,
    pub editing_midi_clip_id: Option<Uuid>,
    pub undo_state: undo::UndoStack,
    pub preferences: PreferencesState,
    pub plugin_registry: Vec<PluginDescriptor>,
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
            file_dialog: None,
            import_requested: false,
            save_dialog: None,
            open_dialog: None,
            new_project_requested: false,
            save_requested: false,
            save_as_requested: false,
            open_requested: false,
            current_path: None,
            error_message: None,
            show_instrument_dialog: false,
            show_piano_roll: false,
            editing_midi_clip_id: None,
            undo_state: undo::UndoStack::new(),
            preferences: prefs_io::load_preferences().unwrap_or_default(),
            plugin_registry: Vec::new(),
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

    pub fn position_seconds(&self) -> f64 {
        self.engine.transport.position_seconds()
    }

    pub fn undo(&mut self) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_state.undo() {
                undo::apply_undo(&mut self.project, &mut tracks, cmd, sr);
            }
        }
    }

    pub fn redo(&mut self) {
        let sr = self.engine.transport.sample_rate();
        if let Ok(mut tracks) = self.engine.tracks.lock() {
            if let Some(cmd) = self.undo_state.redo() {
                undo::apply_redo(&mut self.project, &mut tracks, cmd, sr);
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
        input::handle_keyboard_input(self, ctx);
        input::handle_pending_requests(self, ctx);
        self.handle_file_dialog(ctx);
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
}
