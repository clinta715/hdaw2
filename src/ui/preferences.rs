use crate::app::HdawApp;
use egui::{CollapsingHeader, ComboBox, Context, RichText, Vec2};
use std::path::PathBuf;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum BufferSizePref {
    Small,
    Default,
    Large,
}

impl Default for BufferSizePref {
    fn default() -> Self {
        Self::Default
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
    pub track_height: f32,
    pub header_width: f32,
    pub show_mixer_on_start: bool,
    pub show_pool_on_start: bool,
    pub effect_panel_width: f32,
    #[serde(default)]
    pub last_import_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_open_dir: Option<PathBuf>,
    #[serde(default)]
    pub last_save_dir: Option<PathBuf>,
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
            snap_default: false,
            track_height: 80.0,
            header_width: 220.0,
            show_mixer_on_start: true,
            show_pool_on_start: false,
            effect_panel_width: 280.0,
            last_import_dir: None,
            last_open_dir: None,
            last_save_dir: None,
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

            CollapsingHeader::new("Timeline / UI")
                .default_open(false)
                .show(ui, |ui| {
                    ui.add_space(4.0);
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
                    ui.checkbox(&mut app.preferences.snap_default, "Snap to grid by default");
                    ui.add_space(4.0);
                    ui.checkbox(&mut app.preferences.show_mixer_on_start, "Show Mixer on startup");
                    ui.checkbox(&mut app.preferences.show_pool_on_start, "Show Audio Pool on startup");
                    ui.horizontal(|ui| {
                        ui.label("Effect Panel Width:");
                        ui.add(egui::DragValue::new(&mut app.preferences.effect_panel_width)
                            .speed(5.0)
                            .range(150.0..=500.0));
                    });
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