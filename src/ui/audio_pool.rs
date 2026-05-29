use crate::app::HdawApp;
use egui::{Color32, Context, Id, ScrollArea};

pub struct AudioPoolPanelState {
    pub visible: bool,
    pub width: f32,
    pub dragging_clip_id: Option<uuid::Uuid>,
}

impl Default for AudioPoolPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            width: 250.0,
            dragging_clip_id: None,
        }
    }
}

pub fn render(ctx: &Context, state: &mut AudioPoolPanelState, app: &mut HdawApp) {
    if !state.visible {
        return;
    }

    egui::SidePanel::left(Id::new("audio_pool_panel"))
        .resizable(false)
        .max_width(state.width)
        .show(ctx, |ui| {
            ui.set_width(state.width);
            draw_pool_panel(ui, state, app);
        });
}

fn draw_pool_panel(ui: &mut egui::Ui, state: &mut AudioPoolPanelState, app: &mut HdawApp) {
    ui.add_space(4.0);
    ui.heading("Audio Pool");

    if app.project.audio_pool.is_empty() {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("(empty — deleted clips appear here)").small().color(Color32::from_gray(140)));
        return;
    }

    ui.add_space(4.0);

    let selected_track = app.selected_track;

    if let Some(_drag_id) = state.dragging_clip_id {
        if ui.input(|i| i.pointer.primary_released()) {
            state.dragging_clip_id = None;
        }
    }

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let pool_clips: Vec<(uuid::Uuid, String, f64)> = app.project.audio_pool.iter().map(|p| {
            let secs = p.clip.buffer.as_ref()
                .map(|b| b.frames() as f64 / b.sample_rate() as f64)
                .unwrap_or(0.0);
            (p.id, p.name.clone(), secs)
        }).collect();
        drop(());

        for (id, name, secs) in pool_clips {
            let is_dragging = state.dragging_clip_id == Some(id);
            let (clip_rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 40.0),
                egui::Sense::click_and_drag(),
            );
            if is_dragging {
                ui.painter().rect_filled(clip_rect, 2.0, Color32::from_rgba_premultiplied(0x44, 0x88, 0xcc, 60));
            }
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&name).strong());
                    ui.label(egui::RichText::new(format!("{:.1}s", secs)).small().color(Color32::from_gray(140)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_add = selected_track.is_some();
                    if ui.add_enabled(can_add, egui::Button::new("Add").small()).clicked() {
                        if let Some(track_idx) = selected_track {
                            app.restore_pool_clip_to_track(id, track_idx);
                        }
                    }
                });
            });
            if response.drag_started() {
                state.dragging_clip_id = Some(id);
            }
            ui.add_space(2.0);
        }
    });
}