use egui::{Button, Context, menu};

const BTN_SIZE: egui::Vec2 = egui::vec2(30.0, 24.0);
const RECORD_FONT_SIZE: f32 = 14.0;
const TIME_FONT_SIZE: f32 = 16.0;

pub struct ToolbarState;

impl Default for ToolbarState {
    fn default() -> Self {
        Self
    }
}

pub struct ToolbarAction {
    pub play_clicked: bool,
    pub pause_clicked: bool,
    pub stop_clicked: bool,
    pub import_clicked: bool,
    pub midi_import_clicked: bool,
    pub fx_clicked: bool,
    pub new_clicked: bool,
    pub save_clicked: bool,
    pub save_as_clicked: bool,
    pub open_clicked: bool,
    pub open_file: Option<std::path::PathBuf>,
    pub snap_clicked: bool,
    pub undo_clicked: bool,
    pub redo_clicked: bool,
    pub loop_clicked: bool,
    pub record_clicked: bool,
    pub add_track_clicked: bool,
    pub delete_track_clicked: bool,
    pub add_instrument_clicked: bool,
    pub add_group_clicked: bool,
    pub add_return_clicked: bool,
    pub mixer_clicked: bool,
    pub pool_clicked: bool,
    pub preferences_clicked: bool,
    pub metronome_clicked: bool,
    pub export_clicked: bool,
    pub about_clicked: bool,
    pub shortcuts_clicked: bool,
}

impl Default for ToolbarAction {
    fn default() -> Self {
        Self {
            play_clicked: false,
            pause_clicked: false,
            stop_clicked: false,
            import_clicked: false,
            midi_import_clicked: false,
            fx_clicked: false,
            new_clicked: false,
            save_clicked: false,
            save_as_clicked: false,
            open_clicked: false,
            open_file: None,
            snap_clicked: false,
            undo_clicked: false,
            redo_clicked: false,
            loop_clicked: false,
            record_clicked: false,
            add_track_clicked: false,
            delete_track_clicked: false,
            add_instrument_clicked: false,
            mixer_clicked: false,
            add_group_clicked: false,
            add_return_clicked: false,
            pool_clicked: false,
            preferences_clicked: false,
            metronome_clicked: false,
            export_clicked: false,
            about_clicked: false,
            shortcuts_clicked: false,
        }
    }
}

pub fn render(
    ctx: &Context,
    is_playing: bool,
    position_secs: f64,
    bpm: f64,
    time_sig_num: u8,
    time_sig_den: u8,
    snap_enabled: bool,
    can_undo: bool,
    can_redo: bool,
    loop_enabled: bool,
    metronome_enabled: bool,
    is_recording: bool,
    mixer_visible: bool,
    has_selected_track: bool,
    pool_visible: bool,
    has_instruments: bool,
    recent_files: &[std::path::PathBuf],
) -> ToolbarAction {
    let mut action = ToolbarAction::default();

    // 1. Menu Bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Project").clicked() {
                    action.new_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Open...").clicked() {
                    action.open_clicked = true;
                    ui.close_menu();
                }
                if !recent_files.is_empty() {
                    ui.menu_button("Recent Files", |ui| {
                        for path in recent_files {
                            let name = path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("?")
                                .to_string();
                            if ui.button(name).clicked() {
                                action.open_file = Some(path.clone());
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Clear Recent").clicked() {
                            action.open_file = Some(std::path::PathBuf::from("__clear_recent__"));
                            ui.close_menu();
                        }
                    });
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    action.save_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Save As...").clicked() {
                    action.save_as_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Import WAV...").clicked() {
                    action.import_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Import MIDI...").clicked() {
                    action.midi_import_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Export Audio...").clicked() {
                    action.export_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Edit", |ui| {
                if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                    action.undo_clicked = true;
                    ui.close_menu();
                }
                if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
                    action.redo_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Preferences...").clicked() {
                    action.preferences_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Track", |ui| {
                if ui.button("Add Blank Track").clicked() {
                    action.add_track_clicked = true;
                    ui.close_menu();
                }
                if has_instruments {
                    if ui.button("Add Instrument...").clicked() {
                        action.add_instrument_clicked = true;
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("Add Group Track").clicked() {
                    action.add_group_clicked = true;
                    ui.close_menu();
                }
                if ui.button("Add Return Track").clicked() {
                    action.add_return_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has_selected_track, egui::Button::new("Delete Selected Track")).clicked() {
                    action.delete_track_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Transport", |ui| {
                if ui.button(if is_playing { "Pause" } else { "Play" }).clicked() {
                    if is_playing { action.pause_clicked = true; } else { action.play_clicked = true; }
                    ui.close_menu();
                }
                if ui.button("Stop").clicked() {
                    action.stop_clicked = true;
                    ui.close_menu();
                }
                ui.separator();
                let mut le = loop_enabled;
                if ui.checkbox(&mut le, "Loop Region").clicked() {
                    action.loop_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                let mut mixer_vis = mixer_visible;
                if ui.checkbox(&mut mixer_vis, "Mixer Panel").clicked() {
                    action.mixer_clicked = true;
                    ui.close_menu();
                }
                let mut pool_vis = pool_visible;
                if ui.checkbox(&mut pool_vis, "Audio Pool").clicked() {
                    action.pool_clicked = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts...").clicked() {
                    action.shortcuts_clicked = true;
                    ui.close_menu();
                }
                if ui.button("About HDAW...").clicked() {
                    action.about_clicked = true;
                    ui.close_menu();
                }
            });
        });
    });

    // 2. Tool Bar
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            // Transport Group
            if ui.add(Button::new(if is_playing { "\u{23F8}" } else { "\u{25B6}" })
                .min_size(BTN_SIZE))
                .clicked()
            {
                if is_playing { action.pause_clicked = true; } else { action.play_clicked = true; }
            }
            if ui.add(Button::new("\u{25A0}").min_size(BTN_SIZE)).clicked() {
                action.stop_clicked = true;
            }

            let rec_label = if is_recording { "\u{25A0}" } else { "\u{25CF}" };
            let rec_color = if is_recording {
                let pulse = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    / 500)
                    % 2
                    == 0;
                if pulse {
                    egui::Color32::from_rgb(0xff, 0x22, 0x22)
                } else {
                    egui::Color32::from_rgb(0x88, 0x11, 0x11)
                }
            } else {
                egui::Color32::from_rgb(0xcc, 0x44, 0x44)
            };
            if ui.add(
                egui::Button::new(
                    egui::RichText::new(rec_label).color(rec_color).size(RECORD_FONT_SIZE)
                )
                .min_size(BTN_SIZE)
            )
            .clicked()
            {
                action.record_clicked = true;
            }
            
            if ui.add(egui::Button::new("\u{21BA}")
                .selected(loop_enabled)
                .min_size(BTN_SIZE))
                .clicked()
            {
                action.loop_clicked = true;
            }

            if ui.add(egui::Button::new("\u{2669}")
                .selected(metronome_enabled)
                .min_size(BTN_SIZE))
                .clicked()
            {
                action.metronome_clicked = true;
            }

            ui.separator();

            // Tools / Snap
            let snap_label = "Snap";
            if ui.add(egui::Button::new(snap_label).selected(snap_enabled)).clicked() {
                action.snap_clicked = true;
            }

            ui.separator();

            // Import Dropdown
            ui.menu_button("Import", |ui| {
                if ui.button("WAV...").clicked() {
                    action.import_clicked = true;
                    ui.close_menu();
                }
                if ui.button("MIDI...").clicked() {
                    action.midi_import_clicked = true;
                    ui.close_menu();
                }
            });

            ui.separator();

            // Time Display
            let mins = (position_secs / 60.0) as u32;
            let secs = (position_secs % 60.0) as u32;
            let millis = ((position_secs % 1.0) * 1000.0) as u32;
            ui.monospace(egui::RichText::new(format!("{:02}:{:02}.{:03}", mins, secs, millis))
                .color(egui::Color32::from_rgb(0x8b, 0xc3, 0x4a))
                .size(TIME_FONT_SIZE));

            ui.separator();

            // Project Settings
            ui.label(egui::RichText::new(format!("BPM {:.1}", bpm)).small());
            ui.label(egui::RichText::new(format!("{} / {}", time_sig_num, time_sig_den)).small());

            ui.separator();

            // Panel Toggles (Right Aligned)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("FX").selected(false)).clicked() {
                    action.fx_clicked = true;
                }
                if ui.add(egui::Button::new("Mixer").selected(mixer_visible)).clicked() {
                    action.mixer_clicked = true;
                }
                if ui.add(egui::Button::new("Pool").selected(pool_visible)).clicked() {
                    action.pool_clicked = true;
                }
            });
        });
        ui.add_space(2.0);
    });

    action
}
