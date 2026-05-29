use crate::app::TrackUiState;
use egui::{Color32, Context, Slider};
use std::sync::atomic::Ordering;

pub struct MixerPanelState {
    pub master_volume: f32,
    pub visible: bool,
}

impl Default for MixerPanelState {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            visible: true,
        }
    }
}

pub fn render(ctx: &Context, state: &mut MixerPanelState, track_ui: &[TrackUiState]) {
    egui::TopBottomPanel::bottom("mixer_panel")
        .min_height(180.0)
        .show(ctx, |ui| {
            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_master(ui, state);

                    ui.separator();

                    for (i, tui) in track_ui.iter().enumerate() {
                        draw_channel(ui, i, tui);
                        ui.separator();
                    }
                });
        });
}

fn draw_master(ui: &mut egui::Ui, state: &mut MixerPanelState) {
    ui.allocate_at_least(egui::vec2(100.0, 0.0), egui::Sense::hover());
    ui.vertical(|ui| {
        ui.set_width(100.0);
        ui.colored_label(Color32::from_rgb(0xcc, 0xaa, 0x44), "Master");
        let (master_rect, _) = ui.allocate_exact_size(egui::vec2(20.0, 80.0), egui::Sense::hover());
        draw_vu_meter(ui, master_rect, state.master_volume, false);
        ui.add_space(4.0);
        ui.add(Slider::new(&mut state.master_volume, 0.0..=1.0).text("Vol"));
        ui.label(format!("{:.2}", state.master_volume));
    });
}

fn draw_channel(ui: &mut egui::Ui, _index: usize, tui: &TrackUiState) {
    let name = &tui.name;
    let color = Color32::from_rgb(tui.color[0], tui.color[1], tui.color[2]);
    let muted = tui.mute.load(Ordering::Acquire);
    let peak_l = f32::from_bits(tui.peak_left.load(Ordering::Acquire));
    let peak_r = f32::from_bits(tui.peak_right.load(Ordering::Acquire));
    let peak = peak_l.max(peak_r);

    let mut vol = f32::from_bits(tui.volume.load(Ordering::Acquire));

    ui.allocate_at_least(egui::vec2(80.0, 0.0), egui::Sense::hover());
    ui.vertical(|ui| {
        ui.set_width(80.0);

        ui.add_space(2.0);
        ui.colored_label(color, name);

        ui.add_space(2.0);
        let (meter_rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 60.0), egui::Sense::hover());
        draw_vu_meter(ui, meter_rect, peak, muted);

        ui.add_space(4.0);
        let response = ui.add(Slider::new(&mut vol, 0.0..=1.0).vertical().text(""));
        if response.changed() {
            tui.volume.store(vol.to_bits(), Ordering::Release);
        }

        ui.add_space(2.0);
        ui.label(format!("{:.1}", vol));
    });
}

fn draw_vu_meter(ui: &mut egui::Ui, rect: egui::Rect, level: f32, muted: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 1.0, Color32::from_rgb(0x1a, 0x1a, 0x1a));

    if muted {
        painter.rect_filled(rect, 1.0, Color32::from_rgb(0x33, 0x33, 0x33));
        painter.rect_stroke(rect, 1.0, egui::Stroke::new(1.0, Color32::from_gray(80)));
        return;
    }

    let clamped = level.min(1.0);
    if clamped > 0.0 {
        let fill_height = rect.height() * clamped;
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - fill_height),
            rect.right_bottom(),
        );
        let color = if clamped > 0.9 {
            Color32::from_rgb(0xcc, 0x33, 0x33)
        } else if clamped > 0.7 {
            Color32::from_rgb(0xcc, 0xaa, 0x33)
        } else {
            Color32::from_rgb(0x4c, 0xaf, 0x50)
        };
        painter.rect_filled(fill_rect, 1.0, color);
    }

    painter.rect_stroke(rect, 1.0, egui::Stroke::new(1.0, Color32::from_gray(80)));
}