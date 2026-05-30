use crate::app::HdawApp;
use egui::{pos2, Color32, Context, Rect, Vec2};

pub fn render(app: &mut HdawApp, ctx: &Context) {
    let is_playing = app.is_playing();
    let pos = app.position_seconds();
    let bpm = app.project.bpm;

    let loop_enabled = app.engine.transport.loop_enabled.load(std::sync::atomic::Ordering::Acquire);

    let has_instruments = app.plugin_registry.iter().any(|d| d.is_instrument);
    let action = crate::ui::toolbar::render(
        ctx,
        is_playing,
        pos,
        bpm,
        app.project.time_signature_num,
        app.project.time_signature_den,
        app.timeline_state.snap_enabled,
        app.undo_state.can_undo(),
        app.undo_state.can_redo(),
        loop_enabled,
        app.mixer_state.visible,
        app.selected_track.is_some(),
        app.audio_pool_state.visible,
        has_instruments,
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
    if action.import_clicked {
        app.import_audio();
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
    if action.preferences_clicked {
        app.preferences.show_dialog = true;
    }

    app.mixer_state.master_volume = app.master_volume();
    let mv = app.mixer_state.master_volume;
    app.set_master_volume(mv);
    if app.mixer_state.visible {
        crate::ui::mixer_panel::render(ctx, &mut app.mixer_state, &app.track_ui);
    }

    crate::ui::effect_editor::render(ctx, app);
    crate::ui::piano_roll::render(ctx, app);
    crate::ui::preferences::render(ctx, app);

    if app.show_instrument_dialog {
        let instruments: Vec<_> = app.plugin_registry.iter()
            .filter(|d| d.is_instrument)
            .cloned().collect();
        if !instruments.is_empty() {
            egui::Window::new("Select Instrument")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, (0.0, 0.0))
                .show(ctx, |ui| {
                    for desc in &instruments {
                        if ui.button(&desc.name).clicked() {
                            app.add_instrument_track(desc);
                            app.show_instrument_dialog = false;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        app.show_instrument_dialog = false;
                    }
                });
        } else {
            app.show_instrument_dialog = false;
        }
    }

    let mut pool_state = std::mem::take(&mut app.audio_pool_state);
    crate::ui::audio_pool::render(ctx, &mut pool_state, app);
    app.audio_pool_state = pool_state;

    egui::CentralPanel::default().show(ctx, |ui| {
        crate::ui::timeline::render(ui, app);
    });

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
}
