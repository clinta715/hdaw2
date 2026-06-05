use crate::app::HdawApp;
use egui::{CollapsingHeader, ComboBox, Context, RichText, Vec2};
use std::path::PathBuf;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum BufferSizePref {
    Small,
    #[default]
    Default,
    Large,
}

#[derive(Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GridDivision {
    #[default]
    Adaptive,
    Bar,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl GridDivision {
    pub fn to_beats(self) -> f64 {
        match self {
            Self::Adaptive => 0.0,
            Self::Bar => 4.0,
            Self::Half => 2.0,
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Adaptive => "Adaptive",
            Self::Bar => "1 Bar",
            Self::Half => "1/2 Note",
            Self::Quarter => "1/4 Note",
            Self::Eighth => "1/8 Note",
            Self::Sixteenth => "1/16 Note",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg_fill: egui::Color32,
    pub grid_line: egui::Color32,
    pub grid_bar: egui::Color32,
    pub track_bg: egui::Color32,
    pub track_bg_alt: egui::Color32,
    pub text_normal: egui::Color32,
    pub text_dim: egui::Color32,
    pub clip_default: egui::Color32,
    pub selection: egui::Color32,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            bg_fill: egui::Color32::from_rgb(0x1e, 0x1e, 0x1e),
            grid_line: egui::Color32::from_rgba_premultiplied(80, 80, 90, 40),
            grid_bar: egui::Color32::from_rgba_premultiplied(100, 100, 120, 80),
            track_bg: egui::Color32::from_rgb(0x2c, 0x2c, 0x2c),
            track_bg_alt: egui::Color32::from_rgb(0x22, 0x22, 0x22),
            text_normal: egui::Color32::from_gray(220),
            text_dim: egui::Color32::from_gray(140),
            clip_default: egui::Color32::from_rgb(0x5c, 0x3a, 0x8a),
            selection: egui::Color32::from_rgb(0x64, 0xb5, 0xf6),
        }
    }
    pub fn light() -> Self {
        Self {
            bg_fill: egui::Color32::from_rgb(0xf0, 0xf0, 0xf0),
            grid_line: egui::Color32::from_rgba_premultiplied(160, 160, 170, 60),
            grid_bar: egui::Color32::from_rgba_premultiplied(120, 120, 140, 100),
            track_bg: egui::Color32::from_rgb(0xe0, 0xe0, 0xe0),
            track_bg_alt: egui::Color32::from_rgb(0xd4, 0xd4, 0xd4),
            text_normal: egui::Color32::from_gray(30),
            text_dim: egui::Color32::from_gray(120),
            clip_default: egui::Color32::from_rgb(0x9c, 0x7a, 0xca),
            selection: egui::Color32::from_rgb(0x19, 0x70, 0xd2),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PreferencesState {
    pub show_dialog: bool,
    pub audio_device: String,
    pub sample_rate: u32,
    pub buffer_size: BufferSizePref,
    pub default_bpm: f64,
    pub default_time_sig_num: u8,
    pub default_time_sig_den: u8,
    pub default_zoom: f64,
    pub snap_default: bool,
    pub snap_to_markers: bool,
    pub grid_division: GridDivision,
    pub grid_opacity: f32,
    pub track_height: f32,
    pub header_width: f32,
    pub show_mixer_on_start: bool,
    pub show_pool_on_start: bool,
    pub show_effect_editor_on_start: bool,
    pub effect_panel_width: f32,
    pub mixer_panel_height: f32,
    
    // Playhead / Scrolling
    pub follow_playhead: bool,
    pub piano_roll_follow_playhead: bool,
    pub center_playhead_on_zoom: bool,
    
    // Piano Roll Prefs
    pub piano_roll_row_height: f32,
    pub piano_roll_min_note: u8,
    pub piano_roll_max_note: u8,
    pub piano_roll_default_velocity: u8,

    #[serde(default)]
    pub last_import_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_open_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_save_dir: Option<PathBuf>,
    #[serde(default)]
    pub recent_files: Vec<PathBuf>,
    #[serde(default)]
    pub dark_mode: bool,
}

impl PreferencesState {
    pub fn theme(&self) -> Theme {
        if self.dark_mode { Theme::dark() } else { Theme::light() }
    }
    pub fn push_recent_file(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
    }
}

impl Default for PreferencesState {
    fn default() -> Self {
        Self {
            show_dialog: false,
            audio_device: String::new(),
            sample_rate: 48000,
            buffer_size: BufferSizePref::Default,
            default_bpm: 120.0,
            default_time_sig_num: 4,
            default_time_sig_den: 4,
            default_zoom: 100.0,
            snap_default: true,
            snap_to_markers: true,
            grid_division: GridDivision::Adaptive,
            grid_opacity: 0.5,
            track_height: 80.0,
            header_width: 220.0,
            show_mixer_on_start: true,
            show_pool_on_start: false,
            show_effect_editor_on_start: true,
            effect_panel_width: 280.0,
            mixer_panel_height: 220.0,
            
            follow_playhead: true,
            piano_roll_follow_playhead: true,
            center_playhead_on_zoom: true,
            
            piano_roll_row_height: 14.0,
            piano_roll_min_note: 24, // C1
            piano_roll_max_note: 96, // C7
            piano_roll_default_velocity: 100,

            last_import_dir: None,
            last_open_dir: None,
            last_save_dir: None,
            recent_files: Vec::new(),
            dark_mode: true,
        }
    }
}

pub fn render(ctx: &Context, app: &mut HdawApp) {
    if !app.preferences.show_dialog {
        return;
    }

    let available_devices = crate::audio::engine::AudioEngine::available_devices();
    let mut apply = false;
    let mut close = false;

    egui::Window::new("Preferences")
        .open(&mut app.preferences.show_dialog)
        .default_size(Vec2::new(400.0, 500.0))
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            CollapsingHeader::new("Audio")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.label("Audio Device:");
                    let device_label = if app.preferences.audio_device.is_empty() {
                        "Default".to_string()
                    } else {
                        app.preferences.audio_device.clone()
                    };
                    ComboBox::from_id_salt("audio_device")
                        .selected_text(&device_label)
                        .show_ui(ui, |ui| {
                            let default_sel = app.preferences.audio_device.is_empty();
                            if ui.selectable_label(default_sel, "Default").clicked() {
                                app.preferences.audio_device = String::new();
                            }
                            for dev in &available_devices {
                                let sel = *dev == app.preferences.audio_device;
                                if ui.selectable_label(sel, dev).clicked() {
                                    app.preferences.audio_device = dev.clone();
                                }
                            }
                        });

                    ui.add_space(4.0);
                    ui.label("Sample Rate:");
                    ComboBox::from_id_salt("sample_rate")
                        .selected_text(format!("{} Hz", app.preferences.sample_rate))
                        .show_ui(ui, |ui| {
                            for &rate in &[44100u32, 48000, 96000] {
                                if ui.selectable_label(app.preferences.sample_rate == rate, format!("{} Hz", rate)).clicked() {
                                    app.preferences.sample_rate = rate;
                                }
                            }
                        });

                    ui.add_space(4.0);
                    ui.label("Buffer Size:");
                    ComboBox::from_id_salt("buffer_size")
                        .selected_text(match app.preferences.buffer_size {
                            BufferSizePref::Small => "Small (low latency)",
                            BufferSizePref::Default => "Default",
                            BufferSizePref::Large => "Large (stable)",
                        })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(matches!(app.preferences.buffer_size, BufferSizePref::Small), "Small (low latency)").clicked() {
                                app.preferences.buffer_size = BufferSizePref::Small;
                            }
                            if ui.selectable_label(matches!(app.preferences.buffer_size, BufferSizePref::Default), "Default").clicked() {
                                app.preferences.buffer_size = BufferSizePref::Default;
                            }
                            if ui.selectable_label(matches!(app.preferences.buffer_size, BufferSizePref::Large), "Large (stable)").clicked() {
                                app.preferences.buffer_size = BufferSizePref::Large;
                            }
                        });
                });

            ui.add_space(8.0);

            CollapsingHeader::new("Project Defaults")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Default BPM:");
                        ui.add(egui::DragValue::new(&mut app.preferences.default_bpm)
                            .speed(1.0)
                            .range(20.0..=300.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Time Signature:");
                        ui.add(egui::DragValue::new(&mut app.preferences.default_time_sig_num)
                            .speed(1.0)
                            .range(1..=32));
                        ui.label("/");
                        ui.add(egui::DragValue::new(&mut app.preferences.default_time_sig_den)
                            .speed(1.0)
                            .range(1..=32));
                    });
                });

            ui.add_space(8.0);

            CollapsingHeader::new("Playhead & Scrolling")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.checkbox(&mut app.preferences.follow_playhead, "Follow Playhead on Timeline");
                    ui.checkbox(&mut app.preferences.piano_roll_follow_playhead, "Follow Playhead on Piano Roll");
                    ui.checkbox(&mut app.preferences.center_playhead_on_zoom, "Center Zoom on Playhead");
                });

            CollapsingHeader::new("Timeline / Snap / Grid")
                .default_open(true)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.checkbox(&mut app.preferences.snap_default, "Enable Snapping by default");
                    ui.checkbox(&mut app.preferences.snap_to_markers, "Snap to Markers");
                    
                    ui.horizontal(|ui| {
                        ui.label("Grid Division:");
                        ComboBox::from_id_salt("grid_division")
                            .selected_text(app.preferences.grid_division.label())
                            .show_ui(ui, |ui| {
                                for div in &[GridDivision::Adaptive, GridDivision::Bar, GridDivision::Half, GridDivision::Quarter, GridDivision::Eighth, GridDivision::Sixteenth] {
                                    if ui.selectable_label(app.preferences.grid_division == *div, div.label()).clicked() {
                                        app.preferences.grid_division = *div;
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Grid Intensity:");
                        ui.add(egui::Slider::new(&mut app.preferences.grid_opacity, 0.0..=1.0));
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Default Zoom (px/sec):");
                        ui.add(egui::DragValue::new(&mut app.preferences.default_zoom)
                            .speed(5.0)
                            .range(20.0..=500.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Track Height:");
                        ui.add(egui::DragValue::new(&mut app.preferences.track_height)
                            .speed(1.0)
                            .range(40.0..=200.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Header Width:");
                        ui.add(egui::DragValue::new(&mut app.preferences.header_width)
                            .speed(1.0)
                            .range(100.0..=400.0));
                    });
                    
                    ui.add_space(4.0);
                    ui.checkbox(&mut app.preferences.show_mixer_on_start, "Show Mixer on startup");
                    ui.checkbox(&mut app.preferences.show_pool_on_start, "Show Audio Pool on startup");
                    ui.checkbox(&mut app.preferences.show_effect_editor_on_start, "Show FX Editor on startup");
                    ui.horizontal(|ui| {
                        ui.label("Effect Panel Width:");
                        ui.add(egui::DragValue::new(&mut app.preferences.effect_panel_width)
                            .speed(5.0)
                            .range(150.0..=500.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Mixer Panel Height:");
                        ui.add(egui::DragValue::new(&mut app.preferences.mixer_panel_height)
                            .speed(5.0)
                            .range(100.0..=600.0));
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        let dm = app.preferences.dark_mode;
                        if ui.selectable_label(dm, "Dark").clicked() {
                            app.preferences.dark_mode = true;
                            apply = true;
                        }
                        if ui.selectable_label(!dm, "Light").clicked() {
                            app.preferences.dark_mode = false;
                            apply = true;
                        }
                    });
                });

            ui.add_space(8.0);

            CollapsingHeader::new("Piano Roll")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Row Height:");
                        ui.add(egui::Slider::new(&mut app.preferences.piano_roll_row_height, 8.0..=32.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Default Velocity:");
                        ui.add(egui::Slider::new(&mut app.preferences.piano_roll_default_velocity, 1..=127));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Note Range (MIDI):");
                        ui.add(egui::DragValue::new(&mut app.preferences.piano_roll_min_note)
                            .prefix("Min: ")
                            .range(0..=127));
                        ui.add(egui::DragValue::new(&mut app.preferences.piano_roll_max_note)
                            .prefix("Max: ")
                            .range(0..=127));
                    });
                    if app.preferences.piano_roll_min_note >= app.preferences.piano_roll_max_note {
                        ui.colored_label(egui::Color32::from_rgb(0xff, 0x44, 0x44), "Min must be less than Max");
                    }
                });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                    ui.add_space(4.0);
                    if ui.button(RichText::new("Apply").strong()).clicked() {
                        apply = true;
                    }
                });
            });
            });
        });

    if apply {
        let prefs = app.preferences.clone();
        app.apply_preferences(&prefs);
        crate::app::prefs_io::save_preferences(&prefs);
        close = true;
    }
    if close {
        app.preferences.show_dialog = false;
    }
}