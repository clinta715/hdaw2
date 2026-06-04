use crate::app::prefs_io;
use crate::app::{HdawApp, UnsavedChangesAction};
use crate::project::clip::ClipKind;
use egui_file_dialog::{DialogState, FileDialog};

pub fn execute_confirm_action(app: &mut HdawApp, action: Option<UnsavedChangesAction>) {
    match action {
        Some(UnsavedChangesAction::NewProject) => app.new_project(),
        Some(UnsavedChangesAction::OpenProject) => {
            if let Some(path) = app.pending_open_path.take() {
                if let Err(e) = app.load_project_file(path.to_str().unwrap_or("")) {
                    app.error_message = Some(e);
                } else {
                    app.preferences.push_recent_file(path);
                    crate::app::prefs_io::save_preferences(&app.preferences);
                }
            } else {
                app.open_requested = true;
            }
        }
        Some(UnsavedChangesAction::CloseApp) => {}
        None => {}
    }
}

pub fn handle_keyboard_input(app: &mut HdawApp, ctx: &egui::Context) {
    ctx.input_mut(|input| {
        use egui::Key;
        if input.consume_key(egui::Modifiers::NONE, Key::Space) {
            if app.is_playing() { app.pause_requested = true; }
            else { app.play_requested = true; }
        }
        if input.consume_key(egui::Modifiers::SHIFT, Key::Space) {
            if app.is_playing() { app.pause_requested = true; }
            else { app.play_requested = true; }
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Delete)
            || input.consume_key(egui::Modifiers::NONE, Key::Backspace)
        {
            app.remove_selected_clip();
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::S) && input.modifiers.shift {
            app.save_as_requested = true;
        } else if input.consume_key(egui::Modifiers::CTRL, Key::S) {
            app.save_requested = true;
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::O) {
            app.open_requested = true;
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::N) {
            app.new_project_requested = true;
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::I) && input.modifiers.shift {
            app.import_midi();
        } else if input.consume_key(egui::Modifiers::CTRL, Key::I) {
            app.import_audio();
        }
        if input.consume_key(egui::Modifiers::NONE, Key::F2) {
            app.effect_editor_state.show_editor ^= true;
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::Z) && input.modifiers.shift {
            app.redo();
        } else if input.consume_key(egui::Modifiers::CTRL, Key::Z) {
            app.undo();
        }
        if input.consume_key(egui::Modifiers::CTRL, Key::Comma) {
            app.preferences.show_dialog = true;
        }
        if input.consume_key(egui::Modifiers::NONE, Key::Home) {
            app.seek_frame = 0;
            app.seek_requested = true;
        }
        if input.consume_key(egui::Modifiers::NONE, Key::End) {
            let last = app.project.tracks.iter()
                .flat_map(|t| t.clips.iter())
                .filter_map(|c| match c {
                    ClipKind::Audio(a) => Some(a.position_frames + a.length_frames),
                    ClipKind::Midi(m) => Some(m.position_frames + m.length_frames),
                })
                .max().unwrap_or(0);
            app.seek_frame = last;
            app.seek_requested = true;
        }
        if input.consume_key(egui::Modifiers::NONE, Key::L) {
            app.toggle_loop();
        }
        if input.consume_key(egui::Modifiers::NONE, Key::OpenBracket) {
            app.set_loop_start_at_playhead();
        }
        if input.consume_key(egui::Modifiers::NONE, Key::CloseBracket) {
            app.set_loop_end_at_playhead();
        }
        if input.consume_key(egui::Modifiers::NONE, Key::S) {
            app.split_selected_clip();
        }
        if input.consume_key(egui::Modifiers::NONE, Key::M) {
            let count = app.project.markers.len() + 1;
            app.add_marker_at_playhead(format!("M{}", count));
        }
    });
}

fn make_dialog_with_dir(dir: Option<&std::path::PathBuf>) -> FileDialog {
    let mut dialog = FileDialog::new();
    if let Some(d) = dir {
        dialog = dialog.initial_directory(d.clone());
    }
    dialog
}

pub fn handle_pending_requests(app: &mut HdawApp, ctx: &egui::Context) {
    if app.play_requested {
        app.play();
        app.play_requested = false;
    }
    if app.pause_requested {
        app.pause();
        app.pause_requested = false;
    }
    if app.stop_requested {
        if app.recording {
            app.finish_recording();
        }
        app.stop();
        app.stop_requested = false;
    }
    if app.pause_requested && app.recording {
        app.finish_recording();
    }
    if app.seek_requested {
        app.engine.transport.seek_to_frame(app.seek_frame);
        app.seek_requested = false;
    }

    if app.record_requested {
        app.record_requested = false;
        if app.recording {
            app.finish_recording();
            app.stop();
        } else {
            app.start_recording();
        }
    }

    if app.new_project_requested {
        app.new_project_requested = false;
        if app.has_unsaved_changes() && app.confirm_unsaved.is_none() {
            app.confirm_unsaved = Some(crate::app::UnsavedChangesAction::NewProject);
        } else {
            app.new_project();
        }
    }

    if app.save_as_requested {
        app.save_as_requested = false;
        let mut dialog = make_dialog_with_dir(app.preferences.last_save_dir.as_ref());
        dialog.save_file();
        app.save_dialog = Some(dialog);
    }

    if app.save_requested {
        app.save_requested = false;
        if let Some(path) = &app.current_path.clone() {
            if let Err(e) = app.save_current_project(path.to_str().unwrap_or("")) {
                app.error_message = Some(e);
            }
            app.preferences.push_recent_file(path.clone());
            prefs_io::save_preferences(&app.preferences);
            let pending = app.pending_after_save.take();
            execute_confirm_action(app, pending);
        } else {
            let mut dialog = make_dialog_with_dir(app.preferences.last_save_dir.as_ref());
            dialog.save_file();
            app.save_dialog = Some(dialog);
        }
    }

    if app.open_requested {
        app.open_requested = false;
        if app.has_unsaved_changes() && app.confirm_unsaved.is_none() {
            app.confirm_unsaved = Some(crate::app::UnsavedChangesAction::OpenProject);
        } else {
            let mut dialog = make_dialog_with_dir(app.preferences.last_open_dir.as_ref());
            dialog.pick_file();
            app.open_dialog = Some(dialog);
        }
    }

    if app.export_requested {
        app.export_requested = false;
        let mut dialog = make_dialog_with_dir(app.preferences.last_save_dir.as_ref())
            .title("Export Audio");
        dialog.save_file();
        app.export_dialog = Some(dialog);
    }

    if let Some(dialog) = &mut app.export_dialog {
        dialog.update(ctx);
        match dialog.state().clone() {
            DialogState::Selected(path) => {
                app.preferences.last_save_dir = path.parent().map(|p| p.to_path_buf());
                prefs_io::save_preferences(&app.preferences);
                app.export_save_path = Some(path);
                app.export_dialog = None;
            }
            DialogState::Cancelled => {
                app.export_dialog = None;
            }
            _ => {}
        }
    }

    if app.exporting {
        if let Some(path) = app.export_save_path.clone() {
            use crate::audio::stream::render_export;
            use hound::WavSpec;
            let sr = app.engine.transport.sample_rate();
            let end = if app.export_use_loop_range {
                let (_, loop_out) = app.engine.transport.load_loop_region();
                if loop_out > 0 { loop_out } else {
                    app.project_length_frames()
                }
            } else {
                app.project_length_frames()
            };
            let start = if app.export_use_loop_range {
                let (loop_in, _) = app.engine.transport.load_loop_region();
                loop_in
            } else { 0 };

            if let Ok(mut tracks_guard) = app.engine.tracks.lock() {
                let samples = render_export(
                    &mut *tracks_guard,
                    &*app.engine.master_bus,
                    sr,
                    start,
                    end,
                );
                app.export_progress = 1.0;
                let spec = WavSpec {
                    channels: 2,
                    sample_rate: sr,
                    bits_per_sample: app.export_bit_depth,
                    sample_format: if app.export_bit_depth == 32 {
                        hound::SampleFormat::Float
                    } else {
                        hound::SampleFormat::Int
                    },
                };
                let result = hound::WavWriter::create(&path, spec);
                match result {
                    Ok(mut writer) => {
                        if app.export_bit_depth == 32 {
                            for frame in samples.chunks(2) {
                                if let [l, r] = frame {
                                    writer.write_sample(*l).ok();
                                    writer.write_sample(*r).ok();
                                }
                            }
                        } else {
                            let scale = (1i32 << (app.export_bit_depth - 1)) as f32;
                            for frame in samples.chunks(2) {
                                if let [l, r] = frame {
                                    writer.write_sample((l * scale) as i32).ok();
                                    writer.write_sample((r * scale) as i32).ok();
                                }
                            }
                        }
                        writer.finalize().ok();
                        app.export_done_message = Some(format!("Exported to {}", path.display()));
                    }
                    Err(e) => {
                        app.error_message = Some(format!("Export failed: {}", e));
                    }
                }
            } else {
                app.error_message = Some("Failed to lock tracks for export".to_string());
            }
        }
        app.exporting = false;
        app.export_progress = 0.0;
        app.export_save_path = None;
        app.export_cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    {
        let dialog = &mut app.save_dialog;
        if let Some(dialog) = dialog {
            dialog.update(ctx);
            match dialog.state().clone() {
                DialogState::Selected(path) => {
                    app.preferences.last_save_dir = path.parent().map(|p| p.to_path_buf());
                    app.preferences.push_recent_file(path.clone());
                    prefs_io::save_preferences(&app.preferences);
                    if let Err(e) = app.save_current_project(path.to_str().unwrap_or("")) {
                        app.error_message = Some(e);
                    }
                    app.save_dialog = None;
                    let pending = app.pending_after_save.take();
                    execute_confirm_action(app, pending);
                }
                DialogState::Cancelled => {
                    app.save_dialog = None;
                }
                _ => {}
            }
        }
    }

    {
        let dialog = &mut app.open_dialog;
        if let Some(dialog) = dialog {
            dialog.update(ctx);
            match dialog.state().clone() {
                DialogState::Selected(path) => {
                    app.preferences.last_open_dir = path.parent().map(|p| p.to_path_buf());
                    app.preferences.push_recent_file(path.clone());
                    prefs_io::save_preferences(&app.preferences);
                    if let Err(e) = app.load_project_file(path.to_str().unwrap_or("")) {
                        app.error_message = Some(e);
                    }
                    app.open_dialog = None;
                }
                DialogState::Cancelled => {
                    app.open_dialog = None;
                }
                _ => {}
            }
        }
    }
}