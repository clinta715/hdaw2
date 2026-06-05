use crate::app::TrackUiState;
use egui::{Color32, Context, Slider};

const CHANNEL_WIDTH: f32 = 70.0;
const MIXER_MIN_HEIGHT: f32 = 160.0;
use std::sync::atomic::Ordering;
use uuid::Uuid;

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

pub fn render(ctx: &Context, app: &mut crate::app::HdawApp) {
    let panel_res = egui::TopBottomPanel::bottom("mixer_panel")
        .resizable(true)
        .default_height(app.preferences.mixer_panel_height)
        .min_height(MIXER_MIN_HEIGHT)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            egui::ScrollArea::horizontal()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mv = app.master_volume();
                        let new_mv = draw_master(ui, &mut app.mixer_state, mv);
                        if (new_mv - mv).abs() > 0.001 {
                            app.set_master_volume(new_mv);
                        }
                        ui.separator();
                        let all_tracks: Vec<TrackUiState> = app.track_ui.clone();
                        for i in 0..all_tracks.len() {
                            draw_channel(ui, i, app, &all_tracks);
                            ui.separator();
                        }
                    });
                });
        });
    app.preferences.mixer_panel_height = panel_res.response.rect.height();
}

fn draw_master(ui: &mut egui::Ui, state: &mut MixerPanelState, initial_vol: f32) -> f32 {
    state.master_volume = initial_vol;
    ui.vertical(|ui| {
        ui.set_width(CHANNEL_WIDTH);

        // Master meter
        {
            let mh = ui.available_height().clamp(20.0, 200.0);
            let (master_rect, _) = ui.allocate_exact_size(egui::vec2(12.0, mh), egui::Sense::hover());
            draw_vu_meter(ui, master_rect, state.master_volume, false);

            let resp = ui.add(egui::Slider::new(&mut state.master_volume, 0.0..=1.0)
                .vertical()
                .show_value(false));
            if resp.changed() {
                // return value signals caller to sync to engine
            }
        }

        ui.add_space(2.0);
        ui.centered_and_justified(|ui| {
            ui.label(format!("{:.1} dB", 20.0 * state.master_volume.max(0.0001).log10()));
        });
    });
    state.master_volume
}

fn draw_channel(ui: &mut egui::Ui, index: usize, app: &mut crate::app::HdawApp, all_tracks: &[TrackUiState]) {
    let tui = &all_tracks[index];
    let _color = Color32::from_rgb(tui.color[0], tui.color[1], tui.color[2]);
    let muted = tui.mute.load(Ordering::Acquire);
    let peak_l = f32::from_bits(tui.peak_left.load(Ordering::Acquire));
    let peak_r = f32::from_bits(tui.peak_right.load(Ordering::Acquire));
    let peak = peak_l.max(peak_r);
    let is_group = tui.is_group;
    let is_return = tui.is_return;
    let parent_group = tui.parent_group;
    let send_levels_count = tui.send_levels.len();
    let mut vol = f32::from_bits(tui.volume.load(Ordering::Acquire));

    ui.vertical(|ui| {
        ui.set_width(CHANNEL_WIDTH);

        // Track meter
        {
            let meter_h = ui.available_height().clamp(20.0, 200.0);
            let (meter_rect, _) = ui.allocate_exact_size(egui::vec2(10.0, meter_h), egui::Sense::hover());
            draw_vu_meter(ui, meter_rect, peak, muted);

            let response = ui.add(Slider::new(&mut vol, 0.0..=1.0)
                .vertical()
                .show_value(false));
            if response.changed() {
                app.track_ui[index].volume.store(vol.to_bits(), Ordering::Release);
                if let Some(track) = app.project.tracks.get_mut(index) {
                    track.volume = vol;
                }
            }
        }

        ui.add_space(2.0);
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(format!("{:.1}", vol)).small());
        });

        // Route dropdown (not for group or return tracks)
        if !is_group && !is_return {
            ui.add_space(4.0);
            let groups: Vec<(Uuid, String)> = all_tracks.iter()
                .filter(|t| t.is_group)
                .map(|t| (t.id, t.name.clone()))
                .collect();
            let current_label = if let Some(pid) = parent_group {
                groups.iter().find(|(id, _)| *id == pid).map(|(_, n)| n.clone())
                    .unwrap_or_else(|| "Master".to_string())
            } else {
                "Master".to_string()
            };
            ui.menu_button(format!("RTE: {}", current_label), |ui| {
                if ui.button("Master").clicked() {
                    app.set_track_parent(index, None);
                    ui.close_menu();
                }
                ui.separator();
                for (gid, gname) in &groups {
                    if ui.button(gname).clicked() {
                        app.set_track_parent(index, Some(*gid));
                        ui.close_menu();
                    }
                }
            });
        }

        // Sends section
        if send_levels_count > 0 {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Sends").small().color(Color32::from_gray(160)));
            // Look up return names by target_id from project model
            let project_sends = app.project.tracks.get(index).map(|t| t.sends.clone()).unwrap_or_default();
            for si in 0..send_levels_count.min(4) {
                let mut level = f32::from_bits(app.track_ui[index].send_levels[si].load(Ordering::Acquire));
                let return_name = project_sends.get(si)
                    .and_then(|s| all_tracks.iter().find(|t| t.id == s.target_id))
                    .map(|t| if t.name.len() > 6 { format!("{}…", &t.name[..6]) } else { t.name.clone() })
                    .unwrap_or_else(|| "?".to_string());
                let resp = ui.add(Slider::new(&mut level, 0.0..=1.0).text(return_name));
                if resp.changed() {
                    app.set_send_level(index, si, level);
                }
            }
        }
    });
}

fn draw_vu_meter(ui: &mut egui::Ui, rect: egui::Rect, level: f32, muted: bool) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x12, 0x12, 0x12));

    if muted {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0x22, 0x22, 0x22));
        painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(50)));
        return;
    }

    let clamped = level.min(1.2);
    if clamped > 0.0 {
        let fill_height = (rect.height() * (clamped / 1.0)).min(rect.height());
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - fill_height),
            rect.right_bottom(),
        );

        let color = if clamped > 1.0 {
            Color32::from_rgb(0xff, 0x00, 0x00)
        } else if clamped > 0.8 {
            Color32::from_rgb(0xcc, 0xaa, 0x33)
        } else {
            Color32::from_rgb(0x4c, 0xaf, 0x50)
        };
        painter.rect_filled(fill_rect, 0.0, color);
    }

    for i in 0..=10 {
        let y = rect.bottom() - (rect.height() * i as f32 / 10.0);
        let tick_w = if i % 5 == 0 { 4.0 } else { 2.0 };
        painter.line_segment(
            [egui::pos2(rect.right(), y), egui::pos2(rect.right() - tick_w, y)],
            egui::Stroke::new(1.0, Color32::from_gray(80)),
        );
    }

    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, Color32::from_gray(60)));
}
