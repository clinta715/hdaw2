use crate::app::{HdawApp, UnsavedChangesAction};
use crate::app::input;
use egui::{Context, Vec2};

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

pub fn render_export_dialog(app: &mut HdawApp, ctx: &Context) {
    if app.export_dialog.is_none() && !app.exporting && app.export_done_message.is_none() {
        return;
    }
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

pub fn render_unsaved_dialog(app: &mut HdawApp, ctx: &Context) {
    if app.confirm_unsaved.is_none() { return; }
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

pub fn render_shortcuts_dialog(app: &mut HdawApp, ctx: &Context) {
    if !app.show_shortcuts { return; }
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
                        ("Ctrl+I", "Browse Samples"),
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

pub fn render_about_dialog(app: &mut HdawApp, ctx: &Context) {
    if !app.show_about { return; }
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

pub fn render_instrument_dialog(app: &mut HdawApp, ctx: &Context) {
    if !app.show_instrument_dialog { return; }
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
