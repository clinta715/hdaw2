use crate::app::HdawApp;
use crate::app::prefs_io;
use crate::project::clip::ClipKind;
use egui_file_dialog::{DialogState, FileDialog};

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
        if input.consume_key(egui::Modifiers::CTRL, Key::I) {
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
        app.stop();
        app.stop_requested = false;
    }
    if app.seek_requested {
        app.engine.transport.seek_to_frame(app.seek_frame);
        app.seek_requested = false;
    }

    if app.new_project_requested {
        app.new_project();
        app.new_project_requested = false;
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
        } else {
            let mut dialog = make_dialog_with_dir(app.preferences.last_save_dir.as_ref());
            dialog.save_file();
            app.save_dialog = Some(dialog);
        }
    }

    if app.open_requested {
        app.open_requested = false;
        let mut dialog = make_dialog_with_dir(app.preferences.last_open_dir.as_ref());
        dialog.pick_file();
        app.open_dialog = Some(dialog);
    }

    {
        let dialog = &mut app.save_dialog;
        if let Some(dialog) = dialog {
            dialog.update(ctx);
            match dialog.state().clone() {
                DialogState::Selected(path) => {
                    app.preferences.last_save_dir = path.parent().map(|p| p.to_path_buf());
                    prefs_io::save_preferences(&app.preferences);
                    if let Err(e) = app.save_current_project(path.to_str().unwrap_or("")) {
                        app.error_message = Some(e);
                    }
                    app.save_dialog = None;
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