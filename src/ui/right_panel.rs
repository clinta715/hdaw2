use crate::app::{HdawApp, RightPanelMode};
use egui::{Color32, Context, RichText};

pub fn render(ctx: &Context, app: &mut HdawApp) {
    let panel = egui::SidePanel::right("right_panel")
        .resizable(true)
        .min_width(140.0)
        .default_width(app.preferences.right_panel_width);

    let panel_res = panel.show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (mode, label) in &[
                    (RightPanelMode::Browser, "Browser"),
                    (RightPanelMode::ClipInfo, "Clip"),
                    (RightPanelMode::EffectDetail, "FX"),
                ] {
                    let selected = app.right_panel_mode == *mode;
                    if ui.selectable_label(selected, *label).clicked() {
                        app.right_panel_mode = *mode;
                    }
                }
            });
            ui.separator();

            match app.right_panel_mode {
                RightPanelMode::Browser => render_browser(ui, app),
                RightPanelMode::ClipInfo => render_clip_info(ui, app),
                RightPanelMode::EffectDetail => render_effect_detail(ui, app),
            }
        });
    });

    let response_width = panel_res.response.rect.width();
    if response_width.is_finite() && (response_width - app.preferences.right_panel_width).abs() > 1.0 {
        app.preferences.right_panel_width = response_width;
    }
}

fn render_browser(ui: &mut egui::Ui, app: &mut HdawApp) {
    crate::ui::sample_browser::render(ui, app);
}

fn render_clip_info(ui: &mut egui::Ui, app: &mut HdawApp) {
    let clip_id = match app.timeline_state.selected_clip_id {
        Some(id) => id,
        None => {
            ui.label("No clip selected.");
            return;
        }
    };

    for t in &app.project.tracks {
        for c in &t.clips {
            match c {
                crate::project::clip::ClipKind::Audio(a) if a.id == clip_id => {
                    ui.label(RichText::new(&a.name).strong().size(11.0));
                    ui.label(format!("Position: {:.2}s", a.position_frames as f64 / app.engine.transport.sample_rate() as f64));
                    ui.label(format!("Length: {:.2}s", a.length_frames as f64 / app.engine.transport.sample_rate() as f64));
                    return;
                }
                crate::project::clip::ClipKind::Midi(m) if m.id == clip_id => {
                    ui.label(RichText::new(&m.name).strong().size(11.0));
                    let sr = app.engine.transport.sample_rate();
                    ui.label(format!("Notes: {}", m.notes.len()));
                    ui.label(format!("Position: {:.2}s", m.position_frames as f64 / sr as f64));
                    ui.label(format!("Length: {:.2}s", m.length_frames as f64 / sr as f64));
                    return;
                }
                _ => {}
            }
        }
    }
    ui.label("Clip not found.");
}

fn render_effect_detail(ui: &mut egui::Ui, app: &mut HdawApp) {
    let track_idx = match app.selected_track {
        Some(i) => i,
        None => {
            ui.label("No track selected.");
            return;
        }
    };

    if let Ok(tracks) = app.engine.tracks.lock() {
        if let Some(track) = tracks.get(track_idx) {
            if track.fx_chain.is_empty() {
                ui.label("No effects on this track.");
                return;
            }
            for (fi, fx) in track.fx_chain.iter().enumerate() {
                ui.label(RichText::new(format!("{}: {}", fi + 1, fx.name)).size(10.0));
                for info in fx.parameter_info().iter() {
                    let val = fx.parameter_value(info.id);
                    ui.label(RichText::new(format!("  {}: {:.2}", info.name, val)).size(9.0).color(Color32::from_gray(160)));
                }
                ui.separator();
            }
        }
    }
}
