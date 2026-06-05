use crate::app::{HdawApp, UnsavedChangesAction};
use crate::app::input;
use egui::{pos2, Color32, Context, Rect, Vec2};

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

pub fn render(app: &mut HdawApp, ctx: &Context) {
    ctx.all_styles_mut(|style| {
        style.visuals = if app.preferences.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
    });
    let is_playing = app.is_playing();
    let pos = app.position_seconds();
    let bpm = app.project.bpm;

    let loop_enabled = app.engine.transport.loop_enabled.load(std::sync::atomic::Ordering::Acquire);
    let metronome_enabled = app.engine.transport.metronome_enabled.load(std::sync::atomic::Ordering::Acquire);
    let is_recording = app.engine.is_recording();

    let has_instruments = app.plugin_registry.iter().any(|d| d.is_instrument);
    let action = crate::ui::toolbar::render(
        ctx,
        is_playing,
        pos,
        bpm,
        app.project.time_signature_num,
        app.project.time_signature_den,
        app.timeline_state.snap_enabled,
        app.undo_service.can_undo(),
        app.undo_service.can_redo(),
        loop_enabled,
        metronome_enabled,
        is_recording,
        app.mixer_state.visible,
        app.selected_track.is_some(),
        app.audio_pool_state.visible,
        has_instruments,
        &app.preferences.recent_files,
    );

    if action.play_clicked {
        app.play_requested = true;
    }
    if action.pause_clicked {
        app.pause_requested = true;
    }
    if action.stop_clicked {
        app.stop_requested = true;
    }
    if action.new_clicked {
        app.new_project_requested = true;
    }
    if action.save_clicked {
        app.save_requested = true;
    }
    if action.save_as_clicked {
        app.save_as_requested = true;
    }
    if action.open_clicked {
        app.open_requested = true;
    }
    if let Some(path) = action.open_file {
        if path.to_string_lossy() == "__clear_recent__" {
            app.preferences.recent_files.clear();
            crate::app::prefs_io::save_preferences(&app.preferences);
        } else if path.exists() {
            if app.has_unsaved_changes() && app.confirm_unsaved.is_none() {
                app.pending_open_path = Some(path);
                app.confirm_unsaved = Some(crate::app::UnsavedChangesAction::OpenProject);
            } else {
                if let Err(e) = app.load_project_file(path.to_str().unwrap_or("")) {
                    app.error_message = Some(e);
                } else {
                    app.preferences.push_recent_file(path);
                    crate::app::prefs_io::save_preferences(&app.preferences);
                }
            }
        }
    }
    if action.import_clicked {
        app.import_audio();
    }
    if action.midi_import_clicked {
        app.import_midi();
    }
    if action.fx_clicked {
        app.effect_editor_state.show_editor = !app.effect_editor_state.show_editor;
    }
    if action.snap_clicked {
        app.timeline_state.snap_enabled = !app.timeline_state.snap_enabled;
    }
    if action.loop_clicked {
        app.toggle_loop();
    }
    if action.mixer_clicked {
        app.mixer_state.visible ^= true;
    }
    if action.pool_clicked {
        app.audio_pool_state.visible ^= true;
    }
    if action.add_track_clicked {
        app.add_blank_track();
    }
    if action.add_instrument_clicked {
        app.show_instrument_dialog = true;
    }
    if action.add_group_clicked {
        app.add_group_track();
    }
    if action.add_return_clicked {
        app.add_return_track();
    }
    if action.delete_track_clicked {
        if let Some(idx) = app.selected_track {
            app.delete_track(idx);
        }
    }
    if action.undo_clicked {
        app.undo();
    }
    if action.redo_clicked {
        app.redo();
    }
    if action.metronome_clicked {
        app.engine.transport.toggle_metronome();
    }

    if action.record_clicked {
        app.record_requested = true;
    }

    if action.preferences_clicked {
        app.preferences.show_dialog = true;
    }
    if action.about_clicked {
        app.show_about = true;
    }
    if action.shortcuts_clicked {
        app.show_shortcuts = true;
    }
    if action.export_clicked {
        app.export_requested = true;
    }

    app.mixer_state.master_volume = app.master_volume();
    let mv = app.mixer_state.master_volume;
    app.set_master_volume(mv);

    // 1. Status Bar (Absolute Bottom)
    let err_msg = app.error_message.clone();
    use std::sync::atomic::Ordering;
    let master_peak_l = app.engine.master_bus.peak_left.load(Ordering::Acquire);
    let master_peak_r = app.engine.master_bus.peak_right.load(Ordering::Acquire);
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!(
                "Tracks: {} | Sample Rate: {} Hz | Pos: {:.3}s",
                app.track_ui.len(),
                app.engine.transport.sample_rate(),
                app.position_seconds(),
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ml = f32::from_bits(master_peak_l);
                let mr = f32::from_bits(master_peak_r);
                for (label, peak) in [("L", ml), ("R", mr)] {
                    ui.colored_label(Color32::from_gray(120), label);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(60.0, 10.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x1a, 0x1a, 0x1a));
                    let fill = (rect.width() * peak.min(1.0)).max(0.0);
                    if fill > 0.0 {
                        let color = if peak > 0.9 {
                            Color32::from_rgb(0xcc, 0x33, 0x33)
                        } else {
                            Color32::from_rgb(0x4c, 0xaf, 0x50)
                        };
                        painter.rect_filled(
                            Rect::from_min_size(pos2(rect.left(), rect.top()), Vec2::new(fill, rect.height())),
                            1.0,
                            color,
                        );
                    }
                }
                if let Some(err) = &err_msg {
                    ui.colored_label(Color32::from_rgb(0xff, 0x44, 0x44), err);
                    if ui.button("x").clicked() {
                        app.error_message = None;
                    }
                }
            });
        });
    });

    // 2. Registered panels (mixer, audio pool, effects, piano roll, preferences)
    crate::ui::panels::render_all(app, ctx);

    // 3. Export Dialog
    if app.export_save_path.is_some() || app.exporting || app.export_done_message.is_some() {
        egui::Window::new("Export Audio")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(msg) = &app.export_done_message {
                    ui.label(msg);
                    if ui.button("OK").clicked() {
                        app.export_done_message = None;
                        app.export_save_path = None;
                    }
                } else if app.exporting {
                    ui.label("Exporting...");
                    ui.add(
                        egui::ProgressBar::new(app.export_progress as f32)
                            .show_percentage()
                            .animate(true),
                    );
                    if ui.button("Cancel").clicked() {
                        app.export_cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label("Bit Depth:");
                        for &d in &[16u16, 24, 32] {
                            ui.selectable_value(&mut app.export_bit_depth, d, format!("{}", d));
                        }
                    });
                    ui.checkbox(&mut app.export_use_loop_range, "Loop range only");
                    if ui.button("Start Export").clicked() {
                        app.exporting = true;
                        app.export_progress = 0.0;
                        app.export_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });
    }

    // 4. Intercept window close if unsaved changes
    if ctx.input(|i| i.viewport().close_requested()) && app.confirm_unsaved.is_none()
        && app.has_unsaved_changes()
    {
        app.confirm_unsaved = Some(UnsavedChangesAction::CloseApp);
    }

    // 5. Unsaved changes confirmation dialog
    if app.confirm_unsaved.is_some() {
        let mut keep_open = true;
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .open(&mut keep_open)
            .show(ctx, |ui| {
                ui.label("Save changes before continuing?");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        if let Some(path) = &app.current_path.clone() {
                            if let Err(e) = app.save_current_project(path.to_str().unwrap_or("")) {
                                app.error_message = Some(e);
                            }
                            let act = app.confirm_unsaved.take();
                            app.pending_after_save = act;
                            input::handle_pending_requests(app, ctx);
                        } else {
                            app.pending_after_save = app.confirm_unsaved.take();
                            app.save_as_requested = true;
                        }
                    }
                    if ui.button("Don't Save").clicked() {
                        let act = app.confirm_unsaved.take();
                        app.undo_service.clear();
                        if act == Some(UnsavedChangesAction::CloseApp) {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        } else {
                            input::execute_confirm_action(app, act);
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        app.confirm_unsaved = None;
                    }
                });
            });
    }

    // 6. Keyboard Shortcuts dialog
    if app.show_shortcuts {
        egui::Window::new("Keyboard Shortcuts")
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(400.0, 350.0))
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .open(&mut app.show_shortcuts)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .striped(true)
                    .min_col_width(120.0)
                    .max_col_width(200.0)
                    .show(ui, |ui| {
                        let shortcuts = [
                            ("Space", "Play / Pause"),
                            ("Shift+Space", "Play / Pause (alt)"),
                            ("Delete / Backspace", "Delete selected clip"),
                            ("Ctrl+N", "New Project"),
                            ("Ctrl+O", "Open Project"),
                            ("Ctrl+S", "Save"),
                            ("Ctrl+Shift+S", "Save As"),
                            ("Ctrl+Z", "Undo"),
                            ("Ctrl+Shift+Z", "Redo"),
                            ("Ctrl+I", "Import Audio"),
                            ("Ctrl+Shift+I", "Import MIDI"),
                            ("Ctrl+,", "Preferences"),
                            ("F2", "Toggle FX Editor"),
                            ("Home", "Go to start"),
                            ("End", "Go to end"),
                            ("L", "Toggle Loop"),
                            ("[", "Set loop start"),
                            ("]", "Set loop end"),
                            ("Escape", "Close Piano Roll"),
                        ];
                        for (key, desc) in &shortcuts {
                            ui.label(egui::RichText::new(*key).strong());
                            ui.label(*desc);
                            ui.end_row();
                        }
                    });
                    });
            });
    }

    // 7. About dialog
    if app.show_about {
        egui::Window::new("About HDAW")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
            .open(&mut app.show_about)
            .show(ctx, |ui| {
                ui.heading("HDAW");
                ui.label("Holofonic Digital Audio Workstation");
                ui.separator();
                ui.label("Written by Clint Anderson");
                ui.label("clinta@gmail.com");
                ui.label("and DeepSeek, MiniMax, and GLM");
            });
    }

    // 7. Instrument dialog
    if app.show_instrument_dialog {
        let instruments: Vec<_> = app.plugin_registry.iter()
            .filter(|d| d.is_instrument)
            .cloned().collect();
        if !instruments.is_empty() {
            egui::Window::new("Select Instrument")
                .collapsible(false)
                .resizable(true)
                .default_width(480.0)
                .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
                .show(ctx, |ui| {
                    ui.set_min_width(400.0);
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.set_min_height(24.0);
                            for desc in &instruments {
                                let name = truncate(&desc.name, 35);
                                if ui.button(name).clicked() {
                                    app.add_instrument_track(desc);
                                    app.show_instrument_dialog = false;
                                }
                            }
                        });
                    });
                    ui.separator();
                    if ui.button("Cancel").clicked() {
                        app.show_instrument_dialog = false;
                    }
                });
        } else {
            app.show_instrument_dialog = false;
        }
    }

    // 8. Central Panel (Timeline)
    egui::CentralPanel::default().show(ctx, |ui| {
        crate::ui::timeline::render(ui, app);
    });
}
